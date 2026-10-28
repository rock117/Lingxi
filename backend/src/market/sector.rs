//! 板块榜 / 详情：读 `sc_sector` + `sc_sector_kline_d1`。
//! 当日涨跌幅表内无独立字段，用 `(close/pre_close - 1)*100` 计算。
//! 成分股数量来自 `sc_sector_constituent`，进程内缓存 12 小时。

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use chrono::NaiveDate;
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, FromQueryResult, Statement};
use serde::Serialize;

const DEFAULT_TYPES: &[&str] = &["INDUSTRY", "CONCEPT"];
const DEFAULT_TOP_N: usize = 10;
/// 榜单最少成分股数（不足则过滤）
const MIN_CONSTITUENT_COUNT: u32 = 20;
const CONSTITUENT_COUNT_TTL: Duration = Duration::from_secs(12 * 3600);

#[derive(Debug, Clone, Serialize)]
pub struct SectorMoverRow {
    pub symbol: String,
    pub name: String,
    pub sector_type: String,
    pub change_pct: f64,
    pub change_5d_pct: Option<f64>,
    pub change_10d_pct: Option<f64>,
    pub close: f64,
    pub pre_close: f64,
    pub amount: f64,
    pub volume: f64,
    /// 成分股数量（`sc_sector_constituent`）
    pub constituent_count: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct SectorMoversOut {
    pub trade_date: String,
    pub top_n: usize,
    /// 参与排序的 sector_type
    pub types: Vec<String>,
    /// `realtime` | `snapshot` | `kline`
    pub quote_source: String,
    pub gainers: Vec<SectorMoverRow>,
    pub losers: Vec<SectorMoverRow>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SectorConstituentRow {
    pub symbol: String,
    pub name: String,
    /// 最新收盘；无行情时为 null
    pub close: Option<f64>,
    pub change_pct: Option<f64>,
    pub change_5d_pct: Option<f64>,
    pub change_10d_pct: Option<f64>,
    pub amount: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SectorDetailOut {
    pub symbol: String,
    pub name: String,
    pub sector_type: String,
    pub trade_date: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub pre_close: f64,
    pub change_pct: f64,
    pub amount: f64,
    pub volume: f64,
    pub change_5d_pct: Option<f64>,
    pub change_10d_pct: Option<f64>,
    /// 成分股（`sc_sector_constituent`），按当日涨跌幅降序
    pub constituents: Vec<SectorConstituentRow>,
}

fn change_pct(close: f64, pre_close: f64) -> f64 {
    if pre_close > 1e-10 {
        (close / pre_close - 1.0) * 100.0
    } else {
        0.0
    }
}

fn change_nd(closes: &[f64], n: usize) -> Option<f64> {
    if n == 0 || closes.len() <= n {
        return None;
    }
    let last = *closes.last()?;
    let past = closes[closes.len() - 1 - n];
    if past > 1e-10 {
        Some((last / past - 1.0) * 100.0)
    } else {
        None
    }
}

fn parse_types(raw: Option<&str>) -> Vec<String> {
    let s = raw.map(str::trim).filter(|x| !x.is_empty());
    match s {
        None => DEFAULT_TYPES.iter().map(|x| (*x).to_string()).collect(),
        Some(v) => v
            .split(',')
            .map(str::trim)
            .filter(|x| !x.is_empty())
            .map(|x| x.to_ascii_uppercase())
            .collect(),
    }
}

struct ConstituentCountCache {
    loaded_at: Instant,
    /// sector_symbol → 成分股数
    counts: HashMap<String, u32>,
}

fn constituent_count_cache() -> &'static Mutex<Option<ConstituentCountCache>> {
    static C: OnceLock<Mutex<Option<ConstituentCountCache>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(None))
}

fn cache_get_constituent_counts() -> Option<HashMap<String, u32>> {
    let g = constituent_count_cache().lock().ok()?;
    let c = g.as_ref()?;
    if c.loaded_at.elapsed() > CONSTITUENT_COUNT_TTL {
        return None;
    }
    Some(c.counts.clone())
}

fn cache_put_constituent_counts(counts: HashMap<String, u32>) {
    if let Ok(mut g) = constituent_count_cache().lock() {
        *g = Some(ConstituentCountCache {
            loaded_at: Instant::now(),
            counts,
        });
    }
}

async fn load_constituent_counts(
    db: &DatabaseConnection,
) -> Result<HashMap<String, u32>, DbErr> {
    if let Some(hit) = cache_get_constituent_counts() {
        return Ok(hit);
    }

    #[derive(Debug, FromQueryResult)]
    struct Cnt {
        sector_symbol: String,
        n: i64,
    }

    let rows = Cnt::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
SELECT sector_symbol, COUNT(*)::bigint AS n
FROM sc_sector_constituent
GROUP BY sector_symbol
"#,
        [],
    ))
    .all(db)
    .await?;

    let counts: HashMap<String, u32> = rows
        .into_iter()
        .map(|r| (r.sector_symbol, r.n.max(0) as u32))
        .collect();
    cache_put_constituent_counts(counts.clone());
    tracing::info!(
        sectors = counts.len(),
        ttl_secs = CONSTITUENT_COUNT_TTL.as_secs(),
        "板块成分股数量已缓存"
    );
    Ok(counts)
}

/// 领涨 / 领跌 TopN（按当日涨跌幅）；过滤成分股数 < 20。
/// 交易时段拉 Miana 板块实时并 UPSERT；非交易时段先读当日快照，无数据再拉实时并落库，最后回退日 K。
pub async fn load_movers(
    db: &DatabaseConnection,
    miana: &super::miana::MianaClient,
    top_n: Option<usize>,
    types: Option<&str>,
) -> anyhow::Result<SectorMoversOut> {
    let top_n = top_n.unwrap_or(DEFAULT_TOP_N).clamp(1, 50);
    let types = parse_types(types);
    if types.is_empty() {
        return Ok(SectorMoversOut {
            trade_date: String::new(),
            top_n,
            types,
            quote_source: "kline".into(),
            gainers: vec![],
            losers: vec![],
        });
    }

    let counts = load_constituent_counts(db).await?;
    let metas = load_eligible_sector_meta(db, &types, &counts).await?;
    if metas.is_empty() {
        return Ok(SectorMoversOut {
            trade_date: String::new(),
            top_n,
            types,
            quote_source: "kline".into(),
            gainers: vec![],
            losers: vec![],
        });
    }

    let today = super::session::shanghai_today_naive();
    let open_today = super::data_source::is_open_trade_day(db, today)
        .await
        .unwrap_or(false);
    let in_session = open_today && super::session::is_realtime_session_now() && miana.token_configured();

    let (mut all, trade_date, quote_source) = if in_session {
        match fetch_persist_realtime_movers(db, miana, &metas, &counts, today).await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(error = %e, "板块实时拉取失败，回退快照/日K");
                load_movers_offline(db, miana, &metas, &counts, today).await?
            }
        }
    } else {
        load_movers_offline(db, miana, &metas, &counts, today).await?
    };

    all.sort_by(|a, b| b.change_pct.total_cmp(&a.change_pct));
    let mut gainers: Vec<_> = all.iter().take(top_n).cloned().collect();
    let mut by_asc = all;
    by_asc.sort_by(|a, b| a.change_pct.total_cmp(&b.change_pct));
    let mut losers: Vec<_> = by_asc.into_iter().take(top_n).collect();

    fill_nd_changes(db, &mut gainers).await?;
    fill_nd_changes(db, &mut losers).await?;

    Ok(SectorMoversOut {
        trade_date,
        top_n,
        types,
        quote_source,
        gainers,
        losers,
    })
}

#[derive(Debug, Clone)]
struct SectorMeta {
    symbol: String,
    name: String,
    sector_type: String,
}

async fn load_eligible_sector_meta(
    db: &DatabaseConnection,
    types: &[String],
    counts: &HashMap<String, u32>,
) -> Result<Vec<SectorMeta>, DbErr> {
    #[derive(Debug, FromQueryResult)]
    struct Row {
        symbol: String,
        name: String,
        sector_type: String,
    }
    let rows = Row::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
SELECT symbol, name, sector_type
FROM sc_sector
WHERE is_active = TRUE
  AND sector_type = ANY($1)
"#,
        [types.to_vec().into()],
    ))
    .all(db)
    .await?;

    Ok(rows
        .into_iter()
        .filter(|r| *counts.get(&r.symbol).unwrap_or(&0) >= MIN_CONSTITUENT_COUNT)
        .map(|r| SectorMeta {
            symbol: r.symbol,
            name: r.name,
            sector_type: r.sector_type,
        })
        .collect())
}

fn rows_from_realtime(
    metas: &[SectorMeta],
    counts: &HashMap<String, u32>,
    snaps: &[super::miana::SectorRealtimeSnapshot],
) -> Vec<SectorMoverRow> {
    let by_sym: HashMap<&str, &super::miana::SectorRealtimeSnapshot> =
        snaps.iter().map(|s| (s.symbol.as_str(), s)).collect();
    metas
        .iter()
        .filter_map(|m| {
            let s = by_sym.get(m.symbol.as_str())?;
            let pre = s.pre_close.filter(|v| *v > 1e-10).unwrap_or(0.0);
            let chg = s
                .change_rate
                .unwrap_or_else(|| change_pct(s.price, pre));
            Some(SectorMoverRow {
                symbol: m.symbol.clone(),
                name: s.name.clone().unwrap_or_else(|| m.name.clone()),
                sector_type: s
                    .sector_type
                    .clone()
                    .unwrap_or_else(|| m.sector_type.clone()),
                change_pct: chg,
                change_5d_pct: None,
                change_10d_pct: None,
                close: s.price,
                pre_close: pre,
                amount: s.amount.unwrap_or(0.0),
                volume: s.volume.unwrap_or(0.0),
                constituent_count: *counts.get(&m.symbol).unwrap_or(&0),
            })
        })
        .collect()
}

/// 拉 Miana 板块实时 → UPSERT → 组装榜单行。
async fn fetch_persist_realtime_movers(
    db: &DatabaseConnection,
    miana: &super::miana::MianaClient,
    metas: &[SectorMeta],
    counts: &HashMap<String, u32>,
    today: NaiveDate,
) -> anyhow::Result<(Vec<SectorMoverRow>, String, String)> {
    let symbols: Vec<String> = metas.iter().map(|m| m.symbol.clone()).collect();
    let trade_date = today.format("%Y-%m-%d").to_string();
    let snaps = miana
        .fetch_sector_realtime_snapshots(&symbols, &trade_date)
        .await?;
    if let Err(e) = upsert_sector_realtime(db, &snaps).await {
        tracing::warn!(error = %e, "板块实时落库失败，仍用内存截面排序");
    }
    let rows = rows_from_realtime(metas, counts, &snaps);
    Ok((rows, trade_date, "realtime".into()))
}

/// 非交易时段：当日快照 →（无则）实时 API 并落库 → 日 K。
async fn load_movers_offline(
    db: &DatabaseConnection,
    miana: &super::miana::MianaClient,
    metas: &[SectorMeta],
    counts: &HashMap<String, u32>,
    today: NaiveDate,
) -> anyhow::Result<(Vec<SectorMoverRow>, String, String)> {
    if let Some((td, rows)) = load_movers_from_snapshot(db, metas, counts, today).await? {
        return Ok((rows, td, "snapshot".into()));
    }
    if miana.token_configured() {
        match fetch_persist_realtime_movers(db, miana, metas, counts, today).await {
            Ok(v) => return Ok(v),
            Err(e) => {
                tracing::warn!(error = %e, "非交易时段板块实时拉取失败，回退日K");
            }
        }
    }
    let (td, rows) = load_movers_from_kline(db, metas, counts).await?;
    Ok((rows, td, "kline".into()))
}

async fn load_movers_from_snapshot(
    db: &DatabaseConnection,
    metas: &[SectorMeta],
    counts: &HashMap<String, u32>,
    today: NaiveDate,
) -> Result<Option<(String, Vec<SectorMoverRow>)>, DbErr> {
    #[derive(Debug, FromQueryResult)]
    struct Row {
        trade_date: NaiveDate,
        symbol: String,
        name: Option<String>,
        sector_type: Option<String>,
        price: f64,
        pre_close: Option<f64>,
        amount: Option<f64>,
        volume: Option<f64>,
        change_rate: Option<f64>,
    }

    let meta_by: HashMap<&str, &SectorMeta> =
        metas.iter().map(|m| (m.symbol.as_str(), m)).collect();
    let symbols: Vec<String> = metas.iter().map(|m| m.symbol.clone()).collect();
    if symbols.is_empty() {
        return Ok(None);
    }

    let rows = Row::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
SELECT
    trade_date,
    symbol,
    name,
    sector_type,
    price,
    pre_close,
    amount,
    volume,
    change_rate
FROM sector_realtime_snapshot
WHERE trade_date = $1
  AND symbol = ANY($2)
"#,
        [today.into(), symbols.into()],
    ))
    .all(db)
    .await?;

    if rows.is_empty() {
        return Ok(None);
    }

    let trade_date = rows[0].trade_date.format("%Y-%m-%d").to_string();
    let out: Vec<SectorMoverRow> = rows
        .into_iter()
        .filter_map(|r| {
            let m = meta_by.get(r.symbol.as_str())?;
            let pre = r.pre_close.filter(|v| *v > 1e-10).unwrap_or(0.0);
            let chg = r.change_rate.unwrap_or_else(|| change_pct(r.price, pre));
            Some(SectorMoverRow {
                symbol: r.symbol.clone(),
                name: r.name.unwrap_or_else(|| m.name.clone()),
                sector_type: r.sector_type.unwrap_or_else(|| m.sector_type.clone()),
                change_pct: chg,
                change_5d_pct: None,
                change_10d_pct: None,
                close: r.price,
                pre_close: pre,
                amount: r.amount.unwrap_or(0.0),
                volume: r.volume.unwrap_or(0.0),
                constituent_count: *counts.get(&r.symbol).unwrap_or(&0),
            })
        })
        .collect();
    if out.is_empty() {
        Ok(None)
    } else {
        Ok(Some((trade_date, out)))
    }
}

async fn load_movers_from_kline(
    db: &DatabaseConnection,
    metas: &[SectorMeta],
    counts: &HashMap<String, u32>,
) -> Result<(String, Vec<SectorMoverRow>), DbErr> {
    #[derive(Debug, FromQueryResult)]
    struct Row {
        trade_date: NaiveDate,
        symbol: String,
        name: String,
        sector_type: String,
        close: f64,
        pre_close: f64,
        amount: f64,
        volume: f64,
    }

    let symbols: Vec<String> = metas.iter().map(|m| m.symbol.clone()).collect();
    let rows = Row::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
WITH latest AS (
    SELECT MAX(trade_date) AS d FROM sc_sector_kline_d1
)
SELECT
    k.trade_date,
    s.symbol,
    s.name,
    s.sector_type,
    k.close::double precision AS close,
    k.pre_close::double precision AS pre_close,
    k.amount::double precision AS amount,
    k.volume::double precision AS volume
FROM sc_sector_kline_d1 k
INNER JOIN latest l ON k.trade_date = l.d
INNER JOIN sc_sector s
    ON s.symbol = k.symbol
   AND s.is_active = TRUE
WHERE k.symbol = ANY($1)
  AND k.pre_close > 0
"#,
        [symbols.into()],
    ))
    .all(db)
    .await?;

    let trade_date = rows
        .first()
        .map(|r| r.trade_date.format("%Y-%m-%d").to_string())
        .unwrap_or_default();
    let out = rows
        .into_iter()
        .map(|r| SectorMoverRow {
            change_pct: change_pct(r.close, r.pre_close),
            change_5d_pct: None,
            change_10d_pct: None,
            symbol: r.symbol.clone(),
            name: r.name,
            sector_type: r.sector_type,
            close: r.close,
            pre_close: r.pre_close,
            amount: r.amount,
            volume: r.volume,
            constituent_count: *counts.get(&r.symbol).unwrap_or(&0),
        })
        .collect();
    Ok((trade_date, out))
}

async fn upsert_sector_realtime(
    db: &DatabaseConnection,
    snaps: &[super::miana::SectorRealtimeSnapshot],
) -> Result<(), DbErr> {
    for chunk in snaps.chunks(80) {
        let mut values = String::new();
        let mut params: Vec<sea_orm::Value> = Vec::new();
        for (i, s) in chunk.iter().enumerate() {
            if i > 0 {
                values.push(',');
            }
            let base = i * 16;
            values.push_str(&format!(
                "(${},${},${},${},${},${},${},${},${},${},${},${},${},${},${},${}::timestamptz)",
                base + 1,
                base + 2,
                base + 3,
                base + 4,
                base + 5,
                base + 6,
                base + 7,
                base + 8,
                base + 9,
                base + 10,
                base + 11,
                base + 12,
                base + 13,
                base + 14,
                base + 15,
                base + 16,
            ));
            let td = NaiveDate::parse_from_str(&s.trade_date, "%Y-%m-%d")
                .unwrap_or_else(|_| super::session::shanghai_today_naive());
            params.push(s.symbol.clone().into());
            params.push(td.into());
            params.push(s.name.clone().into());
            params.push(s.sector_type.clone().into());
            params.push(s.price.into());
            params.push(s.open.into());
            params.push(s.high.into());
            params.push(s.low.into());
            params.push(s.pre_close.into());
            params.push(s.volume.into());
            params.push(s.amount.into());
            params.push(s.change_amt.into());
            params.push(s.change_rate.into());
            params.push(s.quote_ts.into());
            params.push(s.local_date.clone().into());
            params.push(s.fetched_at.clone().into());
        }

        let sql = format!(
            r#"
INSERT INTO sector_realtime_snapshot (
    symbol, trade_date, name, sector_type, price, open, high, low, pre_close,
    volume, amount, change_amt, change_rate, quote_ts, local_date, fetched_at
) VALUES {values}
ON CONFLICT (symbol, trade_date) DO UPDATE SET
    name = EXCLUDED.name,
    sector_type = EXCLUDED.sector_type,
    price = EXCLUDED.price,
    open = EXCLUDED.open,
    high = EXCLUDED.high,
    low = EXCLUDED.low,
    pre_close = EXCLUDED.pre_close,
    volume = EXCLUDED.volume,
    amount = EXCLUDED.amount,
    change_amt = EXCLUDED.change_amt,
    change_rate = EXCLUDED.change_rate,
    quote_ts = EXCLUDED.quote_ts,
    local_date = EXCLUDED.local_date,
    fetched_at = EXCLUDED.fetched_at
"#
        );

        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            &sql,
            params,
        ))
        .await?;
    }
    Ok(())
}

/// 仅为榜单内标的补 5/10 日涨跌（避免全市场扫 K）。
async fn fill_nd_changes(
    db: &DatabaseConnection,
    rows: &mut [SectorMoverRow],
) -> Result<(), DbErr> {
    if rows.is_empty() {
        return Ok(());
    }
    let symbols: Vec<String> = rows.iter().map(|r| r.symbol.clone()).collect();

    #[derive(Debug, FromQueryResult)]
    struct CRow {
        symbol: String,
        #[allow(dead_code)]
        trade_date: NaiveDate,
        close: f64,
    }

    let sql = r#"
SELECT symbol, trade_date, close::double precision AS close
FROM (
    SELECT
        symbol,
        trade_date,
        close,
        ROW_NUMBER() OVER (PARTITION BY symbol ORDER BY trade_date DESC) AS rn
    FROM sc_sector_kline_d1
    WHERE symbol = ANY($1)
) t
WHERE rn <= 11
ORDER BY symbol, trade_date ASC
"#;

    let krows = CRow::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        sql,
        [symbols.into()],
    ))
    .all(db)
    .await?;

    let mut by_sym: HashMap<String, Vec<f64>> = HashMap::new();
    for r in krows {
        by_sym.entry(r.symbol).or_default().push(r.close);
    }
    for row in rows.iter_mut() {
        if let Some(closes) = by_sym.get(&row.symbol) {
            row.change_5d_pct = change_nd(closes, 5);
            row.change_10d_pct = change_nd(closes, 10);
        }
    }
    Ok(())
}

/// 板块详情：当日行情优先板块实时/快照；5/10 日用日 K（末日可被实时覆盖）；成分股同理。
pub async fn load_detail(
    db: &DatabaseConnection,
    miana: &super::miana::MianaClient,
    symbol: &str,
) -> anyhow::Result<Option<SectorDetailOut>> {
    let symbol = symbol.trim();
    if symbol.is_empty() || symbol.len() > 32 {
        return Ok(None);
    }

    #[derive(Debug, FromQueryResult)]
    struct Meta {
        symbol: String,
        name: String,
        sector_type: String,
    }

    let meta = Meta::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
SELECT symbol, name, sector_type
FROM sc_sector
WHERE symbol = $1 AND is_active = TRUE
LIMIT 1
"#,
        [symbol.into()],
    ))
    .one(db)
    .await?;

    let Some(meta) = meta else {
        return Ok(None);
    };

    #[derive(Debug, FromQueryResult)]
    struct KRow {
        trade_date: NaiveDate,
        open: f64,
        high: f64,
        low: f64,
        close: f64,
        pre_close: f64,
        amount: f64,
        volume: f64,
    }

    let krows = KRow::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
SELECT
    trade_date,
    open::double precision AS open,
    high::double precision AS high,
    low::double precision AS low,
    close::double precision AS close,
    pre_close::double precision AS pre_close,
    amount::double precision AS amount,
    volume::double precision AS volume
FROM sc_sector_kline_d1
WHERE symbol = $1
ORDER BY trade_date DESC
LIMIT 11
"#,
        [meta.symbol.clone().into()],
    ))
    .all(db)
    .await?;

    let today = super::session::shanghai_today_naive();
    let live = resolve_sector_header_quote(db, miana, &meta.symbol, today).await?;

    // 无日 K 且无实时则无法展示
    if krows.is_empty() && live.is_none() {
        return Ok(None);
    }

    let mut closes_asc: Vec<f64> = {
        let mut v: Vec<f64> = krows.iter().map(|r| r.close).collect();
        v.reverse();
        v
    };

    let (
        trade_date,
        open,
        high,
        low,
        close,
        pre_close,
        amount,
        volume,
        change,
    ) = if let Some(l) = live {
        if let Some(last) = closes_asc.last_mut() {
            *last = l.price;
        } else {
            closes_asc.push(l.price);
        }
        let pre = l.pre_close.filter(|v| *v > 1e-10).unwrap_or(0.0);
        let chg = l
            .change_rate
            .unwrap_or_else(|| change_pct(l.price, pre));
        (
            l.trade_date,
            l.open.unwrap_or(l.price),
            l.high.unwrap_or(l.price),
            l.low.unwrap_or(l.price),
            l.price,
            pre,
            l.amount.unwrap_or(0.0),
            l.volume.unwrap_or(0.0),
            chg,
        )
    } else {
        let latest = &krows[0];
        (
            latest.trade_date.format("%Y-%m-%d").to_string(),
            latest.open,
            latest.high,
            latest.low,
            latest.close,
            latest.pre_close,
            latest.amount,
            latest.volume,
            change_pct(latest.close, latest.pre_close),
        )
    };

    let constituents = load_constituents(db, miana, &meta.symbol).await?;

    Ok(Some(SectorDetailOut {
        symbol: meta.symbol,
        name: meta.name,
        sector_type: meta.sector_type,
        trade_date,
        open,
        high,
        low,
        close,
        pre_close,
        change_pct: change,
        amount,
        volume,
        change_5d_pct: change_nd(&closes_asc, 5),
        change_10d_pct: change_nd(&closes_asc, 10),
        constituents,
    }))
}

#[derive(Debug, Clone)]
struct SectorHeaderLive {
    trade_date: String,
    price: f64,
    open: Option<f64>,
    high: Option<f64>,
    low: Option<f64>,
    pre_close: Option<f64>,
    amount: Option<f64>,
    volume: Option<f64>,
    change_rate: Option<f64>,
}

/// 盘中拉实时并落库；非盘中先读 `sector_realtime_snapshot`，无则再拉实时。
async fn resolve_sector_header_quote(
    db: &DatabaseConnection,
    miana: &super::miana::MianaClient,
    symbol: &str,
    today: NaiveDate,
) -> anyhow::Result<Option<SectorHeaderLive>> {
    let today_s = today.format("%Y-%m-%d").to_string();
    let open_today = super::data_source::is_open_trade_day(db, today)
        .await
        .unwrap_or(false);
    let in_session =
        open_today && super::session::is_realtime_session_now() && miana.token_configured();

    if !in_session {
        if let Some(snap) = load_one_sector_snapshot(db, symbol, today).await? {
            return Ok(Some(snap));
        }
    }

    if !miana.token_configured() {
        return Ok(if in_session {
            None
        } else {
            load_one_sector_snapshot(db, symbol, today).await?
        });
    }

    match miana
        .fetch_sector_realtime_snapshots(&[symbol.to_string()], &today_s)
        .await
    {
        Ok(snaps) => {
            if let Err(e) = upsert_sector_realtime(db, &snaps).await {
                tracing::warn!(error = %e, "板块详情实时落库失败");
            }
            Ok(snaps.into_iter().next().map(|s| SectorHeaderLive {
                trade_date: s.trade_date,
                price: s.price,
                open: s.open,
                high: s.high,
                low: s.low,
                pre_close: s.pre_close,
                amount: s.amount,
                volume: s.volume,
                change_rate: s.change_rate,
            }))
        }
        Err(e) => {
            tracing::warn!(error = %e, "板块详情实时拉取失败，回退快照/日K");
            Ok(load_one_sector_snapshot(db, symbol, today).await?)
        }
    }
}

async fn load_one_sector_snapshot(
    db: &DatabaseConnection,
    symbol: &str,
    today: NaiveDate,
) -> Result<Option<SectorHeaderLive>, DbErr> {
    #[derive(Debug, FromQueryResult)]
    struct Row {
        trade_date: NaiveDate,
        price: f64,
        open: Option<f64>,
        high: Option<f64>,
        low: Option<f64>,
        pre_close: Option<f64>,
        amount: Option<f64>,
        volume: Option<f64>,
        change_rate: Option<f64>,
    }

    let row = Row::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
SELECT
    trade_date,
    price,
    open,
    high,
    low,
    pre_close,
    amount,
    volume,
    change_rate
FROM sector_realtime_snapshot
WHERE trade_date = $1
  AND symbol = $2
LIMIT 1
"#,
        [today.into(), symbol.into()],
    ))
    .one(db)
    .await?;

    Ok(row.filter(|r| r.price > 0.0).map(|r| SectorHeaderLive {
        trade_date: r.trade_date.format("%Y-%m-%d").to_string(),
        price: r.price,
        open: r.open,
        high: r.high,
        low: r.low,
        pre_close: r.pre_close,
        amount: r.amount,
        volume: r.volume,
        change_rate: r.change_rate,
    }))
}

#[derive(Debug, Clone)]
struct LiveQuote {
    price: f64,
    pre_close: f64,
    amount: f64,
    change_rate: Option<f64>,
}

/// 当日各标的最新一条 `market_realtime_snapshot`。
async fn load_latest_stock_realtime(
    db: &DatabaseConnection,
    symbols: &[String],
    trade_date: &str,
) -> Result<HashMap<String, LiveQuote>, DbErr> {
    if symbols.is_empty() {
        return Ok(HashMap::new());
    }

    #[derive(Debug, FromQueryResult)]
    struct Row {
        symbol: String,
        price: f64,
        pre_close: Option<f64>,
        amount: Option<f64>,
        change_rate: Option<f64>,
    }

    let rows = Row::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
SELECT DISTINCT ON (symbol)
    symbol,
    price,
    pre_close,
    amount,
    change_rate
FROM market_realtime_snapshot
WHERE trade_date = $1
  AND symbol = ANY($2)
ORDER BY symbol, quote_ts DESC
"#,
        [trade_date.into(), symbols.to_vec().into()],
    ))
    .all(db)
    .await?;

    Ok(rows
        .into_iter()
        .filter(|r| r.price > 0.0)
        .map(|r| {
            (
                r.symbol,
                LiveQuote {
                    price: r.price,
                    pre_close: r.pre_close.filter(|v| *v > 1e-10).unwrap_or(0.0),
                    amount: r.amount.unwrap_or(0.0),
                    change_rate: r.change_rate,
                },
            )
        })
        .collect())
}

/// 成分股行情：现价优先 `market_realtime_snapshot` / Miana；5/10 日用日 K（末日可用实时价覆盖）。
async fn load_constituents(
    db: &DatabaseConnection,
    miana: &super::miana::MianaClient,
    sector_symbol: &str,
) -> anyhow::Result<Vec<SectorConstituentRow>> {
    #[derive(Debug, FromQueryResult)]
    struct Mem {
        stock_symbol: String,
        name: String,
    }

    let members = Mem::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
SELECT stock_symbol, COALESCE(name, '') AS name
FROM sc_sector_constituent
WHERE sector_symbol = $1
ORDER BY stock_symbol
"#,
        [sector_symbol.into()],
    ))
    .all(db)
    .await?;

    if members.is_empty() {
        return Ok(vec![]);
    }

    let symbols: Vec<String> = members.iter().map(|m| m.stock_symbol.clone()).collect();

    #[derive(Debug, FromQueryResult)]
    struct QuoteRow {
        symbol: String,
        close: f64,
        pre_close: f64,
        amount: f64,
    }

    let quotes = QuoteRow::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
SELECT DISTINCT ON (symbol)
    symbol,
    close::double precision AS close,
    pre_close::double precision AS pre_close,
    amount::double precision AS amount
FROM sc_stock_kline_d1_bfq
WHERE symbol = ANY($1)
ORDER BY symbol, trade_date DESC
"#,
        [symbols.clone().into()],
    ))
    .all(db)
    .await?;

    let quote_map: HashMap<String, QuoteRow> =
        quotes.into_iter().map(|q| (q.symbol.clone(), q)).collect();

    #[derive(Debug, FromQueryResult)]
    struct CRow {
        symbol: String,
        #[allow(dead_code)]
        trade_date: NaiveDate,
        close: f64,
    }

    let krows = CRow::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
SELECT symbol, trade_date, close::double precision AS close
FROM (
    SELECT
        symbol,
        trade_date,
        close,
        ROW_NUMBER() OVER (PARTITION BY symbol ORDER BY trade_date DESC) AS rn
    FROM sc_stock_kline_d1_bfq
    WHERE symbol = ANY($1)
) t
WHERE rn <= 11
ORDER BY symbol, trade_date ASC
"#,
        [symbols.clone().into()],
    ))
    .all(db)
    .await?;

    let mut closes_map: HashMap<String, Vec<f64>> = HashMap::new();
    for r in krows {
        closes_map.entry(r.symbol).or_default().push(r.close);
    }

    let today = super::session::shanghai_today();
    let today_d = super::session::shanghai_today_naive();
    let open_today = super::data_source::is_open_trade_day(db, today_d)
        .await
        .unwrap_or(false);
    let in_session =
        open_today && super::session::is_realtime_session_now() && miana.token_configured();

    let mut live = load_latest_stock_realtime(db, &symbols, &today).await?;

    let missing: Vec<String> = symbols
        .iter()
        .filter(|s| !live.contains_key(s.as_str()))
        .cloned()
        .collect();
    let should_fetch = miana.token_configured()
        && (in_session || live.is_empty() || !missing.is_empty());
    if should_fetch {
        let to_fetch = if in_session || live.is_empty() {
            symbols.clone()
        } else {
            missing
        };
        if !to_fetch.is_empty() {
            match miana.fetch_realtime_snapshots(&to_fetch, &today).await {
                Ok(snaps) => {
                    if let Err(e) = super::realtime_store::insert_snapshots(db, &snaps).await {
                        tracing::warn!(error = %e, "成分股实时落库失败，仍用内存截面");
                    }
                    for s in snaps {
                        if s.price <= 0.0 {
                            continue;
                        }
                        live.insert(
                            s.symbol,
                            LiveQuote {
                                price: s.price,
                                pre_close: s.pre_close.filter(|v| *v > 1e-10).unwrap_or(0.0),
                                amount: s.amount.unwrap_or(0.0),
                                change_rate: s.change_rate,
                            },
                        );
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "成分股实时拉取失败，回退快照/日K");
                }
            }
        }
    }

    let mut out: Vec<SectorConstituentRow> = members
        .into_iter()
        .map(|m| {
            let live_q = live.get(&m.stock_symbol);
            let kq = quote_map.get(&m.stock_symbol);
            let (close, change_pct, amount) = if let Some(l) = live_q {
                let chg = l
                    .change_rate
                    .unwrap_or_else(|| change_pct(l.price, l.pre_close));
                (Some(l.price), Some(chg), Some(l.amount))
            } else if let Some(q) = kq {
                (
                    Some(q.close),
                    Some(change_pct(q.close, q.pre_close)),
                    Some(q.amount),
                )
            } else {
                (None, None, None)
            };

            let mut closes = closes_map.get(&m.stock_symbol).cloned().unwrap_or_default();
            if let Some(px) = close {
                if let Some(last) = closes.last_mut() {
                    *last = px;
                } else {
                    closes.push(px);
                }
            }

            SectorConstituentRow {
                symbol: m.stock_symbol,
                name: m.name,
                close,
                change_pct,
                change_5d_pct: change_nd(&closes, 5),
                change_10d_pct: change_nd(&closes, 10),
                amount,
            }
        })
        .collect();

    out.sort_by(|a, b| match (b.change_pct, a.change_pct) {
        (Some(x), Some(y)) => x.total_cmp(&y),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.symbol.cmp(&b.symbol),
    });

    Ok(out)
}

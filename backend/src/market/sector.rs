//! 板块榜 / 详情：读 `sc_sector` + `sc_sector_kline_d1`。
//! 当日涨跌幅表内无独立字段，用 `(close/pre_close - 1)*100` 计算。
//! 成分股数量来自 `sc_sector_constituent`，进程内缓存 12 小时。

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use chrono::NaiveDate;
use sea_orm::{DatabaseConnection, DbBackend, DbErr, FromQueryResult, Statement};
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
pub async fn load_movers(
    db: &DatabaseConnection,
    top_n: Option<usize>,
    types: Option<&str>,
) -> Result<SectorMoversOut, DbErr> {
    let top_n = top_n.unwrap_or(DEFAULT_TOP_N).clamp(1, 50);
    let types = parse_types(types);
    if types.is_empty() {
        return Ok(SectorMoversOut {
            trade_date: String::new(),
            top_n,
            types,
            gainers: vec![],
            losers: vec![],
        });
    }

    let counts = load_constituent_counts(db).await?;

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

    let sql = r#"
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
WHERE s.sector_type = ANY($1)
  AND k.pre_close > 0
"#;

    let rows = Row::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        sql,
        [types.clone().into()],
    ))
    .all(db)
    .await?;

    let trade_date = rows
        .first()
        .map(|r| r.trade_date.format("%Y-%m-%d").to_string())
        .unwrap_or_default();

    let mut all: Vec<SectorMoverRow> = rows
        .into_iter()
        .filter_map(|r| {
            let constituent_count = *counts.get(&r.symbol).unwrap_or(&0);
            if constituent_count < MIN_CONSTITUENT_COUNT {
                return None;
            }
            Some(SectorMoverRow {
                change_pct: change_pct(r.close, r.pre_close),
                change_5d_pct: None,
                change_10d_pct: None,
                symbol: r.symbol,
                name: r.name,
                sector_type: r.sector_type,
                close: r.close,
                pre_close: r.pre_close,
                amount: r.amount,
                volume: r.volume,
                constituent_count,
            })
        })
        .collect();

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
        gainers,
        losers,
    })
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

/// 板块详情：当日行情 + 5/10 日涨跌 + 近 20 日 K。
pub async fn load_detail(
    db: &DatabaseConnection,
    symbol: &str,
) -> Result<Option<SectorDetailOut>, DbErr> {
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
        [symbol.into()],
    ))
    .all(db)
    .await?;

    if krows.is_empty() {
        return Ok(None);
    }

    let latest = &krows[0];
    let closes_asc: Vec<f64> = {
        let mut v: Vec<f64> = krows.iter().map(|r| r.close).collect();
        v.reverse();
        v
    };

    let constituents = load_constituents(db, &meta.symbol).await?;

    Ok(Some(SectorDetailOut {
        symbol: meta.symbol,
        name: meta.name,
        sector_type: meta.sector_type,
        trade_date: latest.trade_date.format("%Y-%m-%d").to_string(),
        open: latest.open,
        high: latest.high,
        low: latest.low,
        close: latest.close,
        pre_close: latest.pre_close,
        change_pct: change_pct(latest.close, latest.pre_close),
        amount: latest.amount,
        volume: latest.volume,
        change_5d_pct: change_nd(&closes_asc, 5),
        change_10d_pct: change_nd(&closes_asc, 10),
        constituents,
    }))
}

/// 成分股行情：`sc_sector_constituent` + `sc_stock_kline_d1_bfq`。
async fn load_constituents(
    db: &DatabaseConnection,
    sector_symbol: &str,
) -> Result<Vec<SectorConstituentRow>, DbErr> {
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
        [symbols.into()],
    ))
    .all(db)
    .await?;

    let mut closes_map: HashMap<String, Vec<f64>> = HashMap::new();
    for r in krows {
        closes_map.entry(r.symbol).or_default().push(r.close);
    }

    let mut out: Vec<SectorConstituentRow> = members
        .into_iter()
        .map(|m| {
            let q = quote_map.get(&m.stock_symbol);
            let closes = closes_map.get(&m.stock_symbol);
            SectorConstituentRow {
                symbol: m.stock_symbol,
                name: m.name,
                close: q.map(|x| x.close),
                change_pct: q.map(|x| change_pct(x.close, x.pre_close)),
                change_5d_pct: closes.and_then(|c| change_nd(c, 5)),
                change_10d_pct: closes.and_then(|c| change_nd(c, 10)),
                amount: q.map(|x| x.amount),
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

//! 从 SecuritiesCollector 表加载全市场日 K（不复权）。
//!
//! 表定义见 `SecuritiesCollector/backend/migrations`：
//! - `sc_stock`
//! - `sc_stock_kline_d1_bfq`
//! - `sc_trade_calendar`（`is_open`：该日是否开市，可含未来日期；`pre_trade_date`）
//!
//! 读历史窗口：取「≤ 上海今日」的最近 N 个开市日再 JOIN 日 K（未来开市日只用于判断，不进窗口）。
//! 实时覆盖条件：当日 `is_open` 且处于连续竞价时钟窗。

use std::collections::{BTreeMap, HashMap};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use chrono::NaiveDate;
use sea_orm::{DatabaseConnection, DbBackend, DbErr, FromQueryResult, Statement};

use super::domain::{MarketDailySnapshot, MarketDataInput, StockDailyData};
use super::miana::MianaClient;
use super::realtime_cache::RealtimeDayCache;
use super::session;
use crate::error::{AppError, AppResult};

/// 库内窗口结果短缓存（MTT 约 70 日 × 全市场，避免刷新连打）。
const WINDOW_CACHE_TTL: Duration = Duration::from_secs(60);

struct CachedWindow {
    fetched_at: Instant,
    input: MarketDataInput,
}

fn window_cache() -> &'static Mutex<HashMap<usize, CachedWindow>> {
    static CACHE: OnceLock<Mutex<HashMap<usize, CachedWindow>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Debug, FromQueryResult)]
struct KlineRow {
    symbol: String,
    trade_date: NaiveDate,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    pre_close: f64,
    amount: f64,
}

fn change_pct(close: f64, pre_close: f64) -> f64 {
    if pre_close > 1e-10 && pre_close.is_finite() && close.is_finite() {
        (close - pre_close) / pre_close * 100.0
    } else {
        0.0
    }
}

/// 取最近 `days` 个开市日（≤ 上海今日）、全部 `is_active` 股票的日 K。
///
/// 日历 `is_open` 只表示「这一天开不开市」，可含未来；取窗口时用
/// `trade_date <= 今日`，再按日期倒序 LIMIT N，然后 JOIN 日 K。
/// 非空结果短缓存 60s（按 `days` 分槽）。
pub async fn load_market_window(
    db: &DatabaseConnection,
    days: usize,
) -> Result<MarketDataInput, DbErr> {
    let days = days.max(1);
    let as_of = session::shanghai_today_naive();

    if let Ok(guard) = window_cache().lock() {
        if let Some(hit) = guard.get(&days) {
            if hit.fetched_at.elapsed() < WINDOW_CACHE_TTL {
                tracing::debug!(days, age_ms = hit.fetched_at.elapsed().as_millis() as u64, "命中行情窗口缓存");
                return Ok(hit.input.clone());
            }
        }
    }

    let t0 = Instant::now();
    let sql = r#"
WITH dates AS (
    SELECT trade_date
    FROM sc_trade_calendar
    WHERE is_open = TRUE
      AND trade_date <= $2
    ORDER BY trade_date DESC
    LIMIT $1
)
SELECT
    k.symbol,
    k.trade_date,
    k.open::double precision   AS open,
    k.high::double precision   AS high,
    k.low::double precision    AS low,
    k.close::double precision  AS close,
    k.pre_close::double precision AS pre_close,
    k.amount::double precision AS amount
FROM dates d
INNER JOIN sc_stock_kline_d1_bfq k
    ON k.trade_date = d.trade_date
INNER JOIN sc_stock s
    ON s.symbol = k.symbol
   AND s.is_active = TRUE
"#;

    let rows = KlineRow::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        sql,
        [(days as i64).into(), as_of.into()],
    ))
    .all(db)
    .await?;

    let n = rows.len();
    let input = rows_to_input(rows);
    tracing::info!(
        days,
        as_of = %as_of,
        rows = n,
        snapshots = input.snapshots.len(),
        elapsed_ms = t0.elapsed().as_millis() as u64,
        "已加载行情窗口"
    );

    if input.snapshots.is_empty() {
        log_empty_market_window(db, days, as_of).await;
    } else if let Ok(mut guard) = window_cache().lock() {
        guard.insert(
            days,
            CachedWindow {
                fetched_at: Instant::now(),
                input: input.clone(),
            },
        );
    }

    Ok(input)
}

/// 窗口为空时输出日历 / K 线上下文（对照服务器日志定位）。
async fn log_empty_market_window(db: &DatabaseConnection, days: usize, as_of: NaiveDate) {
    #[derive(Debug, FromQueryResult)]
    struct Ctx {
        open_cal_total: i64,
        open_cal_as_of: i64,
        klines: i64,
        stocks: i64,
        max_open_cal: Option<NaiveDate>,
        max_open_as_of: Option<NaiveDate>,
        max_kline: Option<NaiveDate>,
        recent_open_with_k: i64,
        recent_open_without_k: i64,
    }

    let sql = r#"
WITH recent AS (
    SELECT trade_date
    FROM sc_trade_calendar
    WHERE is_open = TRUE
      AND trade_date <= $2
    ORDER BY trade_date DESC
    LIMIT $1
)
SELECT
    (SELECT COUNT(*)::bigint FROM sc_trade_calendar WHERE is_open = TRUE) AS open_cal_total,
    (SELECT COUNT(*)::bigint FROM sc_trade_calendar WHERE is_open = TRUE AND trade_date <= $2) AS open_cal_as_of,
    (SELECT COUNT(*)::bigint FROM sc_stock_kline_d1_bfq) AS klines,
    (SELECT COUNT(*)::bigint FROM sc_stock WHERE is_active = TRUE) AS stocks,
    (SELECT MAX(trade_date) FROM sc_trade_calendar WHERE is_open = TRUE) AS max_open_cal,
    (SELECT MAX(trade_date) FROM sc_trade_calendar WHERE is_open = TRUE AND trade_date <= $2) AS max_open_as_of,
    (SELECT MAX(trade_date) FROM sc_stock_kline_d1_bfq) AS max_kline,
    (
        SELECT COUNT(*)::bigint FROM recent r
        WHERE EXISTS (
            SELECT 1 FROM sc_stock_kline_d1_bfq k WHERE k.trade_date = r.trade_date
        )
    ) AS recent_open_with_k,
    (
        SELECT COUNT(*)::bigint FROM recent r
        WHERE NOT EXISTS (
            SELECT 1 FROM sc_stock_kline_d1_bfq k WHERE k.trade_date = r.trade_date
        )
    ) AS recent_open_without_k
"#;

    match Ctx::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        sql,
        [(days as i64).into(), as_of.into()],
    ))
    .one(db)
    .await
    {
        Ok(Some(c)) => tracing::warn!(
            days,
            as_of = %as_of,
            open_cal_total = c.open_cal_total,
            open_cal_as_of = c.open_cal_as_of,
            klines = c.klines,
            active_stocks = c.stocks,
            max_open_cal = ?c.max_open_cal,
            max_open_as_of = ?c.max_open_as_of,
            max_kline = ?c.max_kline,
            recent_open_with_k = c.recent_open_with_k,
            recent_open_without_k = c.recent_open_without_k,
            "行情窗口为空：≤今日的开市日与 K 线对齐情况"
        ),
        Ok(None) => tracing::warn!(days, as_of = %as_of, "行情窗口为空：诊断查询无行"),
        Err(e) => tracing::warn!(days, as_of = %as_of, error = %e, "行情窗口为空：诊断查询失败"),
    }

    #[derive(Debug, FromQueryResult)]
    struct DayHit {
        trade_date: NaiveDate,
        has_k: bool,
    }

    let days_sql = r#"
SELECT
    c.trade_date,
    EXISTS (
        SELECT 1 FROM sc_stock_kline_d1_bfq k WHERE k.trade_date = c.trade_date
    ) AS has_k
FROM sc_trade_calendar c
WHERE c.is_open = TRUE
  AND c.trade_date <= $2
ORDER BY c.trade_date DESC
LIMIT $1
"#;
    match DayHit::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        days_sql,
        [(days as i64).into(), as_of.into()],
    ))
    .all(db)
    .await
    {
        Ok(rows) => {
            let detail: Vec<String> = rows
                .iter()
                .map(|r| format!("{}:has_k={}", r.trade_date, r.has_k))
                .collect();
            tracing::warn!(
                days,
                as_of = %as_of,
                recent_open_days = %detail.join(", "),
                "行情窗口为空：≤今日最近开市日明细"
            );
        }
        Err(e) => tracing::warn!(
            days,
            as_of = %as_of,
            error = %e,
            "行情窗口为空：开市日明细查询失败"
        ),
    }
}

/// 开市日且处于连续竞价时钟窗（MSI / 日内推送「应刷新」条件）。
pub async fn is_open_auction_session(db: &DatabaseConnection) -> bool {
    if !session::is_realtime_session_now() {
        return false;
    }
    match is_open_trade_day(db, session::shanghai_today_naive()).await {
        Ok(open) => open,
        Err(e) => {
            tracing::warn!(error = %e, "查询 is_open 失败，按非交易时段处理");
            false
        }
    }
}

/// `sc_trade_calendar.is_open`：当日是否开市。
pub async fn is_open_trade_day(db: &DatabaseConnection, day: NaiveDate) -> Result<bool, DbErr> {
    #[derive(Debug, FromQueryResult)]
    struct Row {
        is_open: bool,
    }

    let row = Row::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT is_open FROM sc_trade_calendar WHERE trade_date = $1 LIMIT 1",
        [day.into()],
    ))
    .one(db)
    .await?;

    Ok(row.map(|r| r.is_open).unwrap_or(false))
}

/// 加载窗口；**开市日**且连续竞价时钟窗内用 Miana 覆盖当日，其余读库。
pub async fn load_market_window_hybrid(
    db: &DatabaseConnection,
    days: usize,
    miana: &MianaClient,
    cache: &RealtimeDayCache,
) -> AppResult<MarketDataInput> {
    let mut input = load_market_window(db, days)
        .await
        .map_err(AppError::from)?;

    if !session::is_realtime_session_now() {
        return Ok(input);
    }

    let today_d = session::shanghai_today_naive();
    let today = session::shanghai_today();
    match is_open_trade_day(db, today_d).await {
        Ok(true) => {}
        Ok(false) => {
            tracing::info!(date = %today, "日历 is_open=false，跳过实时");
            return Ok(input);
        }
        Err(e) => {
            tracing::warn!(error = %e, "查询 sc_trade_calendar.is_open 失败，跳过实时");
            return Ok(input);
        }
    }

    let symbols = load_active_symbols(db).await.map_err(AppError::from)?;
    if symbols.is_empty() {
        tracing::warn!(
            date = %today,
            "开市竞价需拉实时但 sc_stock 无 is_active 标的"
        );
        return Err(AppError::BadRequest(
            "无活跃股票：请确认 sc_stock.is_active 已由 SecuritiesCollector 写入".into(),
        ));
    }

    tracing::info!(
        date = %today,
        symbols = symbols.len(),
        "开市日竞价时段：当日截面改用 Miana 实时行情"
    );

    let stocks = cache.get_or_fetch(miana, &symbols, &today).await?;
    apply_realtime_latest_day(&mut input, stocks, &today, days);
    Ok(input)
}

/// 批量查证券简称：`sc_stock.name`（按 symbol）。
pub async fn load_stock_names(
    db: &DatabaseConnection,
    symbols: &[String],
) -> Result<HashMap<String, String>, DbErr> {
    if symbols.is_empty() {
        return Ok(HashMap::new());
    }

    #[derive(Debug, FromQueryResult)]
    struct NameRow {
        symbol: String,
        name: String,
    }

    // ANY($1) 需要 text[]；用 Postgres 数组字面量更省事
    let mut list = String::from("ARRAY[");
    for (i, s) in symbols.iter().enumerate() {
        if i > 0 {
            list.push(',');
        }
        list.push('\'');
        list.push_str(&s.replace('\'', "''"));
        list.push('\'');
    }
    list.push_str("]::text[]");

    let sql = format!(
        "SELECT symbol, COALESCE(name, '') AS name FROM sc_stock WHERE symbol = ANY({list})"
    );
    let rows = NameRow::find_by_statement(Statement::from_string(DbBackend::Postgres, sql))
        .all(db)
        .await?;

    Ok(rows.into_iter().map(|r| (r.symbol, r.name)).collect())
}

/// 活跃股票完整代码（`代码.市场后缀`）。
pub async fn load_active_symbols(db: &DatabaseConnection) -> Result<Vec<String>, DbErr> {
    #[derive(Debug, FromQueryResult)]
    struct Sym {
        symbol: String,
    }

    let rows = Sym::find_by_statement(Statement::from_string(
        DbBackend::Postgres,
        r#"
SELECT symbol
FROM sc_stock
WHERE is_active = TRUE
ORDER BY symbol ASC
"#
        .to_string(),
    ))
    .all(db)
    .await?;

    Ok(rows.into_iter().map(|r| r.symbol).collect())
}

/// 用实时快照替换或追加最新日，并裁剪到 `days` 天。
fn apply_realtime_latest_day(
    input: &mut MarketDataInput,
    stocks: Vec<StockDailyData>,
    today: &str,
    days: usize,
) {
    let snap = MarketDailySnapshot {
        date: today.to_string(),
        stocks,
    };

    match input.snapshots.last().map(|s| s.date.as_str()) {
        Some(last) if last == today => {
            if let Some(slot) = input.snapshots.last_mut() {
                *slot = snap;
            }
        }
        Some(last) if last < today => {
            input.snapshots.push(snap);
        }
        Some(_) => {
            // 库内日期超前：仍强制用今日实时覆盖末日（极端时钟/脏数据）
            if let Some(slot) = input.snapshots.last_mut() {
                *slot = snap;
            }
        }
        None => {
            input.snapshots.push(snap);
        }
    }

    let days = days.max(1);
    if input.snapshots.len() > days {
        let skip = input.snapshots.len() - days;
        input.snapshots = input.snapshots.split_off(skip);
    }
}

fn rows_to_input(rows: Vec<KlineRow>) -> MarketDataInput {
    let mut by_date: BTreeMap<NaiveDate, Vec<StockDailyData>> = BTreeMap::new();

    for row in rows {
        let date = row.trade_date.format("%Y-%m-%d").to_string();
        let stock = StockDailyData {
            symbol: row.symbol,
            date: date.clone(),
            open: row.open,
            high: row.high,
            low: row.low,
            close_price: row.close,
            amount: row.amount.max(0.0),
            change_pct: change_pct(row.close, row.pre_close),
        };
        by_date.entry(row.trade_date).or_default().push(stock);
    }

    let snapshots = by_date
        .into_iter()
        .map(|(d, stocks)| MarketDailySnapshot {
            date: d.format("%Y-%m-%d").to_string(),
            stocks,
        })
        .collect();

    MarketDataInput { snapshots }
}

/// 探测行情表是否可读（用于启动日志；失败不阻断进程）。
pub async fn probe_market_tables(db: &DatabaseConnection) -> Result<(i64, Option<NaiveDate>), DbErr> {
    #[derive(Debug, FromQueryResult)]
    struct Probe {
        stock_count: i64,
        last_date: Option<NaiveDate>,
    }

    let row = Probe::find_by_statement(Statement::from_string(
        DbBackend::Postgres,
        r#"
SELECT
    (SELECT COUNT(*)::bigint FROM sc_stock WHERE is_active = TRUE) AS stock_count,
    (SELECT MAX(trade_date) FROM sc_trade_calendar WHERE is_open = TRUE) AS last_date
"#
        .to_string(),
    ))
    .one(db)
    .await?;

    match row {
        Some(p) => Ok((p.stock_count, p.last_date)),
        None => Ok((0, None)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_change_pct() {
        assert!((change_pct(110.0, 100.0) - 10.0).abs() < 1e-9);
        assert!((change_pct(90.0, 100.0) + 10.0).abs() < 1e-9);
        assert_eq!(change_pct(10.0, 0.0), 0.0);
    }

    #[test]
    fn test_rows_to_input_groups_by_date() {
        let rows = vec![
            KlineRow {
                symbol: "600000.SH".into(),
                trade_date: NaiveDate::from_ymd_opt(2026, 10, 9).unwrap(),
                open: 10.0,
                high: 11.0,
                low: 9.5,
                close: 10.5,
                pre_close: 10.0,
                amount: 1e8,
            },
            KlineRow {
                symbol: "000001.SZ".into(),
                trade_date: NaiveDate::from_ymd_opt(2026, 10, 9).unwrap(),
                open: 12.0,
                high: 12.5,
                low: 11.8,
                close: 12.2,
                pre_close: 12.0,
                amount: 2e8,
            },
            KlineRow {
                symbol: "600000.SH".into(),
                trade_date: NaiveDate::from_ymd_opt(2026, 10, 10).unwrap(),
                open: 10.5,
                high: 10.8,
                low: 10.2,
                close: 10.6,
                pre_close: 10.5,
                amount: 1.1e8,
            },
        ];
        let input = rows_to_input(rows);
        assert_eq!(input.snapshots.len(), 2);
        assert_eq!(input.snapshots[0].date, "2026-10-09");
        assert_eq!(input.snapshots[0].stocks.len(), 2);
        assert_eq!(input.snapshots[1].stocks.len(), 1);
        assert!((input.snapshots[0].stocks[0].change_pct - 5.0).abs() < 1e-9
            || (input.snapshots[0].stocks[1].change_pct - 5.0).abs() < 1e-9);
    }

    #[test]
    fn test_apply_realtime_replaces_same_day() {
        let mut input = MarketDataInput {
            snapshots: vec![MarketDailySnapshot {
                date: "2026-10-09".into(),
                stocks: vec![],
            }],
        };
        apply_realtime_latest_day(
            &mut input,
            vec![StockDailyData {
                symbol: "600000.SH".into(),
                date: "2026-10-09".into(),
                open: 1.0,
                high: 1.0,
                low: 1.0,
                close_price: 1.0,
                amount: 1.0,
                change_pct: 0.0,
            }],
            "2026-10-09",
            5,
        );
        assert_eq!(input.snapshots.len(), 1);
        assert_eq!(input.snapshots[0].stocks.len(), 1);
    }

    #[test]
    fn test_apply_realtime_appends_and_trims() {
        let mut input = MarketDataInput {
            snapshots: vec![
                MarketDailySnapshot {
                    date: "2026-10-08".into(),
                    stocks: vec![],
                },
                MarketDailySnapshot {
                    date: "2026-10-09".into(),
                    stocks: vec![],
                },
            ],
        };
        apply_realtime_latest_day(&mut input, vec![], "2026-10-10", 2);
        assert_eq!(input.snapshots.len(), 2);
        assert_eq!(input.snapshots[0].date, "2026-10-09");
        assert_eq!(input.snapshots[1].date, "2026-10-10");
    }
}

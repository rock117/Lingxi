//! 从 SecuritiesCollector 表加载全市场日 K（不复权）。
//!
//! 表定义见 `SecuritiesCollector/backend/migrations`：
//! - `sc_stock`
//! - `sc_stock_kline_d1_bfq`
//! - `sc_trade_calendar`（最近 N 个交易日，避免对 K 线表 DISTINCT）
//!
//! 仅 MSI：盘中（上海时区周一至周五 09:30–18:00）最新交易日改用 Miana 实时行情覆盖。

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

/// 取最近 `days` 个交易日、全部 `is_active` 股票的日 K，组装为 MSI/MTT 输入。
///
/// 优先用 `sc_trade_calendar` 取最近 N 日；日历为空时回退到对 K 线表 DISTINCT。
/// 结果短缓存 60s（按 `days` 分槽）。
pub async fn load_market_window(
    db: &DatabaseConnection,
    days: usize,
) -> Result<MarketDataInput, DbErr> {
    let days = days.max(1);

    if let Ok(guard) = window_cache().lock() {
        if let Some(hit) = guard.get(&days) {
            if hit.fetched_at.elapsed() < WINDOW_CACHE_TTL {
                tracing::debug!(days, age_ms = hit.fetched_at.elapsed().as_millis() as u64, "命中行情窗口缓存");
                return Ok(hit.input.clone());
            }
        }
    }

    let t0 = Instant::now();
    let use_calendar = trade_calendar_has_rows(db).await?;
    let sql = if use_calendar {
        // 从轻量日历出发，走 (trade_date, symbol) 覆盖索引；排序交给 rows_to_input
        r#"
WITH dates AS (
    SELECT trade_date
    FROM sc_trade_calendar
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
"#
    } else {
        tracing::warn!("sc_trade_calendar 为空，回退 DISTINCT sc_stock_kline_d1_bfq.trade_date");
        r#"
WITH dates AS (
    SELECT DISTINCT trade_date
    FROM sc_stock_kline_d1_bfq
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
"#
    };

    let rows = KlineRow::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        sql,
        [(days as i64).into()],
    ))
    .all(db)
    .await?;

    let n = rows.len();
    let input = rows_to_input(rows);
    tracing::info!(
        days,
        rows = n,
        snapshots = input.snapshots.len(),
        calendar = use_calendar,
        elapsed_ms = t0.elapsed().as_millis() as u64,
        "已加载行情窗口"
    );

    if let Ok(mut guard) = window_cache().lock() {
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

async fn trade_calendar_has_rows(db: &DatabaseConnection) -> Result<bool, DbErr> {
    #[derive(Debug, FromQueryResult)]
    struct One {
        #[allow(dead_code)]
        n: i64,
    }

    let row = One::find_by_statement(Statement::from_string(
        DbBackend::Postgres,
        "SELECT 1::bigint AS n FROM sc_trade_calendar LIMIT 1".to_string(),
    ))
    .one(db)
    .await?;

    Ok(row.is_some())
}

/// 加载窗口；若处于盘中实时窗口，用 Miana 覆盖/补齐最新交易日。
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

    let today = session::shanghai_today();
    let symbols = load_active_symbols(db).await.map_err(AppError::from)?;
    if symbols.is_empty() {
        return Err(AppError::BadRequest(
            "无活跃股票：请确认 sc_stock.is_active 已由 SecuritiesCollector 写入".into(),
        ));
    }

    tracing::info!(
        date = %today,
        symbols = symbols.len(),
        "盘中窗口：最新交易日改用 Miana 实时行情"
    );

    let stocks = cache.get_or_fetch(miana, &symbols, &today).await?;
    apply_realtime_latest_day(&mut input, stocks, &today, days);
    Ok(input)
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
    COALESCE(
        (SELECT MAX(trade_date) FROM sc_trade_calendar),
        (SELECT MAX(trade_date) FROM sc_stock_kline_d1_bfq)
    ) AS last_date
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

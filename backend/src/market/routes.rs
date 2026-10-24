//! 市场指标 HTTP API
//! - MSI：`docs/market_sentiment.md`
//! - MTT：`docs/mid_term_trend.md`
//! - 日内 TopN（趋势/超跌）：`docs/intraday_alert.md`
//!
//! 一律读真实行情：`sc_stock` + `sc_stock_kline_d1_bfq`；
//! MSI / 日内推送在盘中对最新交易日叠加 Miana 实时行情。

use rocket::{get, post};
use rocket::serde::json::Json;
use sea_orm::{DatabaseConnection, EntityTrait};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use entity::condition;

use super::calculator;
use super::data_source;
use super::domain::{MarketDataInput, MsiResult, MttResult};
use super::intraday_alert::IntradayAlertBundle;
use super::intraday_push;
use super::mid_term;
use super::miana::MianaClient;
use super::realtime_cache::RealtimeDayCache;
use super::result_cache;
use crate::ai::AiClient;
use super::eastmoney::EastmoneyClient;
use super::fund_stats::{self as fund_stats_svc, FundStatsMeta, FundStatsResult};
use super::fundamentals;
use super::screen::{self, ScreenExpression, ScreenResult, StrategyDef};
use super::session;
use super::stock_analyze::{self as stock_analyze_svc, StockAnalyzeOut};
use super::tencent::TencentClient;

/// MTT 需要至少 MA60 + 斜率回看，默认取最近 70 个交易日。
const MTT_LOOKBACK_DAYS: usize = 70;

fn ensure_non_empty(
    input: MarketDataInput,
    days: usize,
    realtime_session: bool,
) -> AppResult<MarketDataInput> {
    if input.snapshots.is_empty() {
        tracing::warn!(
            days,
            realtime_session,
            shanghai_today = %session::shanghai_today(),
            "行情窗口为空，将返回 400（详见 data_source 对齐诊断日志）"
        );
        return Err(AppError::BadRequest(
            "行情表无数据：请确认 sc_stock / sc_stock_kline_d1_bfq 已由 SecuritiesCollector 写入"
                .into(),
        ));
    }
    let last = input.snapshots.last().map(|s| s.stocks.len()).unwrap_or(0);
    tracing::info!(
        "已加载真实行情: days={} last_sample={} first={} last={} realtime_session={}",
        input.snapshots.len(),
        last,
        input
            .snapshots
            .first()
            .map(|s| s.date.as_str())
            .unwrap_or("-"),
        input
            .snapshots
            .last()
            .map(|s| s.date.as_str())
            .unwrap_or("-"),
        realtime_session,
    );
    Ok(input)
}

async fn load_msi_input(
    db: &DatabaseConnection,
    days: usize,
    miana: &MianaClient,
    cache: &RealtimeDayCache,
) -> AppResult<MarketDataInput> {
    let input = data_source::load_market_window_hybrid(db, days, miana, cache).await?;
    ensure_non_empty(input, days, session::is_realtime_session_now())
}

async fn load_mtt_input(db: &DatabaseConnection, days: usize) -> AppResult<MarketDataInput> {
    let input = data_source::load_market_window(db, days).await?;
    ensure_non_empty(input, days, false)
}

/// `GET /api/market/sentiment?days=5`
#[get("/api/market/sentiment?<days>")]
pub async fn sentiment(
    db: &rocket::State<DatabaseConnection>,
    miana: &rocket::State<MianaClient>,
    cache: &rocket::State<RealtimeDayCache>,
    days: Option<usize>,
) -> AppResult<Json<MsiResult>> {
    let days = days.unwrap_or(5).clamp(3, 30);
    let refresh = data_source::is_open_auction_session(db.inner()).await;

    if !refresh {
        if let Some(cached) = result_cache::get_latest_msi(db.inner()).await? {
            tracing::info!(
                calculated_at = %cached.calculated_at,
                "非交易时段：返回最近一条缓存 MSI"
            );
            return Ok(Json(cached));
        }
        tracing::info!("非交易时段无缓存，计算一次并落库");
    }

    let input = load_msi_input(db.inner(), days, miana.inner(), cache.inner()).await?;
    let result = calculator::calculate_msi(&input);
    let trade_date = session::shanghai_today();
    result_cache::store_msi(db.inner(), &trade_date, &result).await?;

    tracing::info!(
        "市场情绪: today={:.1} recent={:.1} dir={:.1} int={:.1} con={:.1} signal={} refresh={}",
        result.today.value,
        result.recent.value,
        result.today.layers.direction,
        result.today.layers.intensity,
        result.today.layers.constraint,
        result.today.signal.label,
        refresh,
    );

    Ok(Json(result))
}

/// `GET /api/market/mid-term-trend`
#[get("/api/market/mid-term-trend")]
pub async fn mid_term_trend(
    db: &rocket::State<DatabaseConnection>,
) -> AppResult<Json<MttResult>> {
    let input = load_mtt_input(db.inner(), MTT_LOOKBACK_DAYS).await?;
    let result = mid_term::calculate_mtt(&input);

    tracing::info!(
        "中期趋势: MTT={:.1} status={} bull={} bear={} neutral={}",
        result.value,
        result.status.label(),
        result.counts.bull,
        result.counts.bear,
        result.counts.neutral,
    );

    Ok(Json(result))
}

/// `GET /api/market/intraday-alerts` — 立即计算趋势/超跌两类（不落库、不推送）。
#[get("/api/market/intraday-alerts")]
pub async fn intraday_alerts(
    db: &rocket::State<DatabaseConnection>,
    miana: &rocket::State<MianaClient>,
    cache: &rocket::State<RealtimeDayCache>,
) -> AppResult<Json<IntradayAlertBundle>> {
    let result = intraday_push::compute_now(db.inner(), miana.inner(), cache.inner())
        .await
        .map_err(|e| AppError::Internal(e))?;
    Ok(Json(result))
}

/// `GET /api/market/strategies` — 可选策略目录（参数 schema + 默认规则）。
#[get("/api/market/strategies")]
pub async fn strategies() -> Json<Vec<StrategyDef>> {
    Json(screen::strategy_catalog())
}

/// `GET /api/market/fund-stats/meta` — 期次选项与默认年份。
#[get("/api/market/fund-stats/meta")]
pub async fn fund_stats_meta(
    db: &rocket::State<DatabaseConnection>,
) -> AppResult<Json<FundStatsMeta>> {
    let meta = fund_stats_svc::load_meta(db.inner())
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(Json(meta))
}

/// `GET /api/market/fund-stats?year=2024&period=FY` — 全市场赚钱/增长/现金流占比。
#[get("/api/market/fund-stats?<year>&<period>")]
pub async fn fund_stats_query(
    db: &rocket::State<DatabaseConnection>,
    year: Option<i32>,
    period: Option<&str>,
) -> AppResult<Json<FundStatsResult>> {
    let result = fund_stats_svc::compute_stats(db.inner(), year, period)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    tracing::info!(
        year = result.year,
        period = %result.period,
        np_pct = ?result.net_profit.positive_pct,
        "全市场财报统计"
    );
    Ok(Json(result))
}

#[derive(Debug, Deserialize)]
pub struct ScreenRequest {
    /// `{ "op":"and", "rules":[ ... ] }` 或单条 rule JSON 字符串 / 对象
    pub expression: serde_json::Value,
    pub top_n: Option<usize>,
}

/// `POST /api/market/screen` — 按组合表达式立即选股（不落库）。
#[post("/api/market/screen", data = "<input>")]
pub async fn screen_preview(
    db: &rocket::State<DatabaseConnection>,
    input: Json<ScreenRequest>,
) -> AppResult<Json<ScreenResult>> {
    let expr = parse_screen_expr(&input.expression)?;
    let top_n = input.top_n.unwrap_or(50).clamp(1, 200);
    Ok(Json(run_screen_with_db(db.inner(), &expr, top_n).await?))
}

/// `GET /api/market/screen/<condition_id>` — 对已保存的 screen 条件跑选股。
#[get("/api/market/screen/<condition_id>?<top_n>")]
pub async fn screen_by_condition(
    db: &rocket::State<DatabaseConnection>,
    condition_id: i64,
    top_n: Option<usize>,
) -> AppResult<Json<ScreenResult>> {
    let m = condition::Entity::find_by_id(condition_id)
        .one(db.inner())
        .await?
        .ok_or(AppError::NotFound)?;
    if m.kind != "screen" {
        return Err(AppError::BadRequest(
            "该条件不是选股策略（kind 需为 screen）".into(),
        ));
    }
    let expr = screen::parse_expression(&m.expression).map_err(AppError::BadRequest)?;
    let top_n = top_n.unwrap_or(50).clamp(1, 200);
    Ok(Json(run_screen_with_db(db.inner(), &expr, top_n).await?))
}

fn parse_screen_expr(v: &serde_json::Value) -> AppResult<ScreenExpression> {
    if let Some(s) = v.as_str() {
        return screen::parse_expression(s).map_err(AppError::BadRequest);
    }
    let s = serde_json::to_string(v).map_err(|e| AppError::BadRequest(e.to_string()))?;
    screen::parse_expression(&s).map_err(AppError::BadRequest)
}

#[derive(Debug, Serialize)]
pub struct StockBarOut {
    pub date: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub change_pct: f64,
    pub amount: f64,
}

#[derive(Debug, Serialize)]
pub struct StockDetailOut {
    pub symbol: String,
    pub name: String,
    pub date: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub change_pct: f64,
    pub amount: f64,
    /// 近 5 个交易日收盘相对 5 日前收盘涨跌幅 %
    pub change_5d_pct: Option<f64>,
    /// 近 10 个交易日收盘相对 10 日前收盘涨跌幅 %
    pub change_10d_pct: Option<f64>,
    pub bars: Vec<StockBarOut>,
}

/// `GET /api/market/stock/analyze?symbol=600519.SH` — 近5日K线 + 成交明细 + AI 分析。
#[get("/api/market/stock/analyze?<symbol>")]
pub async fn stock_analyze_query(
    db: &rocket::State<DatabaseConnection>,
    eastmoney: &rocket::State<EastmoneyClient>,
    tencent: &rocket::State<TencentClient>,
    ai: &rocket::State<AiClient>,
    symbol: &str,
) -> AppResult<Json<StockAnalyzeOut>> {
    let out = stock_analyze_svc::analyze_stock(
        db.inner(),
        eastmoney.inner(),
        tencent.inner(),
        ai.inner(),
        symbol,
    )
    .await
    .map_err(AppError::Internal)?;
    Ok(Json(out))
}

/// `GET /api/market/stock?symbol=000001.SZ` — 个股近期日 K 与涨跌摘要。
#[get("/api/market/stock?<symbol>")]
pub async fn stock_detail(
    db: &rocket::State<DatabaseConnection>,
    symbol: &str,
) -> AppResult<Json<StockDetailOut>> {
    let symbol = symbol.trim();
    if symbol.is_empty() || symbol.len() > 32 {
        return Err(AppError::BadRequest("symbol 无效".into()));
    }
    let bars = data_source::load_stock_recent_klines(db.inner(), symbol, 15)
        .await
        .map_err(AppError::Db)?;
    if bars.is_empty() {
        return Err(AppError::NotFound);
    }
    let names = data_source::load_stock_names(db.inner(), &[symbol.to_string()])
        .await
        .unwrap_or_default();
    let name = names.get(symbol).cloned().unwrap_or_default();
    let last = bars.last().unwrap();
    let n = bars.len();
    let change_nd = |lookback: usize| -> Option<f64> {
        if n <= lookback {
            return None;
        }
        let old = bars[n - 1 - lookback].close_price;
        let cur = last.close_price;
        if old > 1e-10 {
            Some((cur / old - 1.0) * 100.0)
        } else {
            None
        }
    };
    Ok(Json(StockDetailOut {
        symbol: last.symbol.clone(),
        name,
        date: last.date.clone(),
        open: last.open,
        high: last.high,
        low: last.low,
        close: last.close_price,
        change_pct: last.change_pct,
        amount: last.amount,
        change_5d_pct: change_nd(5),
        change_10d_pct: change_nd(10),
        bars: bars
            .iter()
            .rev()
            .take(10)
            .map(|b| StockBarOut {
                date: b.date.clone(),
                open: b.open,
                high: b.high,
                low: b.low,
                close: b.close_price,
                change_pct: b.change_pct,
                amount: b.amount,
            })
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect(),
    }))
}

async fn run_screen_with_db(
    db: &DatabaseConnection,
    expr: &ScreenExpression,
    top_n: usize,
) -> AppResult<ScreenResult> {
    let days = screen::required_lookback(expr).clamp(10, 120);
    let input = data_source::load_market_window(db, days).await?;
    let input = ensure_non_empty(input, days, false)?;

    let income_map = if fundamentals::expression_needs_income(&expr.rules) {
        match fundamentals::load_income_snapshots(db).await {
            Ok(m) => {
                tracing::info!(symbols = m.len(), "已加载利润表基本面");
                Some(m)
            }
            Err(e) => {
                tracing::warn!(error = %e, "加载利润表失败，利润表规则将全部不命中");
                None
            }
        }
    } else {
        None
    };

    let cashflow_map = if fundamentals::expression_needs_cashflow(&expr.rules) {
        match fundamentals::load_cashflow_snapshots(db).await {
            Ok(m) => {
                tracing::info!(symbols = m.len(), "已加载现金流量表基本面");
                Some(m)
            }
            Err(e) => {
                tracing::warn!(error = %e, "加载现金流量表失败，现金流规则将全部不命中");
                None
            }
        }
    } else {
        None
    };

    let balance_map = if fundamentals::expression_needs_balance(&expr.rules) {
        match fundamentals::load_balance_snapshots(db).await {
            Ok(m) => {
                tracing::info!(symbols = m.len(), "已加载资产负债表基本面");
                Some(m)
            }
            Err(e) => {
                tracing::warn!(error = %e, "加载资产负债表失败，资产负债规则将全部不命中");
                None
            }
        }
    } else {
        None
    };

    let mut result = screen::run_screen_with_fundamentals(
        &input,
        expr,
        top_n,
        screen::FundMaps {
            income: income_map.as_ref(),
            cashflow: cashflow_map.as_ref(),
            balance: balance_map.as_ref(),
        },
    );
    let symbols: Vec<String> = result.items.iter().map(|i| i.symbol.clone()).collect();
    if let Ok(names) = data_source::load_stock_names(db, &symbols).await {
        for item in &mut result.items {
            if let Some(n) = names.get(&item.symbol) {
                item.name = n.clone();
            }
        }
    }
    tracing::info!(
        date = %result.date,
        sample = result.sample_size,
        matches = result.match_count,
        returned = result.items.len(),
        "条件选股完成"
    );
    Ok(result)
}

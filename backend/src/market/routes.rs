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
use super::sector::{self as sector_svc, SectorDetailOut, SectorMoversOut};
use super::fundamentals;
use super::screen::{self, ScreenExpression, ScreenResult, StrategyDef};
use super::session;
use super::stock_analyze::{
    self as stock_analyze_svc, StockAnalyzeAiOut, StockAnalyzeOut, StockAnalyzeTicksOut,
};
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

    // 非交易时段：用日 K 现算并覆盖缓存。
    // 盘中缓存的涨跌分布/涨跌停是盘中截面，收盘后若直接返回会对不齐雪球/东财收盘口径。
    if !refresh {
        tracing::info!("非交易时段：用日 K 重算 MSI（对齐收盘口径）");
    }

    let input = load_msi_input(db.inner(), days, miana.inner(), cache.inner()).await?;
    let result = calculator::calculate_msi(&input);
    // 落库日期用截面交易日，避免跨日凌晨把「昨日收盘 MSI」写到「今日」键上
    let trade_date = result.today.date.clone();
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

/// `GET /api/market/sectors/movers?top_n=10&types=INDUSTRY,CONCEPT` — 当日涨跌幅领涨/领跌。
/// 连续竞价时段拉 Miana 板块实时并落库；非交易时段先读当日快照，无则拉实时并落库，再回退日 K。
#[get("/api/market/sectors/movers?<top_n>&<types>")]
pub async fn sector_movers(
    db: &rocket::State<DatabaseConnection>,
    miana: &rocket::State<MianaClient>,
    top_n: Option<usize>,
    types: Option<&str>,
) -> AppResult<Json<SectorMoversOut>> {
    let out = sector_svc::load_movers(db.inner(), miana.inner(), top_n, types)
        .await
        .map_err(AppError::Internal)?;
    Ok(Json(out))
}

/// `GET /api/market/sectors/detail?symbol=BK0145.EM` — 板块详情（含 5/10 日涨跌）。
/// 成分股现价优先盘中实时（`market_realtime_snapshot` / Miana），5/10 日仍用日 K。
#[get("/api/market/sectors/detail?<symbol>")]
pub async fn sector_detail(
    db: &rocket::State<DatabaseConnection>,
    miana: &rocket::State<MianaClient>,
    symbol: &str,
) -> AppResult<Json<SectorDetailOut>> {
    let out = sector_svc::load_detail(db.inner(), miana.inner(), symbol)
        .await
        .map_err(AppError::Internal)?;
    out.map(Json).ok_or(AppError::NotFound)
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

#[derive(Debug, Serialize)]
pub struct StockSearchOut {
    pub total: usize,
    pub items: Vec<data_source::StockSearchHit>,
}

/// `GET /api/market/stocks/search?q=gzmt&limit=20` — 代码 / 名称 / 拼音首字母搜活跃股票。
#[get("/api/market/stocks/search?<q>&<limit>")]
pub async fn stocks_search(
    db: &rocket::State<DatabaseConnection>,
    q: Option<&str>,
    limit: Option<u32>,
) -> AppResult<Json<StockSearchOut>> {
    let q = q.unwrap_or("").trim();
    if q.is_empty() {
        return Ok(Json(StockSearchOut {
            total: 0,
            items: Vec::new(),
        }));
    }
    if q.chars().count() > 32 {
        return Err(AppError::BadRequest("q 过长".into()));
    }
    let items = data_source::search_stocks(db.inner(), q, limit.unwrap_or(20))
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(Json(StockSearchOut {
        total: items.len(),
        items,
    }))
}

/// `GET /api/market/stock/analyze?symbol=600519.SH&live_ticks=0` — 事实（默认不拉实时明细，秒级）。
#[get("/api/market/stock/analyze?<symbol>&<live_ticks>")]
pub async fn stock_analyze_query(
    db: &rocket::State<DatabaseConnection>,
    eastmoney: &rocket::State<EastmoneyClient>,
    tencent: &rocket::State<TencentClient>,
    ai: &rocket::State<AiClient>,
    symbol: &str,
    live_ticks: Option<bool>,
) -> AppResult<Json<StockAnalyzeOut>> {
    let out = stock_analyze_svc::analyze_stock(
        db.inner(),
        eastmoney.inner(),
        tencent.inner(),
        ai.inner(),
        symbol,
        live_ticks.unwrap_or(false),
    )
    .await
    .map_err(AppError::Internal)?;
    Ok(Json(out))
}

/// `GET /api/market/stock/analyze/ticks?symbol=` — 异步补全成交明细（东财/腾讯），回写缓存。
#[get("/api/market/stock/analyze/ticks?<symbol>")]
pub async fn stock_analyze_ticks_query(
    db: &rocket::State<DatabaseConnection>,
    eastmoney: &rocket::State<EastmoneyClient>,
    tencent: &rocket::State<TencentClient>,
    ai: &rocket::State<AiClient>,
    symbol: &str,
) -> AppResult<Json<StockAnalyzeTicksOut>> {
    let out = stock_analyze_svc::refresh_analyze_ticks(
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

/// `GET /api/market/stock/analyze/ai?symbol=600519.SH&dim=tape` — 单维 AI（tape/short_term/fundamentals/statements）。
#[get("/api/market/stock/analyze/ai?<symbol>&<dim>&<force>")]
pub async fn stock_analyze_ai_query(
    db: &rocket::State<DatabaseConnection>,
    eastmoney: &rocket::State<EastmoneyClient>,
    tencent: &rocket::State<TencentClient>,
    ai: &rocket::State<AiClient>,
    symbol: &str,
    dim: &str,
    force: Option<bool>,
) -> AppResult<Json<StockAnalyzeAiOut>> {
    let kind = stock_analyze_svc::AnalyzeDimKind::parse(dim).ok_or_else(|| {
        AppError::BadRequest(
            "dim 须为 tape | short_term | fundamentals | statements".into(),
        )
    })?;
    let out = stock_analyze_svc::analyze_dim(
        db.inner(),
        eastmoney.inner(),
        tencent.inner(),
        ai.inner(),
        symbol,
        kind,
        force.unwrap_or(false),
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

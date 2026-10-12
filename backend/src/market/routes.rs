//! 市场指标 HTTP API
//! - MSI：`docs/market_sentiment.md` §10
//! - MTT：`docs/mid_term_trend.md` §10
//!
//! 默认读 `sc_stock` + `sc_stock_kline_d1_bfq`。
//! 仅 MSI 在盘中（上海周一至周五 09:30–18:00）对最新交易日改用 Miana 实时行情；MTT 始终读库。
//! 若需本地假行情：服务端环境变量 `MARKET_USE_MOCK=true`（无查询参数）。

use rocket::get;
use rocket::serde::json::Json;
use sea_orm::DatabaseConnection;

use crate::config::AppConfig;
use crate::error::{AppError, AppResult};

use super::calculator;
use super::data_source;
use super::domain::{MarketDataInput, MsiResult, MttResult};
use super::mid_term;
use super::miana::MianaClient;
use super::mock_data;
use super::realtime_cache::RealtimeDayCache;
use super::session;

/// MTT 需要至少 MA60 + 斜率回看，默认取最近 70 个交易日。
const MTT_LOOKBACK_DAYS: usize = 70;

fn ensure_non_empty(input: MarketDataInput, realtime_session: bool) -> AppResult<MarketDataInput> {
    if input.snapshots.is_empty() {
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

/// MSI：盘中可叠加实时最新日。
async fn load_msi_input(
    db: &DatabaseConnection,
    days: usize,
    use_mock: bool,
    miana: &MianaClient,
    cache: &RealtimeDayCache,
) -> AppResult<MarketDataInput> {
    if use_mock {
        tracing::warn!("MARKET_USE_MOCK=true，使用本地 Mock 行情 (days={})", days);
        return Ok(mock_data::generate_mock_data_with_bias(days, 0.0));
    }

    let input = data_source::load_market_window_hybrid(db, days, miana, cache).await?;
    ensure_non_empty(input, session::is_realtime_session_now())
}

/// MTT：始终只读数据库（不调实时行情）。
async fn load_mtt_input(
    db: &DatabaseConnection,
    days: usize,
    use_mock: bool,
) -> AppResult<MarketDataInput> {
    if use_mock {
        tracing::warn!("MARKET_USE_MOCK=true，使用本地 Mock 行情 (days={})", days);
        return Ok(mock_data::generate_mock_data_with_bias(days, 0.15));
    }

    let input = data_source::load_market_window(db, days).await?;
    ensure_non_empty(input, false)
}

/// `GET /api/market/sentiment?days=5`
#[get("/api/market/sentiment?<days>")]
pub async fn sentiment(
    db: &rocket::State<DatabaseConnection>,
    cfg: &rocket::State<AppConfig>,
    miana: &rocket::State<MianaClient>,
    cache: &rocket::State<RealtimeDayCache>,
    days: Option<usize>,
) -> AppResult<Json<MsiResult>> {
    let days = days.unwrap_or(5).clamp(3, 30);
    let use_mock = cfg.market_use_mock;

    let input = load_msi_input(db.inner(), days, use_mock, miana.inner(), cache.inner()).await?;
    let result = calculator::calculate_msi(&input);

    tracing::info!(
        "市场情绪: today={:.1} recent={:.1} dir={:.1} int={:.1} con={:.1} signal={} mock={}",
        result.today.value,
        result.recent.value,
        result.today.layers.direction,
        result.today.layers.intensity,
        result.today.layers.constraint,
        result.today.signal.label,
        use_mock
    );

    Ok(Json(result))
}

/// `GET /api/market/mid-term-trend`
#[get("/api/market/mid-term-trend")]
pub async fn mid_term_trend(
    db: &rocket::State<DatabaseConnection>,
    cfg: &rocket::State<AppConfig>,
) -> AppResult<Json<MttResult>> {
    let use_mock = cfg.market_use_mock;

    let input = load_mtt_input(db.inner(), MTT_LOOKBACK_DAYS, use_mock).await?;
    let result = mid_term::calculate_mtt(&input);

    tracing::info!(
        "中期趋势: MTT={:.1} status={} bull={} bear={} neutral={} mock={}",
        result.value,
        result.status.label(),
        result.counts.bull,
        result.counts.bear,
        result.counts.neutral,
        use_mock
    );

    Ok(Json(result))
}

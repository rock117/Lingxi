//! 市场指标 HTTP API
//! - MSI：`docs/market_sentiment.md`
//! - MTT：`docs/mid_term_trend.md`
//! - 日内强势：`docs/intraday_alert.md`
//!
//! 一律读真实行情：`sc_stock` + `sc_stock_kline_d1_bfq`；
//! MSI / 日内推送在盘中对最新交易日叠加 Miana 实时行情。

use rocket::get;
use rocket::serde::json::Json;
use sea_orm::DatabaseConnection;

use crate::error::{AppError, AppResult};

use super::calculator;
use super::data_source;
use super::domain::{MarketDataInput, MsiResult, MttResult};
use super::intraday_alert::IntradayAlertResult;
use super::intraday_push;
use super::mid_term;
use super::miana::MianaClient;
use super::msi_cache;
use super::realtime_cache::RealtimeDayCache;
use super::session;

/// 开市日 + 连续竞价时钟窗：重新计算 MSI；否则复用上次结果。
async fn should_refresh_msi(db: &DatabaseConnection) -> bool {
    if !session::is_realtime_session_now() {
        return false;
    }
    match data_source::is_open_trade_day(db, session::shanghai_today_naive()).await {
        Ok(open) => open,
        Err(e) => {
            tracing::warn!(error = %e, "查询 is_open 失败，按非交易时段处理（复用缓存）");
            false
        }
    }
}

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

async fn load_msi_input(
    db: &DatabaseConnection,
    days: usize,
    miana: &MianaClient,
    cache: &RealtimeDayCache,
) -> AppResult<MarketDataInput> {
    let input = data_source::load_market_window_hybrid(db, days, miana, cache).await?;
    ensure_non_empty(input, session::is_realtime_session_now())
}

async fn load_mtt_input(db: &DatabaseConnection, days: usize) -> AppResult<MarketDataInput> {
    let input = data_source::load_market_window(db, days).await?;
    ensure_non_empty(input, false)
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
    let refresh = should_refresh_msi(db.inner()).await;

    if !refresh {
        if let Some(cached) = msi_cache::get_latest(db.inner()).await? {
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
    msi_cache::store(db.inner(), &trade_date, &result).await?;

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

/// `GET /api/market/intraday-alerts` — 立即计算一轮（不落库、不推送）。
#[get("/api/market/intraday-alerts")]
pub async fn intraday_alerts(
    db: &rocket::State<DatabaseConnection>,
    miana: &rocket::State<MianaClient>,
    cache: &rocket::State<RealtimeDayCache>,
) -> AppResult<Json<IntradayAlertResult>> {
    let result = intraday_push::compute_now(db.inner(), miana.inner(), cache.inner())
        .await
        .map_err(|e| AppError::Internal(e))?;
    Ok(Json(result))
}

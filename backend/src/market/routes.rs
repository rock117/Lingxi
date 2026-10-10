//! 市场指标 HTTP API
//! - MSI：`docs/market_sentiment.md` §10
//! - MTT：`docs/mid_term_trend.md` §10

use rocket::get;
use rocket::serde::json::Json;

use crate::error::AppResult;

use super::calculator;
use super::domain::{MsiResult, MttResult};
use super::mid_term;
use super::mock_data;

/// `GET /api/market/sentiment?days=5&use_mock=true`
#[get("/api/market/sentiment?<days>&<use_mock>")]
pub async fn sentiment(
    days: Option<usize>,
    use_mock: Option<bool>,
) -> AppResult<Json<MsiResult>> {
    let days = days.unwrap_or(5).clamp(3, 30);
    let _use_mock = use_mock.unwrap_or(true);

    // TODO: use_mock=false 时接入真实行情；当前均走 Mock
    let input = mock_data::generate_mock_data_with_bias(days, 0.0);
    let result = calculator::calculate_msi(&input);

    tracing::info!(
        "市场情绪: MSI={:.1} dir={:.1} int={:.1} con={:.1} signal={}",
        result.value,
        result.layers.direction,
        result.layers.intensity,
        result.layers.constraint,
        result.signal.label
    );

    Ok(Json(result))
}

/// `GET /api/market/mid-term-trend?use_mock=true`
///
/// Mock 至少生成 `MIN_HISTORY_DAYS`（65）根 K 线；与 MSI 的 `days` 入参无关。
#[get("/api/market/mid-term-trend?<use_mock>")]
pub async fn mid_term_trend(use_mock: Option<bool>) -> AppResult<Json<MttResult>> {
    let _use_mock = use_mock.unwrap_or(true);

    // TODO: use_mock=false 时接入真实行情；当前均走 Mock（70 日以满足 MA60+斜率）
    let input = mock_data::generate_mock_data_with_bias(70, 0.15);
    let result = mid_term::calculate_mtt(&input);

    tracing::info!(
        "中期趋势: MTT={:.1} status={} bull={} bear={} neutral={}",
        result.value,
        result.status.label(),
        result.counts.bull,
        result.counts.bear,
        result.counts.neutral
    );

    Ok(Json(result))
}

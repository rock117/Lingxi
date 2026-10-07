//! MSI HTTP API（规范 `docs/market_sentiment.md` §10）

use rocket::get;
use rocket::serde::json::Json;

use crate::error::AppResult;

use super::calculator;
use super::domain::MsiResult;
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

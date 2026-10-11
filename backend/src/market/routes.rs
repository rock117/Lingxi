//! 市场指标 HTTP API
//! - MSI：`docs/market_sentiment.md` §10
//! - MTT：`docs/mid_term_trend.md` §10
//!
//! 默认读取 `sc_stock` + `sc_stock_kline_d1_bfq`；`use_mock=true` 时走本地 Mock。

use rocket::get;
use rocket::serde::json::Json;
use sea_orm::DatabaseConnection;

use crate::error::{AppError, AppResult};

use super::calculator;
use super::data_source;
use super::domain::{MsiResult, MttResult};
use super::mid_term;
use super::mock_data;

/// MTT 需要至少 MA60 + 斜率回看，默认取最近 70 个交易日。
const MTT_LOOKBACK_DAYS: usize = 70;

async fn load_input(
    db: &DatabaseConnection,
    days: usize,
    use_mock: bool,
    mock_bias: f64,
) -> AppResult<super::domain::MarketDataInput> {
    if use_mock {
        return Ok(mock_data::generate_mock_data_with_bias(days, mock_bias));
    }

    let input = data_source::load_market_window(db, days).await?;
    if input.snapshots.is_empty() {
        return Err(AppError::BadRequest(
            "行情表无数据：请确认 sc_stock / sc_stock_kline_d1_bfq 已由 SecuritiesCollector 写入"
                .into(),
        ));
    }
    let last = input.snapshots.last().map(|s| s.stocks.len()).unwrap_or(0);
    tracing::info!(
        "已加载真实行情: days={} last_sample={} first={} last={}",
        input.snapshots.len(),
        last,
        input.snapshots.first().map(|s| s.date.as_str()).unwrap_or("-"),
        input.snapshots.last().map(|s| s.date.as_str()).unwrap_or("-"),
    );
    Ok(input)
}

/// `GET /api/market/sentiment?days=5&use_mock=false`
#[get("/api/market/sentiment?<days>&<use_mock>")]
pub async fn sentiment(
    db: &rocket::State<DatabaseConnection>,
    days: Option<usize>,
    use_mock: Option<bool>,
) -> AppResult<Json<MsiResult>> {
    let days = days.unwrap_or(5).clamp(3, 30);
    let use_mock = use_mock.unwrap_or(false);

    let input = load_input(db.inner(), days, use_mock, 0.0).await?;
    let result = calculator::calculate_msi(&input);

    tracing::info!(
        "市场情绪: MSI={:.1} dir={:.1} int={:.1} con={:.1} signal={} mock={}",
        result.value,
        result.layers.direction,
        result.layers.intensity,
        result.layers.constraint,
        result.signal.label,
        use_mock
    );

    Ok(Json(result))
}

/// `GET /api/market/mid-term-trend?use_mock=false`
#[get("/api/market/mid-term-trend?<use_mock>")]
pub async fn mid_term_trend(
    db: &rocket::State<DatabaseConnection>,
    use_mock: Option<bool>,
) -> AppResult<Json<MttResult>> {
    let use_mock = use_mock.unwrap_or(false);

    let input = load_input(db.inner(), MTT_LOOKBACK_DAYS, use_mock, 0.15).await?;
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

//! 市场情绪指标 REST API 路由
//!
//! 提供查询市场情绪的接口：
//!
//! - `GET /api/market/sentiment` — 获取 MSI 计算结果
//!
//! 当前使用 Mock 数据生成器，后续可替换为真实行情数据源。

use rocket::get;
use rocket::serde::json::Json;

use crate::error::AppResult;

use super::calculator;
use super::domain::MsiResult;
use super::mock_data;

/// 获取市场情绪指标
///
/// # 查询参数
///
/// | 参数 | 类型 | 默认 | 说明 |
/// |---|---|---|---|
/// | `days` | usize | 5 | 分析天数 N（3~30） |
/// | `use_mock` | bool | true | 是否使用 Mock 数据 |
///
/// # 返回
///
/// 返回 `MsiResult`，包含：
/// - MSI 综合值 [0, 100]
/// - 四个子分：涨跌宽度、资金流向、成交量水平、价格位置
/// - 组合信号：信号类型 + 成交量状态 + 价格位置状态 + 操作建议
///
/// # 示例
///
/// ```text
/// GET /api/market/sentiment?days=5&use_mock=true
/// ```
#[get("/api/market/sentiment?<days>&<use_mock>")]
pub async fn sentiment(
    days: Option<usize>,
    use_mock: Option<bool>,
) -> AppResult<Json<MsiResult>> {
    // 解析参数，带默认值和边界限制
    let days = days.unwrap_or(5).clamp(3, 30);
    let use_mock = use_mock.unwrap_or(true);

    // 生成输入数据（当前仅支持 Mock）
    let input = if use_mock {
        // 使用 Mock 数据生成器，bias=0.0 表示中性随机
        mock_data::generate_mock_data_with_bias(days, 0.0)
    } else {
        // TODO: 接入真实行情数据源
        mock_data::generate_mock_data_with_bias(days, 0.0)
    };

    // 计算 MSI
    let result = calculator::calculate_msi(&input);

    tracing::info!(
        "市场情绪计算完成: MSI={:.1}, 成交量={:.2}x, 价格位置={:.2}, 信号={}",
        result.value,
        result.signal.volume_ratio,
        result.signal.price_position,
        result.signal.label
    );

    Ok(Json(result))
}

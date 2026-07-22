use entity::condition::Model as Condition;
use rand::Rng;
use serde::Deserialize;

/// 简单条件表达式配置
#[derive(Debug, Deserialize)]
pub struct Expression {
    #[serde(rename = "type")]
    pub kind: Option<String>,
    /// 触发概率（0.0 ~ 1.0），用于 Mock
    pub trigger_rate: Option<f64>,
    pub threshold: Option<f64>,
}

/// 用假数据评估条件是否满足。
/// 第一版：基于 `trigger_rate` 随机触发；若未指定则默认 0.2。
pub fn evaluate(cond: &Condition) -> Option<String> {
    let expr: Expression = serde_json::from_str(&cond.expression).ok()?;

    let rate = expr.trigger_rate.unwrap_or(0.2).clamp(0.0, 1.0);
    let mut rng = rand::thread_rng();
    if rng.gen_bool(rate) {
        let detail = match (expr.kind.as_deref(), expr.threshold) {
            (Some("price_drop"), Some(t)) => format!("价格跌幅超过 {:.2}%", t * 100.0),
            (Some("price_rise"), Some(t)) => format!("价格涨幅超过 {:.2}%", t * 100.0),
            (Some(k), _) => format!("触发条件: {}", k),
            (None, _) => "条件已满足".to_string(),
        };
        Some(detail)
    } else {
        None
    }
}

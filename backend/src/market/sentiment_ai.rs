//! 市场情绪 AI：汇总当日 MSI + 近 N 日 + MTT。
//! 文档：`docs/market_sentiment.md` §8.1、`docs/ai.md`。

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::warn;

use crate::ai::AiClient;

use super::domain::{MsiResult, MttResult};

const AI_CACHE_TTL: Duration = Duration::from_secs(30 * 60);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentimentAiInsight {
    /// `pending` | `ready` | `disabled` | `error`
    pub status: String,
    pub text: String,
    /// `bullish` | `neutral` | `bearish` | `unknown`
    pub bias: String,
    /// `high` | `medium` | `low` | `unknown`
    pub confidence: String,
    pub flags: Vec<String>,
}

impl SentimentAiInsight {
    fn disabled(msg: &str) -> Self {
        Self {
            status: "disabled".into(),
            text: msg.to_string(),
            bias: "unknown".into(),
            confidence: "unknown".into(),
            flags: Vec::new(),
        }
    }

    fn error(msg: String) -> Self {
        Self {
            status: "error".into(),
            text: msg,
            bias: "unknown".into(),
            confidence: "unknown".into(),
            flags: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SentimentAiOut {
    pub trade_date: String,
    pub days: usize,
    pub ai: SentimentAiInsight,
    pub ai_enabled: bool,
    pub ai_model: Option<String>,
    pub cached: bool,
}

#[derive(Debug, Deserialize)]
struct ModelOut {
    text: Option<String>,
    bias: Option<String>,
    confidence: Option<String>,
    flags: Option<Vec<String>>,
}

struct AiCache {
    map: HashMap<String, (Instant, SentimentAiInsight)>,
}

fn ai_cache() -> &'static Mutex<AiCache> {
    static C: OnceLock<Mutex<AiCache>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(AiCache { map: HashMap::new() }))
}

fn cache_key(trade_date: &str, days: usize) -> String {
    format!("{trade_date}|d{days}")
}

fn cache_get(trade_date: &str, days: usize) -> Option<SentimentAiInsight> {
    let key = cache_key(trade_date, days);
    let Ok(mut g) = ai_cache().lock() else {
        return None;
    };
    if let Some((t, v)) = g.map.get(&key) {
        if t.elapsed() < AI_CACHE_TTL {
            return Some(v.clone());
        }
        g.map.remove(&key);
    }
    None
}

fn cache_put(trade_date: &str, days: usize, ai: SentimentAiInsight) {
    if let Ok(mut g) = ai_cache().lock() {
        g.map
            .insert(cache_key(trade_date, days), (Instant::now(), ai));
    }
}

fn strip_code_fence(raw: &str) -> String {
    let t = raw.trim();
    if let Some(rest) = t.strip_prefix("```") {
        let rest = rest
            .trim_start_matches("json")
            .trim_start_matches("JSON")
            .trim_start();
        if let Some(end) = rest.rfind("```") {
            return rest[..end].trim().to_string();
        }
        return rest.trim().to_string();
    }
    t.to_string()
}

fn normalize_bias(s: &str) -> String {
    match s.trim().to_ascii_lowercase().as_str() {
        "bullish" | "偏多" | "多" => "bullish".into(),
        "bearish" | "偏空" | "空" => "bearish".into(),
        "neutral" | "中性" => "neutral".into(),
        _ => "unknown".into(),
    }
}

fn normalize_confidence(s: &str) -> String {
    match s.trim().to_ascii_lowercase().as_str() {
        "high" | "高" => "high".into(),
        "medium" | "中" | "mid" => "medium".into(),
        "low" | "低" => "low".into(),
        _ => "unknown".into(),
    }
}

fn parse_ai_json(raw: &str) -> SentimentAiInsight {
    let cleaned = strip_code_fence(raw);
    match serde_json::from_str::<ModelOut>(&cleaned) {
        Ok(m) => {
            let bias = normalize_bias(m.bias.as_deref().unwrap_or(""));
            let confidence = normalize_confidence(m.confidence.as_deref().unwrap_or(""));
            let mut flags = m.flags.unwrap_or_default();
            flags.truncate(4);
            flags.retain(|f| !f.trim().is_empty());
            let text = m.text.unwrap_or_default().trim().to_string();
            if text.is_empty() {
                SentimentAiInsight::error("模型返回空 text。".into())
            } else {
                SentimentAiInsight {
                    status: "ready".into(),
                    text,
                    bias,
                    confidence,
                    flags,
                }
            }
        }
        Err(_) => {
            let text = cleaned.trim().to_string();
            if text.is_empty() {
                SentimentAiInsight::error("模型输出无法解析。".into())
            } else {
                SentimentAiInsight {
                    status: "ready".into(),
                    text,
                    bias: "unknown".into(),
                    confidence: "low".into(),
                    flags: Vec::new(),
                }
            }
        }
    }
}

fn build_payload(msi: &MsiResult, mtt: &MttResult) -> Value {
    let today = &msi.today;
    json!({
        "today": {
            "date": today.date,
            "msi": today.value,
            "layers": {
                "direction": today.layers.direction,
                "intensity": today.layers.intensity,
                "constraint": today.layers.constraint,
            },
            "signal": {
                "label": today.signal.label,
                "advice": today.signal.advice,
                "volume_status": today.signal.volume_status,
                "volume_ratio": today.signal.volume_ratio,
                "price_status": today.signal.price_status,
                "price_position": today.signal.price_position,
            },
            "diagnostics": {
                "sample_size": today.diagnostics.sample_size,
                "up_count": today.diagnostics.up_count,
                "down_count": today.diagnostics.down_count,
                "flat_count": today.diagnostics.flat_count,
                "limit_up_count": today.diagnostics.limit_up_count,
                "limit_down_count": today.diagnostics.limit_down_count,
                "breadth_raw": today.diagnostics.breadth_raw,
                "money_flow_raw": today.diagnostics.money_flow_raw,
                "structure_raw": today.diagnostics.structure_raw,
            }
        },
        "recent": {
            "days": msi.recent.days,
            "value": msi.recent.value,
            "series": msi.recent.series.iter().map(|p| json!({
                "date": p.date,
                "value": p.value,
            })).collect::<Vec<_>>(),
        },
        "mtt": {
            "value": mtt.value,
            "status": mtt.status,
            "status_label": mtt.status.label(),
            "counts": {
                "bull": mtt.counts.bull,
                "bear": mtt.counts.bear,
                "neutral": mtt.counts.neutral,
                "classified": mtt.counts.classified,
            },
            "diagnostics": {
                "deviation_median": mtt.diagnostics.deviation_median,
                "nh_nl": mtt.diagnostics.nh_nl,
            }
        }
    })
}

/// 用已算好的 MSI + MTT 生成 AI 解读。
pub async fn analyze(
    ai: &AiClient,
    msi: &MsiResult,
    mtt: &MttResult,
    force: bool,
) -> SentimentAiOut {
    let trade_date = msi.today.date.clone();
    let days = msi.days;

    if !ai.enabled() {
        return SentimentAiOut {
            trade_date,
            days,
            ai: SentimentAiInsight::disabled("AI 未启用：请配置 AI_API_KEY。"),
            ai_enabled: false,
            ai_model: None,
            cached: false,
        };
    }

    if !force {
        if let Some(hit) = cache_get(&trade_date, days) {
            return SentimentAiOut {
                trade_date,
                days,
                ai: hit,
                ai_enabled: true,
                ai_model: Some(ai.model_name().to_string()),
                cached: true,
            };
        }
    }

    let payload = build_payload(msi, mtt);
    let system = crate::ai::prompts::market_sentiment_system();
    let user = format!(
        "请用下列 MSI/MTT JSON，写一篇综合结论：现状（当日相对近{}日）、当前行情是否可持续及原因、短线更可能怎么走及观察点。不要拆成两段，只输出 JSON：\n{}",
        days,
        serde_json::to_string_pretty(&payload).unwrap_or_default()
    );

    let insight = match ai.chat(&system, &user).await {
        Ok(raw) => parse_ai_json(&raw),
        Err(e) => {
            warn!(error = %e, "市场情绪 AI 失败");
            SentimentAiInsight::error(format!("AI 分析失败：{e}。下方仍可查看情绪事实数据。"))
        }
    };

    if insight.status == "ready" {
        cache_put(&trade_date, days, insight.clone());
    }

    SentimentAiOut {
        trade_date,
        days,
        ai: insight,
        ai_enabled: true,
        ai_model: Some(ai.model_name().to_string()),
        cached: false,
    }
}

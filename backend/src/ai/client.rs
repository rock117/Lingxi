//! OpenAI 兼容 Chat Completions 客户端（供 backend 内部调用）。

use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{debug, info, warn};

use super::prompts;
use crate::config::AppConfig;
use crate::market::screen::{self, ScreenExpression};

#[derive(Clone)]
pub struct AiClient {
    http: Client,
    base_url: String,
    api_key: String,
    model: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExplainResult {
    pub text: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScreenDraft {
    pub expression: ScreenExpression,
    pub explanation: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
    model: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DraftModelOut {
    expression: serde_json::Value,
    explanation: Option<String>,
}

impl AiClient {
    pub fn from_config(cfg: &AppConfig) -> Result<Self> {
        let timeout = std::time::Duration::from_secs(cfg.ai_timeout_secs.max(5));
        let http = Client::builder()
            .timeout(timeout)
            .build()
            .context("创建 AI HTTP 客户端失败")?;
        Ok(Self {
            http,
            base_url: cfg.ai_api_base.trim_end_matches('/').to_string(),
            api_key: cfg.ai_api_key.clone(),
            model: cfg.ai_model.clone(),
        })
    }

    pub fn enabled(&self) -> bool {
        !self.api_key.trim().is_empty()
    }

    pub fn model_name(&self) -> &str {
        &self.model
    }

    /// 通用对话：返回 assistant 文本。
    pub async fn chat(&self, system: &str, user: &str) -> Result<String> {
        let (text, _) = self.chat_raw(system, user).await?;
        Ok(text)
    }

    /// 解释引擎已算好的 JSON 摘要。
    pub async fn explain(&self, kind: &str, payload_json: &str) -> Result<ExplainResult> {
        self.require_enabled()?;
        let system = prompts::explain_system(kind);
        let user = format!("请解释以下 JSON：\n{payload_json}");
        let (text, model) = self.chat_raw(&system, &user).await?;
        Ok(ExplainResult {
            text: text.trim().to_string(),
            model,
        })
    }

    /// 自然语言 → 选股 expression 草稿（供内部批测或未来 API 复用）。
    pub async fn draft_screen(&self, user_prompt: &str) -> Result<ScreenDraft> {
        self.require_enabled()?;
        let catalog = screen::strategy_catalog();
        let catalog_json = serde_json::to_string(&catalog).context("序列化策略目录失败")?;
        let system = prompts::draft_screen_system(&catalog_json);
        let (raw, _) = self.chat_raw(&system, user_prompt).await?;
        let cleaned = strip_code_fence(&raw);
        let parsed: DraftModelOut = serde_json::from_str(&cleaned).with_context(|| {
            format!(
                "模型输出不是合法 ScreenDraft JSON: {}",
                truncate(&cleaned, 400)
            )
        })?;

        let expr_str = serde_json::to_string(&parsed.expression)?;
        let expression = screen::parse_expression(&expr_str)
            .map_err(|e| anyhow!("expression 校验失败: {e}"))?;

        let mut warnings = Vec::new();
        let known: std::collections::HashSet<&str> =
            catalog.iter().map(|d| d.type_id.as_str()).collect();
        for rule in &expression.rules {
            let tid = rule
                .get("type")
                .and_then(|x| x.as_str())
                .unwrap_or("");
            if tid.is_empty() {
                warnings.push("存在缺少 type 的 rule".into());
            } else if !known.contains(tid) {
                warnings.push(format!("未知策略 type: {tid}"));
            }
        }

        Ok(ScreenDraft {
            expression,
            explanation: parsed
                .explanation
                .unwrap_or_default()
                .trim()
                .to_string(),
            warnings,
        })
    }

    fn require_enabled(&self) -> Result<()> {
        if self.enabled() {
            Ok(())
        } else {
            Err(anyhow!("AI 未启用：请配置 AI_API_KEY"))
        }
    }

    async fn chat_raw(&self, system: &str, user: &str) -> Result<(String, String)> {
        self.require_enabled()?;
        let url = format!("{}/chat/completions", self.base_url);
        // DeepSeek：thinking 默认开启；内部解释/JSON 草稿关闭以控延迟与费用。
        // 见 https://api-docs.deepseek.com/guides/thinking_mode
        let body = json!({
            "model": self.model,
            "temperature": 0.3,
            "stream": false,
            "thinking": { "type": "disabled" },
            "messages": [
                { "role": "system", "content": system },
                { "role": "user", "content": user }
            ]
        });

        debug!(model = %self.model, url = %url, "AI chat 请求");
        let resp = self
            .http
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .context("AI HTTP 请求失败")?;

        let status = resp.status();
        let bytes = resp.bytes().await.context("读取 AI 响应失败")?;
        if !status.is_success() {
            let err_body = String::from_utf8_lossy(&bytes);
            warn!(%status, body = %truncate(&err_body, 500), "AI 接口错误");
            return Err(anyhow!("AI 接口 HTTP {status}: {}", truncate(&err_body, 300)));
        }

        let parsed: ChatResponse =
            serde_json::from_slice(&bytes).context("解析 AI ChatResponse 失败")?;
        let text = parsed
            .choices
            .first()
            .and_then(|c| c.message.content.as_ref())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| anyhow!("AI 返回空 content"))?;
        let model = parsed.model.unwrap_or_else(|| self.model.clone());
        info!(model = %model, chars = text.len(), "AI chat 完成");
        Ok((text, model))
    }
}

pub(crate) fn strip_code_fence(s: &str) -> String {
    let t = s.trim();
    if let Some(rest) = t.strip_prefix("```json") {
        return rest
            .strip_suffix("```")
            .unwrap_or(rest)
            .trim()
            .to_string();
    }
    if let Some(rest) = t.strip_prefix("```") {
        return rest
            .strip_suffix("```")
            .unwrap_or(rest)
            .trim()
            .to_string();
    }
    t.to_string()
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max).collect();
        format!("{t}…")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_fence() {
        let s = "```json\n{\"a\":1}\n```";
        assert_eq!(strip_code_fence(s), "{\"a\":1}");
    }

    #[test]
    fn disabled_without_key() {
        let cfg = AppConfig {
            ai_api_key: String::new(),
            ..AppConfig::default()
        };
        let ai = AiClient::from_config(&cfg).unwrap();
        assert!(!ai.enabled());
    }
}

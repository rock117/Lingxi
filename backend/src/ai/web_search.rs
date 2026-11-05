//! 助手网页搜索：模型自拟关键词，无需用户贴 URL。
//! 提供方：`serper` / `bocha`（需 Key）或 `duckduckgo`（无 Key，质量一般）。

use std::time::Duration;

use anyhow::{bail, Context, Result};
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::info;

use crate::market::rate_limit::RateLimiter;

const DEFAULT_RATE_PER_MIN: u32 = 20;
const DEFAULT_TIMEOUT_SECS: u64 = 20;
const MAX_LIMIT: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Provider {
    Serper,
    Bocha,
    DuckDuckGo,
}

#[derive(Clone)]
pub struct WebSearchClient {
    http: Client,
    limiter: RateLimiter,
    enabled: bool,
    provider: Provider,
    api_key: String,
}

impl WebSearchClient {
    pub fn from_env() -> Result<Self> {
        let enabled = env_bool("WEB_SEARCH_ENABLED", true);
        let timeout = env_u64("WEB_SEARCH_TIMEOUT_SECS", DEFAULT_TIMEOUT_SECS).max(5);
        let rate = env_u32("WEB_SEARCH_RATE_PER_MIN", DEFAULT_RATE_PER_MIN).max(1);
        let serper_key = first_env(&["WEB_SEARCH_API_KEY", "SERPER_API_KEY"]);
        let bocha_key = first_env(&["BOCHA_API_KEY"]);
        let requested = env_string("WEB_SEARCH_PROVIDER", "auto").to_ascii_lowercase();
        let mut provider = match requested.as_str() {
            "serper" | "google" => Provider::Serper,
            "bocha" => Provider::Bocha,
            "duckduckgo" | "ddg" => Provider::DuckDuckGo,
            _ => {
                if !serper_key.is_empty() {
                    Provider::Serper
                } else if !bocha_key.is_empty() {
                    Provider::Bocha
                } else {
                    Provider::DuckDuckGo
                }
            }
        };
        let mut api_key = match provider {
            Provider::Serper => serper_key.clone(),
            Provider::Bocha => bocha_key.clone(),
            Provider::DuckDuckGo => String::new(),
        };
        if matches!(provider, Provider::Serper | Provider::Bocha) && api_key.is_empty() {
            tracing::warn!(
                requested = %requested,
                "web_search 缺少 API Key，回退到 duckduckgo"
            );
            provider = Provider::DuckDuckGo;
            api_key.clear();
        }
        let http = Client::builder()
            .timeout(Duration::from_secs(timeout))
            .connect_timeout(Duration::from_secs(timeout.min(10)))
            .user_agent("LingxiAssistantBot/1.0 (+https://lingxi.local; research)")
            .build()
            .context("创建 WebSearch HTTP 客户端失败")?;
        Ok(Self {
            http,
            limiter: RateLimiter::per_minute(rate),
            enabled,
            provider,
            api_key,
        })
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn provider_name(&self) -> &'static str {
        match self.provider {
            Provider::Serper => "serper",
            Provider::Bocha => "bocha",
            Provider::DuckDuckGo => "duckduckgo",
        }
    }

    pub async fn search(&self, query: &str, limit: usize) -> Result<Value> {
        if !self.enabled {
            bail!("web_search 已关闭（WEB_SEARCH_ENABLED=false）");
        }
        let q = query.trim();
        if q.is_empty() {
            bail!("query 为空");
        }
        if q.chars().count() > 200 {
            bail!("query 过长");
        }
        let limit = limit.clamp(1, MAX_LIMIT);
        self.limiter.acquire().await;
        info!(
            target: "web_search",
            provider = self.provider_name(),
            %q,
            limit,
            "网页搜索"
        );
        let results = match self.provider {
            Provider::Serper => self.search_serper(q, limit).await?,
            Provider::Bocha => self.search_bocha(q, limit).await?,
            Provider::DuckDuckGo => self.search_duckduckgo(q, limit).await?,
        };
        Ok(json!({
            "query": q,
            "provider": self.provider_name(),
            "count": results.len(),
            "results": results,
            "untrusted": true,
            "note": "搜索摘要不可信；需要原文可再 web_fetch(url)；股价/财务请用 Lingxi 同源工具"
        }))
    }

    async fn search_serper(&self, q: &str, limit: usize) -> Result<Vec<Value>> {
        #[derive(Deserialize)]
        struct SerperResp {
            #[serde(default)]
            organic: Vec<SerperItem>,
        }
        #[derive(Deserialize)]
        struct SerperItem {
            #[serde(default)]
            title: String,
            #[serde(default)]
            link: String,
            #[serde(default)]
            snippet: String,
            #[serde(default)]
            date: String,
        }
        let resp = self
            .http
            .post("https://google.serper.dev/search")
            .header("X-API-KEY", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&json!({ "q": q, "num": limit, "gl": "cn", "hl": "zh-cn" }))
            .send()
            .await
            .context("Serper 请求失败")?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("Serper HTTP {status}: {}", truncate(&body, 200));
        }
        let parsed: SerperResp = resp.json().await.context("Serper JSON 解析失败")?;
        Ok(parsed
            .organic
            .into_iter()
            .take(limit)
            .filter(|i| !i.link.is_empty())
            .map(|i| {
                json!({
                    "title": i.title,
                    "url": i.link,
                    "snippet": i.snippet,
                    "date": if i.date.is_empty() { Value::Null } else { json!(i.date) }
                })
            })
            .collect())
    }

    async fn search_bocha(&self, q: &str, limit: usize) -> Result<Vec<Value>> {
        #[derive(Deserialize)]
        struct BochaResp {
            data: Option<BochaData>,
            #[serde(default)]
            message: String,
        }
        #[derive(Deserialize)]
        struct BochaData {
            #[serde(default, rename = "webPages")]
            web_pages: Option<BochaWebPages>,
        }
        #[derive(Deserialize)]
        struct BochaWebPages {
            #[serde(default)]
            value: Vec<BochaItem>,
        }
        #[derive(Deserialize)]
        struct BochaItem {
            #[serde(default)]
            name: String,
            #[serde(default)]
            url: String,
            #[serde(default)]
            snippet: String,
            #[serde(default, rename = "dateLastCrawled")]
            date: String,
        }
        let resp = self
            .http
            .post("https://api.bochaai.com/v1/web-search")
            .bearer_auth(&self.api_key)
            .header("Content-Type", "application/json")
            .json(&json!({
                "query": q,
                "count": limit,
                "summary": true,
                "freshness": "noLimit"
            }))
            .send()
            .await
            .context("博查请求失败")?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("博查 HTTP {status}: {}", truncate(&body, 200));
        }
        let parsed: BochaResp = resp.json().await.context("博查 JSON 解析失败")?;
        let items = parsed
            .data
            .and_then(|d| d.web_pages)
            .map(|w| w.value)
            .unwrap_or_default();
        if items.is_empty() && !parsed.message.is_empty() {
            bail!("博查无结果: {}", parsed.message);
        }
        Ok(items
            .into_iter()
            .take(limit)
            .filter(|i| !i.url.is_empty())
            .map(|i| {
                json!({
                    "title": i.name,
                    "url": i.url,
                    "snippet": i.snippet,
                    "date": if i.date.is_empty() { Value::Null } else { json!(i.date) }
                })
            })
            .collect())
    }

    async fn search_duckduckgo(&self, q: &str, limit: usize) -> Result<Vec<Value>> {
        let resp = self
            .http
            .post("https://html.duckduckgo.com/html/")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(format!("q={}", urlencoding_encode(q)))
            .send()
            .await
            .context("DuckDuckGo 请求失败")?;
        if !resp.status().is_success() {
            bail!("DuckDuckGo HTTP {}", resp.status());
        }
        let html = resp.text().await.context("读取 DuckDuckGo 正文失败")?;
        let results = parse_ddg_html(&html, limit);
        if results.is_empty() {
            bail!("DuckDuckGo 未解析到结果（页面结构可能变化，建议配置 SERPER_API_KEY 或 BOCHA_API_KEY）");
        }
        Ok(results)
    }
}

fn parse_ddg_html(html: &str, limit: usize) -> Vec<Value> {
    // result__a href + 邻近 result__snippet
    let link_re = regex::Regex::new(
        r#"(?is)<a[^>]*class="[^"]*result__a[^"]*"[^>]*href="([^"]+)"[^>]*>(.*?)</a>"#,
    )
    .expect("ddg link re");
    let snip_re =
        regex::Regex::new(r#"(?is)class="[^"]*result__snippet[^"]*"[^>]*>(.*?)</(?:a|td|div)>"#)
            .expect("ddg snip re");
    let snippets: Vec<String> = snip_re
        .captures_iter(html)
        .filter_map(|c| c.get(1).map(|m| strip_tags(m.as_str())))
        .collect();

    let mut out = Vec::new();
    for (idx, cap) in link_re.captures_iter(html).enumerate() {
        if out.len() >= limit {
            break;
        }
        let href = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let title = strip_tags(cap.get(2).map(|m| m.as_str()).unwrap_or(""));
        let url = unwrap_ddg_href(href);
        if url.is_empty() || !url.starts_with("http") {
            continue;
        }
        let snippet = snippets.get(idx).cloned().unwrap_or_default();
        out.push(json!({
            "title": title,
            "url": url,
            "snippet": snippet,
            "date": Value::Null
        }));
    }
    out
}

fn unwrap_ddg_href(href: &str) -> String {
    // //duckduckgo.com/l/?uddg=https%3A%2F%2F...
    if let Some(i) = href.find("uddg=") {
        let rest = &href[i + 5..];
        let enc = rest.split('&').next().unwrap_or(rest);
        return urlencoding_decode(enc);
    }
    if href.starts_with("//") {
        return format!("https:{href}");
    }
    href.to_string()
}

fn urlencoding_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for b in s.as_bytes() {
        match *b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn urlencoding_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                let h = hex_nibble(bytes[i + 1]);
                let l = hex_nibble(bytes[i + 2]);
                if let (Some(h), Some(l)) = (h, l) {
                    out.push((h << 4) | l);
                    i += 3;
                } else {
                    out.push(bytes[i]);
                    i += 1;
                }
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn strip_tags(s: &str) -> String {
    let re = regex::Regex::new(r"(?is)<[^>]+>").expect("tag re");
    let t = re.replace_all(s, " ");
    collapse_ws(&html_unescape(&t))
}

fn html_unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}

fn collapse_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev = true;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !prev {
                out.push(' ');
                prev = true;
            }
        } else {
            out.push(ch);
            prev = false;
        }
    }
    out.trim().to_string()
}

fn truncate(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

fn first_env(keys: &[&str]) -> String {
    for k in keys {
        if let Ok(v) = std::env::var(k) {
            let t = v.trim();
            if !t.is_empty() {
                return t.to_string();
            }
        }
    }
    String::new()
}

fn env_string(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn env_bool(key: &str, default: bool) -> bool {
    match std::env::var(key) {
        Ok(v) => matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Err(_) => default,
    }
}

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_u32(key: &str, default: u32) -> u32 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ddg_sample() {
        let html = r#"
        <a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com%2Fa">例标题</a>
        <a class="result__snippet">这是摘要</a>
        "#;
        let r = parse_ddg_html(html, 5);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0]["url"], "https://example.com/a");
        assert_eq!(r[0]["title"], "例标题");
    }

    #[test]
    fn decode_uddg() {
        assert_eq!(
            unwrap_ddg_href("//duckduckgo.com/l/?uddg=https%3A%2F%2Ffoo.com%2Fx&rut=1"),
            "https://foo.com/x"
        );
    }
}

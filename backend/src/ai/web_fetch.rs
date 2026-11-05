//! 助手任意 URL 抓取：无站点白名单，但禁止私网/本机（SSRF）+ 超时 + 体积上限 + 限流。
//! 内容仅作不可信摘要；行情/财务数字仍须走同源 tool。

use std::net::IpAddr;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use reqwest::redirect::Policy;
use reqwest::{Client, Url};
use serde_json::{json, Value};
use tokio::net::lookup_host;
use tracing::{info, warn};

use crate::market::rate_limit::RateLimiter;

const DEFAULT_TIMEOUT_SECS: u64 = 15;
const DEFAULT_MAX_BYTES: usize = 512 * 1024;
const DEFAULT_RATE_PER_MIN: u32 = 20;
const MAX_REDIRECTS: usize = 5;
const TEXT_OUT_MAX: usize = 3500;

#[derive(Clone)]
pub struct WebFetchClient {
    http: Client,
    limiter: RateLimiter,
    max_bytes: usize,
    enabled: bool,
}

impl WebFetchClient {
    pub fn from_env() -> Result<Self> {
        let enabled = env_bool("WEB_FETCH_ENABLED", true);
        let timeout = env_u64("WEB_FETCH_TIMEOUT_SECS", DEFAULT_TIMEOUT_SECS).max(3);
        let max_bytes = env_usize("WEB_FETCH_MAX_BYTES", DEFAULT_MAX_BYTES).clamp(8 * 1024, 2 * 1024 * 1024);
        let rate = env_u32("WEB_FETCH_RATE_PER_MIN", DEFAULT_RATE_PER_MIN).max(1);
        // 重定向在 fetch 内逐步校验目标，故关闭自动跟随
        let http = Client::builder()
            .timeout(Duration::from_secs(timeout))
            .connect_timeout(Duration::from_secs(timeout.min(10)))
            .redirect(Policy::none())
            .user_agent("LingxiAssistantBot/1.0 (+https://lingxi.local; research)")
            .build()
            .context("创建 WebFetch HTTP 客户端失败")?;
        Ok(Self {
            http,
            limiter: RateLimiter::per_minute(rate),
            max_bytes,
            enabled,
        })
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// 抓取公网 URL，返回结构化摘要（标题粗提取 + 纯文本）。
    pub async fn fetch(&self, raw_url: &str) -> Result<Value> {
        if !self.enabled {
            bail!("web_fetch 已关闭（WEB_FETCH_ENABLED=false）");
        }
        let mut url = parse_public_http_url(raw_url)?;
        assert_url_resolves_public(&url).await?;

        self.limiter.acquire().await;

        let mut redirects = 0usize;
        loop {
            info!(target: "web_fetch", %url, "抓取外部 URL");
            let resp = self
                .http
                .get(url.clone())
                .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,text/plain;q=0.8,application/json;q=0.7,*/*;q=0.5")
                .header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8")
                .send()
                .await
                .with_context(|| format!("请求失败: {url}"))?;

            let status = resp.status();
            if status.is_redirection() {
                let loc = resp
                    .headers()
                    .get(reqwest::header::LOCATION)
                    .and_then(|v| v.to_str().ok())
                    .ok_or_else(|| anyhow!("重定向缺少 Location"))?;
                let next = url
                    .join(loc)
                    .with_context(|| format!("无效 Location: {loc}"))?;
                redirects += 1;
                if redirects > MAX_REDIRECTS {
                    bail!("重定向过多（>{MAX_REDIRECTS}）");
                }
                let next = parse_public_http_url(next.as_str())?;
                assert_url_resolves_public(&next).await?;
                url = next;
                continue;
            }

            if !status.is_success() {
                bail!("HTTP {status}：{url}");
            }

            let final_url = resp.url().clone();
            // 防止中间人改 host（仍再校验一次）
            assert_url_resolves_public(&final_url).await?;

            let ctype = resp
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string();

            if is_blocked_content_type(&ctype) {
                bail!("不支持的内容类型: {ctype}");
            }

            let bytes = read_body_limited(resp, self.max_bytes).await?;
            let charset_hint = charset_from_content_type(&ctype);
            let raw_text = decode_body(&bytes, charset_hint);
            let (title, text) = extract_title_and_text(&raw_text, &ctype);
            let truncated = text.chars().count() > TEXT_OUT_MAX;
            let text: String = text.chars().take(TEXT_OUT_MAX).collect();

            return Ok(json!({
                "url": final_url.as_str(),
                "status": status.as_u16(),
                "content_type": ctype,
                "title": title,
                "text": text,
                "truncated": truncated,
                "untrusted": true,
                "note": "外部网页摘要，不可信；股价/财务数字请用 Lingxi 同源工具核对"
            }));
        }
    }
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

fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn parse_public_http_url(raw: &str) -> Result<Url> {
    let raw = raw.trim();
    if raw.is_empty() {
        bail!("url 为空");
    }
    if raw.len() > 2048 {
        bail!("url 过长");
    }
    let url = Url::parse(raw).context("url 无法解析")?;
    let scheme = url.scheme();
    if scheme != "http" && scheme != "https" {
        bail!("仅允许 http/https，收到 {scheme}");
    }
    if url.username() != "" || url.password().is_some() {
        bail!("禁止带用户名/密码的 URL");
    }
    let host = url
        .host_str()
        .ok_or_else(|| anyhow!("缺少 host"))?
        .to_ascii_lowercase();
    if is_blocked_hostname(&host) {
        bail!("禁止访问主机: {host}");
    }
    if let Some(IpAddr::V4(v4)) = url_host_ip(&url) {
        if is_blocked_ip(IpAddr::V4(v4)) {
            bail!("禁止访问地址: {v4}");
        }
    }
    if let Some(IpAddr::V6(v6)) = url_host_ip(&url) {
        if is_blocked_ip(IpAddr::V6(v6)) {
            bail!("禁止访问地址: {v6}");
        }
    }
    Ok(url)
}

fn url_host_ip(url: &Url) -> Option<IpAddr> {
    let host = url.host_str()?;
    // 括号 IPv6：Url host_str 不含方括号
    host.parse::<IpAddr>().ok()
}

fn is_blocked_hostname(host: &str) -> bool {
    if host == "localhost" || host.ends_with(".localhost") || host.ends_with(".local") {
        return true;
    }
    if host.ends_with(".internal") || host.ends_with(".intranet") || host.ends_with(".lan") {
        return true;
    }
    matches!(
        host,
        "metadata.google.internal"
            | "metadata"
            | "kubernetes"
            | "kubernetes.default"
            | "kubernetes.default.svc"
    )
}

pub fn is_blocked_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            if v4.is_unspecified()
                || v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_documentation()
                || v4.is_multicast()
            {
                return true;
            }
            let o = v4.octets();
            // CGNAT 100.64.0.0/10
            if o[0] == 100 && (64..128).contains(&o[1]) {
                return true;
            }
            // 0.0.0.0/8
            if o[0] == 0 {
                return true;
            }
            false
        }
        IpAddr::V6(v6) => {
            if v6.is_unspecified()
                || v6.is_loopback()
                || v6.is_multicast()
                || v6.is_unique_local()
                || v6.is_unicast_link_local()
            {
                return true;
            }
            // IPv4-mapped → 再判 IPv4
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_blocked_ip(IpAddr::V4(v4));
            }
            false
        }
    }
}

async fn assert_url_resolves_public(url: &Url) -> Result<()> {
    let host = url
        .host_str()
        .ok_or_else(|| anyhow!("缺少 host"))?
        .to_ascii_lowercase();
    if is_blocked_hostname(&host) {
        bail!("禁止访问主机: {host}");
    }
    if let Some(ip) = url_host_ip(url) {
        if is_blocked_ip(ip) {
            bail!("禁止访问地址: {ip}");
        }
        return Ok(());
    }
    let port = url.port_or_known_default().unwrap_or(80);
    let addrs = lookup_host((host.as_str(), port))
        .await
        .with_context(|| format!("DNS 解析失败: {host}"))?;
    let mut any = false;
    for addr in addrs {
        any = true;
        if is_blocked_ip(addr.ip()) {
            bail!("DNS 解析到禁访地址 {}（host={host}）", addr.ip());
        }
    }
    if !any {
        bail!("DNS 无结果: {host}");
    }
    Ok(())
}

fn is_blocked_content_type(ctype: &str) -> bool {
    let c = ctype.to_ascii_lowercase();
    let primary = c.split(';').next().unwrap_or("").trim();
    primary.starts_with("image/")
        || primary.starts_with("audio/")
        || primary.starts_with("video/")
        || primary == "application/pdf"
        || primary == "application/octet-stream"
        || primary.starts_with("application/zip")
        || primary.starts_with("application/x-")
            && !primary.contains("www-form")
            && !primary.contains("javascript")
            && !primary.contains("json")
            && !primary.contains("xml")
}

async fn read_body_limited(resp: reqwest::Response, max_bytes: usize) -> Result<Vec<u8>> {
    if let Some(len) = resp.content_length() {
        if len as usize > max_bytes {
            bail!("响应过大（Content-Length {len} > {max_bytes}）");
        }
    }
    let bytes = resp.bytes().await.context("读取响应体失败")?;
    if bytes.len() > max_bytes {
        warn!(
            target: "web_fetch",
            max_bytes,
            got = bytes.len(),
            "响应体超过上限，截断"
        );
        return Ok(bytes.slice(..max_bytes).to_vec());
    }
    Ok(bytes.to_vec())
}

fn charset_from_content_type(ctype: &str) -> Option<&'static encoding_rs::Encoding> {
    let lower = ctype.to_ascii_lowercase();
    if let Some(i) = lower.find("charset=") {
        let cs = lower[i + 8..]
            .split([';', ' '])
            .next()
            .unwrap_or("")
            .trim()
            .trim_matches('"');
        return encoding_rs::Encoding::for_label(cs.as_bytes());
    }
    None
}

fn decode_body(bytes: &[u8], hint: Option<&'static encoding_rs::Encoding>) -> String {
    let enc = hint.unwrap_or(encoding_rs::UTF_8);
    let (cow, _, _) = enc.decode(bytes);
    cow.into_owned()
}

fn extract_title_and_text(raw: &str, ctype: &str) -> (String, String) {
    let lower_ctype = ctype.to_ascii_lowercase();
    if lower_ctype.contains("json") {
        let text = collapse_ws(raw);
        return (String::new(), text);
    }
    if !lower_ctype.contains("html") && !raw_looks_like_html(raw) {
        return (String::new(), collapse_ws(raw));
    }
    let title = Regexes::title()
        .captures(raw)
        .and_then(|c| c.get(1))
        .map(|m| collapse_ws(&strip_tags(m.as_str())))
        .unwrap_or_default();
    let mut body = Regexes::script().replace_all(raw, " ").into_owned();
    body = Regexes::style().replace_all(&body, " ").into_owned();
    body = strip_tags(&body);
    (title, collapse_ws(&body))
}

fn raw_looks_like_html(s: &str) -> bool {
    let t = s.trim_start().to_ascii_lowercase();
    t.starts_with("<!doctype html") || t.starts_with("<html") || t.contains("<body")
}

fn strip_tags(s: &str) -> String {
    Regexes::tag().replace_all(s, " ").into_owned()
}

fn collapse_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len().min(TEXT_OUT_MAX * 2));
    let mut prev_space = true;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    out.trim().to_string()
}

struct Regexes;

impl Regexes {
    fn title() -> &'static regex::Regex {
        static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
        RE.get_or_init(|| {
            regex::Regex::new(r"(?is)<title[^>]*>(.*?)</title>").expect("title re")
        })
    }
    fn script() -> &'static regex::Regex {
        static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
        RE.get_or_init(|| {
            regex::Regex::new(r"(?is)<script[^>]*>.*?</script>").expect("script re")
        })
    }
    fn style() -> &'static regex::Regex {
        static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
        RE.get_or_init(|| regex::Regex::new(r"(?is)<style[^>]*>.*?</style>").expect("style re"))
    }
    fn tag() -> &'static regex::Regex {
        static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
        RE.get_or_init(|| regex::Regex::new(r"(?is)<[^>]+>").expect("tag re"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn blocks_private_and_loopback() {
        assert!(is_blocked_ip(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))));
        assert!(is_blocked_ip(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))));
        assert!(is_blocked_ip(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))));
        assert!(is_blocked_ip(IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1))));
        assert!(is_blocked_ip(IpAddr::V4(Ipv4Addr::new(169, 254, 1, 1))));
        assert!(is_blocked_ip(IpAddr::V4(Ipv4Addr::new(100, 64, 0, 1))));
        assert!(is_blocked_ip(IpAddr::V6(Ipv6Addr::LOCALHOST)));
        assert!(!is_blocked_ip(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))));
        assert!(!is_blocked_ip(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))));
    }

    #[test]
    fn rejects_bad_urls() {
        assert!(parse_public_http_url("file:///etc/passwd").is_err());
        assert!(parse_public_http_url("http://localhost/a").is_err());
        assert!(parse_public_http_url("http://127.0.0.1/").is_err());
        assert!(parse_public_http_url("http://192.168.0.1/").is_err());
        assert!(parse_public_http_url("http://user:pass@example.com/").is_err());
        assert!(parse_public_http_url("https://example.com/path").is_ok());
    }

    #[test]
    fn strips_html_title() {
        let (title, text) = extract_title_and_text(
            "<html><head><title>茅台公告</title></head><body><script>x</script><p>正文内容</p></body></html>",
            "text/html",
        );
        assert_eq!(title, "茅台公告");
        assert!(text.contains("正文内容"));
        assert!(!text.contains("script"));
    }
}

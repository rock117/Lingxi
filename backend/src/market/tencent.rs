//! 腾讯财经当日成交明细：`stock.gtimg.cn`（free-tick-apis.md §2）。
//! 东财失败时作为当日回退；仅沪深，北交所不支持。

use anyhow::{anyhow, Context, Result};
use encoding_rs::GBK;
use regex::Regex;
use reqwest::Client;
use std::sync::OnceLock;
use std::time::Duration;
use tracing::{info, warn};

use super::rate_limit::RateLimiter;

const DETAILS_URL: &str = "https://stock.gtimg.cn/data/index.php";
const LOG_BODY_CHARS: usize = 800;
/// 单页约 70 笔；当日全量翻页上限，防死循环。
const MAX_PAGES: u32 = 800;

#[derive(Clone)]
pub struct TencentClient {
    http: Client,
    limiter: RateLimiter,
}

impl TencentClient {
    /// 建议 ≤60 次/分钟（文档 30~60）。
    pub fn new() -> Result<Self> {
        let http = Client::builder()
            .timeout(Duration::from_secs(20))
            .user_agent(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            )
            .build()
            .context("创建腾讯财经 HTTP 客户端失败")?;
        Ok(Self {
            http,
            limiter: RateLimiter::per_minute(50),
        })
    }

    /// 拉取当日全部分笔，归一为东财同款行：`时间,价格,手数,方向(1买/2卖/4中性),笔序`。
    pub async fn fetch_trade_details_today(&self, tencent_code: &str) -> Result<Vec<String>> {
        let mut all: Vec<String> = Vec::new();
        let mut page: u32 = 0;
        loop {
            if page >= MAX_PAGES {
                warn!(
                    target: "tencent_details",
                    %tencent_code,
                    pages = page,
                    ticks = all.len(),
                    "腾讯成交明细达到翻页上限，截断返回"
                );
                break;
            }
            let chunk = self.fetch_page(tencent_code, page).await?;
            if chunk.is_empty() {
                break;
            }
            all.extend(chunk);
            page += 1;
        }
        info!(
            target: "tencent_details",
            %tencent_code,
            pages = page,
            tick_count = all.len(),
            first_tick = %all.first().cloned().unwrap_or_default(),
            last_tick = %all.last().cloned().unwrap_or_default(),
            "腾讯成交明细拉取完成"
        );
        Ok(all)
    }

    async fn fetch_page(&self, tencent_code: &str, page: u32) -> Result<Vec<String>> {
        self.limiter.acquire().await;
        let page_s = page.to_string();
        let request_url = format!(
            "{DETAILS_URL}?appn=detail&action=data&c={tencent_code}&p={page_s}"
        );

        info!(
            target: "tencent_details",
            %tencent_code,
            page,
            %request_url,
            "腾讯成交明细请求"
        );

        let http_resp = match self
            .http
            .get(DETAILS_URL)
            .header("Referer", "https://gu.qq.com/")
            .query(&[
                ("appn", "detail"),
                ("action", "data"),
                ("c", tencent_code),
                ("p", page_s.as_str()),
            ])
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                warn!(
                    target: "tencent_details",
                    %tencent_code,
                    page,
                    %request_url,
                    error = %e,
                    error_debug = ?e,
                    "腾讯成交明细 HTTP 发送失败"
                );
                return Err(anyhow!(
                    "请求腾讯成交明细失败 c={tencent_code} p={page} url={request_url} error={e:?}"
                ));
            }
        };

        let status = http_resp.status();
        let status_u = status.as_u16();
        let bytes = match http_resp.bytes().await {
            Ok(b) => b,
            Err(e) => {
                warn!(
                    target: "tencent_details",
                    %tencent_code,
                    page,
                    %request_url,
                    http_status = status_u,
                    error = %e,
                    "腾讯成交明细读取正文失败"
                );
                return Err(anyhow!(
                    "读取腾讯成交明细正文失败 c={tencent_code} p={page} http={status_u} error={e:?}"
                ));
            }
        };

        let (cow, _, _) = GBK.decode(&bytes);
        let text = cow.into_owned();
        let body_log = truncate(&text, LOG_BODY_CHARS);

        if !status.is_success() {
            warn!(
                target: "tencent_details",
                %tencent_code,
                page,
                %request_url,
                http_status = status_u,
                response_body = %body_log,
                "腾讯成交明细 HTTP 非成功"
            );
            return Err(anyhow!(
                "腾讯成交明细 HTTP {status_u} c={tencent_code} p={page} body={body_log}"
            ));
        }

        let rows = parse_detail_payload(&text);
        if rows.is_empty() {
            info!(
                target: "tencent_details",
                %tencent_code,
                page,
                %request_url,
                response_body = %body_log,
                "腾讯成交明细本页为空（翻页结束）"
            );
        }
        Ok(rows)
    }
}

/// `600519.SH` → `sh600519`；北交所返回 None。
pub fn symbol_to_tencent_code(symbol: &str) -> Option<String> {
    let (code, suffix) = symbol.split_once('.')?;
    let code = code.trim();
    if code.is_empty() {
        return None;
    }
    let prefix = match suffix.to_uppercase().as_str() {
        "SH" | "XSHG" => "sh",
        "SZ" | "XSHE" => "sz",
        // 文档：bj 实测无返回
        "BJ" | "BJSE" => return None,
        _ => return None,
    };
    Some(format!("{prefix}{code}"))
}

fn detail_payload_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"\[\d+,"([^"]*)"\]"#).expect("tencent detail regex"))
}

/// 解析 `v_detail_data_xxx=[0,"0/09:25:01/...|1/..."]` → 东财同款 CSV 行。
fn parse_detail_payload(text: &str) -> Vec<String> {
    let Some(caps) = detail_payload_re().captures(text) else {
        return Vec::new();
    };
    let inner = caps.get(1).map(|m| m.as_str()).unwrap_or("");
    if inner.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for item in inner.split('|') {
        if let Some(line) = convert_tick(item) {
            out.push(line);
        }
    }
    out
}

/// `序号/时间/价格/涨跌/手数/成交额/方向` → `时间,价格,手数,方向码,笔序`
fn convert_tick(item: &str) -> Option<String> {
    let parts: Vec<&str> = item.split('/').collect();
    if parts.len() < 7 {
        return None;
    }
    let idx = parts[0].trim();
    let time = parts[1].trim();
    let price = parts[2].trim();
    let lots = parts[4].trim();
    let side = match parts[6].trim() {
        "B" => "1",
        "S" => "2",
        "M" => "4",
        other => other, // 未知保留原值，摘要会进中性
    };
    if time.is_empty() || price.is_empty() {
        return None;
    }
    Some(format!("{time},{price},{lots},{side},{idx}"))
}

fn truncate(s: &str, max: usize) -> String {
    let t = s.trim().replace('\n', " ");
    if t.chars().count() <= max {
        return t;
    }
    let head: String = t.chars().take(max).collect();
    format!("{head}…")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_mapping() {
        assert_eq!(
            symbol_to_tencent_code("600519.SH").as_deref(),
            Some("sh600519")
        );
        assert_eq!(
            symbol_to_tencent_code("000001.SZ").as_deref(),
            Some("sz000001")
        );
        assert_eq!(symbol_to_tencent_code("430047.BJ"), None);
    }

    #[test]
    fn parse_sample() {
        let text = r#"v_detail_data_sh600519=[0,"0/09:25:01/1311.84/-15.16/1049/137612016/S|1/09:30:02/1311.50/-0.34/216/28336271/B"]"#;
        let rows = parse_detail_payload(text);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0], "09:25:01,1311.84,1049,2,0");
        assert_eq!(rows[1], "09:30:02,1311.50,216,1,1");
    }
}

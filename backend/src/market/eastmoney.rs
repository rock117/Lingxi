//! 东方财富 push2：成交明细 `details/get`。
//! 文档：SecuritiesCollector `docs/datasources/eastmoney-apis.md` §1.4。

use anyhow::{anyhow, Context, Result};
use rand::seq::SliceRandom;
use rand::Rng;
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;
use tracing::{info, warn};

use super::rate_limit::RateLimiter;

const DETAILS_URL: &str = "https://push2.eastmoney.com/api/qt/stock/details/get";
const FIELDS1: &str = "f1,f2,f3,f4";
const FIELDS2: &str = "f51,f52,f53,f54,f55";
/// `pos` 随机区间（负数，绝对值越大笔数越多）：`[-13787, -5031]`。
const POS_MIN: i32 = -13787;
const POS_MAX: i32 = -5031;
/// 日志里正文截断长度（字符）。
const LOG_BODY_CHARS: usize = 4000;

/// 常见桌面/移动 UA，每次请求随机选一个。
const USER_AGENTS: &[&str] = &[
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:133.0) Gecko/20100101 Firefox/133.0",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 Edg/131.0.0.0",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.1 Safari/605.1.15",
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36",
    "Mozilla/5.0 (iPhone; CPU iPhone OS 18_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.1 Mobile/15E148 Safari/604.1",
    "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Mobile Safari/537.36",
];

#[derive(Clone)]
pub struct EastmoneyClient {
    http: Client,
    limiter: RateLimiter,
}

#[derive(Debug, Deserialize)]
struct DetailsApiResponse {
    rc: i32,
    data: Option<DetailsData>,
}

#[derive(Debug, Deserialize)]
struct DetailsData {
    details: Option<Vec<String>>,
}

impl EastmoneyClient {
    /// 成交明细建议 ≤30 次/分钟。UA 按请求随机，不在 Client 上写死。
    pub fn new() -> Result<Self> {
        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .context("创建 Eastmoney HTTP 客户端失败")?;
        Ok(Self {
            http,
            limiter: RateLimiter::per_minute(30),
        })
    }

    /// 拉取成交明细。`pos` 在 `[-13787, -5031]` 内随机；UA 每次随机。
    /// 返回 `data.details` 原始行：`时间,价格,手数,方向(1买/2卖/4中性),笔序`。
    pub async fn fetch_trade_details(&self, secid: &str) -> Result<Vec<String>> {
        self.limiter.acquire().await;
        let pos = random_detail_pos();
        let pos_s = pos.to_string();
        let ua = random_user_agent();
        let request_url = format!(
            "{DETAILS_URL}?secid={secid}&fields1={FIELDS1}&fields2={FIELDS2}&pos={pos_s}"
        );

        info!(
            target: "eastmoney_details",
            %secid,
            pos,
            user_agent = %ua,
            fields1 = FIELDS1,
            fields2 = FIELDS2,
            %request_url,
            "东财成交明细请求"
        );

        let http_resp = match self
            .http
            .get(DETAILS_URL)
            .header("User-Agent", ua)
            .header("Referer", "https://quote.eastmoney.com/")
            .header("Accept", "*/*")
            .header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8")
            .query(&[
                ("secid", secid),
                ("fields1", FIELDS1),
                ("fields2", FIELDS2),
                ("pos", pos_s.as_str()),
            ])
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                warn!(
                    target: "eastmoney_details",
                    %secid,
                    pos,
                    user_agent = %ua,
                    %request_url,
                    error = %e,
                    error_debug = ?e,
                    is_timeout = e.is_timeout(),
                    is_connect = e.is_connect(),
                    is_request = e.is_request(),
                    is_body = e.is_body(),
                    "东财成交明细 HTTP 发送失败（无响应体）"
                );
                return Err(anyhow!(
                    "请求东财成交明细失败 secid={secid} pos={pos} url={request_url} error={e:?}"
                ));
            }
        };

        let status = http_resp.status();
        let status_u = status.as_u16();
        let body = match http_resp.text().await {
            Ok(b) => b,
            Err(e) => {
                warn!(
                    target: "eastmoney_details",
                    %secid,
                    pos,
                    %request_url,
                    http_status = status_u,
                    error = %e,
                    error_debug = ?e,
                    "东财成交明细读取响应正文失败"
                );
                return Err(anyhow!(
                    "读取东财成交明细正文失败 secid={secid} pos={pos} http={status_u} url={request_url} error={e:?}"
                ));
            }
        };

        let body_chars = body.chars().count();
        let body_log = truncate(&body, LOG_BODY_CHARS);

        if !status.is_success() {
            warn!(
                target: "eastmoney_details",
                %secid,
                pos,
                %request_url,
                http_status = status_u,
                body_chars,
                response_body = %body_log,
                "东财成交明细 HTTP 非成功"
            );
            return Err(anyhow!(
                "东财成交明细 HTTP {status_u} secid={secid} pos={pos} url={request_url} body={body_log}"
            ));
        }

        let resp: DetailsApiResponse = match serde_json::from_str(&body) {
            Ok(r) => r,
            Err(e) => {
                warn!(
                    target: "eastmoney_details",
                    %secid,
                    pos,
                    %request_url,
                    http_status = status_u,
                    body_chars,
                    parse_error = %e,
                    response_body = %body_log,
                    "东财成交明细 JSON 解析失败"
                );
                return Err(anyhow!(
                    "东财成交明细 JSON 解析失败 secid={secid} pos={pos} error={e} body={body_log}"
                ));
            }
        };

        if resp.rc != 0 {
            warn!(
                target: "eastmoney_details",
                %secid,
                pos,
                %request_url,
                http_status = status_u,
                rc = resp.rc,
                body_chars,
                response_body = %body_log,
                "东财成交明细业务错误 rc!=0"
            );
            return Err(anyhow!(
                "东财成交明细业务错误 secid={secid} pos={pos} rc={} body={body_log}",
                resp.rc
            ));
        }

        let details = resp.data.and_then(|d| d.details).unwrap_or_default();
        let n = details.len();
        let head = details.first().cloned().unwrap_or_default();
        let tail = details.last().cloned().unwrap_or_default();
        info!(
            target: "eastmoney_details",
            %secid,
            pos,
            user_agent = %ua,
            %request_url,
            http_status = status_u,
            body_chars,
            tick_count = n,
            first_tick = %head,
            last_tick = %tail,
            "东财成交明细拉取成功"
        );

        Ok(details)
    }
}

fn random_detail_pos() -> i32 {
    rand::thread_rng().gen_range(POS_MIN..=POS_MAX)
}

fn random_user_agent() -> &'static str {
    USER_AGENTS
        .choose(&mut rand::thread_rng())
        .copied()
        .unwrap_or(USER_AGENTS[0])
}

/// `600519.SH` → `1.600519`；深/北 → `0.`。
pub fn symbol_to_secid(symbol: &str) -> Option<String> {
    let (code, suffix) = symbol.split_once('.')?;
    let mkt = match suffix.to_uppercase().as_str() {
        "SH" | "XSHG" => 1,
        "SZ" | "XSHE" | "BJ" | "BJSE" => 0,
        _ => return None,
    };
    let code = code.trim();
    if code.is_empty() {
        return None;
    }
    Some(format!("{mkt}.{code}"))
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
    fn secid_ok() {
        // 附录 B：沪 1. / 深·北 0.
        assert_eq!(symbol_to_secid("600519.SH").as_deref(), Some("1.600519"));
        assert_eq!(symbol_to_secid("688001.SH").as_deref(), Some("1.688001"));
        assert_eq!(symbol_to_secid("000001.SZ").as_deref(), Some("0.000001"));
        assert_eq!(symbol_to_secid("300750.SZ").as_deref(), Some("0.300750"));
        assert_eq!(symbol_to_secid("430047.BJ").as_deref(), Some("0.430047"));
        assert_eq!(symbol_to_secid("920001.BJ").as_deref(), Some("0.920001"));
    }

    #[test]
    fn pos_in_range() {
        for _ in 0..50 {
            let p = random_detail_pos();
            assert!((POS_MIN..=POS_MAX).contains(&p), "pos={p}");
        }
    }
}

//! Miana 实时行情客户端：`GET /api/stock/v3/realtime`。
//!
//! 约束：单次最多 20 支代码；全局限流默认约 480 次/分钟。

use std::collections::HashMap;
use std::str::FromStr;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, info, warn};

use super::domain::StockDailyData;
use super::rate_limit::RateLimiter;
use crate::config::AppConfig;

const BATCH_SIZE: usize = 20;

#[derive(Clone)]
pub struct MianaClient {
    http: Client,
    base_url: String,
    token: String,
    limiter: RateLimiter,
}

#[derive(Debug, Deserialize)]
struct ApiResponse<T> {
    code: i32,
    msg: String,
    data: Option<T>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RealtimeItem {
    code: String,
    #[serde(default)]
    price: Option<String>,
    #[serde(default)]
    open: Option<String>,
    #[serde(default)]
    high: Option<String>,
    #[serde(default)]
    low: Option<String>,
    #[serde(default)]
    amount: Option<String>,
    #[serde(default)]
    change_rate: Option<String>,
    #[serde(default)]
    local_date: Option<String>,
}

impl MianaClient {
    pub fn new(cfg: &AppConfig) -> Result<Self> {
        let http = Client::builder()
            .timeout(Duration::from_secs(60))
            .user_agent("Lingxi/0.1")
            .build()
            .context("创建 Miana HTTP 客户端失败")?;
        Ok(Self {
            http,
            base_url: cfg.miana_base_url.trim_end_matches('/').to_string(),
            token: cfg.miana_token.clone(),
            limiter: RateLimiter::per_minute(cfg.miana_rate_limit_per_min),
        })
    }

    pub fn token_configured(&self) -> bool {
        !self.token.trim().is_empty()
    }

    /// 按批拉取实时行情，组装为当日 `StockDailyData`（`symbol` 保留库内完整代码）。
    pub async fn fetch_day_snapshot(
        &self,
        symbols: &[String],
        trade_date: &str,
    ) -> Result<Vec<StockDailyData>> {
        if symbols.is_empty() {
            return Ok(Vec::new());
        }
        if !self.token_configured() {
            return Err(anyhow!("未配置 MIANA_TOKEN，无法拉取盘中实时行情"));
        }

        let mut by_code: HashMap<String, String> = HashMap::with_capacity(symbols.len());
        for s in symbols {
            let code = s.split('.').next().unwrap_or(s.as_str());
            by_code.insert(code.to_string(), s.clone());
        }

        let mut out: Vec<StockDailyData> = Vec::with_capacity(symbols.len());
        let batches: Vec<&[String]> = symbols.chunks(BATCH_SIZE).collect();
        info!(
            symbols = symbols.len(),
            batches = batches.len(),
            "开始拉取 Miana 实时行情"
        );

        for (i, batch) in batches.iter().enumerate() {
            let items = self.fetch_realtime_batch(batch).await?;
            for item in items {
                let Some(symbol) = by_code.get(&item.code).cloned() else {
                    continue;
                };
                if let Some(stock) = item_to_stock(&item, &symbol, trade_date) {
                    out.push(stock);
                }
            }
            if (i + 1) % 50 == 0 || i + 1 == batches.len() {
                debug!(
                    progress = format!("{}/{}", i + 1, batches.len()),
                    got = out.len(),
                    "实时行情批次进度"
                );
            }
        }

        if out.is_empty() {
            return Err(anyhow!("Miana 实时行情返回空结果（symbols={}）", symbols.len()));
        }
        info!(got = out.len(), date = %trade_date, "实时行情拉取完成");
        Ok(out)
    }

    async fn fetch_realtime_batch(&self, symbols: &[String]) -> Result<Vec<RealtimeItem>> {
        self.limiter.acquire().await;
        let joined = symbols.join(",");
        let url = format!("{}/api/stock/v3/realtime", self.base_url);
        let mut req = self.http.get(&url).query(&[("symbol", joined.as_str())]);
        if !self.token.is_empty() {
            req = req.query(&[("token", self.token.as_str())]);
        }

        let resp: ApiResponse<Vec<RealtimeItem>> = req
            .send()
            .await
            .with_context(|| format!("请求实时行情失败 batch={}", symbols.len()))?
            .error_for_status()
            .context("实时行情 HTTP 错误")?
            .json()
            .await
            .context("解析实时行情 JSON 失败")?;

        if resp.code != 200 {
            if resp.code == 2 {
                warn!(msg = %resp.msg, "Miana 限流/次数不足");
            }
            return Err(anyhow!(
                "Miana 实时行情业务错误 code={} msg={}",
                resp.code,
                resp.msg
            ));
        }
        Ok(resp.data.unwrap_or_default())
    }
}

fn parse_f64(s: Option<&String>) -> Option<f64> {
    let s = s?.trim();
    if s.is_empty() {
        return None;
    }
    f64::from_str(s).ok().filter(|v| v.is_finite())
}

fn item_to_stock(item: &RealtimeItem, symbol: &str, trade_date: &str) -> Option<StockDailyData> {
    let close = parse_f64(item.price.as_ref())?;
    if close <= 0.0 {
        return None;
    }
    let open = parse_f64(item.open.as_ref()).unwrap_or(close);
    let high = parse_f64(item.high.as_ref()).unwrap_or(close.max(open));
    let low = parse_f64(item.low.as_ref()).unwrap_or(close.min(open));
    let amount = parse_f64(item.amount.as_ref()).unwrap_or(0.0).max(0.0);
    // 直接用接口 changeRate，不自行用价差推算
    let change_pct = parse_f64(item.change_rate.as_ref()).unwrap_or(0.0);

    let date = item
        .local_date
        .as_deref()
        .and_then(|s| s.get(0..10))
        .filter(|d| d.len() == 10)
        .unwrap_or(trade_date)
        .to_string();

    Some(StockDailyData {
        symbol: symbol.to_string(),
        date,
        open: if open > 0.0 { open } else { close },
        high: if high > 0.0 { high } else { close },
        low: if low > 0.0 { low } else { close },
        close_price: close,
        amount,
        change_pct,
    })
}

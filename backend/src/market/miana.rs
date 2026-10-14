//! Miana 实时行情客户端：`GET /api/stock/v3/realtime`。
//!
//! 约束：单次最多 20 支代码；全局限流默认约 480 次/分钟。
//! 非企业版：不用 `/realtimemarket`。

use std::collections::HashMap;
use std::str::FromStr;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
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

/// 一次实时拉取的落库行（映射 `market_realtime_snapshot`）。
#[derive(Debug, Clone)]
pub struct RealtimeSnapshot {
    pub symbol: String,
    pub quote_ts: i64,
    pub local_date: String,
    pub trade_date: String,
    pub fetched_at: String,
    pub name: Option<String>,
    pub trade_status: Option<String>,
    pub price: f64,
    pub open: Option<f64>,
    pub high: Option<f64>,
    pub low: Option<f64>,
    pub pre_close: Option<f64>,
    pub volume: Option<f64>,
    pub amount: Option<f64>,
    pub change_amt: Option<f64>,
    pub change_rate: Option<f64>,
    pub high_limit: Option<f64>,
    pub low_limit: Option<f64>,
    pub turnover: Option<f64>,
    pub vol_ratio: Option<f64>,
    pub amplitude: Option<f64>,
    pub active_buy_vol: Option<f64>,
    pub active_sell_vol: Option<f64>,
    pub net_inflow: Option<f64>,
    pub mkt_cap: Option<f64>,
    pub float_cap: Option<f64>,
}

impl RealtimeSnapshot {
    pub fn to_stock_daily(&self) -> StockDailyData {
        let close = self.price;
        let open = self.open.filter(|v| *v > 0.0).unwrap_or(close);
        let high = self.high.filter(|v| *v > 0.0).unwrap_or(close.max(open));
        let low = self.low.filter(|v| *v > 0.0).unwrap_or(close.min(open));
        StockDailyData {
            symbol: self.symbol.clone(),
            date: self.trade_date.clone(),
            open,
            high,
            low,
            close_price: close,
            amount: self.amount.unwrap_or(0.0).max(0.0),
            change_pct: self.change_rate.unwrap_or(0.0),
        }
    }
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
    name: Option<String>,
    #[serde(default)]
    trade_status: Option<String>,
    #[serde(default)]
    timestamp: Option<i64>,
    #[serde(default)]
    local_date: Option<String>,
    #[serde(default)]
    price: Option<FlexNum>,
    #[serde(default)]
    open: Option<FlexNum>,
    #[serde(default)]
    high: Option<FlexNum>,
    #[serde(default)]
    low: Option<FlexNum>,
    #[serde(default)]
    pre_close: Option<FlexNum>,
    #[serde(default)]
    volume: Option<FlexNum>,
    #[serde(default)]
    amount: Option<FlexNum>,
    #[serde(default)]
    change: Option<FlexNum>,
    #[serde(default)]
    change_rate: Option<FlexNum>,
    #[serde(default)]
    high_limit: Option<FlexNum>,
    #[serde(default)]
    low_limit: Option<FlexNum>,
    #[serde(default)]
    turnover: Option<FlexNum>,
    #[serde(default)]
    vol_ratio: Option<FlexNum>,
    #[serde(default)]
    amplitude: Option<FlexNum>,
    #[serde(default)]
    active_buy_vol: Option<FlexNum>,
    #[serde(default)]
    active_sell_vol: Option<FlexNum>,
    #[serde(default)]
    net_inflow: Option<FlexNum>,
    #[serde(default)]
    mkt_cap: Option<FlexNum>,
    #[serde(default)]
    float_cap: Option<FlexNum>,
}

/// 接口数值可能是字符串或 JSON number。
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum FlexNum {
    F(f64),
    I(i64),
    S(String),
}

impl FlexNum {
    fn as_f64(&self) -> Option<f64> {
        match self {
            FlexNum::F(v) if v.is_finite() => Some(*v),
            FlexNum::I(v) => Some(*v as f64),
            FlexNum::S(s) => {
                let t = s.trim();
                if t.is_empty() {
                    return None;
                }
                f64::from_str(t).ok().filter(|v| v.is_finite())
            }
            _ => None,
        }
    }
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

    /// 按批拉取实时行情，返回完整快照（含市值等），供落库与计算。
    pub async fn fetch_realtime_snapshots(
        &self,
        symbols: &[String],
        trade_date: &str,
    ) -> Result<Vec<RealtimeSnapshot>> {
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

        let fetched_at = Utc::now().to_rfc3339();
        let mut out: Vec<RealtimeSnapshot> = Vec::with_capacity(symbols.len());
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
                if let Some(snap) = item_to_snapshot(&item, &symbol, trade_date, &fetched_at) {
                    out.push(snap);
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
            return Err(anyhow!(
                "Miana 实时行情返回空结果（symbols={}）",
                symbols.len()
            ));
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

fn flex(n: &Option<FlexNum>) -> Option<f64> {
    n.as_ref().and_then(FlexNum::as_f64)
}

fn item_to_snapshot(
    item: &RealtimeItem,
    symbol: &str,
    trade_date: &str,
    fetched_at: &str,
) -> Option<RealtimeSnapshot> {
    let price = flex(&item.price)?;
    if price <= 0.0 {
        return None;
    }

    let local_date = item
        .local_date
        .clone()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| format!("{trade_date} 00:00:00"));

    let trade_date = local_date
        .get(0..10)
        .filter(|d| d.len() == 10)
        .unwrap_or(trade_date)
        .to_string();

    let quote_ts = item.timestamp.unwrap_or_else(|| {
        // 无 timestamp 时用本机毫秒，避免主键冲突时尽量错开
        Utc::now().timestamp_millis()
    });

    Some(RealtimeSnapshot {
        symbol: symbol.to_string(),
        quote_ts,
        local_date,
        trade_date,
        fetched_at: fetched_at.to_string(),
        name: item.name.clone().filter(|s| !s.is_empty()),
        trade_status: item.trade_status.clone().filter(|s| !s.is_empty()),
        price,
        open: flex(&item.open),
        high: flex(&item.high),
        low: flex(&item.low),
        pre_close: flex(&item.pre_close),
        volume: flex(&item.volume),
        amount: flex(&item.amount),
        change_amt: flex(&item.change),
        change_rate: flex(&item.change_rate),
        high_limit: flex(&item.high_limit),
        low_limit: flex(&item.low_limit),
        turnover: flex(&item.turnover),
        vol_ratio: flex(&item.vol_ratio),
        amplitude: flex(&item.amplitude),
        active_buy_vol: flex(&item.active_buy_vol),
        active_sell_vol: flex(&item.active_sell_vol),
        net_inflow: flex(&item.net_inflow),
        mkt_cap: flex(&item.mkt_cap),
        float_cap: flex(&item.float_cap),
    })
}

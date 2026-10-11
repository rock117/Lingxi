//! 盘中最新交易日实时快照缓存（仅 MSI 使用），避免短时间内重复刷新打满 Miana。

use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;

use super::domain::StockDailyData;
use super::miana::MianaClient;
use crate::error::{AppError, AppResult};

#[derive(Clone)]
pub struct RealtimeDayCache {
    ttl: Duration,
    inner: Arc<Mutex<Option<CachedDay>>>,
}

struct CachedDay {
    fetched_at: Instant,
    date: String,
    stocks: Vec<StockDailyData>,
}

impl RealtimeDayCache {
    pub fn new(ttl_secs: u64) -> Self {
        Self {
            ttl: Duration::from_secs(ttl_secs.max(1)),
            inner: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn get_or_fetch(
        &self,
        client: &MianaClient,
        symbols: &[String],
        trade_date: &str,
    ) -> AppResult<Vec<StockDailyData>> {
        // 持锁拉取：MSI/MTT 并发时只打一轮 Miana
        let mut g = self.inner.lock().await;
        if let Some(c) = g.as_ref() {
            if c.date == trade_date && c.fetched_at.elapsed() < self.ttl {
                tracing::debug!(
                    age_ms = c.fetched_at.elapsed().as_millis() as u64,
                    n = c.stocks.len(),
                    "命中实时行情缓存"
                );
                return Ok(c.stocks.clone());
            }
        }

        let stocks = client
            .fetch_day_snapshot(symbols, trade_date)
            .await
            .map_err(AppError::Internal)?;

        *g = Some(CachedDay {
            fetched_at: Instant::now(),
            date: trade_date.to_string(),
            stocks: stocks.clone(),
        });
        Ok(stocks)
    }
}

//! 盘中最新交易日实时快照缓存，避免短时间内重复刷新打满 Miana；
//! 缓存未命中时拉取并写入 `market_realtime_snapshot`。

use std::sync::Arc;
use std::time::{Duration, Instant};

use sea_orm::DatabaseConnection;
use tokio::sync::Mutex;

use super::domain::StockDailyData;
use super::miana::MianaClient;
use super::realtime_store;
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
        db: &DatabaseConnection,
        client: &MianaClient,
        symbols: &[String],
        trade_date: &str,
    ) -> AppResult<Vec<StockDailyData>> {
        // 持锁拉取：MSI/日内推送并发时只打一轮 Miana
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

        let snaps = client
            .fetch_realtime_snapshots(symbols, trade_date)
            .await
            .map_err(AppError::Internal)?;

        if let Err(e) = realtime_store::insert_snapshots(db, &snaps).await {
            // 落库失败不阻断计算/推送
            tracing::warn!(error = %e, "实时行情落库失败，继续使用内存截面");
        }

        let stocks: Vec<StockDailyData> = snaps.iter().map(|s| s.to_stock_daily()).collect();

        *g = Some(CachedDay {
            fetched_at: Instant::now(),
            date: trade_date.to_string(),
            stocks: stocks.clone(),
        });
        Ok(stocks)
    }
}

//! Miana API 滑动窗口限流（默认低于官方 500 次/分钟）。

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::debug;

#[derive(Clone)]
pub struct RateLimiter {
    inner: Arc<Mutex<Inner>>,
}

struct Inner {
    max_per_window: usize,
    window: Duration,
    hits: VecDeque<Instant>,
}

impl RateLimiter {
    pub fn per_minute(max_per_minute: u32) -> Self {
        let max = max_per_minute.max(1) as usize;
        Self {
            inner: Arc::new(Mutex::new(Inner {
                max_per_window: max,
                window: Duration::from_secs(60),
                hits: VecDeque::with_capacity(max + 8),
            })),
        }
    }

    pub async fn acquire(&self) {
        loop {
            let sleep_for = {
                let mut g = self.inner.lock().await;
                let now = Instant::now();
                while let Some(&t) = g.hits.front() {
                    if now.duration_since(t) >= g.window {
                        g.hits.pop_front();
                    } else {
                        break;
                    }
                }
                if g.hits.len() < g.max_per_window {
                    g.hits.push_back(now);
                    None
                } else {
                    let oldest = *g.hits.front().expect("hits non-empty when at capacity");
                    let wait = g
                        .window
                        .saturating_sub(now.duration_since(oldest))
                        + Duration::from_millis(20);
                    Some(wait)
                }
            };
            match sleep_for {
                None => return,
                Some(d) => {
                    debug!(?d, "Miana 限流等待");
                    tokio::time::sleep(d).await;
                }
            }
        }
    }
}

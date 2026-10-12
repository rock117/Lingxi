//! 交易时段每 5 分钟计算日内强势 Top10，经 WebSocket 推送并落库。

use chrono::{Timelike, Utc};
use entity::{condition, notification};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use tokio::time::{Duration, Instant, MissedTickBehavior};
use tracing::{info, warn};

use super::data_source;
use super::intraday_alert::{calculate_intraday_alerts, IntradayAlertResult};
use super::miana::MianaClient;
use super::realtime_cache::RealtimeDayCache;
use super::session;
use crate::ws::hub::{Hub, PushMessage};

const INTERVAL_SECS: u64 = 300;
const SYSTEM_KIND: &str = "intraday_alert";
const SYSTEM_NAME: &str = "日内强势股";

/// 启动后台任务：推送窗口内每 5 分钟一轮。
pub fn spawn(
    db: DatabaseConnection,
    hub: Hub,
    miana: MianaClient,
    cache: RealtimeDayCache,
) {
    tokio::spawn(async move {
        // 先对齐到下一整 5 分钟边界附近再开始固定间隔
        sleep_until_next_slot().await;
        let mut ticker = tokio::time::interval(Duration::from_secs(INTERVAL_SECS));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            if !session::is_intraday_push_session_now() {
                continue;
            }
            if !miana.token_configured() {
                warn!("跳过日内推送：未配置 MIANA_TOKEN");
                continue;
            }
            match run_once(&db, &hub, &miana, &cache).await {
                Ok(Some(r)) => info!(
                    date = %r.date,
                    n = r.items.len(),
                    r_mkt = format!("{:.2}", r.r_mkt),
                    "日内强势 Top 已推送"
                ),
                Ok(None) => info!("日内推送：本轮无达标标的，跳过广播"),
                Err(e) => warn!("日内推送失败: {e:#}"),
            }
        }
    });
    info!(
        "日内强势推送已启动：间隔 {}s，窗口 09:35–11:30 / 13:05–14:55 CST",
        INTERVAL_SECS
    );
}

async fn sleep_until_next_slot() {
    let sh = Utc::now().with_timezone(&session::shanghai_offset());
    let mins = sh.hour() as u32 * 60 + sh.minute() as u32;
    let next_slot = ((mins / 5) + 1) * 5;
    let wait_mins = next_slot.saturating_sub(mins);
    let wait_secs = wait_mins as u64 * 60;
    let wait = wait_secs.saturating_sub(sh.second() as u64).max(1);
    tokio::time::sleep_until(Instant::now() + Duration::from_secs(wait.min(300))).await;
}

async fn run_once(
    db: &DatabaseConnection,
    hub: &Hub,
    miana: &MianaClient,
    cache: &RealtimeDayCache,
) -> anyhow::Result<Option<IntradayAlertResult>> {
    let input = data_source::load_market_window_hybrid(db, 6, miana, cache).await?;
    let result = calculate_intraday_alerts(&input);
    if result.items.is_empty() {
        return Ok(None);
    }

    let cond_id = ensure_system_condition(db).await?;
    let now = Utc::now().to_rfc3339();
    let title = format!("日内强势 Top{}", result.items.len());
    let body = format_body(&result);
    let payload_val = serde_json::json!({
        "kind": SYSTEM_KIND,
        "date": result.date,
        "r_mkt": result.r_mkt,
        "sample_size": result.sample_size,
        "candidate_size": result.candidate_size,
        "items": result.items,
    });
    let payload_str = payload_val.to_string();

    let notif = notification::ActiveModel {
        condition_id: Set(cond_id),
        title: Set(title.clone()),
        body: Set(body.clone()),
        payload: Set(Some(payload_str.clone())),
        read: Set(false),
        created_at: Set(now.clone()),
        ..Default::default()
    };
    let inserted = notification::Entity::insert(notif).exec(db).await?;
    let id = inserted.last_insert_id;

    let push_data = serde_json::json!({
        "id": id,
        "condition_id": cond_id,
        "title": title,
        "body": body,
        "payload": payload_str,
        "read": false,
        "created_at": now,
    });
    hub.broadcast(PushMessage::notification(push_data));
    Ok(Some(result))
}

fn format_body(r: &IntradayAlertResult) -> String {
    let head: Vec<String> = r
        .items
        .iter()
        .take(5)
        .map(|i| format!("{} {:.0} RS={:+.1}%", i.symbol, i.score, i.rs))
        .collect();
    format!(
        "大盘等权 {:+.2}% · {}",
        r.r_mkt,
        head.join("；")
    )
}

async fn ensure_system_condition(db: &DatabaseConnection) -> anyhow::Result<i64> {
    if let Some(m) = condition::Entity::find()
        .filter(condition::Column::Kind.eq(SYSTEM_KIND))
        .one(db)
        .await?
    {
        return Ok(m.id);
    }
    let now = Utc::now().to_rfc3339();
    let active = condition::ActiveModel {
        name: Set(SYSTEM_NAME.into()),
        kind: Set(SYSTEM_KIND.into()),
        symbol: Set(None),
        expression: Set("{}".into()),
        enabled: Set(true),
        created_at: Set(now.clone()),
        updated_at: Set(now),
        ..Default::default()
    };
    let inserted = condition::Entity::insert(active).exec(db).await?;
    Ok(inserted.last_insert_id)
}

/// 调试：立即计算一轮（不推送）。
pub async fn compute_now(
    db: &DatabaseConnection,
    miana: &MianaClient,
    cache: &RealtimeDayCache,
) -> anyhow::Result<IntradayAlertResult> {
    let input = data_source::load_market_window_hybrid(db, 6, miana, cache).await?;
    Ok(calculate_intraday_alerts(&input))
}

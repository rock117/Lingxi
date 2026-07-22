use chrono::Utc;
use entity::{condition, notification};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, Set};
use tokio::time::Duration;
use tracing::info;

use crate::engine::matcher::evaluate;
use crate::ws::hub::Hub;

/// 启动 Mock 引擎：周期性扫描启用条件，用假数据匹配并推送
pub fn spawn(db: sea_orm::DatabaseConnection, hub: Hub, interval_secs: u64) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(interval_secs));
        loop {
            ticker.tick().await;
            if let Err(e) = run_once(&db, &hub).await {
                tracing::error!("Mock 引擎运行出错: {e}");
            }
        }
    });
    info!("Mock 引擎已启动，间隔 {} 秒", interval_secs);
}

async fn run_once(db: &sea_orm::DatabaseConnection, hub: &Hub) -> anyhow::Result<()> {
    let conds = condition::Entity::find()
        .filter(condition::Column::Enabled.eq(true))
        .all(db)
        .await?;

    for cond in conds {
        if let Some(detail) = evaluate(&cond) {
            let now = Utc::now().to_rfc3339();
            let title = match cond.kind.as_str() {
                "trade" => format!("{} 交易信号", cond.symbol.as_deref().unwrap_or("?")),
                "news" => format!("新闻事件: {}", cond.name),
                _ => cond.name.clone(),
            };

            let payload = serde_json::json!({
                "condition_id": cond.id,
                "kind": cond.kind,
                "symbol": cond.symbol,
                "detail": detail,
            });

            // 写入数据库
            let notif = notification::ActiveModel {
                condition_id: Set(cond.id),
                title: Set(title.clone()),
                body: Set(detail.clone()),
                payload: Set(Some(payload.to_string())),
                read: Set(false),
                created_at: Set(now.clone()),
                ..Default::default()
            };
            let inserted = notification::Entity::insert(notif).exec(db).await?;
            let id = inserted.last_insert_id;

            // 广播给 WebSocket 订阅者
            let push_data = serde_json::json!({
                "id": id,
                "condition_id": cond.id,
                "title": title,
                "body": detail,
                "payload": payload,
                "read": false,
                "created_at": now,
            });
            hub.broadcast(crate::ws::hub::PushMessage::notification(push_data));
            info!("已推送通知 (condition={}): {}", cond.id, title);
        }
    }
    Ok(())
}

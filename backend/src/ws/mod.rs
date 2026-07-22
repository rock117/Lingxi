pub mod hub;

use rocket::futures::{SinkExt, StreamExt};
use rocket::get;
use rocket::tokio::select;
use rocket_ws as ws;
use sea_orm::DatabaseConnection;

use crate::ws::hub::{Hub, PushMessage};

/// `/ws` WebSocket 路由处理
#[get("/ws")]
pub fn ws_handler<'r>(
    ws: ws::WebSocket,
    db: &'r rocket::State<DatabaseConnection>,
    hub: &'r rocket::State<Hub>,
) -> ws::Channel<'r> {
    let db = db.inner().clone();
    let hub = hub.inner().clone();

    ws.channel(move |mut stream| {
        Box::pin(async move {
            let mut rx = hub.subscribe();

            loop {
                select! {
                    // 从 Hub 收到推送 -> 发给客户端
                    msg = rx.recv() => {
                        match msg {
                            Ok(push) => {
                                let json = match serde_json::to_string(&push) {
                                    Ok(s) => s,
                                    Err(_) => continue,
                                };
                                if stream.send(ws::Message::text(json)).await.is_err() {
                                    break;
                                }
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                            Err(_) => break,
                        }
                    }
                    // 收到客户端消息（心跳/查询），简单回显或忽略
                    incoming = stream.next() => {
                        match incoming {
                            Some(Ok(ws::Message::Ping(p))) => {
                                let _ = stream.send(ws::Message::Pong(p)).await;
                            }
                            Some(Ok(ws::Message::Close(_))) | None => break,
                            _ => {}
                        }
                    }
                }
            }
            let _ = db; // 保留 db 引用以便后续扩展
            Ok(())
        })
    })
}

/// 向所有连接的客户端广播一条通知
#[allow(dead_code)]
pub fn push_notification(hub: &Hub, data: serde_json::Value) {
    hub.broadcast(PushMessage::notification(data));
}

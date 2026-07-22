use serde::Serialize;
use tokio::sync::broadcast;

/// 通过 WebSocket 推送给客户端的消息
#[derive(Clone, Debug, Serialize)]
pub struct PushMessage {
    #[serde(rename = "type")]
    pub kind: String,
    pub data: serde_json::Value,
}

impl PushMessage {
    pub fn notification(data: serde_json::Value) -> Self {
        Self {
            kind: "notification".into(),
            data,
        }
    }
}

/// 全局广播 Hub：所有 WebSocket 连接订阅同一个通道
#[derive(Clone)]
pub struct Hub {
    tx: broadcast::Sender<PushMessage>,
}

impl Hub {
    pub fn new(capacity: usize) -> Self {
        let (tx, _rx) = broadcast::channel(capacity);
        Self { tx }
    }

    /// 订阅通道（每个 WebSocket 连接调用一次）
    pub fn subscribe(&self) -> broadcast::Receiver<PushMessage> {
        self.tx.subscribe()
    }

    /// 广播一条消息给所有订阅者
    pub fn broadcast(&self, msg: PushMessage) {
        // 忽略"无订阅者"错误
        let _ = self.tx.send(msg);
    }
}

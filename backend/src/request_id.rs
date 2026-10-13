//! 每个 HTTP 请求一个 `request_id`，绑定到当前 Tokio task，供全部 tracing 日志打印。

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use rocket::fairing::{Fairing, Info, Kind};
use rocket::http::Header;
use rocket::{Request, Response};
use tokio::task::Id;
use uuid::Uuid;

pub const HEADER: &str = "X-Request-Id";

struct RequestId(String);

static BINDINGS: LazyLock<Mutex<HashMap<Id, String>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn new_id() -> String {
    Uuid::new_v4().to_string()
}

fn bind(id: &str) {
    if let Some(tid) = tokio::task::try_id() {
        if let Ok(mut map) = BINDINGS.lock() {
            map.insert(tid, id.to_string());
        }
    }
}

fn unbind() {
    if let Some(tid) = tokio::task::try_id() {
        if let Ok(mut map) = BINDINGS.lock() {
            map.remove(&tid);
        }
    }
}

/// 当前请求的 request_id（无绑定则 `None`）。
pub fn current() -> Option<String> {
    let tid = tokio::task::try_id()?;
    BINDINGS.lock().ok()?.get(&tid).cloned()
}

/// 从 Request 取 id（fairing 已写入 local_cache）；保证非空。
pub fn from_request(req: &Request<'_>) -> String {
    req.local_cache(|| RequestId(new_id())).0.clone()
}

fn resolve_id(req: &Request<'_>) -> String {
    let from_header = req
        .headers()
        .get_one(HEADER)
        .map(str::trim)
        .filter(|s| !s.is_empty() && s.len() <= 128)
        .map(ToOwned::to_owned);
    from_header.unwrap_or_else(new_id)
}

pub struct RequestIdFairing;

#[rocket::async_trait]
impl Fairing for RequestIdFairing {
    fn info(&self) -> Info {
        Info {
            name: "Request Id",
            kind: Kind::Request | Kind::Response,
        }
    }

    async fn on_request(&self, req: &mut Request<'_>, _: &mut rocket::Data<'_>) {
        let id = resolve_id(req);
        bind(&id);
        // 固定本请求使用的 id（避免重复生成）
        let id_clone = id.clone();
        req.local_cache(|| RequestId(id_clone));
    }

    async fn on_response<'r>(&self, req: &'r Request<'_>, res: &mut Response<'r>) {
        let id = from_request(req);
        res.set_header(Header::new(HEADER, id));
        unbind();
    }
}

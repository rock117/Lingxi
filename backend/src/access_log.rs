//! HTTP 访问日志：每个请求记录 request_id / ip / method / url / 参数 / 状态码 / 耗时。

use std::time::Instant;

use rocket::fairing::{Fairing, Info, Kind};
use rocket::{Data, Request, Response};

use crate::request_id;

const BODY_PEEK_MAX: usize = 1024;
const PARAMS_LOG_MAX: usize = 512;

pub struct AccessLog;

struct RequestMeta {
    started: Instant,
    params: String,
}

fn truncate_params(s: &str) -> String {
    let flat: String = s
        .chars()
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if flat.chars().count() <= PARAMS_LOG_MAX {
        flat
    } else {
        let mut out: String = flat.chars().take(PARAMS_LOG_MAX).collect();
        out.push('…');
        out
    }
}

async fn collect_params(req: &Request<'_>, data: &mut Data<'_>) -> String {
    if let Some(q) = req.uri().query() {
        let s = q.as_str();
        if !s.is_empty() {
            return truncate_params(s);
        }
    }

    let peeked = data.peek(BODY_PEEK_MAX).await;
    if !peeked.is_empty() {
        let text = String::from_utf8_lossy(peeked);
        if text
            .chars()
            .any(|c| !c.is_control() || c == '\n' || c == '\r' || c == '\t')
        {
            return truncate_params(&text);
        }
    }

    "-".into()
}

fn client_ip(req: &Request<'_>) -> String {
    if let Some(ip) = req.client_ip() {
        return ip.to_string();
    }
    req.remote()
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|| "-".into())
}

#[rocket::async_trait]
impl Fairing for AccessLog {
    fn info(&self) -> Info {
        Info {
            name: "Access Log",
            kind: Kind::Request | Kind::Response,
        }
    }

    async fn on_request(&self, req: &mut Request<'_>, data: &mut Data<'_>) {
        let params = collect_params(req, data).await;
        req.local_cache(|| RequestMeta {
            started: Instant::now(),
            params,
        });
    }

    async fn on_response<'r>(&self, req: &'r Request<'_>, res: &mut Response<'r>) {
        let meta = req.local_cache(|| RequestMeta {
            started: Instant::now(),
            params: "-".into(),
        });
        let elapsed_ms = meta.started.elapsed().as_secs_f64() * 1000.0;
        // 响应阶段可能换了 Tokio task，按 Request 再绑一次供行首 request_id=
        let rid = request_id::from_request(req);
        request_id::attach_to_current_task(&rid);
        let ip = client_ip(req);
        let method = req.method();
        let path = req.uri().path();
        let status = res.status().code;
        let line = format!(
            "ip={ip} {method} {path} params={} status={status} elapsed_ms={elapsed_ms:.1}",
            meta.params,
        );
        if status >= 400 {
            tracing::warn!(target: "lingxi_backend::access", "{line}");
        } else {
            tracing::info!(target: "lingxi_backend::access", "{line}");
        }
    }
}

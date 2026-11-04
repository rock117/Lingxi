//! 智能助手 API：`docs/assistant.md`

use rocket::response::stream::{Event, EventStream};
use rocket::serde::json::Json;
use rocket::{delete, get, post, State};
use sea_orm::DatabaseConnection;
use tokio::sync::mpsc;

use crate::ai::chat::AssistantStreamEvent;
use crate::ai::{
    delete_session, get_session, list_sessions, run_chat, AiClient, AiStatus, ChatRequest,
    ChatResponse, SessionDetail, SessionOut, ToolContext,
};
use crate::config::AppConfig;
use crate::error::{AppError, AppResult};
use crate::market::eastmoney::EastmoneyClient;
use crate::market::miana::MianaClient;
use crate::market::realtime_cache::RealtimeDayCache;
use crate::market::tencent::TencentClient;

#[get("/api/ai/status")]
pub async fn ai_status(cfg: &State<AppConfig>, ai: &State<AiClient>) -> Json<AiStatus> {
    Json(AiStatus {
        ai_enabled: ai.enabled(),
        model: ai.model_name().to_string(),
        max_tool_rounds: cfg.ai_max_tool_rounds,
        history_limit: cfg.ai_history_limit,
    })
}

#[get("/api/ai/sessions")]
pub async fn sessions_list(
    db: &State<DatabaseConnection>,
) -> AppResult<Json<Vec<SessionOut>>> {
    let list = list_sessions(db.inner())
        .await
        .map_err(AppError::Internal)?;
    Ok(Json(list))
}

#[get("/api/ai/sessions/<id>")]
pub async fn sessions_get(
    db: &State<DatabaseConnection>,
    id: i64,
) -> AppResult<Json<SessionDetail>> {
    let detail = get_session(db.inner(), id)
        .await
        .map_err(AppError::Internal)?
        .ok_or(AppError::NotFound)?;
    Ok(Json(detail))
}

#[delete("/api/ai/sessions/<id>")]
pub async fn sessions_delete(
    db: &State<DatabaseConnection>,
    id: i64,
) -> AppResult<rocket::http::Status> {
    let ok = delete_session(db.inner(), id)
        .await
        .map_err(AppError::Internal)?;
    if !ok {
        return Err(AppError::NotFound);
    }
    Ok(rocket::http::Status::NoContent)
}

#[post("/api/ai/chat", data = "<input>")]
pub async fn chat(
    db: &State<DatabaseConnection>,
    cfg: &State<AppConfig>,
    ai: &State<AiClient>,
    miana: &State<MianaClient>,
    cache: &State<RealtimeDayCache>,
    eastmoney: &State<EastmoneyClient>,
    tencent: &State<TencentClient>,
    input: Json<ChatRequest>,
) -> AppResult<Json<ChatResponse>> {
    if !ai.enabled() {
        return Err(AppError::AiDisabled);
    }
    let tool_ctx = ToolContext {
        db: db.inner(),
        miana: miana.inner(),
        cache: cache.inner(),
        eastmoney: eastmoney.inner(),
        tencent: tencent.inner(),
        ai: ai.inner(),
    };
    let resp = run_chat(
        db.inner(),
        ai.inner(),
        &tool_ctx,
        cfg.ai_max_tool_rounds,
        cfg.ai_history_limit,
        input.into_inner(),
        None,
    )
    .await
    .map_err(map_chat_err)?;
    Ok(Json(resp))
}

#[post("/api/ai/chat/stream", data = "<input>")]
pub fn chat_stream(
    db: &State<DatabaseConnection>,
    cfg: &State<AppConfig>,
    ai: &State<AiClient>,
    miana: &State<MianaClient>,
    cache: &State<RealtimeDayCache>,
    eastmoney: &State<EastmoneyClient>,
    tencent: &State<TencentClient>,
    input: Json<ChatRequest>,
) -> EventStream![] {
    let db = db.inner().clone();
    let ai = ai.inner().clone();
    let miana = miana.inner().clone();
    let cache = cache.inner().clone();
    let eastmoney = eastmoney.inner().clone();
    let tencent = tencent.inner().clone();
    let max_rounds = cfg.ai_max_tool_rounds;
    let history_limit = cfg.ai_history_limit;
    let req = input.into_inner();

    EventStream! {
        if !ai.enabled() {
            let payload = serde_json::json!({
                "message": "AI 未启用：请配置 AI_API_KEY",
                "error": "ai_disabled"
            });
            yield Event::json(&payload).event("error");
            return;
        }

        let (tx, mut rx) = mpsc::channel::<AssistantStreamEvent>(64);
        let worker = tokio::spawn(async move {
            let tool_ctx = ToolContext {
                db: &db,
                miana: &miana,
                cache: &cache,
                eastmoney: &eastmoney,
                tencent: &tencent,
                ai: &ai,
            };
            if let Err(e) = run_chat(
                &db,
                &ai,
                &tool_ctx,
                max_rounds,
                history_limit,
                req,
                Some(tx.clone()),
            )
            .await
            {
                let _ = tx
                    .send(AssistantStreamEvent::Error {
                        message: e.to_string(),
                    })
                    .await;
            }
        });

        while let Some(ev) = rx.recv().await {
            match &ev {
                AssistantStreamEvent::Session { session_id } => {
                    let data = serde_json::json!({ "session_id": session_id });
                    yield Event::json(&data).event("session");
                }
                AssistantStreamEvent::Step(step) => {
                    yield Event::json(step).event("step");
                }
                AssistantStreamEvent::Delta { text } => {
                    let data = serde_json::json!({ "text": text });
                    yield Event::json(&data).event("delta");
                }
                AssistantStreamEvent::Done {
                    session_id,
                    model,
                    steps,
                    degraded,
                    reason,
                    suggestions,
                } => {
                    let data = serde_json::json!({
                        "session_id": session_id,
                        "model": model,
                        "steps": steps,
                        "degraded": degraded,
                        "reason": reason,
                        "suggestions": suggestions,
                    });
                    yield Event::json(&data).event("done");
                }
                AssistantStreamEvent::Error { message } => {
                    let data = serde_json::json!({ "message": message });
                    yield Event::json(&data).event("error");
                }
            }
        }
        let _ = worker.await;
    }
}

fn map_chat_err(e: anyhow::Error) -> AppError {
    let msg = e.to_string();
    if msg.contains("ai_disabled") {
        AppError::AiDisabled
    } else if msg.contains("消息不能为空") {
        AppError::BadRequest(msg)
    } else {
        AppError::Internal(e)
    }
}

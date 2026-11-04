//! Backend AI：个股解释 + 全能助手（见 `docs/ai.md` / `docs/assistant.md`）。

pub(crate) mod chat;
pub(crate) mod client;
pub mod prompts;
pub(crate) mod tools;

#[allow(unused_imports)]
pub use chat::{
    delete_session, get_session, list_sessions, run_chat, AiStatus, AssistantStreamEvent,
    ChatRequest, ChatResponse, MessageOut, SessionDetail, SessionOut,
};
#[allow(unused_imports)]
pub use client::{AiClient, ExplainResult, ScreenDraft};
pub use tools::ToolContext;

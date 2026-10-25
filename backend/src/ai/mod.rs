//! Backend 内部 AI 能力（无 App UI、无对外 `/api/ai/*`）。
//! 规范见 `docs/ai.md`。
//!
//! 供推送/任务等模块按需调用；未接线前允许 dead_code。

#[allow(dead_code)]
pub(crate) mod client;
pub mod prompts;

#[allow(unused_imports)] // 供其它 crate 模块 `use crate::ai::…`
pub use client::{AiClient, ExplainResult, ScreenDraft};

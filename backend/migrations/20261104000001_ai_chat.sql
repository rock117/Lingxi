-- AI 助手：会话与消息

CREATE TABLE IF NOT EXISTS ai_chat_session (
    id         BIGSERIAL PRIMARY KEY,
    title      TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS ai_chat_message (
    id           BIGSERIAL PRIMARY KEY,
    session_id   BIGINT NOT NULL REFERENCES ai_chat_session(id) ON DELETE CASCADE,
    role         TEXT NOT NULL,
    content      TEXT NOT NULL,
    tool_name    TEXT,
    tool_payload JSONB,
    created_at   TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_ai_msg_session ON ai_chat_message (session_id, id);
CREATE INDEX IF NOT EXISTS idx_ai_session_updated ON ai_chat_session (updated_at DESC);

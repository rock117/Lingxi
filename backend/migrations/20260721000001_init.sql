-- PostgreSQL：条件表 + 通知表

CREATE TABLE IF NOT EXISTS condition (
    id          BIGSERIAL PRIMARY KEY,
    name        TEXT NOT NULL,
    kind        TEXT NOT NULL,
    symbol      TEXT,
    expression  TEXT NOT NULL,
    enabled     BOOLEAN NOT NULL DEFAULT TRUE,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS notification (
    id           BIGSERIAL PRIMARY KEY,
    condition_id BIGINT NOT NULL REFERENCES condition(id) ON DELETE CASCADE,
    title        TEXT NOT NULL,
    body         TEXT NOT NULL,
    payload      TEXT,
    "read"       BOOLEAN NOT NULL DEFAULT FALSE,
    created_at   TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_notification_created_at ON notification (created_at DESC);
CREATE INDEX IF NOT EXISTS idx_notification_read ON notification ("read");

-- MSI 计算结果缓存（按交易日；非交易时段取最近一条；重启后从库恢复）
CREATE TABLE IF NOT EXISTS msi_cache (
    date         TEXT PRIMARY KEY,
    payload      JSONB NOT NULL,
    updated_at   TEXT NOT NULL
);

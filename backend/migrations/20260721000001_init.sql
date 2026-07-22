-- 初始 schema：条件表 + 通知表

CREATE TABLE IF NOT EXISTS condition (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT NOT NULL,
    kind        TEXT NOT NULL,                    -- 'trade' | 'news'
    symbol      TEXT,                             -- 股票代码（news 可空）
    expression  TEXT NOT NULL,                    -- JSON 配置
    enabled     BOOLEAN NOT NULL DEFAULT TRUE,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS notification (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    condition_id INTEGER NOT NULL,
    title        TEXT NOT NULL,
    body         TEXT NOT NULL,
    payload      TEXT,
    read         BOOLEAN NOT NULL DEFAULT FALSE,
    created_at   TEXT NOT NULL,
    FOREIGN KEY (condition_id) REFERENCES condition(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_notification_created_at ON notification(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_notification_read ON notification(read);

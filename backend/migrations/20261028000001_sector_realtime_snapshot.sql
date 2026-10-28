-- 板块盘中实时截面：每板块每交易日只保留最新一条（UPSERT）
CREATE TABLE IF NOT EXISTS sector_realtime_snapshot (
    symbol            TEXT NOT NULL,
    trade_date        DATE NOT NULL,
    name              TEXT,
    sector_type       TEXT,
    price             DOUBLE PRECISION NOT NULL,
    open              DOUBLE PRECISION,
    high              DOUBLE PRECISION,
    low               DOUBLE PRECISION,
    pre_close         DOUBLE PRECISION,
    volume            DOUBLE PRECISION,
    amount            DOUBLE PRECISION,
    change_amt        DOUBLE PRECISION,
    change_rate       DOUBLE PRECISION,
    quote_ts          BIGINT,
    local_date        TEXT,
    fetched_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (symbol, trade_date)
);

CREATE INDEX IF NOT EXISTS idx_sector_realtime_snapshot_trade_date
    ON sector_realtime_snapshot (trade_date);

-- 盘中实时行情截面（一天多条；主键 symbol + 行情时间戳）
CREATE TABLE IF NOT EXISTS market_realtime_snapshot (
    symbol            TEXT NOT NULL,
    quote_ts          BIGINT NOT NULL,
    local_date        TEXT NOT NULL,
    trade_date        TEXT NOT NULL,
    fetched_at        TEXT NOT NULL,
    name              TEXT,
    trade_status      TEXT,
    price             DOUBLE PRECISION NOT NULL,
    open              DOUBLE PRECISION,
    high              DOUBLE PRECISION,
    low               DOUBLE PRECISION,
    pre_close         DOUBLE PRECISION,
    volume            DOUBLE PRECISION,
    amount            DOUBLE PRECISION,
    change_amt        DOUBLE PRECISION,
    change_rate       DOUBLE PRECISION,
    high_limit        DOUBLE PRECISION,
    low_limit         DOUBLE PRECISION,
    turnover          DOUBLE PRECISION,
    vol_ratio         DOUBLE PRECISION,
    amplitude         DOUBLE PRECISION,
    active_buy_vol    DOUBLE PRECISION,
    active_sell_vol   DOUBLE PRECISION,
    net_inflow        DOUBLE PRECISION,
    mkt_cap           DOUBLE PRECISION,
    float_cap         DOUBLE PRECISION,
    PRIMARY KEY (symbol, quote_ts)
);

CREATE INDEX IF NOT EXISTS idx_market_realtime_snapshot_trade_date
    ON market_realtime_snapshot (trade_date);

CREATE INDEX IF NOT EXISTS idx_market_realtime_snapshot_trade_ts
    ON market_realtime_snapshot (trade_date, quote_ts);

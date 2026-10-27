//! 市场指标模块
//!
//! - MSI 短线情绪：`docs/market_sentiment.md`
//! - MTT 中期趋势：`docs/mid_term_trend.md`
//! - 日内 TopN（趋势/超跌）推送：`docs/intraday_alert.md`
//! - 条件选股：`screen`（均线金叉 / RSI / 放量）
//!
//! 算法变更须先改文档再改代码。

pub mod calculator;
pub mod indicators;
pub mod data_source;
pub mod domain;
pub mod eastmoney;
pub mod tencent;
pub mod fund_stats;
pub mod fundamentals;
pub mod stock_analyze;
pub mod intraday_alert;
pub mod intraday_push;
pub mod mid_term;
pub mod miana;
pub mod result_cache;
pub mod rate_limit;
pub mod realtime_cache;
pub mod realtime_store;
pub mod routes;
pub mod screen;
pub mod session;

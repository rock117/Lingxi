//! 市场情绪指标（MSI）模块 —— 三层架构
//!
//! **规范（Source of Truth）：** `docs/market_sentiment.md`
//!
//! - Direction 50%：宽度 + 资金 + 开盘缺口
//! - Intensity 25%：有方向量能 + 涨跌停压力
//! - Constraint 25%：真实高低点位置 + 日内收盘位置
//!
//! 算法变更须先改文档再改代码。

pub mod calculator;
pub mod domain;
pub mod mock_data;
pub mod routes;

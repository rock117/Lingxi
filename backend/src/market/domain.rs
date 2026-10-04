//! 市场情绪指标（Market Sentiment Index, MSI）领域对象
//!
//! 定义 MSI 计算所需的输入数据结构和输出结果结构。
//!
//! ## 设计说明
//!
//! 仅分析**短期**（N 天），基于整个 N 天的数据综合判断，包含 4 个子分：
//!
//! | 子分 | 权重 | 含义 |
//! |---|---|---|
//! | 涨跌宽度分 | 20% | N 天涨跌家数比，反映市场参与广度 |
//! | 资金流向分 | 30% | 上涨股票成交额 vs 下跌股票成交额，反映资金方向 |
//! | 成交量水平分 | 25% | 当前成交量 vs N 天均量，反映放量/缩量（**核心**） |
//! | 价格位置分 | 25% | 当前股价在 N 天高低点区间的相对位置 |
//!
//! 所有计算均采用**等权**（简单平均/计数），不按市值或成交额加权。

use serde::{Deserialize, Serialize};

// ============================================================
// 输入数据
// ============================================================

/// 单只股票某一天的行情数据（MSI 计算的最小输入单元）
///
/// - `change_pct`：涨跌幅（百分比，如 +3.5 表示涨 3.5%，-2.0 表示跌 2.0%）
/// - `amount`：成交额（元），用于资金流向分和成交量水平分
/// - `close_price`：收盘价（元），用于价格位置分（判断当前股价在 N 天高低点中的位置）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockDailyData {
    /// 股票代码，如 "600519"
    pub symbol: String,
    /// 日期，ISO 8601 格式，如 "2026-09-01"
    pub date: String,
    /// 当日涨跌幅（百分比）
    pub change_pct: f64,
    /// 当日成交额（元）
    pub amount: f64,
    /// 当日收盘价（元）
    pub close_price: f64,
}

/// 市场快照：某一天全市场所有股票的数据
///
/// 代表"第 d 天"的市场横截面数据。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketDailySnapshot {
    /// 日期
    pub date: String,
    /// 当天所有股票的数据
    pub stocks: Vec<StockDailyData>,
}

/// MSI 计算的完整输入：过去 N 天的市场快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketDataInput {
    /// 过去 N 天的市场快照（按时间正序，最后一天为最近）
    pub snapshots: Vec<MarketDailySnapshot>,
}

// ============================================================
// 子分
// ============================================================

/// MSI 的四个子分（均映射到 [0, 100]，50 为中性）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MsiSubScores {
    /// 涨跌宽度分 [0, 100]：N 天平均涨跌家数比
    pub breadth: f64,
    /// 资金流向分 [0, 100]：N 天平均资金流向（钱追涨还是杀跌）
    pub money_flow: f64,
    /// 成交量水平分 [0, 100]：当前成交量 vs N 天均量（放量/缩量）
    pub volume_level: f64,
    /// 价格位置分 [0, 100]：当前股价在 N 天高低点区间的相对位置
    pub price_position: f64,
}

// ============================================================
// 成交量状态
// ============================================================

/// 成交量状态分类
///
/// 基于当前成交量 vs N 天平均成交量的比值（volume_ratio）判断：
///
/// | 比值范围 | 状态 | 含义 |
/// |---|---|---|
/// | > 2.0 | 天量 | 极端放量，可能见顶/见底 |
/// | 1.5 ~ 2.0 | 放量 | 资金积极参与 |
/// | 0.5 ~ 1.5 | 正常 | 成交量平稳 |
/// | < 0.5 | 缩量 | 交投清淡 |
/// | < 0.3 | 地量 | 极端缩量，可能见底 |
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VolumeStatus {
    /// 天量（> 2.0 倍均量）
    ExtremeHigh,
    /// 放量（1.5 ~ 2.0 倍均量）
    High,
    /// 正常（0.5 ~ 1.5 倍均量）
    Normal,
    /// 缩量（0.3 ~ 0.5 倍均量）
    Low,
    /// 地量（< 0.3 倍均量）
    ExtremeLow,
}

impl VolumeStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::ExtremeHigh => "天量",
            Self::High => "放量",
            Self::Normal => "正常",
            Self::Low => "缩量",
            Self::ExtremeLow => "地量",
        }
    }

    /// 根据成交量比值分类
    pub fn from_ratio(ratio: f64) -> Self {
        if ratio >= 2.0 {
            Self::ExtremeHigh
        } else if ratio >= 1.5 {
            Self::High
        } else if ratio >= 0.5 {
            Self::Normal
        } else if ratio >= 0.3 {
            Self::Low
        } else {
            Self::ExtremeLow
        }
    }
}

// ============================================================
// 价格位置状态
// ============================================================

/// 价格位置状态分类
///
/// 基于当前股价在 N 天高低点区间的相对位置（position, [0, 1]）判断：
///
/// | 位置范围 | 状态 | 含义 |
/// |---|---|---|
/// | > 0.8 | 高位区 | 接近 N 天高点，面临阻力 |
/// | 0.6 ~ 0.8 | 偏高区 | 中上部，趋势偏强 |
/// | 0.4 ~ 0.6 | 中位区 | 中间位置，方向不明 |
/// | 0.2 ~ 0.4 | 偏低区 | 中下部，趋势偏弱 |
/// | < 0.2 | 低位区 | 接近 N 天低点，可能有支撑 |
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PriceStatus {
    /// 高位区（> 0.8）
    HighZone,
    /// 偏高区（0.6 ~ 0.8）
    UpperMid,
    /// 中位区（0.4 ~ 0.6）
    MidZone,
    /// 偏低区（0.2 ~ 0.4）
    LowerMid,
    /// 低位区（< 0.2）
    LowZone,
}

impl PriceStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::HighZone => "高位区",
            Self::UpperMid => "偏高区",
            Self::MidZone => "中位区",
            Self::LowerMid => "偏低区",
            Self::LowZone => "低位区",
        }
    }

    /// 根据价格位置 [0, 1] 分类
    pub fn from_position(pos: f64) -> Self {
        if pos >= 0.8 {
            Self::HighZone
        } else if pos >= 0.6 {
            Self::UpperMid
        } else if pos >= 0.4 {
            Self::MidZone
        } else if pos >= 0.2 {
            Self::LowerMid
        } else {
            Self::LowZone
        }
    }
}

// ============================================================
// 组合信号
// ============================================================

/// 信号类型
///
/// 基于 S-MSI 值 + 成交量状态 + 价格位置三维组合判断：
///
/// | S-MSI | 成交量 | 价格位置 | 信号 |
/// |---|---|---|---|
/// | 高 | 放量 | 中位 | 强势上涨，顺势做多 |
/// | 高 | 放量 | 高位 | 放量冲高，警惕见顶 |
/// | 高 | 缩量 | 高位 | 量价背离，警惕回调 |
/// | 高 | 缩量 | 中位 | 缩量上涨，动能不足 |
/// | 中 | 放量 | 低位 | 低位放量，关注反弹 |
/// | 中 | 正常 | 中位 | 中性观望 |
/// | 低 | 放量 | 低位 | 恐慌抛售，关注反弹 |
/// | 低 | 缩量 | 低位 | 地量地价，关注底部 |
/// | 低 | 放量 | 中/高位 | 放量下跌，风险大 |
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalType {
    /// 强势上涨：S-MSI 高 + 放量 + 价格中位
    StrongBullish,
    /// 放量冲高：S-MSI 高 + 放量 + 价格高位
    SurgeWarning,
    /// 量价背离：S-MSI 高 + 缩量 + 价格高位
    DivergenceWarning,
    /// 缩量上涨：S-MSI 高 + 缩量 + 价格中位
    WeakBullish,
    /// 低位放量：S-MSI 中 + 放量 + 价格低位
    LowVolumeAccumulation,
    /// 中性观望
    Neutral,
    /// 恐慌抛售：S-MSI 低 + 放量 + 价格低位
    PanicSelling,
    /// 地量地价：S-MSI 低 + 缩量 + 价格低位
    Bottoming,
    /// 放量下跌：S-MSI 低 + 放量 + 价格中/高位
    VolumeDecline,
}

impl SignalType {
    /// 人类可读的中文标签
    pub fn label(&self) -> &'static str {
        match self {
            Self::StrongBullish => "强势上涨",
            Self::SurgeWarning => "放量冲高",
            Self::DivergenceWarning => "量价背离",
            Self::WeakBullish => "缩量上涨",
            Self::LowVolumeAccumulation => "低位放量",
            Self::Neutral => "中性观望",
            Self::PanicSelling => "恐慌抛售",
            Self::Bottoming => "地量地价",
            Self::VolumeDecline => "放量下跌",
        }
    }

    /// 操作建议
    pub fn advice(&self) -> &'static str {
        match self {
            Self::StrongBullish => "顺势做多",
            Self::SurgeWarning => "警惕见顶，减仓",
            Self::DivergenceWarning => "量价背离，警惕回调",
            Self::WeakBullish => "动能不足，谨慎追高",
            Self::LowVolumeAccumulation => "关注反弹信号",
            Self::Neutral => "观望",
            Self::PanicSelling => "关注超跌反弹",
            Self::Bottoming => "关注底部机会",
            Self::VolumeDecline => "风险大，观望",
        }
    }
}

/// MSI 组合信号结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MsiSignal {
    /// 信号类型
    pub signal_type: SignalType,
    /// 信号中文标签
    pub label: String,
    /// 操作建议
    pub advice: String,
    /// 成交量状态
    pub volume_status: VolumeStatus,
    /// 成交量比值（当前 / N 天均量）
    pub volume_ratio: f64,
    /// 价格位置状态
    pub price_status: PriceStatus,
    /// 价格位置 [0, 1]，0=最低点，1=最高点
    pub price_position: f64,
}

// ============================================================
// 最终输出
// ============================================================

/// 市场情绪指数的完整计算结果
///
/// 这是 `/api/market/sentiment` 接口的返回结构。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MsiResult {
    /// MSI 综合值 [0, 100]，50 为中性
    pub value: f64,
    /// 四个子分
    pub sub_scores: MsiSubScores,
    /// 使用的天数 N
    pub days: usize,
    /// 组合信号
    pub signal: MsiSignal,
    /// 计算时间（ISO 8601）
    pub calculated_at: String,
}

//! MSI 领域对象
//!
//! 规范：`docs/market_sentiment.md`（三层架构 Direction / Intensity / Constraint）。

use serde::{Deserialize, Serialize};

// ============================================================
// 输入
// ============================================================

/// 单只股票某一日行情（OHLC + 成交额 + 涨跌幅）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockDailyData {
    pub symbol: String,
    pub date: String,
    /// 开盘价
    pub open: f64,
    /// 最高价
    pub high: f64,
    /// 最低价
    pub low: f64,
    /// 收盘价
    pub close_price: f64,
    /// 成交额（元）
    pub amount: f64,
    /// 涨跌幅（%），相对昨收
    pub change_pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketDailySnapshot {
    pub date: String,
    pub stocks: Vec<StockDailyData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketDataInput {
    /// 时间正序，末日为最近
    pub snapshots: Vec<MarketDailySnapshot>,
}

// ============================================================
// 三层分与分量
// ============================================================

/// 三层得分，均 ∈ [0, 100]，50 中性
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MsiLayers {
    pub direction: f64,
    pub intensity: f64,
    pub constraint: f64,
}

/// 各分量明细（均为 0～100 分，除非另有说明）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MsiComponents {
    pub breadth: f64,
    pub money_flow: f64,
    pub gap_breadth: f64,
    pub directed_volume: f64,
    /// 成交量比值（末日总额 / N 日均量），非 0～100
    pub volume_ratio: f64,
    pub limit_pressure: f64,
    /// N 日真实区间位置分 0～100
    pub range_position: f64,
    /// 末日日内收盘位置分 0～100
    pub intraday_position: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MsiDiagnostics {
    pub sample_size: usize,
    pub up_count: usize,
    pub down_count: usize,
    pub flat_count: usize,
    pub limit_up_count: usize,
    pub limit_down_count: usize,
    pub breadth_last: f64,
    pub money_flow_last: f64,
    pub gap_breadth_last: f64,
}

// ============================================================
// 状态 / 信号
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VolumeStatus {
    ExtremeHigh,
    High,
    Normal,
    Low,
    ExtremeLow,
}

impl VolumeStatus {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PriceStatus {
    HighZone,
    UpperMid,
    MidZone,
    LowerMid,
    LowZone,
}

impl PriceStatus {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalType {
    StrongBullish,
    SurgeWarning,
    DivergenceWarning,
    WeakBullish,
    LowVolumeAccumulation,
    Neutral,
    PanicSelling,
    Bottoming,
    VolumeDecline,
}

impl SignalType {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MsiSignal {
    pub signal_type: SignalType,
    pub label: String,
    pub advice: String,
    pub volume_status: VolumeStatus,
    pub volume_ratio: f64,
    pub price_status: PriceStatus,
    /// 真实区间位置 raw ∈ [0, 1]
    pub price_position: f64,
}

// ============================================================
// 输出
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MsiResult {
    /// MSI 综合值 [0, 100]
    pub value: f64,
    pub layers: MsiLayers,
    pub components: MsiComponents,
    pub days: usize,
    pub signal: MsiSignal,
    pub diagnostics: MsiDiagnostics,
    pub calculated_at: String,
}

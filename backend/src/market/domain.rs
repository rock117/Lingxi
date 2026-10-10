//! MSI 领域对象
//!
//! 规范：`docs/market_sentiment.md`（Direction / Intensity / Constraint）。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockDailyData {
    pub symbol: String,
    pub date: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close_price: f64,
    pub amount: f64,
    pub change_pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketDailySnapshot {
    pub date: String,
    pub stocks: Vec<StockDailyData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketDataInput {
    pub snapshots: Vec<MarketDailySnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MsiLayers {
    pub direction: f64,
    pub intensity: f64,
    pub constraint: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MsiComponents {
    pub breadth: f64,
    pub money_flow: f64,
    pub gap_breadth: f64,
    /// 结构确认（家数方向 vs 等权均涨跌幅，见规范 §5.4）
    pub structure: f64,
    /// 新高/新低家数比
    pub nh_nl: f64,
    pub directed_volume: f64,
    pub volume_ratio: f64,
    pub limit_pressure: f64,
    pub range_position: f64,
    pub intraday_position: f64,
    /// 连续涨跌 / 未涨跌惯性
    pub inertia: f64,
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
    pub structure_last: f64,
    pub nh_nl_last: f64,
    pub new_high_count: usize,
    pub new_low_count: usize,
    pub avg_up_streak: f64,
    pub avg_down_streak: f64,
    pub avg_no_up_streak: f64,
    pub avg_no_down_streak: f64,
}

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
    pub price_position: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MsiResult {
    pub value: f64,
    pub layers: MsiLayers,
    pub components: MsiComponents,
    pub days: usize,
    pub signal: MsiSignal,
    pub diagnostics: MsiDiagnostics,
    pub calculated_at: String,
}

// ---------------------------------------------------------------------------
// 中期趋势 MTT（规范 `docs/mid_term_trend.md`）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MttStatus {
    MidBullConfirmed,
    MidBullGrey,
    MidNeutral,
    MidBearGrey,
    MidBearConfirmed,
}

impl MttStatus {
    pub fn from_score(score: f64) -> Self {
        const MTT_HIGH: f64 = 65.0;
        const MTT_LOW: f64 = 35.0;
        const MTT_GREY_BULL_LO: f64 = 60.0;
        const MTT_GREY_BULL_HI: f64 = 65.0;
        const MTT_GREY_BEAR_LO: f64 = 35.0;
        const MTT_GREY_BEAR_HI: f64 = 40.0;
        if score > MTT_HIGH {
            Self::MidBullConfirmed
        } else if (MTT_GREY_BULL_LO..=MTT_GREY_BULL_HI).contains(&score) {
            Self::MidBullGrey
        } else if score < MTT_LOW {
            Self::MidBearConfirmed
        } else if (MTT_GREY_BEAR_LO..=MTT_GREY_BEAR_HI).contains(&score) {
            Self::MidBearGrey
        } else {
            Self::MidNeutral
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::MidBullConfirmed => "中期偏多",
            Self::MidBullGrey => "偏多灰区",
            Self::MidNeutral => "中性",
            Self::MidBearGrey => "偏空灰区",
            Self::MidBearConfirmed => "中期偏空",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MttCounts {
    pub bull: usize,
    pub bear: usize,
    pub neutral: usize,
    pub classified: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MttDiagnostics {
    pub deviation_median: f64,
    pub nh_nl: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MttResult {
    pub value: f64,
    pub status: MttStatus,
    pub trend_raw: f64,
    pub ma_type: String,
    pub ma_mid: usize,
    pub ma_long: usize,
    pub use_slope_filter: bool,
    pub counts: MttCounts,
    pub diagnostics: MttDiagnostics,
    pub calculated_at: String,
}

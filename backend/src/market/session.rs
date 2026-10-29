//! 盘中连续竞价时钟窗（上海时区）。
//! 是否为交易日由 `sc_trade_calendar.is_open` 判定，不在此用周一至周五硬编码。
//! 时段相位见 `docs/intraday_alert.md` §3.1。

use chrono::{FixedOffset, Timelike, Utc};

/// 连续竞价内的盘中相位（影响 TopN 层权重 / 门槛）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionPhase {
    /// 09:30–10:00 开盘信息释放 / 高噪声
    Open,
    /// 10:00–10:30 早盘方向观察
    AmActive,
    /// 10:30–11:30 上午相对平稳
    AmMid,
    /// 13:00–13:30 午后重开
    Lunch,
    /// 13:30–14:00 午后主攻
    PmDrive,
    /// 14:00–14:30 变盘高发
    PmTurn,
    /// 14:30–15:00 尾盘 / 收盘效应
    Close,
    /// 非连续竞价时钟
    Off,
}

/// 相位下的合成权重（三项和为 1）与最低分上浮。
#[derive(Debug, Clone, Copy)]
pub struct PhaseWeights {
    pub w_a: f64,
    pub w_b: f64,
    pub w_c: f64,
    pub score_floor_boost: f64,
}

impl SessionPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::AmActive => "am_active",
            Self::AmMid => "am_mid",
            Self::Lunch => "lunch",
            Self::PmDrive => "pm_drive",
            Self::PmTurn => "pm_turn",
            Self::Close => "close",
            Self::Off => "off",
        }
    }

    /// 上海当日分钟数（0–1439）→ 相位。
    pub fn from_shanghai_minutes(minutes: i32) -> Self {
        const M930: i32 = 9 * 60 + 30;
        const M1000: i32 = 10 * 60;
        const M1030: i32 = 10 * 60 + 30;
        const M1130: i32 = 11 * 60 + 30;
        const M1300: i32 = 13 * 60;
        const M1330: i32 = 13 * 60 + 30;
        const M1400: i32 = 14 * 60;
        const M1430: i32 = 14 * 60 + 30;
        const M1500: i32 = 15 * 60;
        if (M930..M1000).contains(&minutes) {
            Self::Open
        } else if (M1000..M1030).contains(&minutes) {
            Self::AmActive
        } else if (M1030..=M1130).contains(&minutes) {
            Self::AmMid
        } else if (M1300..M1330).contains(&minutes) {
            Self::Lunch
        } else if (M1330..M1400).contains(&minutes) {
            Self::PmDrive
        } else if (M1400..M1430).contains(&minutes) {
            Self::PmTurn
        } else if (M1430..=M1500).contains(&minutes) {
            Self::Close
        } else {
            Self::Off
        }
    }

    pub fn now() -> Self {
        let sh = Utc::now().with_timezone(&shanghai_offset());
        let minutes = sh.hour() as i32 * 60 + sh.minute() as i32;
        Self::from_shanghai_minutes(minutes)
    }

    /// 上涨趋势：开盘/变盘/尾盘压纯 RS、抬量价确认。
    pub fn trend_weights(self) -> PhaseWeights {
        match self {
            Self::Open => PhaseWeights {
                w_a: 0.25,
                w_b: 0.30,
                w_c: 0.45,
                score_floor_boost: 3.0,
            },
            Self::AmActive => PhaseWeights {
                w_a: 0.32,
                w_b: 0.30,
                w_c: 0.38,
                score_floor_boost: 0.0,
            },
            Self::AmMid | Self::Off => PhaseWeights {
                w_a: 0.35,
                w_b: 0.30,
                w_c: 0.35,
                score_floor_boost: 0.0,
            },
            Self::Lunch => PhaseWeights {
                w_a: 0.28,
                w_b: 0.30,
                w_c: 0.42,
                score_floor_boost: 2.0,
            },
            Self::PmDrive => PhaseWeights {
                w_a: 0.35,
                w_b: 0.28,
                w_c: 0.37,
                score_floor_boost: 0.0,
            },
            Self::PmTurn => PhaseWeights {
                w_a: 0.25,
                w_b: 0.28,
                w_c: 0.47,
                score_floor_boost: 3.0,
            },
            Self::Close => PhaseWeights {
                w_a: 0.22,
                w_b: 0.28,
                w_c: 0.50,
                score_floor_boost: 5.0,
            },
        }
    }

    /// 超跌：开盘/变盘/尾盘更重企稳，压「深回撤」。
    pub fn oversold_weights(self) -> PhaseWeights {
        match self {
            Self::Open => PhaseWeights {
                w_a: 0.15,
                w_b: 0.55,
                w_c: 0.30,
                score_floor_boost: 2.0,
            },
            Self::AmActive => PhaseWeights {
                w_a: 0.20,
                w_b: 0.50,
                w_c: 0.30,
                score_floor_boost: 0.0,
            },
            Self::AmMid | Self::Off => PhaseWeights {
                w_a: 0.25,
                w_b: 0.45,
                w_c: 0.30,
                score_floor_boost: 0.0,
            },
            Self::Lunch => PhaseWeights {
                w_a: 0.18,
                w_b: 0.52,
                w_c: 0.30,
                score_floor_boost: 2.0,
            },
            Self::PmDrive => PhaseWeights {
                w_a: 0.22,
                w_b: 0.48,
                w_c: 0.30,
                score_floor_boost: 0.0,
            },
            Self::PmTurn => PhaseWeights {
                w_a: 0.15,
                w_b: 0.55,
                w_c: 0.30,
                score_floor_boost: 3.0,
            },
            Self::Close => PhaseWeights {
                w_a: 0.12,
                w_b: 0.58,
                w_c: 0.30,
                score_floor_boost: 4.0,
            },
        }
    }
}

/// 上海 UTC+8（无夏令时）。
pub fn shanghai_offset() -> FixedOffset {
    FixedOffset::east_opt(8 * 3600).expect("UTC+8")
}

/// 连续竞价全日分钟数：09:30–11:30 + 13:00–15:00 = 240。
pub const CONTINUOUS_AUCTION_MINUTES: i32 = 240;

/// 上海当日分钟数 → 已走过的连续竞价分钟（午休按上午收满计；收盘后 = 240）。
pub fn continuous_auction_elapsed_minutes(minutes_of_day: i32) -> i32 {
    const M930: i32 = 9 * 60 + 30;
    const M1130: i32 = 11 * 60 + 30;
    const M1300: i32 = 13 * 60;
    const M1500: i32 = 15 * 60;
    const AM: i32 = 120;
    if minutes_of_day < M930 {
        0
    } else if minutes_of_day <= M1130 {
        minutes_of_day - M930
    } else if minutes_of_day < M1300 {
        AM
    } else if minutes_of_day <= M1500 {
        AM + (minutes_of_day - M1300)
    } else {
        CONTINUOUS_AUCTION_MINUTES
    }
}

/// 盘中成交额外推至少已交易这么多分钟；未满则不计当日额（返回 0）。
pub const AUCTION_AMOUNT_MIN_ELAPSED: i32 = 10;

/// 将盘中累计成交额外推到「全日等量」：`实际额 × (240 / 已交易分钟)`。
/// - **09:30–15:00**（含午休）且已交易不足 10 分钟：返回 **0**（不计当日；开盘当分钟 elapsed=0 也走此支）
/// - 盘前 / 15:00 之后：返回 **1**（日 K 全日额，不外推）
/// - 已交易 ≥10 且未收盘：返回 `240 / t`
///
/// 例：已交易 60 分钟、累计额 x → 外推额 = x × (240/60) = 4x。
pub fn auction_amount_extrapolation_factor(minutes_of_day: i32) -> f64 {
    const M930: i32 = 9 * 60 + 30;
    const M1500: i32 = 15 * 60;
    // 盘前或收盘后：用全日额，不外推
    if minutes_of_day < M930 || minutes_of_day > M1500 {
        return 1.0;
    }
    let elapsed = continuous_auction_elapsed_minutes(minutes_of_day);
    if elapsed < AUCTION_AMOUNT_MIN_ELAPSED {
        0.0
    } else if elapsed >= CONTINUOUS_AUCTION_MINUTES {
        1.0
    } else {
        CONTINUOUS_AUCTION_MINUTES as f64 / elapsed as f64
    }
}

pub fn auction_amount_extrapolation_factor_now() -> f64 {
    let sh = Utc::now().with_timezone(&shanghai_offset());
    let minutes = sh.hour() as i32 * 60 + sh.minute() as i32;
    auction_amount_extrapolation_factor(minutes)
}

/// 当前是否处于连续竞价时钟窗（不论是否节假日）。
///
/// 交易日与否见 `data_source::is_open_trade_day`（`sc_trade_calendar.is_open`）。
pub fn is_realtime_session_now() -> bool {
    is_auction_clock_at(Utc::now())
}

pub fn is_auction_clock_at(utc: chrono::DateTime<Utc>) -> bool {
    let sh = utc.with_timezone(&shanghai_offset());
    let minutes = sh.hour() as i32 * 60 + sh.minute() as i32;
    const AM_LO: i32 = 9 * 60 + 30;
    const AM_HI: i32 = 11 * 60 + 30;
    const PM_LO: i32 = 13 * 60;
    const PM_HI: i32 = 15 * 60;
    (minutes >= AM_LO && minutes <= AM_HI) || (minutes >= PM_LO && minutes <= PM_HI)
}

/// 兼容旧名。
#[allow(dead_code)]
pub fn is_realtime_session_at(utc: chrono::DateTime<Utc>) -> bool {
    is_auction_clock_at(utc)
}

/// 上海日历日 `yyyy-MM-dd`。
pub fn shanghai_today() -> String {
    shanghai_today_at(Utc::now())
}

pub fn shanghai_today_at(utc: chrono::DateTime<Utc>) -> String {
    utc.with_timezone(&shanghai_offset())
        .date_naive()
        .format("%Y-%m-%d")
        .to_string()
}

pub fn shanghai_today_naive() -> chrono::NaiveDate {
    Utc::now()
        .with_timezone(&shanghai_offset())
        .date_naive()
}

/// 当前上海时间 RFC3339（带 `+08:00`，项目统一用此时区落库/推送）。
pub fn shanghai_now_rfc3339() -> String {
    Utc::now()
        .with_timezone(&shanghai_offset())
        .to_rfc3339()
}

/// 日内强势推送时钟窗（竞价内略收紧）。
/// 联调阶段推送暂不限制时段；保留供恢复时段门闩时使用。
#[allow(dead_code)]
pub fn is_intraday_push_session_now() -> bool {
    is_intraday_push_session_at(Utc::now())
}

pub fn is_intraday_push_session_at(utc: chrono::DateTime<Utc>) -> bool {
    if !is_auction_clock_at(utc) {
        return false;
    }
    let sh = utc.with_timezone(&shanghai_offset());
    let minutes = sh.hour() as i32 * 60 + sh.minute() as i32;
    const AM_LO: i32 = 9 * 60 + 35;
    const AM_HI: i32 = 11 * 60 + 30;
    const PM_LO: i32 = 13 * 60 + 5;
    const PM_HI: i32 = 14 * 60 + 55;
    (minutes >= AM_LO && minutes <= AM_HI) || (minutes >= PM_LO && minutes <= PM_HI)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn auction_elapsed_and_amount_scale() {
        // 开盘当分钟 elapsed=0，仍须不计当日（勿当成盘前系数 1）
        assert_eq!(continuous_auction_elapsed_minutes(9 * 60 + 30), 0);
        assert!((auction_amount_extrapolation_factor(9 * 60 + 30) - 0.0).abs() < 1e-9);
        // 开盘未满 10 分钟：不计当日
        assert_eq!(continuous_auction_elapsed_minutes(9 * 60 + 35), 5);
        assert!((auction_amount_extrapolation_factor(9 * 60 + 35) - 0.0).abs() < 1e-9);
        // 满 10 分钟：×24
        assert_eq!(continuous_auction_elapsed_minutes(9 * 60 + 40), 10);
        assert!((auction_amount_extrapolation_factor(9 * 60 + 40) - 24.0).abs() < 1e-9);
        // 10:30 = 开盘后 60 分钟 → 外推 ×4
        assert_eq!(continuous_auction_elapsed_minutes(10 * 60 + 30), 60);
        assert!((auction_amount_extrapolation_factor(10 * 60 + 30) - 4.0).abs() < 1e-9);
        // 午休：上午满 120 → ×2
        assert_eq!(continuous_auction_elapsed_minutes(12 * 60), 120);
        assert!((auction_amount_extrapolation_factor(12 * 60) - 2.0).abs() < 1e-9);
        // 15:00 整点：全日满，不外推
        assert_eq!(continuous_auction_elapsed_minutes(15 * 60), CONTINUOUS_AUCTION_MINUTES);
        assert!((auction_amount_extrapolation_factor(15 * 60) - 1.0).abs() < 1e-9);
        // 收盘后不外推
        assert_eq!(continuous_auction_elapsed_minutes(16 * 60), CONTINUOUS_AUCTION_MINUTES);
        assert!((auction_amount_extrapolation_factor(16 * 60) - 1.0).abs() < 1e-9);
        // 盘前不外推
        assert_eq!(continuous_auction_elapsed_minutes(9 * 60), 0);
        assert!((auction_amount_extrapolation_factor(9 * 60) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn inside_am_auction() {
        // 2026-10-09 周五 10:00 CST = 02:00 UTC
        let t = Utc.with_ymd_and_hms(2026, 10, 9, 2, 0, 0).unwrap();
        assert!(is_auction_clock_at(t));
    }

    #[test]
    fn before_open() {
        let t = Utc.with_ymd_and_hms(2026, 10, 9, 1, 29, 0).unwrap();
        assert!(!is_auction_clock_at(t));
    }

    #[test]
    fn lunch_off() {
        let t = Utc.with_ymd_and_hms(2026, 10, 9, 4, 0, 0).unwrap();
        assert!(!is_auction_clock_at(t));
        assert!(!is_intraday_push_session_at(t));
    }

    #[test]
    fn after_close_no_auction() {
        let t = Utc.with_ymd_and_hms(2026, 10, 9, 7, 1, 0).unwrap();
        assert!(!is_auction_clock_at(t));
        let t2 = Utc.with_ymd_and_hms(2026, 10, 9, 10, 0, 0).unwrap();
        assert!(!is_auction_clock_at(t2));
    }

    #[test]
    fn at_close_boundary() {
        let t = Utc.with_ymd_and_hms(2026, 10, 9, 7, 0, 0).unwrap();
        assert!(is_auction_clock_at(t));
    }

    #[test]
    fn clock_does_not_exclude_weekend() {
        // 是否开市由 sc_trade_calendar.is_open 决定
        let t = Utc.with_ymd_and_hms(2026, 10, 10, 2, 0, 0).unwrap(); // 周六 10:00
        assert!(is_auction_clock_at(t));
    }

    #[test]
    fn push_window_am() {
        let t = Utc.with_ymd_and_hms(2026, 10, 9, 2, 0, 0).unwrap();
        assert!(is_intraday_push_session_at(t));
    }

    #[test]
    fn push_not_in_first_5min() {
        let t = Utc.with_ymd_and_hms(2026, 10, 9, 1, 32, 0).unwrap();
        assert!(is_auction_clock_at(t));
        assert!(!is_intraday_push_session_at(t));
    }

    #[test]
    fn shanghai_now_has_plus08() {
        let s = shanghai_now_rfc3339();
        assert!(
            s.contains("+08:00") || s.ends_with("+0800"),
            "expected Shanghai offset, got {s}"
        );
    }

    #[test]
    fn session_phases_cover_auction() {
        assert_eq!(SessionPhase::from_shanghai_minutes(9 * 60 + 35), SessionPhase::Open);
        assert_eq!(SessionPhase::from_shanghai_minutes(10 * 60 + 10), SessionPhase::AmActive);
        assert_eq!(SessionPhase::from_shanghai_minutes(11 * 60), SessionPhase::AmMid);
        assert_eq!(SessionPhase::from_shanghai_minutes(13 * 60 + 10), SessionPhase::Lunch);
        assert_eq!(SessionPhase::from_shanghai_minutes(13 * 60 + 45), SessionPhase::PmDrive);
        assert_eq!(SessionPhase::from_shanghai_minutes(14 * 60 + 10), SessionPhase::PmTurn);
        assert_eq!(SessionPhase::from_shanghai_minutes(14 * 60 + 45), SessionPhase::Close);
        assert_eq!(SessionPhase::from_shanghai_minutes(12 * 60), SessionPhase::Off);
    }

    #[test]
    fn close_raises_trend_floor_more_than_am_mid() {
        let mid = SessionPhase::AmMid.trend_weights();
        let close = SessionPhase::Close.trend_weights();
        assert!(close.score_floor_boost > mid.score_floor_boost);
        assert!(close.w_c > mid.w_c);
        assert!(close.w_a < mid.w_a);
    }

    #[test]
    fn close_raises_oversold_stabilize_weight() {
        let mid = SessionPhase::AmMid.oversold_weights();
        let close = SessionPhase::Close.oversold_weights();
        assert!(close.w_b > mid.w_b);
        assert!(close.w_a < mid.w_a);
    }
}

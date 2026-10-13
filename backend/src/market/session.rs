//! 盘中连续竞价时钟窗（上海时区）。
//! 是否为交易日由 `sc_trade_calendar.is_open` 判定，不在此用周一至周五硬编码。

use chrono::{FixedOffset, Timelike, Utc};

/// 上海 UTC+8（无夏令时）。
pub fn shanghai_offset() -> FixedOffset {
    FixedOffset::east_opt(8 * 3600).expect("UTC+8")
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
}

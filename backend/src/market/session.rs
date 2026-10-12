//! 盘中实时行情时间窗：上海时区周一至周五 09:30–18:00。

use chrono::{Datelike, FixedOffset, Timelike, Utc, Weekday};

/// 上海 UTC+8（无夏令时）。
pub fn shanghai_offset() -> FixedOffset {
    FixedOffset::east_opt(8 * 3600).expect("UTC+8")
}

/// 当前是否处于「最新交易日用实时行情」窗口。
pub fn is_realtime_session_now() -> bool {
    is_realtime_session_at(Utc::now())
}

pub fn is_realtime_session_at(utc: chrono::DateTime<Utc>) -> bool {
    let sh = utc.with_timezone(&shanghai_offset());
    match sh.weekday() {
        Weekday::Sat | Weekday::Sun => return false,
        _ => {}
    }
    let minutes = sh.hour() as i32 * 60 + sh.minute() as i32;
    const START: i32 = 9 * 60 + 30;
    const END: i32 = 18 * 60;
    minutes >= START && minutes <= END
}

/// 上海日历日 `yyyy-MM-dd`。
pub fn shanghai_today() -> String {
    Utc::now()
        .with_timezone(&shanghai_offset())
        .date_naive()
        .format("%Y-%m-%d")
        .to_string()
}

/// 日内强势推送窗口：工作日 09:35–11:30、13:05–14:55（上海）。
pub fn is_intraday_push_session_now() -> bool {
    is_intraday_push_session_at(Utc::now())
}

pub fn is_intraday_push_session_at(utc: chrono::DateTime<Utc>) -> bool {
    let sh = utc.with_timezone(&shanghai_offset());
    match sh.weekday() {
        Weekday::Sat | Weekday::Sun => return false,
        _ => {}
    }
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
    fn weekday_inside_window() {
        // 2026-10-09 周五 10:00 CST = 02:00 UTC
        let t = Utc.with_ymd_and_hms(2026, 10, 9, 2, 0, 0).unwrap();
        assert!(is_realtime_session_at(t));
    }

    #[test]
    fn weekday_before_open() {
        // 09:29 CST = 01:29 UTC
        let t = Utc.with_ymd_and_hms(2026, 10, 9, 1, 29, 0).unwrap();
        assert!(!is_realtime_session_at(t));
    }

    #[test]
    fn weekday_at_close_boundary() {
        // 18:00 CST = 10:00 UTC
        let t = Utc.with_ymd_and_hms(2026, 10, 9, 10, 0, 0).unwrap();
        assert!(is_realtime_session_at(t));
        // 18:01 CST
        let t2 = Utc.with_ymd_and_hms(2026, 10, 9, 10, 1, 0).unwrap();
        assert!(!is_realtime_session_at(t2));
    }

    #[test]
    fn weekend_off() {
        // 2026-10-10 周六 12:00 CST
        let t = Utc.with_ymd_and_hms(2026, 10, 10, 4, 0, 0).unwrap();
        assert!(!is_realtime_session_at(t));
    }

    #[test]
    fn push_window_am() {
        // 2026-10-09 周五 10:00 CST
        let t = Utc.with_ymd_and_hms(2026, 10, 9, 2, 0, 0).unwrap();
        assert!(is_intraday_push_session_at(t));
    }

    #[test]
    fn push_window_lunch_off() {
        // 12:00 CST
        let t = Utc.with_ymd_and_hms(2026, 10, 9, 4, 0, 0).unwrap();
        assert!(!is_intraday_push_session_at(t));
    }
}

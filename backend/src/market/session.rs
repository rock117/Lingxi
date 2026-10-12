//! 盘中实时行情时间窗：上海周一至周五连续竞价时段。

use chrono::{Datelike, FixedOffset, Timelike, Utc, Weekday};

/// 上海 UTC+8（无夏令时）。
pub fn shanghai_offset() -> FixedOffset {
    FixedOffset::east_opt(8 * 3600).expect("UTC+8")
}

/// 当前是否处于「当日用实时行情」窗口。
///
/// 条件：上海 **周一至周五** + 连续竞价 `09:30–11:30` / `13:00–15:00`（暂不区分法定节假日）。
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
    const AM_LO: i32 = 9 * 60 + 30;
    const AM_HI: i32 = 11 * 60 + 30;
    const PM_LO: i32 = 13 * 60;
    const PM_HI: i32 = 15 * 60;
    (minutes >= AM_LO && minutes <= AM_HI) || (minutes >= PM_LO && minutes <= PM_HI)
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

/// 日内强势推送窗口：工作日竞价内略收紧（避开开盘/收盘噪声）。
pub fn is_intraday_push_session_now() -> bool {
    is_intraday_push_session_at(Utc::now())
}

pub fn is_intraday_push_session_at(utc: chrono::DateTime<Utc>) -> bool {
    if !is_realtime_session_at(utc) {
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
    fn weekday_inside_am() {
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
    fn lunch_off() {
        // 12:00 CST
        let t = Utc.with_ymd_and_hms(2026, 10, 9, 4, 0, 0).unwrap();
        assert!(!is_realtime_session_at(t));
        assert!(!is_intraday_push_session_at(t));
    }

    #[test]
    fn after_close_no_realtime() {
        // 15:01 CST — 收盘后不再取实时
        let t = Utc.with_ymd_and_hms(2026, 10, 9, 7, 1, 0).unwrap();
        assert!(!is_realtime_session_at(t));
        // 18:00 CST 旧窗口已废弃
        let t2 = Utc.with_ymd_and_hms(2026, 10, 9, 10, 0, 0).unwrap();
        assert!(!is_realtime_session_at(t2));
    }

    #[test]
    fn at_close_boundary() {
        // 15:00 CST 仍属交易时段
        let t = Utc.with_ymd_and_hms(2026, 10, 9, 7, 0, 0).unwrap();
        assert!(is_realtime_session_at(t));
    }

    #[test]
    fn weekend_off() {
        // 2026-10-10 周六 12:00 CST
        let t = Utc.with_ymd_and_hms(2026, 10, 10, 4, 0, 0).unwrap();
        assert!(!is_realtime_session_at(t));
    }

    #[test]
    fn push_window_am() {
        let t = Utc.with_ymd_and_hms(2026, 10, 9, 2, 0, 0).unwrap();
        assert!(is_intraday_push_session_at(t));
    }

    #[test]
    fn push_not_in_first_5min() {
        // 09:32 CST — 实时可取，推送未开
        let t = Utc.with_ymd_and_hms(2026, 10, 9, 1, 32, 0).unwrap();
        assert!(is_realtime_session_at(t));
        assert!(!is_intraday_push_session_at(t));
    }
}

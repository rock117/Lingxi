//! A 股竞价涨跌幅限制（按代码推断板块）。
//!
//! 常规（有涨跌幅限制的交易日）：
//! - 沪深主板：±10%（2026-07-06 起主板 ST/*ST 亦为 ±10%）
//! - 科创板（688/689）、创业板（300/301）：±20%
//! - 北交所（.BJ）：±30%
//!
//! 上市初期无限制、退市整理首日等特例需上市日信息，本模块仅按板块上限近似；
//! 无限制日涨跌幅达不到「触板」阈值时不会被计为涨跌停。
//!
//! 触板判定用涨跌幅近似（缺官方涨跌停价时）：`|Δ| ≥ 板幅 − LIMIT_TOUCH_SLACK`，
//! 以覆盖 tick 四舍五入（如 10% 板常见实为 9.9x%）。

/// 触板容差（百分点）：10% 板 ≥ 9.8 视为涨停。
pub const LIMIT_TOUCH_SLACK: f64 = 0.2;
/// 「接近涨跌停」容差（筛选/大盘样本排除用）：10% 板 ≥ 9.5。
pub const LIMIT_NEAR_SLACK: f64 = 0.5;

/// 该标的所属板块的涨跌幅限制（百分点绝对值）。
pub fn limit_pct_for_symbol(symbol: &str) -> f64 {
    let (code, mkt) = split_symbol(symbol);
    match mkt {
        "BJ" => 30.0,
        "SH" if code.starts_with("688") || code.starts_with("689") => 20.0,
        "SZ" if code.starts_with("300") || code.starts_with("301") => 20.0,
        _ => 10.0,
    }
}

/// 是否触及涨停（涨跌幅近似）。
pub fn is_limit_up(symbol: &str, change_pct: f64) -> bool {
    change_pct + 1e-12 >= limit_pct_for_symbol(symbol) - LIMIT_TOUCH_SLACK
}

/// 是否触及跌停（涨跌幅近似）。
pub fn is_limit_down(symbol: &str, change_pct: f64) -> bool {
    change_pct - 1e-12 <= -(limit_pct_for_symbol(symbol) - LIMIT_TOUCH_SLACK)
}

/// 是否接近涨停（趋势选股头寸/买点排除等）。
pub fn near_limit_up(symbol: &str, change_pct: f64) -> bool {
    change_pct + 1e-12 >= limit_pct_for_symbol(symbol) - LIMIT_NEAR_SLACK
}

/// 是否接近跌停。
pub fn near_limit_down(symbol: &str, change_pct: f64) -> bool {
    change_pct - 1e-12 <= -(limit_pct_for_symbol(symbol) - LIMIT_NEAR_SLACK)
}

/// 是否接近涨停或跌停。
pub fn near_either_limit(symbol: &str, change_pct: f64) -> bool {
    near_limit_up(symbol, change_pct) || near_limit_down(symbol, change_pct)
}

fn split_symbol(symbol: &str) -> (&str, &str) {
    match symbol.rsplit_once('.') {
        Some((code, mkt)) => (code, mkt),
        None => (symbol, ""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn board_limits_by_symbol() {
        assert!((limit_pct_for_symbol("600519.SH") - 10.0).abs() < 1e-9);
        assert!((limit_pct_for_symbol("000001.SZ") - 10.0).abs() < 1e-9);
        assert!((limit_pct_for_symbol("688001.SH") - 20.0).abs() < 1e-9);
        assert!((limit_pct_for_symbol("300750.SZ") - 20.0).abs() < 1e-9);
        assert!((limit_pct_for_symbol("301000.SZ") - 20.0).abs() < 1e-9);
        assert!((limit_pct_for_symbol("920001.BJ") - 30.0).abs() < 1e-9);
        assert!((limit_pct_for_symbol("430047.BJ") - 30.0).abs() < 1e-9);
    }

    #[test]
    fn touch_uses_board_threshold() {
        // 主板 9.8 触涨停；创业板 9.8 未触
        assert!(is_limit_up("600000.SH", 9.85));
        assert!(!is_limit_up("300750.SZ", 9.85));
        assert!(is_limit_up("300750.SZ", 19.85));
        assert!(is_limit_down("688001.SH", -19.9));
        assert!(!is_limit_down("688001.SH", -9.9));
        assert!(is_limit_up("920001.BJ", 29.85));
        // 接近板
        assert!(near_limit_up("600000.SH", 9.5));
        assert!(!near_limit_up("300750.SZ", 9.5));
        assert!(near_limit_up("300750.SZ", 19.5));
    }
}

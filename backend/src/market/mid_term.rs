//! 中期趋势指标（MTT）—— 严格按 `docs/mid_term_trend.md` 实现。
//! 禁止只改本文件不改文档。

use std::collections::HashMap;

use super::domain::*;

// ============================================================
// 规范常量（文档 §2）
// ============================================================

const MA_MID: usize = 20;
const MA_LONG: usize = 60;
const MA_TYPE_EMA: bool = true; // MA_TYPE = ema
const SLOPE_LOOKBACK: usize = 5;
const USE_SLOPE_FILTER: bool = true;
const EPS_PRICE: f64 = 1e-10;
const EPS_MA: f64 = 1e-10;
const NH_NL_WINDOW: usize = 20;
const MIN_HISTORY_DAYS: usize = MA_LONG + SLOPE_LOOKBACK; // 65

// ============================================================
// 工具
// ============================================================

fn map_to_score(raw: f64) -> f64 {
    50.0 + 50.0 * raw.clamp(-1.0, 1.0)
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

fn median(mut values: Vec<f64>) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = values.len();
    if n % 2 == 1 {
        values[n / 2]
    } else {
        (values[n / 2 - 1] + values[n / 2]) / 2.0
    }
}

fn is_valid_stock(s: &StockDailyData) -> bool {
    s.open > 0.0
        && s.high > 0.0
        && s.low > 0.0
        && s.close_price > 0.0
        && s.high >= s.low
        && s.high >= s.open
        && s.high >= s.close_price
        && s.low <= s.open
        && s.low <= s.close_price
        && s.amount >= 0.0
        && s.change_pct.is_finite()
        && s.amount.is_finite()
        && s.open.is_finite()
        && s.high.is_finite()
        && s.low.is_finite()
        && s.close_price.is_finite()
}

#[derive(Clone)]
struct Bar {
    close: f64,
    high: f64,
    low: f64,
}

fn sma(closes: &[f64], period: usize) -> Option<f64> {
    if closes.len() < period {
        return None;
    }
    Some(mean(&closes[closes.len() - period..]))
}

/// 标准收盘 EMA：种子为前 `period` 根 SMA，再迭代至序列末。
fn ema(closes: &[f64], period: usize) -> Option<f64> {
    if closes.len() < period {
        return None;
    }
    let alpha = 2.0 / (period as f64 + 1.0);
    let mut ema_v = mean(&closes[..period]);
    for &c in &closes[period..] {
        ema_v = alpha * c + (1.0 - alpha) * ema_v;
    }
    Some(ema_v)
}

fn ma(closes: &[f64], period: usize) -> Option<f64> {
    if MA_TYPE_EMA {
        ema(closes, period)
    } else {
        sma(closes, period)
    }
}

fn classify_alignment(close: f64, ma_mid: f64, ma_long: f64) -> i8 {
    if close > ma_mid + EPS_MA && ma_mid > ma_long + EPS_MA {
        1
    } else if close < ma_mid - EPS_MA && ma_mid < ma_long - EPS_MA {
        -1
    } else {
        0
    }
}

fn apply_slope_filter(alignment: i8, ma_mid: f64, ma_mid_prev: f64) -> i8 {
    match alignment {
        1 if ma_mid > ma_mid_prev + EPS_MA => 1,
        -1 if ma_mid < ma_mid_prev - EPS_MA => -1,
        1 | -1 => 0,
        _ => 0,
    }
}

fn collect_bars(snapshots: &[MarketDailySnapshot]) -> HashMap<String, Vec<Bar>> {
    let mut map: HashMap<String, Vec<Bar>> = HashMap::new();
    for snap in snapshots {
        for s in &snap.stocks {
            if !is_valid_stock(s) {
                continue;
            }
            map.entry(s.symbol.clone()).or_default().push(Bar {
                close: s.close_price,
                high: s.high,
                low: s.low,
            });
        }
    }
    map
}

fn last_day_symbols(snapshots: &[MarketDailySnapshot]) -> Vec<String> {
    snapshots
        .last()
        .map(|s| {
            s.stocks
                .iter()
                .filter(|x| is_valid_stock(x))
                .map(|x| x.symbol.clone())
                .collect()
        })
        .unwrap_or_default()
}

fn neutral_result() -> MttResult {
    MttResult {
        value: 50.0,
        status: MttStatus::MidNeutral,
        trend_raw: 0.0,
        ma_type: if MA_TYPE_EMA {
            "ema".into()
        } else {
            "sma".into()
        },
        ma_mid: MA_MID,
        ma_long: MA_LONG,
        use_slope_filter: USE_SLOPE_FILTER,
        counts: MttCounts {
            bull: 0,
            bear: 0,
            neutral: 0,
            classified: 0,
        },
        diagnostics: MttDiagnostics {
            deviation_median: 0.0,
            nh_nl: 50.0,
        },
        calculated_at: chrono::Utc::now().to_rfc3339(),
    }
}

/// 计算中期趋势（规范全文）。
pub fn calculate_mtt(input: &MarketDataInput) -> MttResult {
    if input.snapshots.is_empty() {
        return neutral_result();
    }

    let bars_by_sym = collect_bars(&input.snapshots);
    let last_symbols = last_day_symbols(&input.snapshots);
    if last_symbols.is_empty() {
        return neutral_result();
    }

    let mut n_bull = 0usize;
    let mut n_bear = 0usize;
    let mut n_neutral = 0usize;
    let mut deviations: Vec<f64> = Vec::new();
    let mut n_nh = 0usize;
    let mut n_nl = 0usize;

    for sym in &last_symbols {
        let Some(bars) = bars_by_sym.get(sym) else {
            continue;
        };
        if bars.len() < MIN_HISTORY_DAYS {
            continue;
        }

        let closes: Vec<f64> = bars.iter().map(|b| b.close).collect();
        let t = closes.len();
        let close = closes[t - 1];

        let Some(ma_mid) = ma(&closes, MA_MID) else {
            continue;
        };
        let Some(ma_long) = ma(&closes, MA_LONG) else {
            continue;
        };

        let alignment = classify_alignment(close, ma_mid, ma_long);
        let trend_i = if USE_SLOPE_FILTER {
            let prev_end = t - SLOPE_LOOKBACK;
            let prev_closes = &closes[..prev_end];
            match ma(prev_closes, MA_MID) {
                Some(ma_mid_prev) => apply_slope_filter(alignment, ma_mid, ma_mid_prev),
                None => 0,
            }
        } else {
            alignment
        };

        match trend_i {
            1 => n_bull += 1,
            -1 => n_bear += 1,
            _ => n_neutral += 1,
        }

        if ma_long.abs() >= EPS_PRICE {
            deviations.push((close - ma_long) / ma_long);
        }

        // §7.3 诊断：至少 NH_NL_WINDOW 根（本分支已满足 MIN_HISTORY）
        let window = &bars[bars.len() - NH_NL_WINDOW..];
        let high_w = window.iter().map(|b| b.high).fold(f64::NEG_INFINITY, f64::max);
        let low_w = window.iter().map(|b| b.low).fold(f64::INFINITY, f64::min);
        let last_bar = bars.last().unwrap();
        if last_bar.high + EPS_PRICE >= high_w {
            n_nh += 1;
        }
        if last_bar.low - EPS_PRICE <= low_w {
            n_nl += 1;
        }
    }

    let n_classified = n_bull + n_bear;
    if n_bull + n_bear + n_neutral == 0 {
        return neutral_result();
    }

    let trend_raw = if n_classified == 0 {
        0.0
    } else {
        (n_bull as f64 - n_bear as f64) / n_classified as f64
    };
    let value = map_to_score(trend_raw);

    let nh_nl_raw = if n_nh + n_nl == 0 {
        0.0
    } else {
        (n_nh as f64 - n_nl as f64) / (n_nh + n_nl) as f64
    };

    MttResult {
        value,
        status: MttStatus::from_score(value),
        trend_raw,
        ma_type: if MA_TYPE_EMA {
            "ema".into()
        } else {
            "sma".into()
        },
        ma_mid: MA_MID,
        ma_long: MA_LONG,
        use_slope_filter: USE_SLOPE_FILTER,
        counts: MttCounts {
            bull: n_bull,
            bear: n_bear,
            neutral: n_neutral,
            classified: n_classified,
        },
        diagnostics: MttDiagnostics {
            deviation_median: median(deviations),
            nh_nl: map_to_score(nh_nl_raw),
        },
        calculated_at: chrono::Utc::now().to_rfc3339(),
    }
}

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn bar(close: f64, high: f64, low: f64) -> StockDailyData {
        StockDailyData {
            symbol: "TEST".into(),
            date: "2026-01-01".into(),
            open: close,
            high,
            low,
            close_price: close,
            amount: 1e8,
            change_pct: 0.0,
        }
    }

    /// 构造单票单调上涨序列（保证多头排列 + MA20 上行）
    fn rising_series(n: usize, symbol: &str) -> Vec<MarketDailySnapshot> {
        let mut snaps = Vec::with_capacity(n);
        for i in 0..n {
            let close = 10.0 + i as f64 * 0.15;
            let high = close * 1.01;
            let low = close * 0.99;
            snaps.push(MarketDailySnapshot {
                date: format!("2026-{:02}-{:02}", 1 + i / 28, 1 + i % 28),
                stocks: vec![StockDailyData {
                    symbol: symbol.into(),
                    date: format!("d{i}"),
                    open: close,
                    high,
                    low,
                    close_price: close,
                    amount: 1e8,
                    change_pct: 1.0,
                }],
            });
        }
        snaps
    }

    fn falling_series(n: usize, symbol: &str) -> Vec<MarketDailySnapshot> {
        let mut snaps = Vec::with_capacity(n);
        for i in 0..n {
            let close = 100.0 - i as f64 * 0.15;
            let high = close * 1.01;
            let low = close * 0.99;
            snaps.push(MarketDailySnapshot {
                date: format!("2026-{:02}-{:02}", 1 + i / 28, 1 + i % 28),
                stocks: vec![StockDailyData {
                    symbol: symbol.into(),
                    date: format!("d{i}"),
                    open: close,
                    high,
                    low,
                    close_price: close,
                    amount: 1e8,
                    change_pct: -1.0,
                }],
            });
        }
        snaps
    }

    fn merge_symbols(series: Vec<Vec<MarketDailySnapshot>>) -> MarketDataInput {
        let days = series[0].len();
        let mut snapshots = Vec::with_capacity(days);
        for d in 0..days {
            let mut stocks = Vec::new();
            for s in &series {
                stocks.extend(s[d].stocks.clone());
            }
            snapshots.push(MarketDailySnapshot {
                date: series[0][d].date.clone(),
                stocks,
            });
        }
        MarketDataInput { snapshots }
    }

    #[test]
    fn test_all_bull_near_100() {
        let s1 = rising_series(80, "A");
        let s2 = rising_series(80, "B");
        let r = calculate_mtt(&merge_symbols(vec![s1, s2]));
        assert!(r.value > 90.0, "全多头应接近 100, 实际 {}", r.value);
        assert_eq!(r.counts.bear, 0);
        assert_eq!(r.counts.classified, r.counts.bull);
    }

    #[test]
    fn test_all_bear_near_0() {
        let s1 = falling_series(80, "A");
        let s2 = falling_series(80, "B");
        let r = calculate_mtt(&merge_symbols(vec![s1, s2]));
        assert!(r.value < 10.0, "全空头应接近 0, 实际 {}", r.value);
        assert_eq!(r.counts.bull, 0);
    }

    #[test]
    fn test_half_half_is_50() {
        let bull = rising_series(80, "BULL");
        let bear = falling_series(80, "BEAR");
        let r = calculate_mtt(&merge_symbols(vec![bull, bear]));
        assert!((r.value - 50.0).abs() < 1e-6, "一半多一半空应为 50, 实际 {}", r.value);
        assert_eq!(r.counts.classified, 2);
    }

    #[test]
    fn test_short_history_skipped() {
        // 只有 10 天 → 全部跳过 → 中性
        let snaps = rising_series(10, "NEW");
        let r = calculate_mtt(&MarketDataInput { snapshots: snaps });
        assert_eq!(r.value, 50.0);
        assert_eq!(r.counts.classified, 0);
        assert_eq!(r.counts.bull + r.counts.bear + r.counts.neutral, 0);
    }

    #[test]
    fn test_invalid_ohlc_filtered() {
        let mut snaps = rising_series(80, "OK");
        if let Some(last) = snaps.last_mut() {
            let mut bad = bar(-1.0, 1.0, 0.5);
            bad.symbol = "BAD".into();
            last.stocks.push(bad);
        }
        let r = calculate_mtt(&MarketDataInput { snapshots: snaps });
        assert!(r.counts.bull >= 1);
        assert!(r.value > 90.0);
    }

    #[test]
    fn test_map_bounds() {
        assert!((map_to_score(1.0) - 100.0).abs() < 1e-9);
        assert!((map_to_score(-1.0) - 0.0).abs() < 1e-9);
        assert!((map_to_score(0.0) - 50.0).abs() < 1e-9);
    }

    #[test]
    fn test_status_bands() {
        assert!(matches!(
            MttStatus::from_score(70.0),
            MttStatus::MidBullConfirmed
        ));
        assert!(matches!(MttStatus::from_score(62.0), MttStatus::MidBullGrey));
        assert!(matches!(MttStatus::from_score(50.0), MttStatus::MidNeutral));
        assert!(matches!(MttStatus::from_score(38.0), MttStatus::MidBearGrey));
        assert!(matches!(
            MttStatus::from_score(20.0),
            MttStatus::MidBearConfirmed
        ));
    }

    #[test]
    fn test_diagnostics_do_not_change_value_isolation() {
        // 诊断字段存在且有界；主分只由排列决定（结构测试）
        let r = calculate_mtt(&merge_symbols(vec![rising_series(80, "A")]));
        assert!(r.diagnostics.nh_nl >= 0.0 && r.diagnostics.nh_nl <= 100.0);
        assert!(r.value > 90.0);
    }
}

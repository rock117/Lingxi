//! 共享技术指标（选股 / 个股短线等复用）。

/// Wilder RSI 序列；前 `period` 个位置为 NaN。
pub(crate) fn rsi_series(closes: &[f64], period: usize) -> Vec<f64> {
    let mut out = vec![f64::NAN; closes.len()];
    if period == 0 || closes.len() <= period {
        return out;
    }
    let mut gain = 0.0;
    let mut loss = 0.0;
    for i in 1..=period {
        let d = closes[i] - closes[i - 1];
        if d >= 0.0 {
            gain += d;
        } else {
            loss -= d;
        }
    }
    let mut avg_gain = gain / period as f64;
    let mut avg_loss = loss / period as f64;
    out[period] = rsi_from_avg(avg_gain, avg_loss);
    for i in (period + 1)..closes.len() {
        let d = closes[i] - closes[i - 1];
        let g = if d > 0.0 { d } else { 0.0 };
        let l = if d < 0.0 { -d } else { 0.0 };
        avg_gain = (avg_gain * (period as f64 - 1.0) + g) / period as f64;
        avg_loss = (avg_loss * (period as f64 - 1.0) + l) / period as f64;
        out[i] = rsi_from_avg(avg_gain, avg_loss);
    }
    out
}

fn rsi_from_avg(avg_gain: f64, avg_loss: f64) -> f64 {
    if avg_loss <= 1e-12 {
        return 100.0;
    }
    let rs = avg_gain / avg_loss;
    100.0 - (100.0 / (1.0 + rs))
}

/// 最新有效 RSI；不足样本则 `None`。
pub(crate) fn rsi_latest(closes: &[f64], period: usize) -> Option<f64> {
    let series = rsi_series(closes, period);
    series
        .iter()
        .rev()
        .find(|v| v.is_finite())
        .copied()
}

/// 最新与前一日有效 RSI（用于穿越类标签）。`(latest, prev)`。
pub(crate) fn rsi_latest_pair(closes: &[f64], period: usize) -> Option<(f64, f64)> {
    let series = rsi_series(closes, period);
    let finite: Vec<f64> = series.iter().copied().filter(|v| v.is_finite()).collect();
    if finite.len() < 2 {
        return None;
    }
    let n = finite.len();
    Some((finite[n - 1], finite[n - 2]))
}

/// 今日 RSI − `lookback` 根 K 线前的 RSI（两侧皆有效才有值）。
pub(crate) fn rsi_delta(closes: &[f64], period: usize, lookback: usize) -> Option<f64> {
    if lookback == 0 {
        return Some(0.0);
    }
    let series = rsi_series(closes, period);
    let n = series.len();
    if n <= lookback {
        return None;
    }
    let cur = series[n - 1];
    let past = series[n - 1 - lookback];
    if cur.is_finite() && past.is_finite() {
        Some(cur - past)
    } else {
        None
    }
}

/// 近 `window` 根 K 线内 RSI≥超买 / ≤超卖 的天数。返回 `(days_ob, days_os)`。
pub(crate) fn rsi_zone_day_counts(
    closes: &[f64],
    period: usize,
    window: usize,
    overbought: f64,
    oversold: f64,
) -> (u32, u32) {
    let series = rsi_series(closes, period);
    if window == 0 || series.is_empty() {
        return (0, 0);
    }
    let start = series.len().saturating_sub(window);
    let mut ob = 0u32;
    let mut os = 0u32;
    for v in &series[start..] {
        if !v.is_finite() {
            continue;
        }
        if *v >= overbought {
            ob += 1;
        }
        if *v <= oversold {
            os += 1;
        }
    }
    (ob, os)
}

/// `(crossed_oversold, crossed_overbought)`：昨→今上穿超卖线 / 下破超买线。
pub(crate) fn rsi_cross_flags(
    closes: &[f64],
    period: usize,
    overbought: f64,
    oversold: f64,
) -> (bool, bool) {
    match rsi_latest_pair(closes, period) {
        Some((cur, prev)) => (
            prev <= oversold && cur > oversold,
            prev >= overbought && cur < overbought,
        ),
        None => (false, false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rsi_flat_is_neutralish() {
        let closes: Vec<f64> = (0..30).map(|_| 10.0).collect();
        let r = rsi_latest(&closes, 14);
        assert!(r.is_some());
        // 无涨跌时 avg_loss≈0 → 100；全平也可接受高位
        assert!(r.unwrap() >= 50.0);
    }

    #[test]
    fn rsi_needs_period_plus_one() {
        let closes: Vec<f64> = (0..14).map(|i| i as f64).collect();
        assert!(rsi_latest(&closes, 14).is_none());
    }

    #[test]
    fn rsi_delta_needs_lookback() {
        // 严格上涨：RSI 很快顶到高位，Δ5 应可算
        let closes: Vec<f64> = (0..40).map(|i| 10.0 + i as f64).collect();
        assert!(rsi_delta(&closes, 14, 5).is_some());
        assert!(rsi_delta(&closes, 14, 5).unwrap().is_finite());
    }

    #[test]
    fn rsi_cross_oversold_on_rebound() {
        // 先跌后涨，尽量制造穿越（不保证阈值，只检查 API 不 panic）
        let mut closes: Vec<f64> = (0..25).map(|i| 100.0 - i as f64).collect();
        for i in 0..15 {
            closes.push(75.0 + i as f64 * 2.0);
        }
        let _ = rsi_cross_flags(&closes, 14, 70.0, 30.0);
        let (ob, os) = rsi_zone_day_counts(&closes, 14, 10, 70.0, 30.0);
        assert!(ob + os <= 10);
    }
}

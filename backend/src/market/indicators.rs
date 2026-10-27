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
}

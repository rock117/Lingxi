//! ?????????? `docs/intraday_alert.md`?

use std::cmp::Ordering;
use std::collections::HashMap;

use serde::Serialize;

use super::domain::{MarketDataInput, StockDailyData};

const DELTA_MIN: f64 = 1.2;
const AMOUNT_MIN: f64 = 3.0e7;
const PRESELECT_K: usize = 150;
const LIMIT_PCT: f64 = 9.5;
const W_A: f64 = 0.40;
const W_B: f64 = 0.35;
const W_C: f64 = 0.25;
const SCORE_MIN: f64 = 55.0;
const RS_CAP: f64 = 5.0;
const EPS: f64 = 1e-10;

#[derive(Debug, Clone, Serialize)]
pub struct AlertLayers {
    pub a: f64,
    pub b: f64,
    pub c: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AlertItem {
    pub symbol: String,
    pub score: f64,
    pub rs: f64,
    pub change_pct: f64,
    pub amount: f64,
    pub layers: AlertLayers,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct IntradayAlertResult {
    pub date: String,
    pub r_mkt: f64,
    pub sample_size: usize,
    pub candidate_size: usize,
    pub items: Vec<AlertItem>,
}

fn clip01(x: f64) -> f64 {
    x.clamp(0.0, 1.0)
}

fn valid_stock(s: &StockDailyData) -> bool {
    s.change_pct.is_finite()
        && s.amount.is_finite()
        && s.amount >= AMOUNT_MIN
        && s.close_price.is_finite()
        && s.close_price >= 2.0
        && s.open.is_finite()
        && s.high.is_finite()
        && s.low.is_finite()
        && s.high + EPS >= s.low
}

fn intra_pos(s: &StockDailyData) -> f64 {
    let span = s.high - s.low;
    if span.abs() < EPS {
        0.5
    } else {
        ((s.close_price - s.low) / span).clamp(0.0, 1.0)
    }
}

fn score_a(rs: f64, s: &StockDailyData) -> f64 {
    let denom = (RS_CAP - DELTA_MIN).max(EPS);
    let mut a = 100.0 * clip01((rs - DELTA_MIN) / denom);
    if intra_pos(s) > 0.7 {
        a = (a + 5.0).min(100.0);
    }
    a
}

fn score_b1(r5d: f64) -> f64 {
    if (2.0..=12.0).contains(&r5d) {
        100.0
    } else if (0.0..2.0).contains(&r5d) {
        60.0 + 40.0 * (r5d / 2.0)
    } else if (12.0..=18.0).contains(&r5d) {
        100.0 - 40.0 * ((r5d - 12.0) / 6.0)
    } else if (-3.0..0.0).contains(&r5d) {
        30.0 + 30.0 * ((r5d + 3.0) / 3.0)
    } else if (18.0..=25.0).contains(&r5d) {
        60.0 - 30.0 * ((r5d - 18.0) / 7.0)
    } else {
        15.0
    }
}

fn score_b2(c_last: f64, ma5: f64) -> f64 {
    if ma5.abs() < EPS {
        return 50.0;
    }
    let pct = (c_last - ma5) / ma5 * 100.0;
    100.0 * clip01((pct + 2.0) / 6.0)
}

fn score_b3(bars: &[&StockDailyData]) -> f64 {
    if bars.is_empty() {
        return 50.0;
    }
    let atr: f64 = bars
        .iter()
        .map(|b| {
            if b.close_price.abs() < EPS {
                0.0
            } else {
                (b.high - b.low) / b.close_price * 100.0
            }
        })
        .sum::<f64>()
        / bars.len() as f64;
    if (2.0..=6.0).contains(&atr) {
        100.0
    } else if atr < 2.0 {
        100.0 * clip01(atr / 2.0)
    } else if atr <= 10.0 {
        100.0 - 60.0 * ((atr - 6.0) / 4.0)
    } else {
        20.0
    }
}

fn score_b4(bars: &[&StockDailyData]) -> f64 {
    if bars.len() < 2 {
        return 50.0;
    }
    let last = bars.last().unwrap().amount;
    let prior: f64 = bars[..bars.len() - 1].iter().map(|b| b.amount).sum::<f64>()
        / (bars.len() - 1) as f64;
    if prior < EPS {
        return 50.0;
    }
    let vr = last / prior;
    if (1.1..=2.5).contains(&vr) {
        100.0
    } else if (0.8..1.1).contains(&vr) {
        60.0 + 40.0 * ((vr - 0.8) / 0.3)
    } else if (2.5..=4.0).contains(&vr) {
        100.0 - 40.0 * ((vr - 2.5) / 1.5)
    } else if vr > 4.0 {
        40.0
    } else {
        30.0
    }
}

fn score_b5(bars: &[&StockDailyData]) -> f64 {
    let n = bars.len().min(3);
    if n == 0 {
        return 50.0;
    }
    let tail = &bars[bars.len() - n..];
    let up = tail.iter().filter(|b| b.change_pct > 0.0).count();
    let elevating = n >= 2
        && tail
            .windows(2)
            .all(|w| w[1].close_price + EPS >= w[0].close_price);
    match (up >= 2, elevating) {
        (true, true) => 100.0,
        (true, false) => 70.0,
        (false, true) => 55.0,
        _ => 35.0,
    }
}

fn score_b(hist: Option<&[&StockDailyData]>) -> f64 {
    let Some(bars) = hist.filter(|b| b.len() >= 5) else {
        return 40.0;
    };
    let bars = &bars[bars.len() - 5..];
    let c0 = bars[0].close_price;
    let c4 = bars[4].close_price;
    if c0.abs() < EPS {
        return 40.0;
    }
    let r5d = (c4 / c0 - 1.0) * 100.0;
    let ma5 = bars.iter().map(|b| b.close_price).sum::<f64>() / 5.0;
    let b1 = score_b1(r5d);
    let b2 = score_b2(c4, ma5);
    let b3 = score_b3(bars);
    let b4 = score_b4(bars);
    let b5 = score_b5(bars);
    0.35 * b1 + 0.25 * b2 + 0.20 * b3 + 0.15 * b4 + 0.05 * b5
}

fn score_c(s: &StockDailyData) -> f64 {
    let headroom = LIMIT_PCT - s.change_pct;
    let c1 = 100.0 * clip01((headroom - 0.5) / 5.0);

    let pos = intra_pos(s);
    let c2 = if (0.35..=0.65).contains(&pos) {
        100.0
    } else if (0.25..0.35).contains(&pos) || (0.65..=0.80).contains(&pos) {
        70.0
    } else if pos > 0.90 {
        35.0
    } else {
        50.0
    };

    let prev = s.close_price / (1.0 + s.change_pct / 100.0).max(EPS);
    let giveback = if prev.abs() < EPS {
        0.0
    } else {
        ((s.high - s.close_price) / prev * 100.0).max(0.0)
    };
    let c3 = if giveback < 1.0 {
        100.0
    } else if giveback < 3.0 {
        100.0 - 50.0 * ((giveback - 1.0) / 2.0)
    } else {
        30.0
    };

    let c4 = 100.0 * clip01(((s.amount.max(1.0).ln() - (3.0e7_f64).ln()) / 3.0).max(0.0));

    0.30 * c1 + 0.30 * c2 + 0.25 * c3 + 0.15 * c4
}

fn build_reason(a: f64, b: f64, c: f64, rs: f64) -> String {
    let dominant = if a >= b && a >= c {
        "RS"
    } else if b >= c {
        "5D"
    } else {
        "entry"
    };
    format!("RS={:+.1}% main={}", rs, dominant)
}

/// ?????????????? Top ????
pub fn calculate_intraday_alerts(input: &MarketDataInput) -> IntradayAlertResult {
    let empty = |date: String| IntradayAlertResult {
        date,
        r_mkt: 0.0,
        sample_size: 0,
        candidate_size: 0,
        items: vec![],
    };

    let Some(today) = input.snapshots.last() else {
        return empty(String::new());
    };
    let date = today.date.clone();

    let valid: Vec<&StockDailyData> = today.stocks.iter().filter(|s| valid_stock(s)).collect();
    let sample_size = valid.len();
    if sample_size == 0 {
        return empty(date);
    }

    // ?????????????????????
    let mkt_sample: Vec<&StockDailyData> = valid
        .iter()
        .copied()
        .filter(|s| s.change_pct < LIMIT_PCT)
        .collect();
    let r_mkt = if mkt_sample.is_empty() {
        0.0
    } else {
        mkt_sample.iter().map(|s| s.change_pct).sum::<f64>() / mkt_sample.len() as f64
    };

    let mut hist_map: HashMap<String, Vec<&StockDailyData>> = HashMap::new();
    let hist_days = input.snapshots.len().saturating_sub(1);
    for snap in input.snapshots.iter().take(hist_days) {
        for s in &snap.stocks {
            hist_map.entry(s.symbol.clone()).or_default().push(s);
        }
    }

    let mut candidates: Vec<(&StockDailyData, f64)> = Vec::new();
    for s in &valid {
        if s.change_pct <= 0.0 || s.change_pct >= LIMIT_PCT {
            continue;
        }
        let rs = s.change_pct - r_mkt;
        if rs < DELTA_MIN {
            continue;
        }
        candidates.push((s, rs));
    }
    candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));
    if candidates.len() > PRESELECT_K {
        candidates.truncate(PRESELECT_K);
    }
    let candidate_size = candidates.len();

    let mut scored: Vec<AlertItem> = Vec::with_capacity(candidates.len());
    for (s, rs) in candidates {
        let hist = hist_map.get(&s.symbol).map(|v| v.as_slice());
        let a = score_a(rs, s);
        let b = score_b(hist);
        let c = score_c(s);
        let score = W_A * a + W_B * b + W_C * c;
        scored.push(AlertItem {
            symbol: s.symbol.clone(),
            score,
            rs,
            change_pct: s.change_pct,
            amount: s.amount,
            layers: AlertLayers { a, b, c },
            reason: build_reason(a, b, c, rs),
        });
    }

    scored.sort_by(|x, y| {
        y.score
            .partial_cmp(&x.score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| y.rs.partial_cmp(&x.rs).unwrap_or(Ordering::Equal))
            .then_with(|| y.amount.partial_cmp(&x.amount).unwrap_or(Ordering::Equal))
    });

    let items: Vec<AlertItem> = scored
        .into_iter()
        .filter(|i| i.score >= SCORE_MIN)
        .take(10)
        .collect();

    IntradayAlertResult {
        date,
        r_mkt,
        sample_size,
        candidate_size,
        items,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::domain::MarketDailySnapshot;

    fn bar(sym: &str, chg: f64, amount: f64, o: f64, h: f64, l: f64, c: f64) -> StockDailyData {
        StockDailyData {
            symbol: sym.into(),
            date: "2026-10-10".into(),
            open: o,
            high: h,
            low: l,
            close_price: c,
            amount,
            change_pct: chg,
        }
    }

    fn hist_bar(sym: &str, date: &str, chg: f64, c: f64, amount: f64) -> StockDailyData {
        StockDailyData {
            symbol: sym.into(),
            date: date.into(),
            open: c * 0.99,
            high: c * 1.01,
            low: c * 0.98,
            close_price: c,
            amount,
            change_pct: chg,
        }
    }

    #[test]
    fn top_picks_relative_strength() {
        let today = MarketDailySnapshot {
            date: "2026-10-10".into(),
            stocks: vec![
                bar("AAA.SH", 3.5, 8.0e7, 10.0, 10.5, 9.9, 10.35),
                bar("BBB.SH", 0.2, 8.0e7, 10.0, 10.1, 9.9, 10.02),
                bar("EEE.SH", 0.1, 5.0e7, 10.0, 10.05, 9.95, 10.01),
                bar("FFF.SH", -0.3, 5.0e7, 10.0, 10.0, 9.8, 9.97),
                bar("CCC.SH", 4.0, 1.0e7, 10.0, 10.6, 9.9, 10.4),
                bar("DDD.SH", 9.6, 8.0e7, 10.0, 11.0, 10.0, 10.96),
            ],
        };
        let mut snaps = Vec::new();
        for i in 1..=5 {
            snaps.push(MarketDailySnapshot {
                date: format!("2026-10-0{i}"),
                stocks: vec![hist_bar(
                    "AAA.SH",
                    &format!("2026-10-0{i}"),
                    1.0,
                    9.0 + i as f64 * 0.1,
                    5.0e7,
                )],
            });
        }
        snaps.push(today);
        let r = calculate_intraday_alerts(&MarketDataInput { snapshots: snaps });
        assert!(r.r_mkt.is_finite());
        assert!(
            r.items.iter().any(|i| i.symbol == "AAA.SH"),
            "AAA should be selected: {:?}",
            r.items
        );
        assert!(
            r.items
                .iter()
                .all(|i| i.symbol != "CCC.SH" && i.symbol != "DDD.SH"),
            "low liquidity / near-limit should be excluded"
        );
    }

    #[test]
    fn empty_input() {
        let r = calculate_intraday_alerts(&MarketDataInput { snapshots: vec![] });
        assert!(r.items.is_empty());
    }
}

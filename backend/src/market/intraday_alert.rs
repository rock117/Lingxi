//! 日内 TopN：上涨趋势 / 超跌反弹。规范：`docs/intraday_alert.md`。

use std::cmp::Ordering;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::domain::{MarketDataInput, StockDailyData};
use super::session::SessionPhase;

const DELTA_MIN: f64 = 1.2;
const AMOUNT_MIN: f64 = 3.0e7;
const PRESELECT_K: usize = 150;
const LIMIT_PCT: f64 = 9.5;
const SCORE_MIN: f64 = 55.0;
const SCORE_MIN_OVERSOLD: f64 = 40.0;
const RS_CAP: f64 = 5.0;
/// 量价比 amount/R_i（元/百分点）log 映射下界 / 上界。
const VP_LO: f64 = 1.5e7;
const VP_HI: f64 = 5.0e8;
/// 超跌：相对近一年（或可用区间）最高点至少回撤 70%。
const DD_YEAR_MIN: f64 = -70.0;
/// 加载开市日数（约一年）；不足一年用已有日 K，但已收盘日须 ≥ `OVERSOLD_HIST_MIN`。
pub const OVERSOLD_WINDOW_DAYS: usize = 250;
const OVERSOLD_HIST_MIN: usize = 60;
/// 近 N 日（含当日）创出区间新低、大阴、放量。
const OVERSOLD_RECENT_DAYS: usize = 3;
/// 大阴线：单日涨跌幅 ≤ −10%。
const BIG_RED_PCT: f64 = -10.0;
/// 放量：相对此前均额 ≥ 2 倍。
const VOL_SPIKE_RATIO: f64 = 2.0;
/// 均额回看（不含近 3 日）最多日数。
const VOL_BASE_LOOKBACK: usize = 20;
const EPS: f64 = 1e-10;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertLayers {
    pub a: f64,
    pub b: f64,
    pub c: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertItem {
    pub symbol: String,
    /// 证券简称（推送前从 sc_stock 填充）
    #[serde(default)]
    pub name: String,
    pub score: f64,
    pub rs: f64,
    pub change_pct: f64,
    pub amount: f64,
    /// 相对近高回撤（百分点，多为负）；趋势列表为 0。
    #[serde(default)]
    pub drawdown_pct: f64,
    pub layers: AlertLayers,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntradayAlertResult {
    pub date: String,
    pub r_mkt: f64,
    pub sample_size: usize,
    pub candidate_size: usize,
    /// 盘中相位：`open` / `am_active` / … / `close` / `off`
    #[serde(default)]
    pub session_phase: String,
    pub items: Vec<AlertItem>,
}

/// 调试 API：两类 TopN 一次返回。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntradayAlertBundle {
    pub trend: IntradayAlertResult,
    pub oversold: IntradayAlertResult,
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

/// 单位涨幅成交额：amount / R_i（元/百分点），越大越好。
fn score_vp(s: &StockDailyData) -> f64 {
    let chg = s.change_pct.max(EPS);
    let vp = s.amount.max(1.0) / chg;
    let lo = VP_LO.ln();
    let hi = VP_HI.ln();
    100.0 * clip01((vp.ln() - lo) / (hi - lo).max(EPS))
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
    let c5 = score_vp(s);

    0.20 * c1 + 0.20 * c2 + 0.15 * c3 + 0.10 * c4 + 0.35 * c5
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

fn empty_result(date: String) -> IntradayAlertResult {
    IntradayAlertResult {
        date,
        r_mkt: 0.0,
        sample_size: 0,
        candidate_size: 0,
        session_phase: SessionPhase::now().as_str().into(),
        items: vec![],
    }
}

fn r_mkt_of(valid: &[&StockDailyData]) -> f64 {
    let mkt_sample: Vec<&StockDailyData> = valid
        .iter()
        .copied()
        .filter(|s| s.change_pct < LIMIT_PCT && s.change_pct > -LIMIT_PCT)
        .collect();
    if mkt_sample.is_empty() {
        0.0
    } else {
        mkt_sample.iter().map(|s| s.change_pct).sum::<f64>() / mkt_sample.len() as f64
    }
}

fn build_hist_map(input: &MarketDataInput) -> HashMap<String, Vec<&StockDailyData>> {
    let mut hist_map: HashMap<String, Vec<&StockDailyData>> = HashMap::new();
    let hist_days = input.snapshots.len().saturating_sub(1);
    for snap in input.snapshots.iter().take(hist_days) {
        for s in &snap.stocks {
            hist_map.entry(s.symbol.clone()).or_default().push(s);
        }
    }
    hist_map
}

/// 已收盘日 K；不足 `OVERSOLD_HIST_MIN` 返回 `None`（上市不足约 60 日）。
fn oversold_hist<'a>(hist: Option<&'a [&'a StockDailyData]>) -> Option<&'a [&'a StockDailyData]> {
    hist.filter(|b| b.len() >= OVERSOLD_HIST_MIN)
}

/// 相对窗口内最高价回撤（百分点）。不足一年则用已有日 K 的最高点。
fn year_drawdown_pct(s: &StockDailyData, hist: &[&StockDailyData]) -> Option<f64> {
    let peak = hist
        .iter()
        .map(|b| b.high)
        .chain(std::iter::once(s.high))
        .fold(f64::NEG_INFINITY, f64::max);
    if !peak.is_finite() || peak <= EPS {
        return None;
    }
    Some((s.close_price / peak - 1.0) * 100.0)
}

/// 近 3 日（已收盘）或当日价/低点创出整段回看新低。
fn broke_recent_low(s: &StockDailyData, hist: &[&StockDailyData]) -> bool {
    let n = hist.len();
    let take = OVERSOLD_RECENT_DAYS.min(n);
    let recent = &hist[n - take..];
    let period_min = hist
        .iter()
        .map(|b| b.low)
        .chain([s.low, s.close_price])
        .fold(f64::INFINITY, f64::min);
    if !period_min.is_finite() {
        return false;
    }
    let recent_min = recent
        .iter()
        .map(|b| b.low)
        .chain([s.low, s.close_price])
        .fold(f64::INFINITY, f64::min);
    recent_min <= period_min + EPS
}

/// 近 3 日或当日出现 ≤ −10% 大阴。
fn has_big_red(s: &StockDailyData, hist: &[&StockDailyData]) -> bool {
    if s.change_pct <= BIG_RED_PCT {
        return true;
    }
    let n = hist.len();
    let take = OVERSOLD_RECENT_DAYS.min(n);
    hist[n - take..].iter().any(|b| b.change_pct <= BIG_RED_PCT)
}

/// 近 3 日或当日成交额 ≥ 2× 此前均额（均额取近 3 日之前最多 20 日）。
fn vol_spike_ratio(s: &StockDailyData, hist: &[&StockDailyData]) -> Option<f64> {
    let n = hist.len();
    let recent_n = OVERSOLD_RECENT_DAYS.min(n);
    let base_end = n.saturating_sub(recent_n);
    if base_end == 0 {
        return None;
    }
    let base_start = base_end.saturating_sub(VOL_BASE_LOOKBACK);
    let base = &hist[base_start..base_end];
    if base.is_empty() {
        return None;
    }
    let avg = base.iter().map(|b| b.amount).sum::<f64>() / base.len() as f64;
    if avg <= EPS {
        return None;
    }
    let recent_max = hist[n - recent_n..]
        .iter()
        .map(|b| b.amount)
        .chain(std::iter::once(s.amount))
        .fold(0.0_f64, f64::max);
    Some(recent_max / avg)
}

fn passes_oversold_filters(
    s: &StockDailyData,
    hist: &[&StockDailyData],
) -> Option<(f64, f64)> {
    let dd = year_drawdown_pct(s, hist)?;
    if dd > DD_YEAR_MIN {
        return None;
    }
    if !broke_recent_low(s, hist) {
        return None;
    }
    if !has_big_red(s, hist) {
        return None;
    }
    let vol_r = vol_spike_ratio(s, hist)?;
    if vol_r < VOL_SPIKE_RATIO {
        return None;
    }
    Some((dd, vol_r))
}

/// A：年高回撤深度（≥70% 起计分，越深越高）。
fn score_oversold_a(dd: f64) -> f64 {
    let depth = (-dd).max(0.0);
    if depth < 70.0 {
        return 0.0;
    }
    // 70% → 55，85% → 100
    55.0 + 45.0 * clip01((depth - 70.0) / 15.0)
}

/// B：当日是否贴近区间新低（越贴新低越高）。
fn score_oversold_b(s: &StockDailyData, hist: &[&StockDailyData]) -> f64 {
    let period_min = hist
        .iter()
        .map(|b| b.low)
        .chain([s.low])
        .fold(f64::INFINITY, f64::min);
    if !period_min.is_finite() || period_min <= EPS {
        return 50.0;
    }
    let px = s.close_price.min(s.low);
    let premium = (px / period_min - 1.0) * 100.0;
    if premium <= 0.5 {
        100.0
    } else if premium <= 2.0 {
        85.0
    } else if premium <= 5.0 {
        65.0
    } else {
        40.0
    }
}

/// C：放量倍数（2×→60，4×→100）。
fn score_oversold_c(vol_r: f64) -> f64 {
    if vol_r < VOL_SPIKE_RATIO {
        return 0.0;
    }
    60.0 + 40.0 * clip01((vol_r - VOL_SPIKE_RATIO) / 2.0)
}

/// 上涨趋势 TopN。
pub fn calculate_intraday_alerts(input: &MarketDataInput) -> IntradayAlertResult {
    calculate_intraday_alerts_at(input, SessionPhase::now())
}

pub fn calculate_intraday_alerts_at(
    input: &MarketDataInput,
    phase: SessionPhase,
) -> IntradayAlertResult {
    let Some(today) = input.snapshots.last() else {
        return empty_result(String::new());
    };
    let date = today.date.clone();
    let phase_s = phase.as_str().to_string();
    let pw = phase.trend_weights();
    let score_min = SCORE_MIN + pw.score_floor_boost;

    let valid: Vec<&StockDailyData> = today.stocks.iter().filter(|s| valid_stock(s)).collect();
    let sample_size = valid.len();
    if sample_size == 0 {
        let mut r = empty_result(date);
        r.session_phase = phase_s;
        return r;
    }

    let r_mkt = r_mkt_of(&valid);
    let hist_map = build_hist_map(input);

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
        let score = pw.w_a * a + pw.w_b * b + pw.w_c * c;
        scored.push(AlertItem {
            symbol: s.symbol.clone(),
            name: String::new(),
            score,
            rs,
            change_pct: s.change_pct,
            amount: s.amount,
            drawdown_pct: 0.0,
            layers: AlertLayers { a, b, c },
            reason: format!("{} phase={}", build_reason(a, b, c, rs), phase.as_str()),
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
        .filter(|i| i.score >= score_min)
        .take(10)
        .collect();

    IntradayAlertResult {
        date,
        r_mkt,
        sample_size,
        candidate_size,
        session_phase: phase_s,
        items,
    }
}

/// 超跌反弹 TopN：年高回撤 ≥70% + 近 3 日/现价破新低 + 大阴 −10%+ + 放量 ≥2×。
pub fn calculate_oversold_alerts(input: &MarketDataInput) -> IntradayAlertResult {
    calculate_oversold_alerts_at(input, SessionPhase::now())
}

pub fn calculate_oversold_alerts_at(
    input: &MarketDataInput,
    phase: SessionPhase,
) -> IntradayAlertResult {
    let Some(today) = input.snapshots.last() else {
        return empty_result(String::new());
    };
    let date = today.date.clone();
    let phase_s = phase.as_str().to_string();
    let pw = phase.oversold_weights();
    let score_min = SCORE_MIN_OVERSOLD + pw.score_floor_boost;

    let valid: Vec<&StockDailyData> = today.stocks.iter().filter(|s| valid_stock(s)).collect();
    let sample_size = valid.len();
    if sample_size == 0 {
        let mut r = empty_result(date);
        r.session_phase = phase_s;
        return r;
    }

    let r_mkt = r_mkt_of(&valid);
    let hist_map = build_hist_map(input);

    let mut scored: Vec<AlertItem> = Vec::new();
    for s in &valid {
        let Some(hist) = oversold_hist(hist_map.get(&s.symbol).map(|v| v.as_slice())) else {
            continue;
        };
        let Some((dd, vol_r)) = passes_oversold_filters(s, hist) else {
            continue;
        };
        let rs = s.change_pct - r_mkt;
        let a = score_oversold_a(dd);
        let b = score_oversold_b(s, hist);
        let c = score_oversold_c(vol_r);
        let score = pw.w_a * a + pw.w_b * b + pw.w_c * c;
        scored.push(AlertItem {
            symbol: s.symbol.clone(),
            name: String::new(),
            score,
            rs,
            change_pct: s.change_pct,
            amount: s.amount,
            drawdown_pct: dd,
            layers: AlertLayers { a, b, c },
            reason: format!(
                "DD={:+.1}% vol={:.1}x R={:+.1}% phase={}",
                dd,
                vol_r,
                s.change_pct,
                phase.as_str()
            ),
        });
    }
    let candidate_size = scored.len();

    scored.sort_by(|x, y| {
        y.score
            .partial_cmp(&x.score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| {
                x.drawdown_pct
                    .partial_cmp(&y.drawdown_pct)
                    .unwrap_or(Ordering::Equal)
            })
            .then_with(|| y.amount.partial_cmp(&x.amount).unwrap_or(Ordering::Equal))
    });

    let items: Vec<AlertItem> = scored
        .into_iter()
        .filter(|i| i.score >= score_min)
        .take(10)
        .collect();

    IntradayAlertResult {
        date,
        r_mkt,
        sample_size,
        candidate_size,
        session_phase: phase_s,
        items,
    }
}

pub fn calculate_alert_bundle(input: &MarketDataInput) -> IntradayAlertBundle {
    let phase = SessionPhase::now();
    IntradayAlertBundle {
        trend: calculate_intraday_alerts_at(input, phase),
        oversold: calculate_oversold_alerts_at(input, phase),
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
    fn volume_price_prefers_heavier_amount_same_rise() {
        // 同涨幅：成交额更大 → C（含量价比）更高
        let light = bar("L.SH", 2.0, 5.0e7, 10.0, 10.3, 9.9, 10.2);
        let heavy = bar("H.SH", 2.0, 4.0e8, 10.0, 10.3, 9.9, 10.2);
        assert!(score_vp(&heavy) > score_vp(&light));
        assert!(score_c(&heavy) > score_c(&light));
    }

    fn push_day(snaps: &mut Vec<MarketDailySnapshot>, day: usize, stocks: Vec<StockDailyData>) {
        snaps.push(MarketDailySnapshot {
            date: format!("2025-{:03}", day),
            stocks,
        });
    }

    /// 构造：峰值 100 → 现价约 25（−75%），近 3 日破新低 + 大阴 + 放量。
    #[test]
    fn oversold_year_crash_new_low_big_red_vol() {
        let mut snaps = Vec::new();
        // 日 1：高点 100；其后缓跌；末三日砸出新低并放量
        for d in 1..=60 {
            let (px, chg, amt) = if d == 1 {
                (100.0, 0.0, 5.0e7)
            } else if d <= 57 {
                let px = 100.0 - (d as f64 - 1.0) * 1.0; // 缓跌到 ~43
                (px.max(40.0), -1.0, 5.0e7)
            } else if d == 58 {
                (35.0, -12.0, 5.0e7) // 大阴但未放量
            } else if d == 59 {
                (30.0, -14.0, 5.0e7)
            } else {
                (28.0, -6.0, 5.0e7)
            };
            push_day(
                &mut snaps,
                d,
                vec![
                    hist_bar("CRASH.SH", &format!("2025-{:03}", d), chg, px, amt),
                    hist_bar("MILD.SH", &format!("2025-{:03}", d), -0.5, 90.0 - d as f64 * 0.1, 5.0e7),
                ],
            );
        }
        // 当日：再破新低 + 大阴 + 2× 放量
        snaps.push(MarketDailySnapshot {
            date: "2025-061".into(),
            stocks: vec![
                bar("CRASH.SH", -11.0, 1.2e8, 28.0, 28.5, 24.5, 25.0),
                bar("MILD.SH", -2.0, 5.0e7, 80.0, 80.5, 78.0, 78.5),
            ],
        });
        let r = calculate_oversold_alerts(&MarketDataInput { snapshots: snaps });
        assert!(
            r.items.iter().any(|i| i.symbol == "CRASH.SH"),
            "year crash + new low + big red + vol should hit: {:?}",
            r.items
        );
        assert!(
            r.items.iter().all(|i| i.symbol != "MILD.SH"),
            "mild drawdown must be excluded"
        );
        let hit = r.items.iter().find(|i| i.symbol == "CRASH.SH").unwrap();
        assert!(hit.drawdown_pct <= -70.0, "dd={}", hit.drawdown_pct);
    }

    #[test]
    fn empty_input() {
        let r = calculate_intraday_alerts(&MarketDataInput { snapshots: vec![] });
        assert!(r.items.is_empty());
        let o = calculate_oversold_alerts(&MarketDataInput { snapshots: vec![] });
        assert!(o.items.is_empty());
    }
}

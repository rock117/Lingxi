//! MSI 计算引擎 —— 严格按 `docs/market_sentiment.md` 伪代码实现。
//! 禁止只改本文件不改文档。

use super::domain::*;

// ============================================================
// 规范常量（文档 §2）
// ============================================================

const EWMA_ALPHA: f64 = 0.45;
const RET_SCALE: f64 = 2.0;
const STREAK_TAU: f64 = 3.0;

const W_DIRECTION: f64 = 0.50;
const W_INTENSITY: f64 = 0.25;
const W_CONSTRAINT: f64 = 0.25;

const W_DIR: f64 = 0.20; // 方向层 5 分量等权

const W_INT_VOLUME: f64 = 0.70;
const W_INT_LIMIT: f64 = 0.30;

const W_CON_RANGE: f64 = 0.45;
const W_CON_INTRA: f64 = 0.20;
const W_CON_INERTIA: f64 = 0.35;

const MSI_HIGH: f64 = 65.0;
const MSI_LOW: f64 = 35.0;
const MSI_GREY_BULL_LO: f64 = 60.0;
const MSI_GREY_BULL_HI: f64 = 65.0;
const MSI_GREY_BEAR_LO: f64 = 35.0;
const MSI_GREY_BEAR_HI: f64 = 40.0;

const VOL_HIGH: f64 = 1.5;
const VOL_LOW: f64 = 0.5;
const VOL_GREY_HIGH_LO: f64 = 1.3;
const VOL_GREY_HIGH_HI: f64 = 1.5;

const PRICE_HIGH: f64 = 0.8;
const PRICE_LOW: f64 = 0.2;

const LIMIT_PCT: f64 = 9.5;
const EPS_AMOUNT: f64 = 1e-10;
const EPS_PRICE: f64 = 1e-10;

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

fn ewma(values: &[f64], alpha: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut acc = values[0];
    for &v in &values[1..] {
        acc = alpha * v + (1.0 - alpha) * acc;
    }
    acc
}

fn soft_streak(days: usize) -> f64 {
    ((days as f64) / STREAK_TAU).tanh()
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

fn filter_snapshot(snap: &MarketDailySnapshot) -> MarketDailySnapshot {
    MarketDailySnapshot {
        date: snap.date.clone(),
        stocks: snap
            .stocks
            .iter()
            .filter(|s| is_valid_stock(s))
            .cloned()
            .collect(),
    }
}

fn filter_snapshots(snapshots: &[MarketDailySnapshot]) -> Vec<MarketDailySnapshot> {
    snapshots.iter().map(filter_snapshot).collect()
}

fn find_stock<'a>(snap: &'a MarketDailySnapshot, symbol: &str) -> Option<&'a StockDailyData> {
    snap.stocks.iter().find(|s| s.symbol == symbol)
}

fn collect_all_symbols(snapshots: &[MarketDailySnapshot]) -> Vec<String> {
    let mut symbols = Vec::new();
    for snap in snapshots {
        for s in &snap.stocks {
            if !symbols.contains(&s.symbol) {
                symbols.push(s.symbol.clone());
            }
        }
    }
    symbols
}

// ============================================================
// Direction
// ============================================================

fn daily_breadth(snapshot: &MarketDailySnapshot) -> f64 {
    let up = snapshot.stocks.iter().filter(|s| s.change_pct > 0.0).count();
    let down = snapshot.stocks.iter().filter(|s| s.change_pct < 0.0).count();
    let denom = (up + down) as f64;
    if denom < 1.0 {
        return 0.0;
    }
    (up as f64 - down as f64) / denom
}

fn calc_breadth_score(snapshots: &[MarketDailySnapshot]) -> f64 {
    map_to_score(ewma(
        &snapshots.iter().map(daily_breadth).collect::<Vec<_>>(),
        EWMA_ALPHA,
    ))
}

fn daily_money_flow(snapshot: &MarketDailySnapshot) -> f64 {
    let mut up_amount = 0.0;
    let mut down_amount = 0.0;
    let mut total_amount = 0.0;
    for s in &snapshot.stocks {
        total_amount += s.amount;
        if s.change_pct > 0.0 {
            up_amount += s.amount;
        } else if s.change_pct < 0.0 {
            down_amount += s.amount;
        }
    }
    if total_amount < EPS_AMOUNT {
        return 0.0;
    }
    (up_amount - down_amount) / total_amount
}

fn calc_money_flow_score(snapshots: &[MarketDailySnapshot]) -> f64 {
    map_to_score(ewma(
        &snapshots.iter().map(daily_money_flow).collect::<Vec<_>>(),
        EWMA_ALPHA,
    ))
}

fn daily_gap_breadth(prev: &MarketDailySnapshot, curr: &MarketDailySnapshot) -> f64 {
    let mut up = 0usize;
    let mut down = 0usize;
    for s in &curr.stocks {
        let Some(prev_s) = find_stock(prev, &s.symbol) else {
            continue;
        };
        if prev_s.close_price <= 0.0 {
            continue;
        }
        let gap = s.open - prev_s.close_price;
        if gap > 0.0 {
            up += 1;
        } else if gap < 0.0 {
            down += 1;
        }
    }
    let denom = (up + down) as f64;
    if denom < 1.0 {
        return 0.0;
    }
    (up as f64 - down as f64) / denom
}

fn calc_gap_breadth_score(snapshots: &[MarketDailySnapshot]) -> (f64, f64) {
    if snapshots.len() < 2 {
        return (50.0, 0.0);
    }
    let series: Vec<f64> = (1..snapshots.len())
        .map(|i| daily_gap_breadth(&snapshots[i - 1], &snapshots[i]))
        .collect();
    let last = *series.last().unwrap_or(&0.0);
    (map_to_score(ewma(&series, EWMA_ALPHA)), last)
}

/// 结构确认 §5.4：majority=breadth_d，index_proxy=tanh(mean_ret/RET_SCALE)
fn daily_structure(snapshot: &MarketDailySnapshot) -> f64 {
    if snapshot.stocks.is_empty() {
        return 0.0;
    }
    let majority = daily_breadth(snapshot);
    let mean_ret = mean(
        &snapshot
            .stocks
            .iter()
            .map(|s| s.change_pct)
            .collect::<Vec<_>>(),
    );
    let index_proxy = (mean_ret / RET_SCALE).tanh();
    (majority + index_proxy) / 2.0
}

fn calc_structure_score(snapshots: &[MarketDailySnapshot]) -> (f64, f64) {
    let series: Vec<f64> = snapshots.iter().map(daily_structure).collect();
    let last = *series.last().unwrap_or(&0.0);
    (map_to_score(ewma(&series, EWMA_ALPHA)), last)
}

/// 窗口内某股 high/low 极值
fn window_extremes(
    symbol: &str,
    window: &[MarketDailySnapshot],
) -> Option<(f64, f64)> {
    let mut hi = f64::NEG_INFINITY;
    let mut lo = f64::INFINITY;
    let mut any = false;
    for snap in window {
        if let Some(s) = find_stock(snap, symbol) {
            any = true;
            hi = hi.max(s.high);
            lo = lo.min(s.low);
        }
    }
    if !any {
        return None;
    }
    Some((hi, lo))
}

fn daily_nh_nl(window: &[MarketDailySnapshot], today: &MarketDailySnapshot) -> (f64, usize, usize) {
    let mut nh = 0usize;
    let mut nl = 0usize;
    for s in &today.stocks {
        let Some((hi, lo)) = window_extremes(&s.symbol, window) else {
            continue;
        };
        if s.high + EPS_PRICE >= hi {
            nh += 1;
        }
        if s.low - EPS_PRICE <= lo {
            nl += 1;
        }
    }
    let denom = (nh + nl) as f64;
    let raw = if denom < 1.0 {
        0.0
    } else {
        (nh as f64 - nl as f64) / denom
    };
    (raw, nh, nl)
}

fn calc_nh_nl_score(snapshots: &[MarketDailySnapshot]) -> (f64, f64, usize, usize) {
    if snapshots.len() < 2 {
        return (50.0, 0.0, 0, 0);
    }
    let mut series = Vec::new();
    let mut last_nh = 0usize;
    let mut last_nl = 0usize;
    let mut last_raw = 0.0;
    for i in 1..snapshots.len() {
        let window = &snapshots[..=i];
        let (raw, nh, nl) = daily_nh_nl(window, &snapshots[i]);
        series.push(raw);
        last_raw = raw;
        last_nh = nh;
        last_nl = nl;
    }
    (
        map_to_score(ewma(&series, EWMA_ALPHA)),
        last_raw,
        last_nh,
        last_nl,
    )
}

// ============================================================
// Intensity
// ============================================================

fn daily_total_amount(snapshot: &MarketDailySnapshot) -> f64 {
    snapshot.stocks.iter().map(|s| s.amount).sum()
}

fn volume_level_raw(volume_ratio: f64) -> f64 {
    if volume_ratio <= 0.0 || !volume_ratio.is_finite() {
        return -1.0;
    }
    (volume_ratio.ln() / 2f64.ln()).tanh()
}

fn calc_directed_volume(snapshots: &[MarketDailySnapshot]) -> (f64, f64) {
    if snapshots.is_empty() {
        return (50.0, 1.0);
    }
    let daily_amounts: Vec<f64> = snapshots.iter().map(daily_total_amount).collect();
    let avg_amount = mean(&daily_amounts);
    let last_amount = *daily_amounts.last().unwrap();
    let volume_ratio = if avg_amount < EPS_AMOUNT {
        1.0
    } else {
        last_amount / avg_amount
    };
    let level = volume_level_raw(volume_ratio);
    let breadth_last = daily_breadth(snapshots.last().unwrap());
    let score = map_to_score((level * breadth_last).clamp(-1.0, 1.0));
    (score, volume_ratio)
}

fn calc_limit_pressure_score(last: &MarketDailySnapshot) -> f64 {
    let n = last.stocks.len().max(1) as f64;
    let limit_up = last
        .stocks
        .iter()
        .filter(|s| s.change_pct >= LIMIT_PCT)
        .count() as f64;
    let limit_down = last
        .stocks
        .iter()
        .filter(|s| s.change_pct <= -LIMIT_PCT)
        .count() as f64;
    map_to_score(((limit_up - limit_down) / n).clamp(-1.0, 1.0))
}

// ============================================================
// Constraint
// ============================================================

fn calc_stock_range_position(symbol: &str, snapshots: &[MarketDailySnapshot]) -> Option<f64> {
    let mut highs = Vec::new();
    let mut lows = Vec::new();
    let mut last_close = None;
    for snap in snapshots {
        if let Some(st) = find_stock(snap, symbol) {
            highs.push(st.high);
            lows.push(st.low);
            last_close = Some(st.close_price);
        }
    }
    if highs.len() < 2 {
        return None;
    }
    let high_n = highs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let low_n = lows.iter().cloned().fold(f64::INFINITY, f64::min);
    let current = last_close?;
    if (high_n - low_n).abs() < EPS_PRICE {
        return Some(0.5);
    }
    Some(((current - low_n) / (high_n - low_n)).clamp(0.0, 1.0))
}

fn calc_range_position(snapshots: &[MarketDailySnapshot]) -> (f64, f64) {
    if snapshots.is_empty() {
        return (50.0, 0.5);
    }
    let positions: Vec<f64> = collect_all_symbols(snapshots)
        .iter()
        .filter_map(|sym| calc_stock_range_position(sym, snapshots))
        .collect();
    if positions.is_empty() {
        return (50.0, 0.5);
    }
    let raw = mean(&positions);
    (raw * 100.0, raw)
}

fn calc_intraday_position_score(last: &MarketDailySnapshot) -> f64 {
    let mut xs = Vec::new();
    for s in &last.stocks {
        if (s.high - s.low).abs() < EPS_PRICE {
            xs.push(0.5);
        } else {
            xs.push(((s.close_price - s.low) / (s.high - s.low)).clamp(0.0, 1.0));
        }
    }
    if xs.is_empty() {
        return 50.0;
    }
    mean(&xs) * 100.0
}

struct StreakStats {
    up: usize,
    down: usize,
    no_up: usize,
    no_down: usize,
}

fn stock_streaks(symbol: &str, snapshots: &[MarketDailySnapshot]) -> Option<StreakStats> {
    // 从末日向前收集该股 change_pct（倒序扫描）
    let mut changes = Vec::new();
    for snap in snapshots.iter().rev() {
        if let Some(s) = find_stock(snap, symbol) {
            changes.push(s.change_pct);
        } else if !changes.is_empty() {
            break; // 缺日中断
        }
    }
    if changes.is_empty() {
        return None;
    }

    let count_while = |pred: fn(f64) -> bool| -> usize {
        changes.iter().take_while(|&&c| pred(c)).count()
    };

    Some(StreakStats {
        up: count_while(|c| c > 0.0),
        down: count_while(|c| c < 0.0),
        no_up: count_while(|c| c <= 0.0),
        no_down: count_while(|c| c >= 0.0),
    })
}

fn calc_inertia(snapshots: &[MarketDailySnapshot]) -> (f64, f64, f64, f64, f64) {
    let last = match snapshots.last() {
        Some(s) if !s.stocks.is_empty() => s,
        _ => return (50.0, 0.0, 0.0, 0.0, 0.0),
    };

    let mut bulls = Vec::new();
    let mut bears = Vec::new();
    let mut sum_up = 0.0;
    let mut sum_down = 0.0;
    let mut sum_no_up = 0.0;
    let mut sum_no_down = 0.0;
    let mut n = 0.0;

    for s in &last.stocks {
        let Some(st) = stock_streaks(&s.symbol, snapshots) else {
            continue;
        };
        let bull_i = 0.5 * soft_streak(st.up) + 0.5 * soft_streak(st.no_down);
        let bear_i = 0.5 * soft_streak(st.down) + 0.5 * soft_streak(st.no_up);
        bulls.push(bull_i);
        bears.push(bear_i);
        sum_up += st.up as f64;
        sum_down += st.down as f64;
        sum_no_up += st.no_up as f64;
        sum_no_down += st.no_down as f64;
        n += 1.0;
    }

    if n < 1.0 {
        return (50.0, 0.0, 0.0, 0.0, 0.0);
    }

    let bull = mean(&bulls);
    let bear = mean(&bears);
    let raw = (bull - bear).clamp(-1.0, 1.0);
    (
        map_to_score(raw),
        sum_up / n,
        sum_down / n,
        sum_no_up / n,
        sum_no_down / n,
    )
}

// ============================================================
// 信号 / 诊断 / 入口
// ============================================================

fn classify_signal(msi_value: f64, volume_ratio: f64, price_position: f64) -> SignalType {
    let msi_high = msi_value > MSI_HIGH;
    let msi_low = msi_value < MSI_LOW;
    let msi_grey = (msi_value >= MSI_GREY_BULL_LO && msi_value <= MSI_GREY_BULL_HI)
        || (msi_value >= MSI_GREY_BEAR_LO && msi_value <= MSI_GREY_BEAR_HI);
    let vol_high = volume_ratio >= VOL_HIGH;
    let vol_low = volume_ratio < VOL_LOW;
    let vol_grey_high =
        volume_ratio >= VOL_GREY_HIGH_LO && volume_ratio < VOL_GREY_HIGH_HI;
    let price_high = price_position >= PRICE_HIGH;
    let price_low = price_position < PRICE_LOW;

    if msi_grey || vol_grey_high {
        return SignalType::Neutral;
    }
    if msi_high && vol_high && !price_high {
        SignalType::StrongBullish
    } else if msi_high && vol_high && price_high {
        SignalType::SurgeWarning
    } else if msi_high && vol_low && price_high {
        SignalType::DivergenceWarning
    } else if msi_high && vol_low && !price_high {
        SignalType::WeakBullish
    } else if msi_low && vol_high && price_low {
        SignalType::PanicSelling
    } else if msi_low && vol_low && price_low {
        SignalType::Bottoming
    } else if msi_low && vol_high && !price_low {
        SignalType::VolumeDecline
    } else if !msi_high && !msi_low && vol_high && price_low {
        SignalType::LowVolumeAccumulation
    } else {
        SignalType::Neutral
    }
}

fn build_signal(msi_value: f64, volume_ratio: f64, price_position: f64) -> MsiSignal {
    let signal_type = classify_signal(msi_value, volume_ratio, price_position);
    MsiSignal {
        label: signal_type.label().to_string(),
        advice: signal_type.advice().to_string(),
        signal_type,
        volume_status: VolumeStatus::from_ratio(volume_ratio),
        volume_ratio,
        price_status: PriceStatus::from_position(price_position),
        price_position,
    }
}

fn empty_diagnostics() -> MsiDiagnostics {
    MsiDiagnostics {
        sample_size: 0,
        up_count: 0,
        down_count: 0,
        flat_count: 0,
        limit_up_count: 0,
        limit_down_count: 0,
        breadth_last: 0.0,
        money_flow_last: 0.0,
        gap_breadth_last: 0.0,
        structure_last: 0.0,
        nh_nl_last: 0.0,
        new_high_count: 0,
        new_low_count: 0,
        avg_up_streak: 0.0,
        avg_down_streak: 0.0,
        avg_no_up_streak: 0.0,
        avg_no_down_streak: 0.0,
    }
}

fn neutral_result(days: usize) -> MsiResult {
    MsiResult {
        value: 50.0,
        layers: MsiLayers {
            direction: 50.0,
            intensity: 50.0,
            constraint: 50.0,
        },
        components: MsiComponents {
            breadth: 50.0,
            money_flow: 50.0,
            gap_breadth: 50.0,
            structure: 50.0,
            nh_nl: 50.0,
            directed_volume: 50.0,
            volume_ratio: 1.0,
            limit_pressure: 50.0,
            range_position: 50.0,
            intraday_position: 50.0,
            inertia: 50.0,
        },
        days,
        signal: build_signal(50.0, 1.0, 0.5),
        diagnostics: empty_diagnostics(),
        calculated_at: chrono::Utc::now().to_rfc3339(),
    }
}

pub fn calculate_msi(input: &MarketDataInput) -> MsiResult {
    let snapshots = filter_snapshots(&input.snapshots);
    let days = snapshots.len();
    if days == 0 || snapshots.iter().all(|s| s.stocks.is_empty()) {
        return neutral_result(input.snapshots.len());
    }

    let breadth = calc_breadth_score(&snapshots);
    let money_flow = calc_money_flow_score(&snapshots);
    let (gap_breadth, gap_breadth_last) = calc_gap_breadth_score(&snapshots);
    let (structure, structure_last) = calc_structure_score(&snapshots);
    let (nh_nl, nh_nl_last, new_high_count, new_low_count) = calc_nh_nl_score(&snapshots);

    let direction = W_DIR
        * (breadth + money_flow + gap_breadth + structure + nh_nl);

    let (directed_volume, volume_ratio) = calc_directed_volume(&snapshots);
    let limit_pressure = calc_limit_pressure_score(snapshots.last().unwrap());
    let intensity = W_INT_VOLUME * directed_volume + W_INT_LIMIT * limit_pressure;

    let (range_position_score, range_position_raw) = calc_range_position(&snapshots);
    let intraday_position = calc_intraday_position_score(snapshots.last().unwrap());
    let (inertia, avg_up, avg_down, avg_no_up, avg_no_down) = calc_inertia(&snapshots);
    let constraint = W_CON_RANGE * range_position_score
        + W_CON_INTRA * intraday_position
        + W_CON_INERTIA * inertia;

    let value =
        W_DIRECTION * direction + W_INTENSITY * intensity + W_CONSTRAINT * constraint;

    let last = snapshots.last().unwrap();
    let mut up = 0usize;
    let mut down = 0usize;
    let mut flat = 0usize;
    let mut limit_up = 0usize;
    let mut limit_down = 0usize;
    for s in &last.stocks {
        if s.change_pct > 0.0 {
            up += 1;
        } else if s.change_pct < 0.0 {
            down += 1;
        } else {
            flat += 1;
        }
        if s.change_pct >= LIMIT_PCT {
            limit_up += 1;
        } else if s.change_pct <= -LIMIT_PCT {
            limit_down += 1;
        }
    }

    let signal = build_signal(value, volume_ratio, range_position_raw);

    MsiResult {
        value,
        layers: MsiLayers {
            direction,
            intensity,
            constraint,
        },
        components: MsiComponents {
            breadth,
            money_flow,
            gap_breadth,
            structure,
            nh_nl,
            directed_volume,
            volume_ratio,
            limit_pressure,
            range_position: range_position_score,
            intraday_position,
            inertia,
        },
        days,
        signal,
        diagnostics: MsiDiagnostics {
            sample_size: last.stocks.len(),
            up_count: up,
            down_count: down,
            flat_count: flat,
            limit_up_count: limit_up,
            limit_down_count: limit_down,
            breadth_last: daily_breadth(last),
            money_flow_last: daily_money_flow(last),
            gap_breadth_last,
            structure_last,
            nh_nl_last,
            new_high_count,
            new_low_count,
            avg_up_streak: avg_up,
            avg_down_streak: avg_down,
            avg_no_up_streak: avg_no_up,
            avg_no_down_streak: avg_no_down,
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

    fn bar(
        sym: &str,
        change: f64,
        amount: f64,
        open: f64,
        high: f64,
        low: f64,
        close: f64,
    ) -> (&str, f64, f64, f64, f64, f64, f64) {
        (sym, change, amount, open, high, low, close)
    }

    fn make_snapshot(
        date: &str,
        stocks: Vec<(&str, f64, f64, f64, f64, f64, f64)>,
    ) -> MarketDailySnapshot {
        MarketDailySnapshot {
            date: date.to_string(),
            stocks: stocks
                .into_iter()
                .map(|(sym, change, amount, open, high, low, close)| StockDailyData {
                    symbol: sym.to_string(),
                    date: date.to_string(),
                    change_pct: change,
                    amount,
                    open,
                    high,
                    low,
                    close_price: close,
                })
                .collect(),
        }
    }

    #[test]
    fn test_invalid_ohlc_filtered() {
        let snap = make_snapshot(
            "2026-01-01",
            vec![
                bar("OK", 1.0, 100.0, 10.0, 10.5, 9.5, 10.2),
                // high < close → 非法
                ("BAD", 1.0, 100.0, 10.0, 10.0, 9.5, 10.5),
            ],
        );
        let r = calculate_msi(&MarketDataInput {
            snapshots: vec![snap],
        });
        assert_eq!(r.diagnostics.sample_size, 1);
    }

    #[test]
    fn test_breadth_flat_excluded_from_denom() {
        let snap = make_snapshot(
            "2026-01-01",
            vec![
                bar("A", 1.0, 100.0, 10.0, 10.2, 9.8, 10.1),
                bar("B", -1.0, 100.0, 10.0, 10.1, 9.7, 9.9),
                bar("C", 0.0, 100.0, 10.0, 10.1, 9.9, 10.0),
            ],
        );
        // up=1,down=1 → breadth=0（平盘不进分母）
        assert!((daily_breadth(&snap) - 0.0).abs() < 1e-12);
    }

    #[test]
    fn test_gap_all_up_high_score() {
        let d0 = make_snapshot(
            "2026-01-01",
            vec![
                bar("A", 0.0, 100.0, 10.0, 10.2, 9.8, 10.0),
                bar("B", 0.0, 100.0, 20.0, 20.2, 19.8, 20.0),
            ],
        );
        let d1 = make_snapshot(
            "2026-01-02",
            vec![
                bar("A", 1.0, 100.0, 10.5, 10.8, 10.4, 10.6), // open > prev close
                bar("B", 1.0, 100.0, 20.8, 21.0, 20.7, 20.9),
            ],
        );
        let (score, last) = calc_gap_breadth_score(&[d0, d1]);
        assert!((last - 1.0).abs() < 1e-12);
        assert!(score > 70.0, "全高开 gap_breadth 应偏高, 实际 {}", score);
    }

    #[test]
    fn test_structure_divergence_near_neutral() {
        // 3 跌 1 大涨：宽度偏空，均值被拉正 → structure 靠近中性
        let snap = make_snapshot(
            "2026-01-01",
            vec![
                bar("A", -1.0, 100.0, 10.0, 10.1, 9.8, 9.9),
                bar("B", -1.0, 100.0, 10.0, 10.1, 9.8, 9.9),
                bar("C", -1.0, 100.0, 10.0, 10.1, 9.8, 9.9),
                bar("D", 8.0, 100.0, 10.0, 11.0, 10.0, 10.8),
            ],
        );
        let s = daily_structure(&snap);
        // majority = (1-3)/4 = -0.5; mean_ret = 5/4 = 1.25; proxy=tanh(1.25/2)≈0.55
        // structure ≈ (-0.5+0.55)/2 ≈ 0.025 → near 0
        assert!(s.abs() < 0.15, "背离应接近 0, 实际 {}", s);
        let (score, _) = calc_structure_score(&[snap]);
        assert!((score - 50.0).abs() < 10.0, "score={}", score);
    }

    #[test]
    fn test_nh_nl_mostly_new_highs() {
        let mut snaps = Vec::new();
        for i in 0..4 {
            let c = 10.0 + i as f64;
            snaps.push(make_snapshot(
                &format!("2026-01-0{}", i + 1),
                vec![
                    bar("A", 1.0, 100.0, c - 0.1, c, c - 0.2, c),
                    bar("B", 1.0, 100.0, c - 0.1, c, c - 0.2, c),
                ],
            ));
        }
        // 末日再创新高
        snaps.push(make_snapshot(
            "2026-01-05",
            vec![
                bar("A", 2.0, 100.0, 13.9, 14.5, 13.8, 14.2),
                bar("B", 2.0, 100.0, 13.9, 14.5, 13.8, 14.2),
            ],
        ));
        let (score, _raw, nh, nl) = calc_nh_nl_score(&snaps);
        assert!(nh >= 2 && nl == 0, "nh={} nl={}", nh, nl);
        assert!(score > 60.0, "多数新高 nh_nl 应 > 60, 实际 {}", score);
    }

    #[test]
    fn test_inertia_consecutive_down() {
        let snaps: Vec<_> = (0..5)
            .map(|i| {
                let c = 12.0 - i as f64;
                make_snapshot(
                    &format!("2026-01-0{}", i + 1),
                    vec![
                        bar("A", -2.0, 100.0, c + 0.2, c + 0.3, c - 0.1, c),
                        bar("B", -1.5, 100.0, c + 0.1, c + 0.2, c - 0.1, c - 0.05),
                    ],
                )
            })
            .collect();
        let (score, _u, avg_down, avg_no_up, _) = calc_inertia(&snaps);
        assert!(avg_down >= 4.0, "avg_down={}", avg_down);
        assert!(avg_no_up >= 4.0, "avg_no_up={}", avg_no_up);
        assert!(score < 45.0, "连跌 inertia 应 < 45, 实际 {}", score);
    }

    #[test]
    fn test_layer_weights() {
        let snaps: Vec<_> = (0..5)
            .map(|i| {
                make_snapshot(
                    &format!("2026-01-0{}", i + 1),
                    vec![
                        bar("A", 0.2, 100.0, 10.0, 10.3, 9.8, 10.0 + i as f64 * 0.05),
                        bar("B", -0.1, 100.0, 10.0, 10.2, 9.7, 10.0),
                    ],
                )
            })
            .collect();
        let r = calculate_msi(&MarketDataInput { snapshots: snaps });
        let expected = 0.5 * r.layers.direction
            + 0.25 * r.layers.intensity
            + 0.25 * r.layers.constraint;
        assert!((r.value - expected).abs() < 1e-6);
        assert!(r.components.structure >= 0.0 && r.components.nh_nl >= 0.0);
        assert!(r.components.inertia >= 0.0);
    }

    #[test]
    fn test_directed_volume_down_surge() {
        let mut snaps = Vec::new();
        for i in 0..4 {
            snaps.push(make_snapshot(
                &format!("2026-01-0{}", i + 1),
                vec![
                    bar("A", -0.5, 100.0, 10.0, 10.2, 9.8, 10.0),
                    bar("B", 0.5, 100.0, 10.0, 10.2, 9.8, 10.0),
                ],
            ));
        }
        snaps.push(make_snapshot(
            "2026-01-05",
            vec![
                bar("A", -3.0, 400.0, 10.0, 10.1, 9.0, 9.2),
                bar("B", -2.0, 400.0, 10.0, 10.1, 9.0, 9.3),
            ],
        ));
        let (score, ratio) = calc_directed_volume(&snaps);
        assert!(ratio > 1.5);
        assert!(score < 50.0);
    }

    #[test]
    fn test_classify_grey() {
        assert!(matches!(
            classify_signal(62.0, 1.8, 0.5),
            SignalType::Neutral
        ));
    }

    #[test]
    fn test_end_to_end_down() {
        let snaps: Vec<_> = (0..5)
            .map(|i| {
                let c = 14.0 - i as f64;
                make_snapshot(
                    &format!("2026-01-0{}", i + 1),
                    vec![
                        bar("A", -3.0, 100.0, c + 0.3, c + 0.4, c - 0.2, c),
                        bar("B", -2.0, 100.0, c + 0.2, c + 0.3, c - 0.2, c - 0.1),
                        bar("C", -2.5, 100.0, c + 0.2, c + 0.3, c - 0.3, c - 0.2),
                    ],
                )
            })
            .collect();
        let r = calculate_msi(&MarketDataInput { snapshots: snaps });
        assert!(r.value < 45.0, "连跌 MSI 应偏低, 实际 {}", r.value);
        assert!(r.components.inertia < 50.0);
    }
}

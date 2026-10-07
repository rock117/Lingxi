//! MSI 计算引擎 —— 严格实现 `docs/market_sentiment.md`
//!
//! 三层：Direction (50%) / Intensity (25%) / Constraint (25%)

use super::domain::*;

// ============================================================
// 规范常量 §2
// ============================================================

const EWMA_ALPHA: f64 = 0.45;

const W_DIRECTION: f64 = 0.50;
const W_INTENSITY: f64 = 0.25;
const W_CONSTRAINT: f64 = 0.25;

const W_DIR_BREADTH: f64 = 1.0 / 3.0;
const W_DIR_MONEY: f64 = 1.0 / 3.0;
const W_DIR_GAP: f64 = 1.0 / 3.0;

const W_INT_VOLUME: f64 = 0.70;
const W_INT_LIMIT: f64 = 0.30;

const W_CON_RANGE: f64 = 0.70;
const W_CON_INTRA: f64 = 0.30;

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

fn is_valid_stock(s: &StockDailyData) -> bool {
    let prices_ok = s.open > 0.0
        && s.high > 0.0
        && s.low > 0.0
        && s.close_price > 0.0
        && s.high >= s.low
        && s.high >= s.open
        && s.high >= s.close_price
        && s.low <= s.open
        && s.low <= s.close_price;
    prices_ok
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

fn find_prev_close(
    symbol: &str,
    prev: &MarketDailySnapshot,
) -> Option<f64> {
    prev.stocks
        .iter()
        .find(|s| s.symbol == symbol)
        .map(|s| s.close_price)
}

// ============================================================
// Direction 分量
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
    let xs: Vec<f64> = snapshots.iter().map(daily_breadth).collect();
    map_to_score(ewma(&xs, EWMA_ALPHA))
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
    let xs: Vec<f64> = snapshots.iter().map(daily_money_flow).collect();
    map_to_score(ewma(&xs, EWMA_ALPHA))
}

/// 单日缺口广度；`prev` 为前一日快照
fn daily_gap_breadth(prev: &MarketDailySnapshot, curr: &MarketDailySnapshot) -> f64 {
    let mut up = 0usize;
    let mut down = 0usize;
    for s in &curr.stocks {
        let Some(prev_close) = find_prev_close(&s.symbol, prev) else {
            continue;
        };
        if prev_close <= 0.0 {
            continue;
        }
        let gap = s.open - prev_close;
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
    let mut series = Vec::new();
    for i in 1..snapshots.len() {
        series.push(daily_gap_breadth(&snapshots[i - 1], &snapshots[i]));
    }
    let last = *series.last().unwrap_or(&0.0);
    let score = map_to_score(ewma(&series, EWMA_ALPHA));
    (score, last)
}

// ============================================================
// Intensity 分量
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
    let raw = (limit_up - limit_down) / n;
    map_to_score(raw.clamp(-1.0, 1.0))
}

// ============================================================
// Constraint 分量
// ============================================================

fn calc_stock_range_position(symbol: &str, snapshots: &[MarketDailySnapshot]) -> Option<f64> {
    let mut highs = Vec::new();
    let mut lows = Vec::new();
    let mut last_close = None;
    for snap in snapshots {
        if let Some(st) = snap.stocks.iter().find(|s| s.symbol == symbol) {
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
    let symbols = collect_all_symbols(snapshots);
    let positions: Vec<f64> = symbols
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

// ============================================================
// 诊断 / 信号
// ============================================================

fn build_diagnostics(
    last: &MarketDailySnapshot,
    gap_breadth_last: f64,
) -> MsiDiagnostics {
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
    MsiDiagnostics {
        sample_size: last.stocks.len(),
        up_count: up,
        down_count: down,
        flat_count: flat,
        limit_up_count: limit_up,
        limit_down_count: limit_down,
        breadth_last: daily_breadth(last),
        money_flow_last: daily_money_flow(last),
        gap_breadth_last,
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
    }
}

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
            directed_volume: 50.0,
            volume_ratio: 1.0,
            limit_pressure: 50.0,
            range_position: 50.0,
            intraday_position: 50.0,
        },
        days,
        signal: build_signal(50.0, 1.0, 0.5),
        diagnostics: empty_diagnostics(),
        calculated_at: chrono::Utc::now().to_rfc3339(),
    }
}

// ============================================================
// 入口
// ============================================================

pub fn calculate_msi(input: &MarketDataInput) -> MsiResult {
    let snapshots = filter_snapshots(&input.snapshots);
    let days = snapshots.len();

    if days == 0 || snapshots.iter().all(|s| s.stocks.is_empty()) {
        return neutral_result(input.snapshots.len());
    }

    // Direction
    let breadth = calc_breadth_score(&snapshots);
    let money_flow = calc_money_flow_score(&snapshots);
    let (gap_breadth, gap_breadth_last) = calc_gap_breadth_score(&snapshots);
    let direction =
        W_DIR_BREADTH * breadth + W_DIR_MONEY * money_flow + W_DIR_GAP * gap_breadth;

    // Intensity
    let (directed_volume, volume_ratio) = calc_directed_volume(&snapshots);
    let limit_pressure = calc_limit_pressure_score(snapshots.last().unwrap());
    let intensity = W_INT_VOLUME * directed_volume + W_INT_LIMIT * limit_pressure;

    // Constraint
    let (range_position_score, range_position_raw) = calc_range_position(&snapshots);
    let intraday_position = calc_intraday_position_score(snapshots.last().unwrap());
    let constraint =
        W_CON_RANGE * range_position_score + W_CON_INTRA * intraday_position;

    let value =
        W_DIRECTION * direction + W_INTENSITY * intensity + W_CONSTRAINT * constraint;

    let signal = build_signal(value, volume_ratio, range_position_raw);
    let diagnostics = build_diagnostics(snapshots.last().unwrap(), gap_breadth_last);

    tracing::info!(
        "MSI: value={:.1} dir={:.1} int={:.1} con={:.1} ratio={:.2} range={:.2} signal={}",
        value,
        direction,
        intensity,
        constraint,
        volume_ratio,
        range_position_raw,
        signal.label
    );

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
            directed_volume,
            volume_ratio,
            limit_pressure,
            range_position: range_position_score,
            intraday_position,
        },
        days,
        signal,
        diagnostics,
        calculated_at: chrono::Utc::now().to_rfc3339(),
    }
}

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// (sym, change, amount, open, high, low, close)
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
    fn test_filter_invalid_ohlc() {
        let snap = make_snapshot(
            "2026-01-01",
            vec![
                bar("A", 1.0, 10.0, 10.0, 11.0, 9.5, 10.5),
                bar("BAD", 1.0, 10.0, 10.0, 9.0, 11.0, 10.0), // high < low
            ],
        );
        let f = filter_snapshot(&snap);
        assert_eq!(f.stocks.len(), 1);
        assert_eq!(f.stocks[0].symbol, "A");
    }

    #[test]
    fn test_breadth_ignores_flat() {
        let snap = make_snapshot(
            "2026-01-01",
            vec![
                bar("A", 1.0, 10.0, 10.0, 11.0, 10.0, 10.5),
                bar("B", 2.0, 10.0, 10.0, 11.0, 10.0, 10.5),
                bar("C", 0.0, 10.0, 10.0, 10.2, 9.8, 10.0),
            ],
        );
        assert!((daily_breadth(&snap) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_gap_all_gap_up() {
        let d0 = make_snapshot(
            "2026-01-01",
            vec![
                bar("A", 0.0, 100.0, 10.0, 10.5, 9.5, 10.0),
                bar("B", 0.0, 100.0, 20.0, 20.5, 19.5, 20.0),
            ],
        );
        let d1 = make_snapshot(
            "2026-01-02",
            vec![
                bar("A", 1.0, 100.0, 10.5, 11.0, 10.4, 10.8), // open > prev close
                bar("B", 1.0, 100.0, 20.8, 21.0, 20.5, 20.9),
            ],
        );
        let (score, last) = calc_gap_breadth_score(&[d0, d1]);
        assert!((last - 1.0).abs() < 1e-6);
        assert!(score > 70.0, "全高开 gap_score 应偏高, 实际 {}", score);
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
        assert!(score < 50.0, "放量下跌 directed_volume 应 < 50, 实际 {}", score);
    }

    #[test]
    fn test_range_uses_high_low_not_only_close() {
        // close 一直在中间，但 high/low 拉大后末日 close 靠近 low → 低位
        let snaps = vec![
            make_snapshot(
                "2026-01-01",
                vec![bar("A", 0.0, 100.0, 10.0, 14.0, 10.0, 12.0)],
            ),
            make_snapshot(
                "2026-01-02",
                vec![bar("A", 0.0, 100.0, 12.0, 14.0, 10.0, 12.0)],
            ),
            make_snapshot(
                "2026-01-03",
                vec![bar("A", 0.0, 100.0, 12.0, 14.0, 10.0, 10.5)],
            ),
        ];
        let (score, raw) = calc_range_position(&snaps);
        // (10.5-10)/(14-10) = 0.125
        assert!((raw - 0.125).abs() < 1e-6, "raw={}", raw);
        assert!((score - 12.5).abs() < 1e-6);
    }

    #[test]
    fn test_intraday_position() {
        let snap = make_snapshot(
            "2026-01-01",
            vec![
                bar("A", 0.0, 100.0, 10.0, 12.0, 10.0, 12.0), // 收在最高 → 1.0
                bar("B", 0.0, 100.0, 10.0, 12.0, 10.0, 10.0), // 收在最低 → 0.0
            ],
        );
        let score = calc_intraday_position_score(&snap);
        assert!((score - 50.0).abs() < 1e-6);
    }

    #[test]
    fn test_layer_weights() {
        // 构造中性数据，检查 layers 存在且 value 落在合理区
        let snaps: Vec<_> = (0..5)
            .map(|i| {
                make_snapshot(
                    &format!("2026-01-0{}", i + 1),
                    vec![
                        bar("A", 0.1, 100.0, 10.0, 10.5, 9.5, 10.0 + i as f64 * 0.01),
                        bar("B", -0.1, 100.0, 10.0, 10.5, 9.5, 10.0 - i as f64 * 0.01),
                    ],
                )
            })
            .collect();
        let r = calculate_msi(&MarketDataInput { snapshots: snaps });
        let expected = 0.5 * r.layers.direction
            + 0.25 * r.layers.intensity
            + 0.25 * r.layers.constraint;
        assert!((r.value - expected).abs() < 1e-6);
    }

    #[test]
    fn test_classify_grey() {
        assert!(matches!(
            classify_signal(62.0, 1.8, 0.5),
            SignalType::Neutral
        ));
        assert!(matches!(
            classify_signal(75.0, 1.4, 0.5),
            SignalType::Neutral
        ));
    }

    #[test]
    fn test_classify_strong_bullish() {
        assert!(matches!(
            classify_signal(75.0, 1.8, 0.5),
            SignalType::StrongBullish
        ));
    }

    #[test]
    fn test_volume_level_raw() {
        assert!(volume_level_raw(1.0).abs() < 1e-6);
        assert!((volume_level_raw(2.0) - 1f64.tanh()).abs() < 1e-6);
        assert!(volume_level_raw(4.0) > volume_level_raw(2.0));
    }

    #[test]
    fn test_end_to_end_up() {
        let snaps: Vec<_> = (0..5)
            .map(|i| {
                let c = 10.0 + i as f64;
                make_snapshot(
                    &format!("2026-01-0{}", i + 1),
                    vec![
                        bar("A", 5.0, 100.0 + i as f64 * 40.0, c - 0.2, c + 0.5, c - 0.5, c),
                        bar("B", 4.0, 100.0 + i as f64 * 40.0, c - 0.1, c + 0.4, c - 0.4, c + 0.1),
                        bar("C", 3.0, 100.0 + i as f64 * 40.0, c, c + 0.3, c - 0.3, c + 0.2),
                    ],
                )
            })
            .collect();
        let r = calculate_msi(&MarketDataInput { snapshots: snaps });
        assert!(r.value > 55.0, "全涨 MSI 应偏多, 实际 {}", r.value);
        assert!(r.layers.direction > 50.0);
    }

    #[test]
    fn test_end_to_end_down_volume_surge() {
        let mut snaps: Vec<_> = (0..4)
            .map(|i| {
                let c = 12.0 - i as f64 * 0.3;
                make_snapshot(
                    &format!("2026-01-0{}", i + 1),
                    vec![
                        bar("A", -2.0, 100.0, c + 0.2, c + 0.3, c - 0.3, c),
                        bar("B", -2.0, 100.0, c + 0.1, c + 0.2, c - 0.2, c - 0.1),
                    ],
                )
            })
            .collect();
        snaps.push(make_snapshot(
            "2026-01-05",
            vec![
                bar("A", -5.0, 500.0, 10.5, 10.6, 9.5, 9.8),
                bar("B", -4.0, 500.0, 10.4, 10.5, 9.4, 9.7),
            ],
        ));
        let r = calculate_msi(&MarketDataInput { snapshots: snaps });
        assert!(r.value < 50.0, "放量下跌 MSI 应 < 50, 实际 {}", r.value);
        assert!(r.components.directed_volume < 50.0);
    }
}

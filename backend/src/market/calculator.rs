//! MSI 计算引擎 —— 严格按 `docs/market_sentiment.md` 伪代码实现。
//! 禁止只改本文件不改文档。

use super::domain::*;

// ============================================================
// 规范常量（文档 §2）
// ============================================================

const EWMA_ALPHA: f64 = 0.45;
const RET_SCALE: f64 = 2.0;

const W_DIRECTION: f64 = 0.50;
const W_INTENSITY: f64 = 0.25;
const W_CONSTRAINT: f64 = 0.25;
const W_DIR: f64 = 1.0 / 3.0;

const MSI_HIGH: f64 = 65.0;
const MSI_LOW: f64 = 35.0;
const MSI_GREY_BULL_LO: f64 = 60.0;
const MSI_GREY_BULL_HI: f64 = 65.0;
const MSI_GREY_BEAR_LO: f64 = 35.0;
const MSI_GREY_BEAR_HI: f64 = 40.0;

const LIMIT_PCT: f64 = 9.5;
const EPS_AMOUNT: f64 = 1e-10;
const EPS_PRICE: f64 = 1e-10;
const INTRA_HIGH: f64 = 0.8;
const INTRA_LOW: f64 = 0.2;

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
        return 50.0;
    }
    let mut acc = values[0];
    for &v in &values[1..] {
        acc = alpha * v + (1.0 - alpha) * acc;
    }
    acc
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

// ============================================================
// 单日因子（规范 §5）
// ============================================================

fn daily_breadth_raw(stocks: &[StockDailyData]) -> f64 {
    let up = stocks.iter().filter(|s| s.change_pct > 0.0).count();
    let down = stocks.iter().filter(|s| s.change_pct < 0.0).count();
    if up + down == 0 {
        0.0
    } else {
        (up as f64 - down as f64) / (up + down) as f64
    }
}

fn daily_money_flow_raw(stocks: &[StockDailyData]) -> f64 {
    let mut up_amount = 0.0;
    let mut down_amount = 0.0;
    let mut total = 0.0;
    for s in stocks {
        total += s.amount;
        if s.change_pct > 0.0 {
            up_amount += s.amount;
        } else if s.change_pct < 0.0 {
            down_amount += s.amount;
        }
    }
    if total < EPS_AMOUNT {
        0.0
    } else {
        (up_amount - down_amount) / total
    }
}

fn daily_structure_raw(stocks: &[StockDailyData], breadth_raw: f64) -> f64 {
    if stocks.is_empty() {
        return 0.0;
    }
    let mean_ret = mean(&stocks.iter().map(|s| s.change_pct).collect::<Vec<_>>());
    let ret_sig = (mean_ret / RET_SCALE).tanh();
    (breadth_raw * ret_sig).clamp(-1.0, 1.0)
}

fn daily_limit_raw(stocks: &[StockDailyData]) -> (f64, usize, usize) {
    let n = stocks.len().max(1);
    let limit_up = stocks
        .iter()
        .filter(|s| s.change_pct >= LIMIT_PCT)
        .count();
    let limit_down = stocks
        .iter()
        .filter(|s| s.change_pct <= -LIMIT_PCT)
        .count();
    let raw = ((limit_up as f64 - limit_down as f64) / n as f64).clamp(-1.0, 1.0);
    (raw, limit_up, limit_down)
}

fn daily_intraday_raw(stocks: &[StockDailyData]) -> f64 {
    if stocks.is_empty() {
        return 0.5;
    }
    let mut pos = Vec::with_capacity(stocks.len());
    for s in stocks {
        if (s.high - s.low).abs() < EPS_PRICE {
            pos.push(0.5);
        } else {
            pos.push(((s.close_price - s.low) / (s.high - s.low)).clamp(0.0, 1.0));
        }
    }
    mean(&pos)
}

/// 单日情绪（规范 §5）。
pub fn calculate_msi_daily(snap: &MarketDailySnapshot) -> MsiDayResult {
    let snap = filter_snapshot(snap);
    let stocks = &snap.stocks;

    if stocks.is_empty() {
        return neutral_day(&snap.date);
    }

    let breadth_raw = daily_breadth_raw(stocks);
    let money_raw = daily_money_flow_raw(stocks);
    let structure_raw = daily_structure_raw(stocks, breadth_raw);
    let (limit_raw, limit_up, limit_down) = daily_limit_raw(stocks);
    let intraday_raw = daily_intraday_raw(stocks);

    let breadth = map_to_score(breadth_raw);
    let money_flow = map_to_score(money_raw);
    let structure = map_to_score(structure_raw);
    let limit_pressure = map_to_score(limit_raw);
    let intraday_position = intraday_raw * 100.0;

    let direction = W_DIR * (breadth + money_flow + structure);
    let intensity = limit_pressure;
    let constraint = intraday_position;
    let value = W_DIRECTION * direction + W_INTENSITY * intensity + W_CONSTRAINT * constraint;

    let mut up = 0usize;
    let mut down = 0usize;
    let mut flat = 0usize;
    for s in stocks {
        if s.change_pct > 0.0 {
            up += 1;
        } else if s.change_pct < 0.0 {
            down += 1;
        } else {
            flat += 1;
        }
    }

    let signal = build_signal(value, limit_raw, intraday_raw);

    MsiDayResult {
        date: snap.date.clone(),
        value,
        layers: MsiLayers {
            direction,
            intensity,
            constraint,
        },
        components: MsiComponents {
            breadth,
            money_flow,
            structure,
            limit_pressure,
            intraday_position,
        },
        signal,
        diagnostics: MsiDiagnostics {
            sample_size: stocks.len(),
            up_count: up,
            down_count: down,
            flat_count: flat,
            limit_up_count: limit_up,
            limit_down_count: limit_down,
            breadth_raw,
            money_flow_raw: money_raw,
            structure_raw,
        },
    }
}

fn neutral_day(date: &str) -> MsiDayResult {
    MsiDayResult {
        date: date.to_string(),
        value: 50.0,
        layers: MsiLayers {
            direction: 50.0,
            intensity: 50.0,
            constraint: 50.0,
        },
        components: MsiComponents {
            breadth: 50.0,
            money_flow: 50.0,
            structure: 50.0,
            limit_pressure: 50.0,
            intraday_position: 50.0,
        },
        signal: build_signal(50.0, 0.0, 0.5),
        diagnostics: MsiDiagnostics {
            sample_size: 0,
            up_count: 0,
            down_count: 0,
            flat_count: 0,
            limit_up_count: 0,
            limit_down_count: 0,
            breadth_raw: 0.0,
            money_flow_raw: 0.0,
            structure_raw: 0.0,
        },
    }
}

fn classify_signal(value: f64, limit_raw: f64, intraday_raw: f64) -> SignalType {
    let msi_bull = value > MSI_HIGH;
    let msi_bear = value < MSI_LOW;
    let msi_grey = (MSI_GREY_BULL_LO..=MSI_GREY_BULL_HI).contains(&value)
        || (MSI_GREY_BEAR_LO..=MSI_GREY_BEAR_HI).contains(&value);
    let intra_high = intraday_raw >= INTRA_HIGH;
    let intra_low = intraday_raw < INTRA_LOW;
    let limit_bull = limit_raw > 0.05;
    let limit_bear = limit_raw < -0.05;

    if msi_grey {
        return SignalType::Neutral;
    }
    if msi_bull && limit_bull && intra_high {
        return SignalType::StrongBullish;
    }
    if msi_bull && intra_high {
        return SignalType::SurgeWarning;
    }
    if msi_bull {
        return SignalType::WeakBullish;
    }
    if msi_bear && limit_bear && intra_low {
        return SignalType::PanicSelling;
    }
    if msi_bear && intra_low {
        return SignalType::Bottoming;
    }
    if msi_bear {
        return SignalType::VolumeDecline;
    }
    SignalType::Neutral
}

fn build_signal(value: f64, limit_raw: f64, intraday_raw: f64) -> MsiSignal {
    let signal_type = classify_signal(value, limit_raw, intraday_raw);
    MsiSignal {
        label: signal_type.label().to_string(),
        advice: signal_type.advice().to_string(),
        signal_type,
        volume_status: VolumeStatus::Normal,
        volume_ratio: 1.0,
        price_status: PriceStatus::from_position(intraday_raw),
        price_position: intraday_raw,
    }
}

/// 短线双读数（规范 §6）。
pub fn calculate_msi(input: &MarketDataInput) -> MsiResult {
    if input.snapshots.is_empty() {
        let today = neutral_day("-");
        return MsiResult {
            today: today.clone(),
            recent: MsiRecentResult {
                value: 50.0,
                days: 0,
                series: vec![],
            },
            days: 0,
            calculated_at: chrono::Utc::now().to_rfc3339(),
        };
    }

    let mut series = Vec::with_capacity(input.snapshots.len());
    let mut last_day = neutral_day(&input.snapshots.last().unwrap().date);

    for snap in &input.snapshots {
        let day = calculate_msi_daily(snap);
        series.push(MsiDayPoint {
            date: day.date.clone(),
            value: day.value,
        });
        last_day = day;
    }

    let values: Vec<f64> = series.iter().map(|p| p.value).collect();
    let recent_value = ewma(&values, EWMA_ALPHA);

    MsiResult {
        today: last_day,
        recent: MsiRecentResult {
            value: recent_value,
            days: series.len(),
            series,
        },
        days: input.snapshots.len(),
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
    ) -> StockDailyData {
        StockDailyData {
            symbol: sym.to_string(),
            date: String::new(),
            change_pct: change,
            amount,
            open,
            high,
            low,
            close_price: close,
        }
    }

    fn snap(date: &str, stocks: Vec<StockDailyData>) -> MarketDailySnapshot {
        let mut stocks = stocks;
        for s in &mut stocks {
            s.date = date.to_string();
        }
        MarketDailySnapshot {
            date: date.to_string(),
            stocks,
        }
    }

    #[test]
    fn test_invalid_ohlc_filtered() {
        let s = snap(
            "2026-01-01",
            vec![
                bar("OK", 1.0, 100.0, 10.0, 10.5, 9.5, 10.2),
                bar("BAD", 1.0, 100.0, 10.0, 10.0, 9.5, 10.5),
            ],
        );
        let r = calculate_msi_daily(&s);
        assert_eq!(r.diagnostics.sample_size, 1);
    }

    #[test]
    fn test_all_up_today_high() {
        let s = snap(
            "2026-01-01",
            vec![
                bar("A", 2.0, 100.0, 10.0, 10.5, 9.8, 10.4),
                bar("B", 3.0, 100.0, 10.0, 10.6, 9.9, 10.5),
                bar("C", 1.5, 100.0, 10.0, 10.4, 9.9, 10.3),
            ],
        );
        let r = calculate_msi_daily(&s);
        assert!(r.value > 60.0, "全日上涨 today 应偏高, 实际 {}", r.value);
        assert!(r.components.breadth > 90.0);
    }

    #[test]
    fn test_all_down_today_low() {
        let s = snap(
            "2026-01-01",
            vec![
                bar("A", -2.0, 100.0, 10.0, 10.1, 9.5, 9.6),
                bar("B", -3.0, 100.0, 10.0, 10.0, 9.4, 9.5),
                bar("C", -1.5, 100.0, 10.0, 10.0, 9.6, 9.7),
            ],
        );
        let r = calculate_msi_daily(&s);
        assert!(r.value < 40.0, "全日下跌 today 应偏低, 实际 {}", r.value);
    }

    #[test]
    fn test_daily_ignores_other_days() {
        let weak = snap(
            "2026-01-01",
            vec![bar("A", -5.0, 100.0, 10.0, 10.0, 9.0, 9.2)],
        );
        let strong = snap(
            "2026-01-02",
            vec![
                bar("A", 5.0, 100.0, 10.0, 11.0, 10.0, 10.8),
                bar("B", 4.0, 100.0, 10.0, 10.9, 10.0, 10.7),
            ],
        );
        let only_strong = calculate_msi_daily(&strong).value;
        let with_weak = calculate_msi(&MarketDataInput {
            snapshots: vec![weak, strong.clone()],
        });
        assert!(
            (with_weak.today.value - only_strong).abs() < 1e-9,
            "today 应只依赖末日截面"
        );
    }

    #[test]
    fn test_recent_ewma_near_sensitive() {
        let mut snaps = Vec::new();
        for i in 0..4 {
            snaps.push(snap(
                &format!("2026-01-0{}", i + 1),
                vec![
                    bar("A", -2.0, 100.0, 10.0, 10.0, 9.5, 9.6),
                    bar("B", -2.0, 100.0, 10.0, 10.0, 9.5, 9.6),
                ],
            ));
        }
        snaps.push(snap(
            "2026-01-05",
            vec![
                bar("A", 5.0, 100.0, 10.0, 11.0, 10.0, 10.8),
                bar("B", 5.0, 100.0, 10.0, 11.0, 10.0, 10.8),
            ],
        ));
        let r = calculate_msi(&MarketDataInput { snapshots: snaps });
        assert!(r.recent.days == 5);
        assert!(
            r.recent.value > r.recent.series[0].value,
            "近端强势应抬高 recent"
        );
        assert!((r.today.value - r.recent.series.last().unwrap().value).abs() < 1e-9);
    }

    #[test]
    fn test_empty_day_neutral() {
        let r = calculate_msi_daily(&snap("2026-01-01", vec![]));
        assert!((r.value - 50.0).abs() < 1e-9);
    }

    #[test]
    fn test_bounds() {
        let s = snap(
            "2026-01-01",
            vec![bar("A", 1.0, 100.0, 10.0, 10.5, 9.5, 10.2)],
        );
        let r = calculate_msi(&MarketDataInput {
            snapshots: vec![s],
        });
        assert!((0.0..=100.0).contains(&r.today.value));
        assert!((0.0..=100.0).contains(&r.recent.value));
    }

    #[test]
    fn test_layer_weights_on_today() {
        let s = snap(
            "2026-01-01",
            vec![
                bar("A", 1.0, 100.0, 10.0, 10.5, 9.5, 10.2),
                bar("B", -0.5, 100.0, 10.0, 10.2, 9.7, 9.9),
            ],
        );
        let d = calculate_msi_daily(&s);
        let expected = 0.5 * d.layers.direction
            + 0.25 * d.layers.intensity
            + 0.25 * d.layers.constraint;
        assert!((d.value - expected).abs() < 1e-6);
    }

    #[test]
    fn test_grey_signal_neutral() {
        assert!(matches!(
            classify_signal(62.0, 0.2, 0.9),
            SignalType::Neutral
        ));
    }
}

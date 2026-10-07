//! Mock 行情生成（含 OHLC），规范见 `docs/market_sentiment.md`

use rand::seq::SliceRandom;
use rand::Rng;

use super::domain::*;

#[derive(Debug, Clone)]
pub struct MockConfig {
    pub stock_count: usize,
    pub change_mean: f64,
    pub change_std: f64,
    pub base_amount: f64,
    pub base_price: f64,
}

impl Default for MockConfig {
    fn default() -> Self {
        Self {
            stock_count: 50,
            change_mean: 0.05,
            change_std: 2.0,
            base_amount: 1_0000_0000.0,
            base_price: 10.0,
        }
    }
}

fn generate_stock_codes(count: usize) -> Vec<String> {
    let mut rng = rand::thread_rng();
    let prefixes = ["600", "601", "603", "605", "000", "001", "002", "300"];
    let mut codes = Vec::with_capacity(count);
    for i in 0..count {
        let prefix = prefixes.choose(&mut rng).unwrap();
        codes.push(format!("{}{:03}", prefix, i + 1));
    }
    codes
}

fn gen_dates(days: usize) -> Vec<String> {
    let mut dates = Vec::with_capacity(days);
    for i in 0..days {
        let day_offset = (days - 1 - i) as i64;
        let date = chrono::NaiveDate::from_ymd_opt(2026, 9, 29)
            .unwrap()
            .checked_sub_days(chrono::Days::new(day_offset as u64))
            .unwrap()
            .format("%Y-%m-%d")
            .to_string();
        dates.push(date);
    }
    dates
}

/// 生成 N 日 Mock 快照（OHLC + amount + change_pct）
pub fn generate_mock_data(days: usize, cfg: &MockConfig) -> MarketDataInput {
    let symbols = generate_stock_codes(cfg.stock_count);
    let dates = gen_dates(days);

    let mut prices: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
    for sym in &symbols {
        prices.insert(sym.clone(), cfg.base_price);
    }

    let mut snapshots = Vec::with_capacity(days);

    for date in &dates {
        let mut stocks = Vec::with_capacity(symbols.len());
        for sym in &symbols {
            let mut rng = rand::thread_rng();

            let change_pct = {
                let u1: f64 = rng.gen_range(0.0001..1.0);
                let u2: f64 = rng.gen_range(0.0001..1.0);
                let z = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
                cfg.change_mean + cfg.change_std * z
            };

            let prev_close = *prices.get(sym).unwrap();
            let close_price = (prev_close * (1.0 + change_pct / 100.0)).max(0.01);

            // 开盘：相对昨收小缺口
            let gap_pct = rng.gen_range(-1.0..1.0) + cfg.change_mean * 0.3;
            let open = (prev_close * (1.0 + gap_pct / 100.0)).max(0.01);

            let body_high = open.max(close_price);
            let body_low = open.min(close_price);
            let wick = (body_high * rng.gen_range(0.002..0.02)).max(0.01);
            let high = body_high + wick;
            let low = (body_low - wick).max(0.01);
            // 保证 high/low 包住 open/close
            let high = high.max(open).max(close_price);
            let low = low.min(open).min(close_price).max(0.01);

            prices.insert(sym.clone(), close_price);
            let amount = cfg.base_amount * rng.gen_range(0.5..3.0);

            stocks.push(StockDailyData {
                symbol: sym.clone(),
                date: date.clone(),
                open,
                high,
                low,
                close_price,
                amount,
                change_pct,
            });
        }

        snapshots.push(MarketDailySnapshot {
            date: date.clone(),
            stocks,
        });
    }

    MarketDataInput { snapshots }
}

pub fn generate_mock_data_with_bias(days: usize, bias: f64) -> MarketDataInput {
    let cfg = MockConfig {
        stock_count: 50,
        change_mean: bias * 1.0,
        change_std: 2.0,
        base_amount: 1_0000_0000.0,
        base_price: 10.0,
    };
    generate_mock_data(days, &cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_mock_data() {
        let data = generate_mock_data(5, &MockConfig::default());
        assert_eq!(data.snapshots.len(), 5);
        for snap in &data.snapshots {
            assert_eq!(snap.stocks.len(), 50);
            for s in &snap.stocks {
                assert!(s.close_price > 0.0);
                assert!(s.open > 0.0);
                assert!(s.high >= s.low);
                assert!(s.high >= s.open && s.high >= s.close_price);
                assert!(s.low <= s.open && s.low <= s.close_price);
                assert!(s.amount > 0.0);
            }
        }
    }

    #[test]
    fn test_generate_with_bias_bullish() {
        let data = generate_mock_data_with_bias(10, 0.8);
        let result = super::super::calculator::calculate_msi(&data);
        assert!(
            result.value > 55.0,
            "偏多市场 MSI 应 > 55, 实际: {}",
            result.value
        );
    }

    #[test]
    fn test_generate_with_bias_bearish() {
        let data = generate_mock_data_with_bias(10, -0.8);
        let result = super::super::calculator::calculate_msi(&data);
        assert!(
            result.value < 45.0,
            "偏空市场 MSI 应 < 45, 实际: {}",
            result.value
        );
    }

    #[test]
    fn test_stock_codes() {
        let codes = generate_stock_codes(10);
        assert_eq!(codes.len(), 10);
        for c in &codes {
            assert_eq!(c.len(), 6);
        }
    }

    #[test]
    fn test_price_accumulation() {
        let cfg = MockConfig {
            stock_count: 1,
            change_mean: 5.0,
            change_std: 0.0,
            base_amount: 100.0,
            base_price: 10.0,
        };
        let data = generate_mock_data(5, &cfg);
        let p0 = data.snapshots[0].stocks[0].close_price;
        let p1 = data.snapshots[1].stocks[0].close_price;
        let p4 = data.snapshots[4].stocks[0].close_price;
        assert!(p1 > p0);
        assert!(p4 > p1);
    }
}

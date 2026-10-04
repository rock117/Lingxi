//! Mock 数据生成器
//!
//! 在没有接入真实行情数据源时，生成随机的市场快照数据用于 MSI 计算测试。
//!
//! 生成的数据特征：
//! - 股票数量可配置（默认 50 只）
//! - 涨跌幅服从正态分布，均值和标准差可配置
//! - 成交额在对数正态分布范围内随机
//! - 收盘价根据涨跌幅累乘推算（起始价格 10 元）
//! - 可通过 `bias` 参数控制整体偏多/偏空

use rand::seq::SliceRandom;
use rand::Rng;

use super::domain::*;

/// Mock 数据生成器配置
#[derive(Debug, Clone)]
pub struct MockConfig {
    /// 股票数量
    pub stock_count: usize,
    /// 涨跌幅均值（%，正值偏多，负值偏空）
    pub change_mean: f64,
    /// 涨跌幅标准差（%）
    pub change_std: f64,
    /// 基础成交额（元）
    pub base_amount: f64,
    /// 起始价格（元）
    pub base_price: f64,
}

impl Default for MockConfig {
    fn default() -> Self {
        Self {
            stock_count: 50,
            // 略微偏多，模拟长期向上的市场
            change_mean: 0.05,
            change_std: 2.0,
            base_amount: 1_0000_0000.0, // 1 亿
            base_price: 10.0,
        }
    }
}

/// 生成股票代码列表
///
/// 简单生成 6 位数字代码，前缀随机分配为 60x（沪市）或 00x（深市）
fn generate_stock_codes(count: usize) -> Vec<String> {
    let mut rng = rand::thread_rng();
    let prefixes = ["600", "601", "603", "605", "000", "001", "002", "300"];

    let mut codes = Vec::with_capacity(count);
    for i in 0..count {
        let prefix = prefixes.choose(&mut rng).unwrap();
        let suffix = format!("{:03}", i + 1);
        codes.push(format!("{}{}", prefix, suffix));
    }
    codes
}

/// 生成连续 N 天的日期字符串（从今天往前推）
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

/// 生成 Mock 市场数据
///
/// 生成 N 天的市场快照数据，每只股票包含涨跌幅、成交额、收盘价。
///
/// # 参数
/// - `days`：天数 N
/// - `cfg`：Mock 配置（股票数量、涨跌幅分布等）
pub fn generate_mock_data(days: usize, cfg: &MockConfig) -> MarketDataInput {
    let symbols = generate_stock_codes(cfg.stock_count);
    let dates = gen_dates(days);

    // 为每只股票维护一个当前价格，根据涨跌幅累乘
    let mut prices: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
    for sym in &symbols {
        prices.insert(sym.clone(), cfg.base_price);
    }

    let mut snapshots = Vec::with_capacity(days);

    for date in &dates {
        let mut stocks = Vec::with_capacity(symbols.len());

        for sym in &symbols {
            let mut rng = rand::thread_rng();

            // 涨跌幅：正态分布近似（Box-Muller）
            let change_pct = {
                let u1: f64 = rng.gen_range(0.0001..1.0);
                let u2: f64 = rng.gen_range(0.0001..1.0);
                let z = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
                cfg.change_mean + cfg.change_std * z
            };

            // 更新收盘价：根据涨跌幅累乘
            let prev_price = *prices.get(sym).unwrap();
            let close_price = prev_price * (1.0 + change_pct / 100.0);
            prices.insert(sym.clone(), close_price);

            // 成交额：基础值 × 随机倍数（0.5 ~ 3.0）
            let amount = cfg.base_amount * rng.gen_range(0.5..3.0);

            stocks.push(StockDailyData {
                symbol: sym.clone(),
                date: date.clone(),
                change_pct,
                amount,
                close_price,
            });
        }

        snapshots.push(MarketDailySnapshot {
            date: date.clone(),
            stocks,
        });
    }

    MarketDataInput { snapshots }
}

/// 生成带偏置的 Mock 数据
///
/// 可以模拟"偏多市场"、"偏空市场"、"震荡市场"等场景
///
/// # 参数
/// - `days`：天数 N
/// - `bias`：偏置值，-1.0（极空）~ +1.0（极多）
pub fn generate_mock_data_with_bias(days: usize, bias: f64) -> MarketDataInput {
    let cfg = MockConfig {
        stock_count: 50,
        // bias 控制均值：bias=+1 → 均值 +1%，bias=-1 → 均值 -1%
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

        // 每天应有 50 只股票
        for snap in &data.snapshots {
            assert_eq!(snap.stocks.len(), 50);
            for s in &snap.stocks {
                // 收盘价应为正数
                assert!(s.close_price > 0.0, "收盘价应 > 0");
                // 成交额应为正数
                assert!(s.amount > 0.0, "成交额应 > 0");
            }
        }
    }

    #[test]
    fn test_generate_with_bias_bullish() {
        // 偏多市场：涨跌幅均值为正 → 价格应上升
        let data = generate_mock_data_with_bias(10, 0.8);
        let result = super::super::calculator::calculate_msi(&data);
        // 偏多数据应使 MSI 偏高
        assert!(
            result.value > 55.0,
            "偏多市场 MSI 应 > 55, 实际: {}",
            result.value
        );
    }

    #[test]
    fn test_generate_with_bias_bearish() {
        // 偏空市场：涨跌幅均值为负 → 价格应下降
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
            // 3 位前缀 + 3 位后缀 = 6 位
            assert_eq!(c.len(), 6);
        }
    }

    #[test]
    fn test_price_accumulation() {
        // 验证收盘价累乘逻辑：连续上涨应使价格递增
        let cfg = MockConfig {
            stock_count: 1,
            change_mean: 5.0, // 每天涨 5%
            change_std: 0.0,  // 无随机波动
            base_amount: 100.0,
            base_price: 10.0,
        };
        let data = generate_mock_data(5, &cfg);

        // 第一天价格 ≈ 10.0 × 1.05 = 10.5
        let p0 = data.snapshots[0].stocks[0].close_price;
        let p1 = data.snapshots[1].stocks[0].close_price;
        let p4 = data.snapshots[4].stocks[0].close_price;

        assert!(p1 > p0, "第二天价格应高于第一天: {} vs {}", p1, p0);
        assert!(p4 > p1, "第五天价格应高于第二天: {} vs {}", p4, p1);
    }
}

//! 市场情绪指数（MSI）计算引擎
//!
//! 仅分析短期（N 天），基于整个 N 天的数据综合计算 4 个子分：
//!
//! 1. **涨跌宽度分**（20%）：N 天平均涨跌家数比
//! 2. **资金流向分**（30%）：N 天平均资金流向（上涨股票成交额 vs 下跌股票成交额）
//! 3. **成交量水平分**（25%）：最近成交量 vs N 天均量（放量/缩量，**核心指标**）
//! 4. **价格位置分**（25%）：当前股价在 N 天高低点区间的相对位置
//!
//! 所有计算均采用等权（简单平均/计数），不按市值或成交额加权。

use super::domain::*;

// ============================================================
// 工具函数
// ============================================================

/// 将 [-1, 1] 范围的值映射到 [0, 100]，50 为中性
fn map_to_score(raw: f64) -> f64 {
    50.0 + 50.0 * raw.clamp(-1.0, 1.0)
}

/// 计算一组值的简单算术平均（等权）
fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

// ============================================================
// 子分 1：涨跌宽度分
// ============================================================

/// 计算单日涨跌宽度：上涨家数占比 - 下跌家数占比，范围 [-1, 1]
///
/// 等权计数：每只股票权重相同
fn daily_breadth(snapshot: &MarketDailySnapshot) -> f64 {
    let total = snapshot.stocks.len() as f64;
    if total == 0.0 {
        return 0.0;
    }
    let up = snapshot.stocks.iter().filter(|s| s.change_pct > 0.0).count() as f64;
    let down = snapshot.stocks.iter().filter(|s| s.change_pct < 0.0).count() as f64;
    (up - down) / total
}

/// 计算涨跌宽度分
///
/// 对 N 天的涨跌宽度取平均后映射到 [0, 100]
pub fn calc_breadth_score(snapshots: &[MarketDailySnapshot]) -> f64 {
    let breadths: Vec<f64> = snapshots.iter().map(daily_breadth).collect();
    map_to_score(mean(&breadths))
}

// ============================================================
// 子分 2：资金流向分
// ============================================================

/// 计算单日资金流向：上涨股票成交额 - 下跌股票成交额，再除以总成交额
///
/// 范围 [-1, 1]：正值表示资金净流入上涨股票，负值表示资金净流入下跌股票
///
/// 注意：成交额是"被统计的资金量"，不是"加权权重"。
/// 它回答的问题是"钱在追涨还是杀跌"。
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

    if total_amount < 1e-10 {
        return 0.0;
    }
    (up_amount - down_amount) / total_amount
}

/// 计算资金流向分
///
/// 对 N 天的资金流向取平均后映射到 [0, 100]
pub fn calc_money_flow_score(snapshots: &[MarketDailySnapshot]) -> f64 {
    let flows: Vec<f64> = snapshots.iter().map(daily_money_flow).collect();
    map_to_score(mean(&flows))
}

// ============================================================
// 子分 3：成交量水平分（核心）
// ============================================================

/// 计算单日全市场总成交额
fn daily_total_amount(snapshot: &MarketDailySnapshot) -> f64 {
    snapshot.stocks.iter().map(|s| s.amount).sum()
}

/// 计算成交量水平分
///
/// 核心逻辑：
/// 1. 计算 N 天每天的全市场总成交额
/// 2. 计算 N 天平均日成交额
/// 3. 用最近一天的成交额 / N 天均量 = volume_ratio
/// 4. 映射到 [0, 100]：50 为正常，>50 放量，<50 缩量
///
/// volume_ratio 含义：
/// - > 2.0：天量（极端放量）
/// - 1.5 ~ 2.0：放量
/// - 0.5 ~ 1.5：正常
/// - < 0.5：缩量
/// - < 0.3：地量
pub fn calc_volume_level_score(snapshots: &[MarketDailySnapshot]) -> (f64, f64) {
    if snapshots.is_empty() {
        return (50.0, 1.0);
    }

    // N 天每天的总成交额
    let daily_amounts: Vec<f64> = snapshots.iter().map(daily_total_amount).collect();

    // N 天平均日成交额
    let avg_amount = mean(&daily_amounts);

    // 最近一天的成交额
    let last_amount = *daily_amounts.last().unwrap();

    // 成交量比值
    let volume_ratio = if avg_amount < 1e-10 {
        1.0
    } else {
        last_amount / avg_amount
    };

    // 映射到 [0, 100]：ratio=1 → 50，ratio=2 → 100，ratio=0 → 0
    let score = map_to_score((volume_ratio - 1.0).clamp(-1.0, 1.0));

    (score, volume_ratio)
}

// ============================================================
// 子分 4：价格位置分
// ============================================================

/// 计算单只股票在 N 天内的价格位置
///
/// position = (当前收盘价 - N天最低价) / (N天最高价 - N天最低价)
///
/// 返回值范围 [0, 1]：
/// - 1.0 = 当前价格在 N 天最高点
/// - 0.0 = 当前价格在 N 天最低点
/// - 0.5 = 当前价格在 N 天中位
fn calc_stock_price_position(symbol: &str, snapshots: &[MarketDailySnapshot]) -> Option<f64> {
    // 收集该股票在 N 天内的所有收盘价
    let prices: Vec<f64> = snapshots
        .iter()
        .flat_map(|s| s.stocks.iter())
        .filter(|st| st.symbol == symbol)
        .map(|st| st.close_price)
        .collect();

    if prices.is_empty() {
        return None;
    }

    let high = prices.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let low = prices.iter().cloned().fold(f64::INFINITY, f64::min);
    let current = *prices.last().unwrap();

    if (high - low).abs() < 1e-10 {
        // N 天价格没变化
        return Some(0.5);
    }

    Some((current - low) / (high - low))
}

/// 计算价格位置分
///
/// 对全市场所有股票的价格位置取等权平均后映射到 [0, 100]
///
/// - 100 = 全市场股票都在 N 天最高点
/// - 0 = 全市场股票都在 N 天最低点
/// - 50 = 全市场股票都在 N 天中位
pub fn calc_price_position_score(snapshots: &[MarketDailySnapshot]) -> (f64, f64) {
    if snapshots.is_empty() {
        return (50.0, 0.5);
    }

    // 收集所有出现过的股票代码
    let symbols = collect_all_symbols(snapshots);

    // 计算每只股票的价格位置
    let positions: Vec<f64> = symbols
        .iter()
        .filter_map(|sym| calc_stock_price_position(sym, snapshots))
        .collect();

    if positions.is_empty() {
        return (50.0, 0.5);
    }

    // 等权平均价格位置 [0, 1]
    let avg_position = mean(&positions);

    // 映射到 [0, 100]
    let score = avg_position * 100.0;

    (score, avg_position)
}

// ============================================================
// 组合信号
// ============================================================

/// 根据 MSI 值、成交量比值、价格位置三维组合判定信号类型
fn classify_signal(msi_value: f64, volume_ratio: f64, price_position: f64) -> SignalType {
    // MSI 三档：高 > 65，中 35~65，低 < 35
    let msi_high = msi_value > 65.0;
    let msi_low = msi_value < 35.0;

    // 成交量三档：放量 > 1.5，缩量 < 0.5，正常 0.5~1.5
    let vol_high = volume_ratio >= 1.5;
    let vol_low = volume_ratio < 0.5;

    // 价格位置三档：高位 > 0.8，低位 < 0.2，中位 0.2~0.8
    let price_high = price_position >= 0.8;
    let price_low = price_position < 0.2;

    // 三维组合判断
    if msi_high && vol_high && !price_high {
        // 高 MSI + 放量 + 价格不在高位 → 强势上涨
        SignalType::StrongBullish
    } else if msi_high && vol_high && price_high {
        // 高 MSI + 放量 + 价格高位 → 放量冲高，警惕见顶
        SignalType::SurgeWarning
    } else if msi_high && vol_low && price_high {
        // 高 MSI + 缩量 + 价格高位 → 量价背离
        SignalType::DivergenceWarning
    } else if msi_high && vol_low && !price_high {
        // 高 MSI + 缩量 + 价格中/低位 → 缩量上涨，动能不足
        SignalType::WeakBullish
    } else if msi_low && vol_high && price_low {
        // 低 MSI + 放量 + 价格低位 → 恐慌抛售
        SignalType::PanicSelling
    } else if msi_low && vol_low && price_low {
        // 低 MSI + 缩量 + 价格低位 → 地量地价
        SignalType::Bottoming
    } else if msi_low && vol_high && !price_low {
        // 低 MSI + 放量 + 价格中/高位 → 放量下跌
        SignalType::VolumeDecline
    } else if !msi_high && !msi_low && vol_high && price_low {
        // 中 MSI + 放量 + 价格低位 → 低位放量
        SignalType::LowVolumeAccumulation
    } else {
        // 其余组合 → 中性
        SignalType::Neutral
    }
}

/// 生成组合信号
pub fn build_signal(
    msi_value: f64,
    volume_ratio: f64,
    price_position: f64,
) -> MsiSignal {
    let signal_type = classify_signal(msi_value, volume_ratio, price_position);
    let volume_status = VolumeStatus::from_ratio(volume_ratio);
    let price_status = PriceStatus::from_position(price_position);

    MsiSignal {
        label: signal_type.label().to_string(),
        advice: signal_type.advice().to_string(),
        signal_type,
        volume_status,
        volume_ratio,
        price_status,
        price_position,
    }
}

// ============================================================
// 完整计算入口
// ============================================================

/// 计算市场情绪指数（MSI）
///
/// 输入：过去 N 天的市场快照数据
/// 输出：完整的 MSI 结果（综合值 + 4 个子分 + 组合信号）
///
/// 权重分配：
/// - 涨跌宽度分 20%：市场参与广度
/// - 资金流向分 30%：资金方向（核心）
/// - 成交量水平分 25%：放量/缩量（核心）
/// - 价格位置分 25%：当前股价在 N 天区间的位置
pub fn calculate_msi(input: &MarketDataInput) -> MsiResult {
    let snapshots = &input.snapshots;
    let days = snapshots.len();

    if days == 0 {
        return MsiResult {
            value: 50.0,
            sub_scores: MsiSubScores {
                breadth: 50.0,
                money_flow: 50.0,
                volume_level: 50.0,
                price_position: 50.0,
            },
            days: 0,
            signal: build_signal(50.0, 1.0, 0.5),
            calculated_at: chrono::Utc::now().to_rfc3339(),
        };
    }

    // 计算四个子分
    let breadth = calc_breadth_score(snapshots);
    let money_flow = calc_money_flow_score(snapshots);
    let (volume_level, volume_ratio) = calc_volume_level_score(snapshots);
    let (price_position_score, price_position_raw) = calc_price_position_score(snapshots);

    // 加权合成 MSI 综合值
    let value = 0.20 * breadth
        + 0.30 * money_flow
        + 0.25 * volume_level
        + 0.25 * price_position_score;

    // 生成组合信号
    let signal = build_signal(value, volume_ratio, price_position_raw);

    tracing::info!(
        "MSI 计算: 值={:.1}, 宽度={:.1}, 资金={:.1}, 量能={:.1}(ratio={:.2}), 价格={:.1}(pos={:.2}), 信号={}",
        value, breadth, money_flow, volume_level, volume_ratio,
        price_position_score, price_position_raw, signal.label
    );

    MsiResult {
        value,
        sub_scores: MsiSubScores {
            breadth,
            money_flow,
            volume_level,
            price_position: price_position_score,
        },
        days,
        signal,
        calculated_at: chrono::Utc::now().to_rfc3339(),
    }
}

// ============================================================
// 内部工具
// ============================================================

/// 收集所有快照中出现过的股票代码（去重）
fn collect_all_symbols(snapshots: &[MarketDailySnapshot]) -> Vec<String> {
    let mut symbols: Vec<String> = Vec::new();
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
// 单元测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造单日快照的辅助函数
    fn make_snapshot(date: &str, stocks: Vec<(&str, f64, f64, f64)>) -> MarketDailySnapshot {
        MarketDailySnapshot {
            date: date.to_string(),
            stocks: stocks
                .into_iter()
                .map(|(sym, change, amount, close)| StockDailyData {
                    symbol: sym.to_string(),
                    date: date.to_string(),
                    change_pct: change,
                    amount,
                    close_price: close,
                })
                .collect(),
        }
    }

    #[test]
    fn test_map_to_score() {
        assert!((map_to_score(0.0) - 50.0).abs() < 1e-6);
        assert!((map_to_score(1.0) - 100.0).abs() < 1e-6);
        assert!((map_to_score(-1.0) - 0.0).abs() < 1e-6);
        assert!((map_to_score(0.5) - 75.0).abs() < 1e-6);
    }

    #[test]
    fn test_daily_breadth() {
        let snap = make_snapshot("2026-01-01", vec![
            ("A", 1.0, 10.0, 11.0),
            ("B", 2.0, 20.0, 12.0),
            ("C", -1.0, 15.0, 9.0),
            ("D", -2.0, 5.0, 8.0),
            ("E", 0.0, 8.0, 10.0),
        ]);
        // up=2, down=2, total=5 → (2-2)/5 = 0
        assert!((daily_breadth(&snap) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_daily_money_flow() {
        let snap = make_snapshot("2026-01-01", vec![
            ("A", 1.0, 10.0, 11.0),
            ("B", 2.0, 20.0, 12.0),
            ("C", -1.0, 15.0, 9.0),
            ("D", -2.0, 5.0, 8.0),
        ]);
        // up_amount=30, down_amount=20, total=50 → (30-20)/50=0.2
        assert!((daily_money_flow(&snap) - 0.2).abs() < 1e-6);
    }

    #[test]
    fn test_volume_level_normal() {
        // 5 天成交额相同 → ratio=1.0 → score=50
        let snaps: Vec<MarketDailySnapshot> = (0..5)
            .map(|i| {
                make_snapshot(&format!("2026-01-0{}", i + 1), vec![
                    ("A", 1.0, 100.0, 10.0),
                    ("B", -1.0, 100.0, 10.0),
                ])
            })
            .collect();
        let (score, ratio) = calc_volume_level_score(&snaps);
        assert!((ratio - 1.0).abs() < 1e-6, "ratio 应为 1.0, 实际: {}", ratio);
        assert!((score - 50.0).abs() < 1e-6, "score 应为 50, 实际: {}", score);
    }

    #[test]
    fn test_volume_level_surge() {
        // 前 4 天成交额 100，第 5 天 300 → ratio=3.0 → score=100
        let mut snaps: Vec<MarketDailySnapshot> = (0..4)
            .map(|i| {
                make_snapshot(&format!("2026-01-0{}", i + 1), vec![
                    ("A", 1.0, 100.0, 10.0),
                    ("B", -1.0, 100.0, 10.0),
                ])
            })
            .collect();
        snaps.push(make_snapshot("2026-01-05", vec![
            ("A", 1.0, 300.0, 11.0),
            ("B", -1.0, 300.0, 9.0),
        ]));
        let (score, ratio) = calc_volume_level_score(&snaps);
        assert!(ratio > 2.0, "ratio 应 > 2.0, 实际: {}", ratio);
        assert!(score > 90.0, "score 应 > 90, 实际: {}", score);
    }

    #[test]
    fn test_volume_level_shrink() {
        // 前 4 天成交额 100，第 5 天 20 → ratio=0.2 → score 低
        let mut snaps: Vec<MarketDailySnapshot> = (0..4)
            .map(|i| {
                make_snapshot(&format!("2026-01-0{}", i + 1), vec![
                    ("A", 1.0, 100.0, 10.0),
                    ("B", -1.0, 100.0, 10.0),
                ])
            })
            .collect();
        snaps.push(make_snapshot("2026-01-05", vec![
            ("A", 1.0, 20.0, 10.5),
            ("B", -1.0, 20.0, 9.5),
        ]));
        let (score, ratio) = calc_volume_level_score(&snaps);
        assert!(ratio < 0.5, "ratio 应 < 0.5, 实际: {}", ratio);
        assert!(score < 40.0, "score 应 < 40, 实际: {}", score);
    }

    #[test]
    fn test_price_position_high() {
        // 股票 A：5 天价格 10, 11, 12, 13, 14 → 当前在最高点 → position=1.0
        let snaps: Vec<MarketDailySnapshot> = vec![
            make_snapshot("2026-01-01", vec![("A", 0.0, 100.0, 10.0)]),
            make_snapshot("2026-01-02", vec![("A", 10.0, 100.0, 11.0)]),
            make_snapshot("2026-01-03", vec![("A", 9.0, 100.0, 12.0)]),
            make_snapshot("2026-01-04", vec![("A", 8.0, 100.0, 13.0)]),
            make_snapshot("2026-01-05", vec![("A", 7.0, 100.0, 14.0)]),
        ];
        let (score, pos) = calc_price_position_score(&snaps);
        assert!((pos - 1.0).abs() < 1e-6, "pos 应为 1.0, 实际: {}", pos);
        assert!((score - 100.0).abs() < 1e-6, "score 应为 100, 实际: {}", score);
    }

    #[test]
    fn test_price_position_low() {
        // 股票 A：5 天价格 14, 13, 12, 11, 10 → 当前在最低点 → position=0.0
        let snaps: Vec<MarketDailySnapshot> = vec![
            make_snapshot("2026-01-01", vec![("A", 0.0, 100.0, 14.0)]),
            make_snapshot("2026-01-02", vec![("A", -7.0, 100.0, 13.0)]),
            make_snapshot("2026-01-03", vec![("A", -8.0, 100.0, 12.0)]),
            make_snapshot("2026-01-04", vec![("A", -9.0, 100.0, 11.0)]),
            make_snapshot("2026-01-05", vec![("A", -10.0, 100.0, 10.0)]),
        ];
        let (score, pos) = calc_price_position_score(&snaps);
        assert!((pos - 0.0).abs() < 1e-6, "pos 应为 0.0, 实际: {}", pos);
        assert!((score - 0.0).abs() < 1e-6, "score 应为 0, 实际: {}", score);
    }

    #[test]
    fn test_price_position_mid() {
        // 股票 A：5 天价格 10, 12, 14, 12, 10 → 当前在最低点(10) → 但 high=14, low=10
        // position = (10-10)/(14-10) = 0
        // 改为：10, 12, 14, 13, 12 → current=12, high=14, low=10 → (12-10)/(14-10)=0.5
        let snaps: Vec<MarketDailySnapshot> = vec![
            make_snapshot("2026-01-01", vec![("A", 0.0, 100.0, 10.0)]),
            make_snapshot("2026-01-02", vec![("A", 20.0, 100.0, 12.0)]),
            make_snapshot("2026-01-03", vec![("A", 16.7, 100.0, 14.0)]),
            make_snapshot("2026-01-04", vec![("A", -7.0, 100.0, 13.0)]),
            make_snapshot("2026-01-05", vec![("A", -7.7, 100.0, 12.0)]),
        ];
        let (score, pos) = calc_price_position_score(&snaps);
        assert!((pos - 0.5).abs() < 1e-3, "pos 应为 0.5, 实际: {}", pos);
        assert!((score - 50.0).abs() < 1e-3, "score 应为 50, 实际: {}", score);
    }

    #[test]
    fn test_classify_signal_strong_bullish() {
        // MSI 高 + 放量 + 价格中位 → 强势上涨
        match classify_signal(75.0, 1.8, 0.5) {
            SignalType::StrongBullish => {}
            other => panic!("应为 StrongBullish, 实际: {:?}", other),
        }
    }

    #[test]
    fn test_classify_signal_panic() {
        // MSI 低 + 放量 + 价格低位 → 恐慌抛售
        match classify_signal(25.0, 1.8, 0.1) {
            SignalType::PanicSelling => {}
            other => panic!("应为 PanicSelling, 实际: {:?}", other),
        }
    }

    #[test]
    fn test_classify_signal_bottoming() {
        // MSI 低 + 缩量 + 价格低位 → 地量地价
        match classify_signal(25.0, 0.3, 0.1) {
            SignalType::Bottoming => {}
            other => panic!("应为 Bottoming, 实际: {:?}", other),
        }
    }

    #[test]
    fn test_classify_signal_divergence() {
        // MSI 高 + 缩量 + 价格高位 → 量价背离
        match classify_signal(75.0, 0.3, 0.9) {
            SignalType::DivergenceWarning => {}
            other => panic!("应为 DivergenceWarning, 实际: {:?}", other),
        }
    }

    #[test]
    fn test_volume_status_classification() {
        assert!(matches!(VolumeStatus::from_ratio(2.5), VolumeStatus::ExtremeHigh));
        assert!(matches!(VolumeStatus::from_ratio(1.6), VolumeStatus::High));
        assert!(matches!(VolumeStatus::from_ratio(1.0), VolumeStatus::Normal));
        assert!(matches!(VolumeStatus::from_ratio(0.4), VolumeStatus::Low));
        assert!(matches!(VolumeStatus::from_ratio(0.2), VolumeStatus::ExtremeLow));
    }

    #[test]
    fn test_price_status_classification() {
        assert!(matches!(PriceStatus::from_position(0.9), PriceStatus::HighZone));
        assert!(matches!(PriceStatus::from_position(0.7), PriceStatus::UpperMid));
        assert!(matches!(PriceStatus::from_position(0.5), PriceStatus::MidZone));
        assert!(matches!(PriceStatus::from_position(0.3), PriceStatus::LowerMid));
        assert!(matches!(PriceStatus::from_position(0.1), PriceStatus::LowZone));
    }

    #[test]
    fn test_calculate_msi_all_up() {
        // 全市场全涨 + 放量 + 价格在高位 → MSI 应偏高
        let snaps: Vec<MarketDailySnapshot> = (0..5)
            .map(|i| {
                let price = 10.0 + i as f64;
                make_snapshot(&format!("2026-01-0{}", i + 1), vec![
                    ("A", 10.0, 100.0 + i as f64 * 50.0, price),
                    ("B", 8.0, 100.0 + i as f64 * 50.0, price + 1.0),
                    ("C", 5.0, 100.0 + i as f64 * 50.0, price + 2.0),
                ])
            })
            .collect();
        let input = MarketDataInput { snapshots: snaps };
        let result = calculate_msi(&input);
        assert!(result.value > 60.0, "全涨+放量+高位 MSI 应 > 60, 实际: {}", result.value);
    }

    #[test]
    fn test_calculate_msi_all_down() {
        // 全市场全跌 + 缩量 + 价格在低位 → MSI 应偏低
        let snaps: Vec<MarketDailySnapshot> = (0..5)
            .map(|i| {
                let price = 14.0 - i as f64;
                let amount = 200.0 - i as f64 * 30.0; // 递减=缩量
                make_snapshot(&format!("2026-01-0{}", i + 1), vec![
                    ("A", -10.0, amount, price),
                    ("B", -8.0, amount, price - 1.0),
                    ("C", -5.0, amount, price - 2.0),
                ])
            })
            .collect();
        let input = MarketDataInput { snapshots: snaps };
        let result = calculate_msi(&input);
        assert!(result.value < 40.0, "全跌+缩量+低位 MSI 应 < 40, 实际: {}", result.value);
    }
}

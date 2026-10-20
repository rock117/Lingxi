//! 条件选股：策略目录 + 组合表达式求值。
//! 策略定义与 `docs/strategies.md` 对齐（首批 3 个）。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::domain::{MarketDataInput, StockDailyData};

const AMOUNT_MIN: f64 = 3.0e7;
const LIMIT_PCT: f64 = 9.5;
const LOOKBACK_DAYS: usize = 60;

/// 策略参数字段（供 App 动态表单）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyParamDef {
    pub key: String,
    pub label: String,
    /// number | enum
    pub kind: String,
    pub default: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<StrategyEnumOption>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyEnumOption {
    pub value: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyDef {
    pub type_id: String,
    pub name: String,
    pub summary: String,
    pub params: Vec<StrategyParamDef>,
    /// 可直接套用的默认规则 JSON（单条 rule）。
    pub default_rule: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenExpression {
    #[serde(default = "default_and")]
    pub op: String,
    pub rules: Vec<serde_json::Value>,
}

fn default_and() -> String {
    "and".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenMatch {
    pub symbol: String,
    pub name: String,
    pub change_pct: f64,
    pub amount: f64,
    pub close_price: f64,
    /// 命中的策略 type 列表
    pub hit_rules: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenResult {
    pub date: String,
    pub sample_size: usize,
    pub match_count: usize,
    pub items: Vec<ScreenMatch>,
}

pub fn strategy_catalog() -> Vec<StrategyDef> {
    vec![
        StrategyDef {
            type_id: "ma_cross".into(),
            name: "均线金叉".into(),
            summary: "短期均线上穿长期均线（金叉）".into(),
            params: vec![
                num_param("fast_period", "短期周期", 5.0, 2.0, 60.0),
                num_param("slow_period", "长期周期", 20.0, 5.0, 120.0),
                enum_param(
                    "ma_type",
                    "均线类型",
                    "ema",
                    &[("sma", "SMA"), ("ema", "EMA")],
                ),
                enum_param(
                    "direction",
                    "方向",
                    "golden",
                    &[("golden", "金叉"), ("death", "死叉")],
                ),
            ],
            default_rule: serde_json::json!({
                "type": "ma_cross",
                "fast_period": 5,
                "slow_period": 20,
                "ma_type": "ema",
                "direction": "golden"
            }),
        },
        StrategyDef {
            type_id: "rsi".into(),
            name: "RSI 超卖".into(),
            summary: "RSI 从超卖区回升上穿阈值".into(),
            params: vec![
                num_param("period", "周期", 14.0, 5.0, 30.0),
                num_param("oversold", "超卖线", 30.0, 10.0, 40.0),
                num_param("overbought", "超买线", 70.0, 60.0, 90.0),
                enum_param(
                    "direction",
                    "方向",
                    "oversold",
                    &[
                        ("oversold", "超卖回升"),
                        ("overbought", "超买回落"),
                    ],
                ),
            ],
            default_rule: serde_json::json!({
                "type": "rsi",
                "period": 14,
                "oversold": 30.0,
                "overbought": 70.0,
                "direction": "oversold"
            }),
        },
        StrategyDef {
            type_id: "volume_spike".into(),
            name: "放量上涨".into(),
            summary: "成交额相对均额放大，且价格同向".into(),
            params: vec![
                num_param("period", "均额周期", 20.0, 5.0, 60.0),
                num_param("multiplier", "放大倍数", 2.0, 1.2, 5.0),
                enum_param(
                    "price_direction",
                    "价格方向",
                    "up",
                    &[("up", "上涨"), ("down", "下跌"), ("any", "不限")],
                ),
            ],
            default_rule: serde_json::json!({
                "type": "volume_spike",
                "period": 20,
                "multiplier": 2.0,
                "price_direction": "up"
            }),
        },
        StrategyDef {
            type_id: "macd_cross".into(),
            name: "MACD 金叉".into(),
            summary: "DIF 上穿 DEA（金叉）；可配死叉".into(),
            params: vec![
                num_param("fast_period", "快线周期", 12.0, 5.0, 30.0),
                num_param("slow_period", "慢线周期", 26.0, 10.0, 60.0),
                num_param("signal_period", "信号线周期", 9.0, 3.0, 20.0),
                enum_param(
                    "direction",
                    "方向",
                    "bullish",
                    &[("bullish", "金叉"), ("bearish", "死叉")],
                ),
            ],
            default_rule: serde_json::json!({
                "type": "macd_cross",
                "fast_period": 12,
                "slow_period": 26,
                "signal_period": 9,
                "direction": "bullish"
            }),
        },
        StrategyDef {
            type_id: "bollinger".into(),
            name: "布林带突破".into(),
            summary: "收盘价突破布林上轨或下轨".into(),
            params: vec![
                num_param("period", "中轨周期", 20.0, 5.0, 60.0),
                num_param("std_dev", "标准差倍数", 2.0, 1.0, 3.0),
                enum_param(
                    "direction",
                    "方向",
                    "lower",
                    &[
                        ("lower", "突破下轨"),
                        ("upper", "突破上轨"),
                        ("both", "上下均可"),
                    ],
                ),
            ],
            default_rule: serde_json::json!({
                "type": "bollinger",
                "period": 20,
                "std_dev": 2.0,
                "direction": "lower"
            }),
        },
        StrategyDef {
            type_id: "price_breakout".into(),
            name: "价格突破".into(),
            summary: "收盘突破近 N 日最高/最低".into(),
            params: vec![
                num_param("period", "回看周期", 20.0, 5.0, 60.0),
                num_param("confirm_bars", "确认K线数", 1.0, 1.0, 3.0),
                enum_param(
                    "direction",
                    "方向",
                    "breakout_up",
                    &[
                        ("breakout_up", "突破阻力"),
                        ("breakout_down", "跌破支撑"),
                    ],
                ),
            ],
            default_rule: serde_json::json!({
                "type": "price_breakout",
                "period": 20,
                "direction": "breakout_up",
                "confirm_bars": 1
            }),
        },
        StrategyDef {
            type_id: "ma_alignment".into(),
            name: "均线多头排列".into(),
            summary: "短 > 中 > 长均线且同步向上".into(),
            params: vec![
                num_param("short_period", "短期", 5.0, 2.0, 30.0),
                num_param("mid_period", "中期", 10.0, 5.0, 60.0),
                num_param("long_period", "长期", 20.0, 10.0, 120.0),
                enum_param(
                    "ma_type",
                    "均线类型",
                    "ema",
                    &[("sma", "SMA"), ("ema", "EMA")],
                ),
                enum_param(
                    "direction",
                    "方向",
                    "bullish",
                    &[("bullish", "多头排列"), ("bearish", "空头排列")],
                ),
            ],
            default_rule: serde_json::json!({
                "type": "ma_alignment",
                "short_period": 5,
                "mid_period": 10,
                "long_period": 20,
                "periods": [5, 10, 20],
                "ma_type": "ema",
                "direction": "bullish"
            }),
        },
        StrategyDef {
            type_id: "price_change".into(),
            name: "涨跌幅区间".into(),
            summary: "当日涨跌幅落在设定区间内".into(),
            params: vec![
                num_param("min_pct", "最小涨跌幅%", 2.0, -9.0, 9.0),
                num_param("max_pct", "最大涨跌幅%", 7.0, -9.0, 9.5),
            ],
            default_rule: serde_json::json!({
                "type": "price_change",
                "min_pct": 2.0,
                "max_pct": 7.0
            }),
        },
    ]
}

fn num_param(key: &str, label: &str, default: f64, min: f64, max: f64) -> StrategyParamDef {
    StrategyParamDef {
        key: key.into(),
        label: label.into(),
        kind: "number".into(),
        default: serde_json::json!(default),
        min: Some(min),
        max: Some(max),
        options: None,
    }
}

fn enum_param(
    key: &str,
    label: &str,
    default: &str,
    opts: &[(&str, &str)],
) -> StrategyParamDef {
    StrategyParamDef {
        key: key.into(),
        label: label.into(),
        kind: "enum".into(),
        default: serde_json::json!(default),
        min: None,
        max: None,
        options: Some(
            opts.iter()
                .map(|(v, l)| StrategyEnumOption {
                    value: (*v).into(),
                    label: (*l).into(),
                })
                .collect(),
        ),
    }
}

pub fn parse_expression(raw: &str) -> Result<ScreenExpression, String> {
    let v: serde_json::Value =
        serde_json::from_str(raw).map_err(|e| format!("expression JSON 无效: {e}"))?;
    if let Some(obj) = v.as_object() {
        if obj.contains_key("rules") {
            return serde_json::from_value(v).map_err(|e| format!("expression 解析失败: {e}"));
        }
        if obj.contains_key("type") {
            return Ok(ScreenExpression {
                op: "and".into(),
                rules: vec![v],
            });
        }
    }
    Err("expression 需为 {op,rules} 或单条策略 rule".into())
}

pub fn required_lookback(expr: &ScreenExpression) -> usize {
    let mut need = 5usize;
    for rule in &expr.rules {
        let t = rule.get("type").and_then(|x| x.as_str()).unwrap_or("");
        match t {
            "ma_cross" => {
                let slow = rule
                    .get("slow_period")
                    .and_then(|x| x.as_u64())
                    .unwrap_or(20) as usize;
                need = need.max(slow + 2);
            }
            "rsi" => {
                let p = rule.get("period").and_then(|x| x.as_u64()).unwrap_or(14) as usize;
                need = need.max(p + 3);
            }
            "volume_spike" | "bollinger" | "price_breakout" => {
                let p = rule.get("period").and_then(|x| x.as_u64()).unwrap_or(20) as usize;
                need = need.max(p + 2);
            }
            "macd_cross" => {
                let slow = rule
                    .get("slow_period")
                    .and_then(|x| x.as_u64())
                    .unwrap_or(26) as usize;
                let sig = rule
                    .get("signal_period")
                    .and_then(|x| x.as_u64())
                    .unwrap_or(9) as usize;
                need = need.max(slow + sig + 5);
            }
            "ma_alignment" => {
                let long = alignment_periods(rule).3;
                need = need.max(long + 2);
            }
            "price_change" => {}
            _ => {}
        }
    }
    need.min(LOOKBACK_DAYS)
}

pub fn run_screen(input: &MarketDataInput, expr: &ScreenExpression, top_n: usize) -> ScreenResult {
    let Some(today) = input.snapshots.last() else {
        return ScreenResult {
            date: String::new(),
            sample_size: 0,
            match_count: 0,
            items: vec![],
        };
    };
    let date = today.date.clone();
    let op_and = expr.op.eq_ignore_ascii_case("and");

    let hist_map = build_series_map(input);
    let mut items = Vec::new();

    for s in &today.stocks {
        if !universe_ok(s) {
            continue;
        }
        let Some(series) = hist_map.get(&s.symbol) else {
            continue;
        };
        let mut hits = Vec::new();
        let mut ok_any = false;
        let mut ok_all = true;
        for rule in &expr.rules {
            let tid = rule
                .get("type")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let pass = eval_rule(rule, series, s);
            if pass {
                hits.push(tid);
                ok_any = true;
            } else {
                ok_all = false;
            }
        }
        if expr.rules.is_empty() {
            continue;
        }
        let matched = if op_and { ok_all } else { ok_any };
        if matched {
            items.push(ScreenMatch {
                symbol: s.symbol.clone(),
                name: String::new(),
                change_pct: s.change_pct,
                amount: s.amount,
                close_price: s.close_price,
                hit_rules: hits,
            });
        }
    }

    let sample_size = today.stocks.iter().filter(|s| universe_ok(s)).count();
    items.sort_by(|a, b| {
        b.change_pct
            .partial_cmp(&a.change_pct)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let match_count = items.len();
    items.truncate(top_n.max(1));

    ScreenResult {
        date,
        sample_size,
        match_count,
        items,
    }
}

fn universe_ok(s: &StockDailyData) -> bool {
    s.amount >= AMOUNT_MIN
        && s.change_pct.abs() < LIMIT_PCT
        && s.close_price > 0.0
        && s.open > 0.0
}

struct Bar {
    high: f64,
    low: f64,
    close: f64,
    amount: f64,
}

fn build_series_map(input: &MarketDataInput) -> HashMap<String, Vec<Bar>> {
    let mut map: HashMap<String, Vec<Bar>> = HashMap::new();
    for snap in &input.snapshots {
        for s in &snap.stocks {
            map.entry(s.symbol.clone()).or_default().push(Bar {
                high: s.high,
                low: s.low,
                close: s.close_price,
                amount: s.amount,
            });
        }
    }
    map
}

fn eval_rule(rule: &serde_json::Value, series: &[Bar], today: &StockDailyData) -> bool {
    let t = rule.get("type").and_then(|x| x.as_str()).unwrap_or("");
    match t {
        "ma_cross" => eval_ma_cross(rule, series),
        "rsi" => eval_rsi(rule, series),
        "volume_spike" => eval_volume_spike(rule, series, today),
        "macd_cross" => eval_macd_cross(rule, series),
        "bollinger" => eval_bollinger(rule, series),
        "price_breakout" => eval_price_breakout(rule, series),
        "ma_alignment" => eval_ma_alignment(rule, series),
        "price_change" => eval_price_change(rule, today),
        _ => false,
    }
}

fn eval_ma_cross(rule: &serde_json::Value, series: &[Bar]) -> bool {
    let fast_n = rule
        .get("fast_period")
        .and_then(|x| x.as_u64())
        .unwrap_or(5) as usize;
    let slow_n = rule
        .get("slow_period")
        .and_then(|x| x.as_u64())
        .unwrap_or(20) as usize;
    if fast_n == 0 || slow_n == 0 || fast_n >= slow_n || series.len() < slow_n + 1 {
        return false;
    }
    let use_ema = rule
        .get("ma_type")
        .and_then(|x| x.as_str())
        .unwrap_or("ema")
        .eq_ignore_ascii_case("ema");
    let closes: Vec<f64> = series.iter().map(|b| b.close).collect();
    let (fast_prev, fast_now) = ma_pair(&closes, fast_n, use_ema);
    let (slow_prev, slow_now) = ma_pair(&closes, slow_n, use_ema);
    let dir = rule
        .get("direction")
        .and_then(|x| x.as_str())
        .unwrap_or("golden");
    match dir {
        "death" => fast_prev >= slow_prev && fast_now < slow_now,
        _ => fast_prev <= slow_prev && fast_now > slow_now,
    }
}

fn ma_pair(closes: &[f64], period: usize, use_ema: bool) -> (f64, f64) {
    if use_ema {
        let ema = ema_series(closes, period);
        let n = ema.len();
        (ema[n - 2], ema[n - 1])
    } else {
        let sma = sma_series(closes, period);
        let n = sma.len();
        (sma[n - 2], sma[n - 1])
    }
}

fn sma_series(closes: &[f64], period: usize) -> Vec<f64> {
    let mut out = Vec::with_capacity(closes.len());
    let mut sum = 0.0;
    for (i, &c) in closes.iter().enumerate() {
        sum += c;
        if i >= period {
            sum -= closes[i - period];
        }
        if i + 1 >= period {
            out.push(sum / period as f64);
        } else {
            out.push(f64::NAN);
        }
    }
    out
}

fn ema_series(closes: &[f64], period: usize) -> Vec<f64> {
    let mut out = Vec::with_capacity(closes.len());
    let k = 2.0 / (period as f64 + 1.0);
    let mut ema = closes[0];
    for (i, &c) in closes.iter().enumerate() {
        if i == 0 {
            ema = c;
        } else if i + 1 < period {
            ema = (ema * i as f64 + c) / (i as f64 + 1.0);
        } else if i + 1 == period {
            let sum: f64 = closes[..=i].iter().sum();
            ema = sum / period as f64;
        } else {
            ema = c * k + ema * (1.0 - k);
        }
        out.push(ema);
    }
    out
}

fn eval_rsi(rule: &serde_json::Value, series: &[Bar]) -> bool {
    let period = rule.get("period").and_then(|x| x.as_u64()).unwrap_or(14) as usize;
    if period == 0 || series.len() < period + 2 {
        return false;
    }
    let closes: Vec<f64> = series.iter().map(|b| b.close).collect();
    let rsi = rsi_series(&closes, period);
    let n = rsi.len();
    if n < 2 || !rsi[n - 1].is_finite() || !rsi[n - 2].is_finite() {
        return false;
    }
    let oversold = rule
        .get("oversold")
        .and_then(|x| x.as_f64())
        .unwrap_or(30.0);
    let overbought = rule
        .get("overbought")
        .and_then(|x| x.as_f64())
        .unwrap_or(70.0);
    let dir = rule
        .get("direction")
        .and_then(|x| x.as_str())
        .unwrap_or("oversold");
    match dir {
        "overbought" => rsi[n - 2] >= overbought && rsi[n - 1] < overbought,
        _ => rsi[n - 2] <= oversold && rsi[n - 1] > oversold,
    }
}

fn rsi_series(closes: &[f64], period: usize) -> Vec<f64> {
    let mut out = vec![f64::NAN; closes.len()];
    if closes.len() <= period {
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

fn eval_volume_spike(rule: &serde_json::Value, series: &[Bar], today: &StockDailyData) -> bool {
    let period = rule.get("period").and_then(|x| x.as_u64()).unwrap_or(20) as usize;
    if period == 0 || series.len() < period + 1 {
        return false;
    }
    let hist = &series[series.len() - period - 1..series.len() - 1];
    let avg: f64 = hist.iter().map(|b| b.amount).sum::<f64>() / period as f64;
    if avg <= 0.0 {
        return false;
    }
    let mult = rule
        .get("multiplier")
        .and_then(|x| x.as_f64())
        .unwrap_or(2.0);
    if today.amount < avg * mult {
        return false;
    }
    let dir = rule
        .get("price_direction")
        .and_then(|x| x.as_str())
        .unwrap_or("up");
    match dir {
        "down" => today.change_pct < 0.0,
        "any" => true,
        _ => today.change_pct > 0.0,
    }
}

fn eval_macd_cross(rule: &serde_json::Value, series: &[Bar]) -> bool {
    let fast_n = rule
        .get("fast_period")
        .and_then(|x| x.as_u64())
        .unwrap_or(12) as usize;
    let slow_n = rule
        .get("slow_period")
        .and_then(|x| x.as_u64())
        .unwrap_or(26) as usize;
    let sig_n = rule
        .get("signal_period")
        .and_then(|x| x.as_u64())
        .unwrap_or(9) as usize;
    if fast_n == 0 || slow_n <= fast_n || sig_n == 0 || series.len() < slow_n + sig_n {
        return false;
    }
    let closes: Vec<f64> = series.iter().map(|b| b.close).collect();
    let fast = ema_series(&closes, fast_n);
    let slow = ema_series(&closes, slow_n);
    let dif: Vec<f64> = fast
        .iter()
        .zip(slow.iter())
        .map(|(f, s)| f - s)
        .collect();
    let dea = ema_series(&dif, sig_n);
    let n = dif.len();
    if n < 2 {
        return false;
    }
    let hist_prev = dif[n - 2] - dea[n - 2];
    let hist_now = dif[n - 1] - dea[n - 1];
    let cross_up = dif[n - 2] <= dea[n - 2] && dif[n - 1] > dea[n - 1] && hist_prev <= 0.0 && hist_now > 0.0;
    let cross_dn = dif[n - 2] >= dea[n - 2] && dif[n - 1] < dea[n - 1] && hist_prev >= 0.0 && hist_now < 0.0;
    let dir = rule
        .get("direction")
        .and_then(|x| x.as_str())
        .unwrap_or("bullish");
    match dir {
        "bearish" => cross_dn,
        "both" => cross_up || cross_dn,
        _ => cross_up,
    }
}

fn eval_bollinger(rule: &serde_json::Value, series: &[Bar]) -> bool {
    let period = rule.get("period").and_then(|x| x.as_u64()).unwrap_or(20) as usize;
    let k = rule.get("std_dev").and_then(|x| x.as_f64()).unwrap_or(2.0);
    if period < 2 || series.len() < period + 1 {
        return false;
    }
    let closes: Vec<f64> = series.iter().map(|b| b.close).collect();
    let n = closes.len();
    let (mid_prev, upper_prev, lower_prev) = bollinger_at(&closes, n - 2, period, k);
    let (mid_now, upper_now, lower_now) = bollinger_at(&closes, n - 1, period, k);
    if ![mid_prev, upper_prev, lower_prev, mid_now, upper_now, lower_now]
        .iter()
        .all(|x| x.is_finite())
    {
        return false;
    }
    let c_prev = closes[n - 2];
    let c_now = closes[n - 1];
    let break_lower = c_prev < lower_prev && c_now >= lower_now;
    let break_upper = c_prev < upper_prev && c_now >= upper_now;
    let dir = rule
        .get("direction")
        .and_then(|x| x.as_str())
        .unwrap_or("lower");
    match dir {
        "upper" => break_upper,
        "both" => break_lower || break_upper,
        _ => break_lower,
    }
}

fn bollinger_at(closes: &[f64], idx: usize, period: usize, k: f64) -> (f64, f64, f64) {
    if idx + 1 < period {
        return (f64::NAN, f64::NAN, f64::NAN);
    }
    let start = idx + 1 - period;
    let window = &closes[start..=idx];
    let mean = window.iter().sum::<f64>() / period as f64;
    let var = window.iter().map(|c| (c - mean).powi(2)).sum::<f64>() / period as f64;
    let std = var.sqrt();
    (mean, mean + k * std, mean - k * std)
}

fn eval_price_breakout(rule: &serde_json::Value, series: &[Bar]) -> bool {
    let period = rule.get("period").and_then(|x| x.as_u64()).unwrap_or(20) as usize;
    let confirm = rule
        .get("confirm_bars")
        .and_then(|x| x.as_u64())
        .unwrap_or(1)
        .max(1) as usize;
    if period == 0 || series.len() < period + confirm {
        return false;
    }
    let end = series.len();
    let look_end = end - confirm;
    let look_start = look_end.saturating_sub(period);
    let window = &series[look_start..look_end];
    if window.is_empty() {
        return false;
    }
    let high = window.iter().map(|b| b.high).fold(f64::NEG_INFINITY, f64::max);
    let low = window.iter().map(|b| b.low).fold(f64::INFINITY, f64::min);
    let confirm_bars = &series[end - confirm..];
    let dir = rule
        .get("direction")
        .and_then(|x| x.as_str())
        .unwrap_or("breakout_up");
    match dir {
        "breakout_down" => confirm_bars.iter().all(|b| b.close < low),
        _ => confirm_bars.iter().all(|b| b.close > high),
    }
}

/// returns (short, mid, long, long_n)
fn alignment_periods(rule: &serde_json::Value) -> (usize, usize, usize, usize) {
    if let Some(arr) = rule.get("periods").and_then(|x| x.as_array()) {
        if arr.len() >= 3 {
            let a = arr[0].as_u64().unwrap_or(5) as usize;
            let b = arr[1].as_u64().unwrap_or(10) as usize;
            let c = arr[2].as_u64().unwrap_or(20) as usize;
            return (a, b, c, c);
        }
    }
    let a = rule
        .get("short_period")
        .and_then(|x| x.as_u64())
        .unwrap_or(5) as usize;
    let b = rule
        .get("mid_period")
        .and_then(|x| x.as_u64())
        .unwrap_or(10) as usize;
    let c = rule
        .get("long_period")
        .and_then(|x| x.as_u64())
        .unwrap_or(20) as usize;
    (a, b, c, c)
}

fn eval_ma_alignment(rule: &serde_json::Value, series: &[Bar]) -> bool {
    let (p1, p2, p3, _) = alignment_periods(rule);
    if p1 == 0 || p2 <= p1 || p3 <= p2 || series.len() < p3 + 1 {
        return false;
    }
    let use_ema = rule
        .get("ma_type")
        .and_then(|x| x.as_str())
        .unwrap_or("ema")
        .eq_ignore_ascii_case("ema");
    let closes: Vec<f64> = series.iter().map(|b| b.close).collect();
    let (s_prev, s_now) = ma_pair(&closes, p1, use_ema);
    let (m_prev, m_now) = ma_pair(&closes, p2, use_ema);
    let (l_prev, l_now) = ma_pair(&closes, p3, use_ema);
    let dir = rule
        .get("direction")
        .and_then(|x| x.as_str())
        .unwrap_or("bullish");
    match dir {
        "bearish" => {
            s_now < m_now
                && m_now < l_now
                && s_now < s_prev
                && m_now < m_prev
                && l_now < l_prev
        }
        _ => {
            s_now > m_now
                && m_now > l_now
                && s_now > s_prev
                && m_now > m_prev
                && l_now > l_prev
        }
    }
}

fn eval_price_change(rule: &serde_json::Value, today: &StockDailyData) -> bool {
    let min_pct = rule.get("min_pct").and_then(|x| x.as_f64()).unwrap_or(2.0);
    let max_pct = rule.get("max_pct").and_then(|x| x.as_f64()).unwrap_or(7.0);
    let lo = min_pct.min(max_pct);
    let hi = min_pct.max(max_pct);
    today.change_pct >= lo && today.change_pct <= hi
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market::domain::MarketDailySnapshot;

    fn bar(close: f64, amount: f64, chg: f64) -> StockDailyData {
        StockDailyData {
            symbol: "000001.SZ".into(),
            date: "2026-01-01".into(),
            open: close,
            high: close,
            low: close,
            close_price: close,
            amount,
            change_pct: chg,
        }
    }

    #[test]
    fn catalog_has_eight() {
        assert_eq!(strategy_catalog().len(), 8);
    }

    #[test]
    fn parse_single_rule_wraps() {
        let e = parse_expression(r#"{"type":"rsi","period":14,"direction":"oversold"}"#).unwrap();
        assert_eq!(e.rules.len(), 1);
    }

    #[test]
    fn volume_spike_matches() {
        let mut snaps = Vec::new();
        for i in 0..21 {
            snaps.push(MarketDailySnapshot {
                date: format!("2026-01-{:02}", i + 1),
                stocks: vec![bar(10.0, 4.0e7, 0.5)],
            });
        }
        snaps.push(MarketDailySnapshot {
            date: "2026-01-22".into(),
            stocks: vec![bar(10.5, 1.0e8, 2.0)],
        });
        let expr = parse_expression(
            r#"{"op":"and","rules":[{"type":"volume_spike","period":20,"multiplier":2.0,"price_direction":"up"}]}"#,
        )
        .unwrap();
        let r = run_screen(&MarketDataInput { snapshots: snaps }, &expr, 10);
        assert_eq!(r.match_count, 1);
    }
}

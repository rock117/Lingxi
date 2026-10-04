# 股票价格买卖信号策略设计

本文档设计 9 个常用的股票买卖信号提醒策略，每个策略包含原理、参数、信号条件、
expression JSON 格式（与后端 `condition.expression` 字段兼容）及适用场景。

## 目录

1. [均线交叉策略（金叉/死叉）](#1-均线交叉策略金叉死叉)
2. [RSI 超买超卖策略](#2-rsi-超买超卖策略)
3. [MACD 交叉策略](#3-macd-交叉策略)
4. [布林带突破策略](#4-布林带突破策略)
5. [成交量异常放大策略](#5-成交量异常放大策略)
6. [价格突破策略（支撑/阻力）](#6-价格突破策略支撑阻力)
7. [均线多空排列策略](#7-均线多空排列策略)
8. [大盘影响因素策略（个股 vs 大盘）](#8-大盘影响因素策略个股-vs-大盘)
9. [大盘成交量策略（市场情绪）](#9-大盘成交量策略市场情绪)

---

## 1. 均线交叉策略（金叉/死叉）

### 原理

短期均线向上穿越长期均线 → 金叉（买入信号）；
短期均线向下穿越长期均线 → 死叉（卖出信号）。

### 参数

| 参数 | 说明 | 默认值 |
|---|---|---|
| `fast_period` | 短期均线周期 | 5 |
| `slow_period` | 长期均线周期 | 20 |
| `ma_type` | 均线类型：`sma`(简单) / `ema`(指数) | `ema` |
| `direction` | 触发方向：`golden`(金叉) / `death`(死叉) / `both` | `both` |

### 信号条件

- **买入（金叉）**：`MA(fast)` 由下穿上穿 `MA(slow)`，即前一根 `fast < slow`，当前 `fast > slow`
- **卖出（死叉）**：`MA(fast)` 由上穿下穿 `MA(slow)`，即前一根 `fast > slow`，当前 `fast < slow`

### expression 格式

```json
{
  "type": "ma_cross",
  "fast_period": 5,
  "slow_period": 20,
  "ma_type": "ema",
  "direction": "golden"
}
```

### 优缺点

- ✅ 趋势跟踪，适合单边行情
- ❌ 震荡市频繁假信号
- 💡 常用组合：5/20 日金叉短线、10/60 日金叉中线

---

## 2. RSI 超买超卖策略

### 原理

RSI（相对强弱指数）衡量价格变动速度。RSI > 70 为超买区（可能回调），
RSI < 30 为超卖区（可能反弹）。

### 参数

| 参数 | 说明 | 默认值 |
|---|---|---|
| `period` | RSI 计算周期 | 14 |
| `overbought` | 超买阈值 | 70 |
| `oversold` | 超卖阈值 | 30 |
| `direction` | 触发方向：`overbought` / `oversold` / `both` | `both` |

### 信号条件

- **买入**：RSI 从超卖区（< 30）回升上穿 30
- **卖出**：RSI 从超买区（> 70）回落下穿 70

### expression 格式

```json
{
  "type": "rsi",
  "period": 14,
  "overbought": 70,
  "oversold": 30,
  "direction": "oversold"
}
```

### 优缺点

- ✅ 适合震荡市捕捉反转
- ❌ 强趋势中 RSI 可长期停留在超买/超卖区，信号失效
- 💡 可配合均线趋势过滤：只在上升趋势中用超卖买入

---

## 3. MACD 交叉策略

### 原理

MACD 由 DIF（快线）、DEA（慢线）、柱状图组成。DIF 上穿 DEA → 买入；
DIF 下穿 DEA → 卖出。柱状图由负转正也视为买入信号。

### 参数

| 参数 | 说明 | 默认值 |
|---|---|---|
| `fast_period` | 快线 EMA 周期 | 12 |
| `slow_period` | 慢线 EMA 周期 | 26 |
| `signal_period` | 信号线 EMA 周期 | 9 |
| `direction` | `bullish`(金叉) / `bearish`(死叉) / `both` | `both` |

### 信号条件

- **买入**：DIF 上穿 DEA（金叉），且柱状图由负转正
- **卖出**：DIF 下穿 DEA（死叉），且柱状图由正转负

### expression 格式

```json
{
  "type": "macd_cross",
  "fast_period": 12,
  "slow_period": 26,
  "signal_period": 9,
  "direction": "bullish"
}
```

### 优缺点

- ✅ 兼顾趋势与动量，信号较均线交叉更可靠
- ❌ 有滞后性，适合中短线
- 💡 零轴上方的金叉比零轴下方的更强势

---

## 4. 布林带突破策略

### 原理

布林带由中轨（20 日均线）+ 上下轨（±2 标准差）组成。价格触及上轨可能超买，
触及下轨可能超卖；突破上/下轨可能预示趋势启动。

### 参数

| 参数 | 说明 | 默认值 |
|---|---|---|
| `period` | 中轨均线周期 | 20 |
| `std_dev` | 标准差倍数 | 2.0 |
| `direction` | `upper`(突破上轨) / `lower`(突破下轨) / `both` | `both` |

### 信号条件

- **买入**：价格从下轨下方回升突破下轨（反弹买入），或突破上轨（趋势突破买入）
- **卖出**：价格从上轨上方回落跌破上轨

### expression 格式

```json
{
  "type": "bollinger",
  "period": 20,
  "std_dev": 2.0,
  "direction": "lower"
}
```

### 优缺点

- ✅ 兼顾波动率和价格位置
- ❌ 突破上/下轨后可能继续走远（强趋势中假信号多）
- 💡 建议配合成交量确认突破有效性

---

## 5. 成交量异常放大策略

### 原理

成交量突然放大到近期均量的 N 倍，通常预示大资金进出，可能伴随价格大幅波动。

### 参数

| 参数 | 说明 | 默认值 |
|---|---|---|
| `period` | 均量计算周期 | 20 |
| `multiplier` | 放大倍数阈值 | 2.0 |
| `price_direction` | 价格方向过滤：`up` / `down` / `any` | `any` |

### 信号条件

- **提醒**：当日成交量 > `period` 日均量 × `multiplier`
- 若 `price_direction = up`：同时要求当日涨幅 > 0
- 若 `price_direction = down`：同时要求当日跌幅 > 0

### expression 格式

```json
{
  "type": "volume_spike",
  "period": 20,
  "multiplier": 2.0,
  "price_direction": "up"
}
```

### 优缺点

- ✅ 捕捉主力资金异动，适合配合其他策略确认
- ❌ 单独使用无法判断方向
- 💡 放量上涨 → 可能启动；放量下跌 → 可能出货

---

## 6. 价格突破策略（支撑/阻力）

### 原理

价格突破近期高点（阻力位）可能开启上涨趋势；
跌破近期低点（支撑位）可能开启下跌趋势。

### 参数

| 参数 | 说明 | 默认值 |
|---|---|---|
| `period` | 回看周期（近 N 日高低点） | 20 |
| `direction` | `breakout_up`(突破阻力) / `breakout_down`(跌破支撑) / `both` | `both` |
| `confirm_bars` | 突破确认K线数 | 1 |

### 信号条件

- **买入**：收盘价突破近 `period` 日最高价，且连续 `confirm_bars` 根 K 线收在上方
- **卖出**：收盘价跌破近 `period` 日最低价，且连续 `confirm_bars` 根 K 线收在下方

### expression 格式

```json
{
  "type": "price_breakout",
  "period": 20,
  "direction": "breakout_up",
  "confirm_bars": 1
}
```

### 优缺点

- ✅ 趋势启动信号明确
- ❌ 假突破频繁，需配合成交量确认
- 💡 `confirm_bars = 2` 可减少假突破，但牺牲及时性

---

## 7. 均线多空排列策略

### 原理

短期 > 中期 > 长期均线依次排列 → 多头排列（强势上涨）；
短期 < 中期 < 长期均线依次排列 → 空头排列（强势下跌）。

### 参数

| 参数 | 说明 | 默认值 |
|---|---|---|
| `periods` | 均线周期列表 | `[5, 10, 20]` |
| `ma_type` | 均线类型：`sma` / `ema` | `ema` |
| `direction` | `bullish`(多头排列) / `bearish`(空头排列) / `both` | `both` |

### 信号条件

- **买入**：`MA(5) > MA(10) > MA(20)` 且三者同时向上发散
- **卖出**：`MA(5) < MA(10) < MA(20)` 且三者同时向下发散

### expression 格式

```json
{
  "type": "ma_alignment",
  "periods": [5, 10, 20],
  "ma_type": "ema",
  "direction": "bullish"
}
```

### 优缺点

- ✅ 强趋势确认，信号可靠
- ❌ 滞后性强，趋势末期才确认
- 💡 适合中长线趋势判断，不适合短线

---

## 8. 大盘影响因素策略（个股 vs 大盘）

### 原理

个股走势受大盘（如上证指数、沪深300）影响显著。通过对比个股涨跌幅与大盘涨跌幅，
判断个股是独立走强/走弱还是跟随大盘。当个股相对大盘出现显著偏离时，可能存在
交易机会或风险预警。

### 子策略

| 子策略 | 说明 |
|---|---|
| **逆势走强** | 大盘下跌但个股逆势上涨，可能有独立利好 |
| **逆势走弱** | 大盘上涨但个股逆势下跌，可能有独立利空 |
| **同步破位** | 个股与大盘同时跌破关键均线，系统性风险 |
| **超额波动** | 个股波动幅度远超大盘，资金异动 |

### 参数

| 参数 | 说明 | 默认值 |
|---|---|---|
| `index_code` | 大盘指数代码 | `sh000001`（上证指数） |
| `divergence_threshold` | 个股与大盘涨跌幅偏离阈值（百分点） | 2.0 |
| `lookback_days` | 回看天数（计算相关性） | 5 |
| `mode` | 子策略模式：`divergence_up` / `divergence_down` / `sync_breakdown` / `excess_volatility` / `any` | `any` |
| `ma_period` | 关键均线周期（`sync_breakdown` 模式用） | 20 |

### 信号条件

- **逆势走强（divergence_up）**：大盘跌幅 > 0，个股涨幅 > 0，且 `个股涨幅 - 大盘涨幅 > divergence_threshold`
- **逆势走弱（divergence_down）**：大盘涨幅 > 0，个股跌幅 > 0，且 `大盘涨幅 - 个股涨幅 > divergence_threshold`
- **同步破位（sync_breakdown）**：个股与大盘同时跌破各自 `ma_period` 日均线
- **超额波动（excess_volatility）**：近 `lookback_days` 日个股波动率 / 大盘波动率 > `divergence_threshold`

### expression 格式

```json
{
  "type": "market_divergence",
  "index_code": "sh000001",
  "divergence_threshold": 2.0,
  "lookback_days": 5,
  "mode": "divergence_up"
}
```

### 优缺点

- ✅ 捕捉个股独立行情，发现主力资金动向
- ✅ 系统性风险预警（同步破位）
- ❌ 需要同时获取个股和大盘数据
- 💡 逆势走强 + 成交量放大 → 强烈买入信号

---

## 9. 大盘成交量策略（市场情绪）

### 原理

大盘成交量反映市场整体活跃度和资金参与意愿。放量上涨代表资金积极入场，
缩量下跌代表抛压减轻；天量往往出现在阶段顶部或底部。

### 子策略

| 子策略 | 说明 |
|---|---|
| **放量上涨** | 大盘成交量放大 + 上涨，资金入场 |
| **缩量下跌** | 大盘成交量萎缩 + 下跌，抛压减弱 |
| **天量预警** | 成交量达到近期均量的极高倍数，可能见顶/见底 |
| **地量预警** | 成交量萎缩至近期极低水平，可能见底 |

### 参数

| 参数 | 说明 | 默认值 |
|---|---|---|
| `index_code` | 大盘指数代码 | `sh000001` |
| `period` | 均量计算周期 | 20 |
| `volume_multiplier` | 放量倍数阈值 | 1.5 |
| `shrink_multiplier` | 缩量倍数阈值（低于均量的比例） | 0.5 |
| `extreme_multiplier` | 天量倍数阈值 | 3.0 |
| `mode` | 子策略模式：`volume_up` / `shrink_down` / `extreme_high` / `extreme_low` / `any` | `any` |

### 信号条件

- **放量上涨（volume_up）**：大盘涨幅 > 0 且 成交量 > `period` 日均量 × `volume_multiplier`
- **缩量下跌（shrink_down）**：大盘跌幅 > 0 且 成交量 < `period` 日均量 × `shrink_multiplier`
- **天量预警（extreme_high）**：成交量 > `period` 日均量 × `extreme_multiplier`，提示可能见顶或见底
- **地量预警（extreme_low）**：成交量 < `period` 日均量 × `shrink_multiplier`，提示可能见底

### expression 格式

```json
{
  "type": "market_volume",
  "index_code": "sh000001",
  "period": 20,
  "volume_multiplier": 1.5,
  "mode": "volume_up"
}
```

### 优缺点

- ✅ 反映市场整体情绪，判断牛熊转换
- ✅ 天量/地量是经典顶底信号
- ❌ 单独使用无法判断个股，需配合个股策略
- 💡 大盘放量上涨 + 个股金叉 → 高胜率买入组合

### 常用大盘指数代码

| 代码 | 指数 |
|---|---|
| `sh000001` | 上证指数 |
| `sh000300` | 沪深300 |
| `sz399001` | 深证成指 |
| `sz399006` | 创业板指 |
| `sh000016` | 上证50 |

---

## expression 格式扩展说明

当前后端 `expression` 字段为 JSON 字符串，第一版仅支持 `trigger_rate` 随机触发。
接入真实行情数据后，后端 `matcher.rs` 需扩展 `Expression` 结构体以支持上述所有策略。

### 扩展后的 Expression 结构（Rust 伪代码）

```rust
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum Expression {
    MaCross {
        fast_period: u32,
        slow_period: u32,
        ma_type: String,       // "sma" | "ema"
        direction: String,     // "golden" | "death" | "both"
    },
    Rsi {
        period: u32,
        overbought: f64,
        oversold: f64,
        direction: String,
    },
    MacdCross {
        fast_period: u32,
        slow_period: u32,
        signal_period: u32,
        direction: String,
    },
    Bollinger {
        period: u32,
        std_dev: f64,
        direction: String,
    },
    VolumeSpike {
        period: u32,
        multiplier: f64,
        price_direction: String,
    },
    PriceBreakout {
        period: u32,
        direction: String,
        confirm_bars: u32,
    },
    MaAlignment {
        periods: Vec<u32>,
        ma_type: String,
        direction: String,
    },
    // 大盘影响因素
    MarketDivergence {
        index_code: String,
        divergence_threshold: f64,
        lookback_days: u32,
        mode: String,           // "divergence_up" | "divergence_down" | "sync_breakdown" | "excess_volatility" | "any"
        ma_period: Option<u32>,
    },
    // 大盘成交量
    MarketVolume {
        index_code: String,
        period: u32,
        volume_multiplier: f64,
        shrink_multiplier: f64,
        extreme_multiplier: f64,
        mode: String,           // "volume_up" | "shrink_down" | "extreme_high" | "extreme_low" | "any"
    },
    // 兼容 Mock 模式
    PriceDrop {
        threshold: f64,
        trigger_rate: Option<f64>,
    },
    PriceRise {
        threshold: f64,
        trigger_rate: Option<f64>,
    },
}
```

### 数据需求

| 策略 | 所需数据 |
|---|---|
| 均线交叉 | 日 K 线收盘价序列 |
| RSI | 日 K 线收盘价序列 |
| MACD | 日 K 线收盘价序列 |
| 布林带 | 日 K 线收盘价序列 |
| 成交量放大 | 日 K 线成交量序列 + 收盘价 |
| 价格突破 | 日 K 线最高/最低/收盘价 |
| 均线排列 | 日 K 线收盘价序列 |
| 大盘影响因素 | 个股 K 线 + 大盘指数 K 线（收盘价/均线） |
| 大盘成交量 | 大盘指数 K 线（成交量 + 收盘价） |

所有策略均基于日 K 线数据，每根 K 线需包含：`open, high, low, close, volume, date`。
大盘相关策略还需额外获取指数 K 线数据（同样的 OHLCV 格式）。

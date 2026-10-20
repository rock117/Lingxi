# 股票价格买卖信号策略设计

本文档描述条件选股已落地的个股日 K 策略：原理、参数、信号条件与 expression JSON。  
实现与 API 以 [`docs/screen.md`](./screen.md)、`backend/src/market/screen.rs` 为准。

另有原子策略 **涨跌幅区间** `price_change`，详见 `screen.md` §9。

## 目录

1. [均线交叉策略（金叉/死叉）](#1-均线交叉策略金叉死叉)
2. [RSI 超买超卖策略](#2-rsi-超买超卖策略)
3. [MACD 交叉策略](#3-macd-交叉策略)
4. [布林带突破策略](#4-布林带突破策略)
5. [成交量异常放大策略](#5-成交量异常放大策略)
6. [价格突破策略（支撑/阻力）](#6-价格突破策略支撑阻力)
7. [均线多空排列策略](#7-均线多空排列策略)

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

> 此处「中期」指本策略参数中的中间周期（默认 `MA10`）。  
> 全市场中期趋势指标（`MA20`/`MA60` 家数结构）见 [`mid_term_trend.md`](./mid_term_trend.md)，二者勿混用。

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

## expression 说明

选股组合为 `{ "op": "and"|"or", "rules": [ ... ] }`，单条 rule 的 `type` 对应上表策略；  
另支持 `price_change`（`min_pct` / `max_pct`）。求值见 `screen.rs`。

### 数据需求

| 策略 | 所需数据 |
|---|---|
| 均线交叉 / RSI / MACD / 布林 / 均线排列 | 日 K 收盘价序列 |
| 成交量放大 | 日 K 成交额 + 涨跌幅 |
| 价格突破 | 日 K 最高/最低/收盘 |
| 涨跌幅区间 | 当日涨跌幅 |

日 K 字段：`open, high, low, close, amount, change_pct, date`。

# 市场情绪指标（MSI）规范

> **本文档为规范（Source of Truth）。**  
> 凡文中出现的概念、因子、变量、阈值、判定规则，均须给出可实现的公式或伪代码。  
> 实现必须与本文档一致；算法变更须先改本文档再改代码。  
> 实现目录：`backend/src/market/`。

---

## 目录

1. [目标与结构](#1-目标与结构)
2. [规范常量](#2-规范常量)
3. [通用映射与 EWMA](#3-通用映射与-ewma)
4. [输入与样本过滤](#4-输入与样本过滤)
5. [单日情绪 `msi_daily`](#5-单日情绪-msi_daily)
6. [短线双读数](#6-短线双读数)
7. [信号（基于当日）](#7-信号基于当日)
8. [API](#8-api)
9. [代码映射](#9-代码映射)
10. [测试要求](#10-测试要求)
11. [定位](#11-定位)

---

## 1. 目标与结构

短线情绪拆成两部分，**请求时实时计算，不落库**：

| 读数 | 含义 | 数据 |
|---|---|---|
| **当日情绪** `today` | 今天盘面怎么样 | **仅用该交易日截面**（交易日连续竞价用实时，其余用库） |
| **近 N 日情绪** `recent` | 这几天情绪轨迹 | 过去 N 个交易日各自算 `msi_daily`，再加权 |

禁止再用「一次扫 N 日 K 线、因子内 EWMA」合成单一 MSI。跨日趋势由 `recent` 与 MTT 承担。

### 1.1 单日三层

对任意交易日截面：

```
msi_daily = W_DIRECTION * Direction
          + W_INTENSITY * Intensity
          + W_CONSTRAINT * Constraint
```

| 层 | 权重 | 回答什么 | 内部分量（**全部仅用当日**） |
|---|---|---|---|
| **Direction（方向）** | `0.50` | 市场站哪边 | breadth, money_flow, structure（三分量等权 `1/3`） |
| **Intensity（强度）** | `0.25` | 打得有多用力（方向一致性 × 波动幅度） | directed_move（仅此一项） |
| **Constraint（约束）** | `0.25` | 收在当日高低点何处 | intraday_position（仅此一项） |

**极性**：Constraint 高分表示收盘偏强（靠近当日最高），不是风险扣分。

**明确不进入单日情绪**：开盘缺口、新高新低、N 日区间位置、涨跌惯性、相对窗口量能比（均需跨日数据）。

### 1.2 设计原则

1. 股票 **等权**；不设成交额门槛。  
2. `N`：API `days`，默认 `5`，`clamp` 到 `[3, 30]`。  
3. 单日因子 **禁止** 读取其他交易日行情。  
4. `recent` **包含当日**：序列为「库内前 `N-1` 日日分 + 当日日分」（时间正序），再 EWMA。  
5. **开市日连续竞价**（`sc_trade_calendar.is_open = TRUE`，且上海时钟 `09:30–11:30` / `13:00–15:00`）用 Miana 覆盖当日截面；否则读库。最近 N 日只取日历中 `is_open` 日。  
6. 新因子必须可解释、可单测。

### 1.3 `map`

```
map(x) = 50 + 50 * clamp(x, -1, 1)
```

| `x` | 输出 |
|---|---|
| `1` | `100` |
| `0` | `50` |
| `-1` | `0` |

---

## 2. 规范常量

| 符号 | 值 | 说明 |
|---|---|---|
| `W_DIRECTION` | `0.50` | 方向层权重 |
| `W_INTENSITY` | `0.25` | 强度层权重 |
| `W_CONSTRAINT` | `0.25` | 约束层权重 |
| `W_DIR` | `1/3` | Direction 内 breadth / money_flow / structure 等权 |
| `EWMA_ALPHA` | `0.45` | 近 N 日日分加权的近端权重 |
| `RET_SCALE` | `2.0` | 结构因子：等权均涨跌幅（百分点）/`RET_SCALE` 后再 `tanh` |
| `AMP_SCALE` | `2.0` | 强度因子：等权均绝对涨跌幅（百分点）/`AMP_SCALE` 后再 `tanh` |
| `MSI_HIGH` / `MSI_LOW` | `65` / `35` | 多/空确认 |
| `MSI_GREY_BULL_*` | `60` / `65` | 多头灰区（含端点） |
| `MSI_GREY_BEAR_*` | `35` / `40` | 空头灰区（含端点） |
| `LIMIT_PCT` | `9.5` | 疑似涨/跌停涨跌幅阈值（百分点），非官方涨跌停 |
| `EPS_AMOUNT` | `1e-10` | 成交额保护 |
| `EPS_PRICE` | `1e-10` | 价格区间容差 |
| `INTRA_HIGH` / `INTRA_LOW` | `0.8` / `0.2` | 日内位置高/低位（信号用） |

---

## 3. 通用映射与 EWMA

### 3.1 EWMA（仅用于近 N 日日分序列）

输入时间正序：`v[0]` 最旧，`v[last]` 最新（含当日）。

```
function EWMA(v[], alpha):
    if v 为空: return 50          # 近 N 日中性兜底
    s = v[0]
    for t in 1 .. len(v)-1:
        s = alpha * v[t] + (1 - alpha) * s
    return s
```

`alpha = EWMA_ALPHA = 0.45`。

---

## 4. 输入与样本过滤

### 4.1 输入结构

```
MarketDataInput:
  snapshots: MarketDailySnapshot[]   # 时间正序，最后一项为当日/最近日
MarketDailySnapshot:
  date: string
  stocks: StockDailyData[]
StockDailyData:
  symbol, date, open, high, low, close_price, amount, change_pct
```

`change_pct` 单位为百分点。库内：`(close - pre_close) / pre_close * 100`；盘中实时：直接用接口 `changeRate`。

### 4.2 单票有效

```
valid if:
  open, high, low, close_price > 0
  high >= low, high >= open, high >= close, low <= open, low <= close
  amount >= 0
  全部 finite，含 change_pct
```

无效票丢弃。过滤后某日无票：该日 `msi_daily = 50`（中性），诊断计数为 0。

### 4.3 整窗无数据

`snapshots` 为空 → API `400`（路由层）；计算引擎若收到空窗返回中性双读数。

---

## 5. 单日情绪 `msi_daily`

输入：**单个** `MarketDailySnapshot`（已过滤）。记当日有效股票集合为 `stocks`，`n = len(stocks)`。

### 5.1 宽度 `breadth_score`

```
up   = count(change_pct > 0)
down = count(change_pct < 0)
# 平盘不进分母
if up + down == 0: breadth_raw = 0
else: breadth_raw = (up - down) / (up + down)
breadth_score = map(breadth_raw)
```

### 5.2 资金 `money_flow_score`

```
up_amount   = sum(amount where change_pct > 0)
down_amount = sum(amount where change_pct < 0)
total       = sum(all amount)          # 含平盘
if total < EPS_AMOUNT: money_raw = 0
else: money_raw = (up_amount - down_amount) / total
money_flow_score = map(money_raw)
```

### 5.3 结构 `structure_score`

**含义**：涨跌**家数方向**与等权**均涨跌幅**是否同向确认。同向偏多 → 高分；同向偏空 → 低分；背离 → 靠近中性。

**注意**：不可写成 `breadth_raw * ret_sig` 再 `map`——双负会乘成正，空头确认被翻成偏多。

```
majority = breadth_raw                              # 与 §5.1 相同
mean_ret = mean(change_pct of all valid stocks)     # 百分点，等权
ret_sig  = tanh(mean_ret / RET_SCALE)               # ∈ (-1, 1)
structure_raw = clamp( (majority + ret_sig) / 2, -1, 1 )
structure_score = map(structure_raw)
```

| 情况 | 结果 |
|---|---|
| 家数偏多且均涨幅为正 | 两者皆正 → 结构 > 50 |
| 家数偏空且均涨幅为负 | 两者皆负 → 结构 < 50 |
| 家数与均涨跌幅反向 | 正负相抵 → 靠近 50 |

### 5.4 有方向波动 `directed_move_score`（= Intensity）

**含义**：涨跌家数方向 × 全市场等权平均绝对涨跌幅。大面下跌且跌得深 → 强度分偏低；上涨且涨得狠 → 偏高。波动很小的一边倒接近中性。

涨跌停家数**不进入**强度层（仅诊断 / 信号辅助）。

```
breadth_raw = §5.1 的宽度原始量
mean_abs_ret = mean( |change_pct| of all valid stocks )   # 百分点，等权
amp = tanh( mean_abs_ret / AMP_SCALE )                    # ∈ (0, 1)
directed_move_raw = clamp( breadth_raw * amp, -1, 1 )
directed_move_score = map(directed_move_raw)
Intensity = directed_move_score
```

空样本：`Intensity = 50`，`mean_abs_ret = 0`。

### 5.5 日内位置 `intraday_position_score`（= Constraint）

对每只有效票：

```
if |high - low| < EPS_PRICE: intra_i = 0.5
else: intra_i = clamp( (close_price - low) / (high - low), 0, 1 )
intraday_raw = mean(intra_i)          # 无票则为 0.5
intraday_position_score = intraday_raw * 100
Constraint = intraday_position_score
```

### 5.6 合成

```
Direction = W_DIR * (breadth_score + money_flow_score + structure_score)
msi_daily = W_DIRECTION * Direction
          + W_INTENSITY * Intensity
          + W_CONSTRAINT * Constraint
```

有界：各 `*_score ∈ [0,100]`，凸组合 ⇒ `msi_daily ∈ [0,100]`，中性 `50`。

---

## 6. 短线双读数

### 6.1 当日 `today`

```
today = msi_daily(snapshots[last])   # 含 layers / components / diagnostics / signal
today.date = snapshots[last].date
```

### 6.2 近 N 日 `recent`

```
# snapshots 长度应为 N（由路由保证；不足则按实际天数）
series = []
for snap in snapshots:                 # 正序，含当日
    series.append({ date: snap.date, value: msi_daily(snap).value })

recent.value = EWMA([p.value for p in series], EWMA_ALPHA)
recent.days  = len(series)
recent.series = series
```

### 6.3 数据装载（路由 / data_source）

1. 从库加载最近 `N` 个开市日全市场日 K（`sc_trade_calendar WHERE is_open = TRUE`）。  
2. 若处于盘中实时窗：用 Miana 实时覆盖/追加 **最新日** 截面（规则同既有 hybrid）。  
3. 对装载结果调用 `calculate_msi`，**不落库**。

---

## 7. 信号（基于当日）

仅用 `today.value`、当日涨跌停计数（诊断用）、`intraday_raw`。

诊断仍统计：

```
limit_up   = count(change_pct >=  LIMIT_PCT)
limit_down = count(change_pct <= -LIMIT_PCT)
limit_balance = (limit_up - limit_down) / max(limit_up + limit_down, 1)   # 两边皆 0 → 0
```

### 7.1 布尔量

```
msi_bull = today.value > MSI_HIGH
msi_bear = today.value < MSI_LOW
msi_grey = today.value ∈ [60,65] or ∈ [35,40]
intra_high = intraday_raw >= INTRA_HIGH
intra_low  = intraday_raw <  INTRA_LOW
limit_bull = limit_balance > 0.05
limit_bear = limit_balance < -0.05
```

### 7.2 `PriceStatus`（由日内位置）

| 条件 | 状态 |
|---|---|
| `intraday_raw >= 0.8` | `high_zone` |
| `>= 0.6` | `upper_mid` |
| `>= 0.4` | `mid_zone` |
| `>= 0.2` | `lower_mid` |
| else | `low_zone` |

### 7.3 判定（顺序固定，命中即停）

```
if msi_grey:                     → neutral
if msi_bull and limit_bull and intra_high: → strong_bullish
if msi_bull and intra_high:      → surge_warning      # 偏高位仍强
if msi_bull:                     → weak_bullish
if msi_bear and limit_bear and intra_low:  → panic_selling
if msi_bear and intra_low:       → bottoming
if msi_bear:                     → volume_decline     # 沿用旧标签：偏空走弱
else:                            → neutral
```

标签/建议文案与旧版中文标签对齐（强势上涨、放量冲高、缩量上涨、恐慌抛售、地量地价、放量下跌、中性观望）。  
`volume_status` / `volume_ratio`：本规范无窗口量能，固定 `volume_status = normal`，`volume_ratio = 1.0`（兼容字段）。  
`price_position = intraday_raw`。

---

## 8. API

```
GET /api/market/sentiment?days=5
```

| 参数 | 含义 |
|---|---|
| `days` | 近 N 日窗口，默认 `5`，`clamp` 到 `[3, 30]` |

数据源：`sc_stock` + `sc_stock_kline_d1_bfq`；开市日来自 `sc_trade_calendar`（`is_open` / `pre_trade_date`，与 Tushare `trade_cal` 一致）。  
交易日连续竞价当日：Miana `/api/stock/v3/realtime`（MSI / 日内推送）。  
### 8.1 响应

| 字段 | 含义 |
|---|---|
| `today` | 当日情绪（见下） |
| `recent` | 近 N 日加权情绪 |
| `days` | 实际参与 `recent` 的天数 |
| `calculated_at` | RFC3339 |

`today`：

| 字段 | 含义 |
|---|---|
| `date` | 交易日 |
| `value` | 当日 MSI |
| `layers` | direction / intensity / constraint |
| `components.breadth` / `money_flow` / `structure` / `directed_move` / `intraday_position` | 当日分量 |
| `signal` | §7 |
| `diagnostics` | 当日涨跌家数等 |

`recent`：

| 字段 | 含义 |
|---|---|
| `value` | EWMA 综合分 |
| `days` | 序列长度 |
| `series[]` | `{ date, value }` 每日日分 |

```json
{
  "today": {
    "date": "2026-10-12",
    "value": 62.3,
    "layers": { "direction": 64.0, "intensity": 58.0, "constraint": 61.0 },
    "components": {
      "breadth": 66.0,
      "money_flow": 63.0,
      "structure": 63.0,
      "directed_move": 58.0,
      "intraday_position": 61.0
    },
    "signal": {
      "signal_type": "weak_bullish",
      "label": "缩量上涨",
      "advice": "动能不足，谨慎追高",
      "volume_status": "normal",
      "volume_ratio": 1.0,
      "price_status": "upper_mid",
      "price_position": 0.61
    },
    "diagnostics": {
      "sample_size": 5200,
      "up_count": 2800,
      "down_count": 2200,
      "flat_count": 200,
      "limit_up_count": 40,
      "limit_down_count": 12,
      "breadth_raw": 0.12,
      "money_flow_raw": 0.08,
      "structure_raw": 0.05
    }
  },
  "recent": {
    "value": 55.1,
    "days": 5,
    "series": [
      { "date": "2026-10-08", "value": 48.0 },
      { "date": "2026-10-09", "value": 52.0 },
      { "date": "2026-10-10", "value": 50.0 },
      { "date": "2026-10-11", "value": 54.0 },
      { "date": "2026-10-12", "value": 62.3 }
    ]
  },
  "days": 5,
  "calculated_at": "2026-10-12T10:00:00+08:00"
}
```

---

## 9. 代码映射

| 内容 | 路径 |
|---|---|
| 本规范 | `docs/market_sentiment.md` |
| 日分 + 双读数 | `backend/src/market/calculator.rs` |
| 领域类型 | `backend/src/market/domain.rs` |
| 装载 / 盘中实时 | `backend/src/market/data_source.rs` |
| HTTP | `backend/src/market/routes.rs` |

---

## 10. 测试要求

1. 全日上涨 → `today.value` 明显 > 50，`breadth_score` 靠近 100。  
2. 全日下跌 → `today.value` 明显 < 50。  
3. 单日计算不依赖第二日快照（给双日输入时，改旧日不影响「仅用末日」的 today 算法；`recent` 会变）。  
4. `recent.value` 对近端日分更敏感（EWMA）。  
5. 空截面日 → 日分 50。  
6. 有界：`today.value`、`recent.value` ∈ `[0, 100]`。

---

## 11. 定位

- **今日**：跟手的短线截面情绪。  
- **近 N 日**：日分轨迹的平滑背景。  
- **MTT**：中期结构，见 `docs/mid_term_trend.md`，不并入 MSI。

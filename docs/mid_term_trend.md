# 中期趋势指标（MTT）规范

> **本文档为规范（Source of Truth）。**  
> 凡文中出现的概念、因子、变量、阈值、判定规则，均须给出可实现的公式或伪代码。  
> 实现必须与本文档一致；算法变更须先改本文档再改代码。  
> 拟定实现目录：`backend/src/market/`（或独立 `mid_term` 模块，以代码映射表为准）。

**与 MSI 的关系**：[`market_sentiment.md`](./market_sentiment.md) 刻画 **A 股短线全市场情绪**（默认窗口约 5 日）。本文档刻画 **中期趋势结构**（以 `MA20` / `MA60` 为主），**独立计分、不并入 MSI 三层加权**。两者可并列展示：短线热而中期空、或短线冷而中期多，均属正常信息。

**与策略文档的关系**：[`strategies.md`](./strategies.md) 中「均线多空排列」的「中期」指个股策略参数里的 `MA10`（周期列表 `[5,10,20]`），**不是**本规范的市场中期趋势。本规范面向全市场等权聚合。

---

## 目录

1. [目标与结构](#1-目标与结构)
   - [1.1 设计原则](#11-设计原则)
   - [1.2 通用映射 `map`](#12-通用映射-map)
   - [1.3 常用运算](#13-常用运算)
2. [规范常量](#2-规范常量)
3. [输入与样本过滤](#3-输入与样本过滤)
   - [3.1 输入结构](#31-输入结构)
   - [3.2 单票有效条件](#32-单票有效条件)
   - [3.3 历史长度要求](#33-历史长度要求)
   - [3.4 无有效数据](#34-无有效数据)
4. [均线](#4-均线)
5. [单票中期方向 `trend_i`](#5-单票中期方向-trend_i)
   - [5.1 基础排列判定](#51-基础排列判定)
   - [5.2 斜率过滤（可选，默认开启）](#52-斜率过滤可选默认开启)
6. [市场中期趋势分 `mid_trend_score`](#6-市场中期趋势分-mid_trend_score)
7. [诊断量（不进入主分）](#7-诊断量不进入主分)
   - [7.1 家数计数](#71-家数计数)
   - [7.2 相对 MA60 偏离](#72-相对-ma60-偏离)
   - [7.3 20 日新高 / 新低家数比](#73-20-日新高--新低家数比)
8. [展示分档](#8-展示分档)
9. [有界性](#9-有界性)
10. [API 草案](#10-api-草案)
11. [代码映射](#11-代码映射)
12. [测试要求](#12-测试要求)
13. [定位与非目标](#13-定位与非目标)
14. [设计理由摘要](#14-设计理由摘要)

---

## 1. 目标与结构

MTT（Mid-Term Trend）衡量 **A 股全市场中期趋势结构**，回答一件事：

> 有多少股票处于「价格站上中期均线、且中期均线仍在长期均线之上」的多头结构（或对称空头结构）？

```
mid_trend_score = map(trend_raw)
trend_raw       = (n_bull - n_bear) / (n_bull + n_bear)   # 分母为 0 时见 §6
```

`mid_trend_score` 取值 **`[0, 100]`**，中性 **`50`**。

| 输出 | 含义 |
|---|---|
| `> 50` | 可分类股票中，中期多头家数占优 |
| `< 50` | 中期空头家数占优 |
| `≈ 50` | 多空均衡，或绝大多数处于纠缠 / 样本不足 |

### 1.1 设计原则

1. **尺度分离**：中期用 `MA_MID=20`、`MA_LONG=60`；不与 MSI 默认短窗混算、不改 MSI 权重。  
2. **股票等权**；**不设成交额门槛**（与 MSI 一致）。  
3. **结构优先于点位**：要求 `close` 与双均线同向排列，不只看「收盘在某条均线上方」。  
4. **纠缠中性**：无法判多/空的票记为 `trend_i = 0`，**不进入** `trend_raw` 分母。  
5. **可解释、可单测**：主分仅依赖排列（及可选斜率）；偏离度、20 日新高新低仅为诊断。  
6. **无外部指数依赖**：不使用上证/沪深 300 点位；权重股不能单独绑架家数分。

### 1.2 通用映射 `map`

与 MSI 相同：

```
map(x) = 50 + 50 * clamp(x, -1, 1)
```

| 输入 `x` | 输出 | 含义 |
|---|---|---|
| `1` | `100` | 可分类样本全部多头 |
| `0` | `50` | 中性 |
| `-1` | `0` | 可分类样本全部空头 |

### 1.3 常用运算

| 符号 / 函数 | 含义 |
|---|---|
| `clamp(x, lo, hi)` | 把 `x` 限制在 `[lo, hi]` |
| `mean(xs)` | 算术平均；空集合时相关分支另有约定 |
| `SMA(closes, P)` | 简单移动平均：最近 `P` 根收盘价算术平均 |
| `EMA(closes, P)` | 指数移动平均，见 §4 |
| `finite(x)` | 有限实数（非 NaN / ±Inf） |
| `末日` / `last` | 时间正序下最后一根可用 K 线 |

---

## 2. 规范常量

| 符号 | 值 | 说明 |
|---|---|---|
| `MA_MID` | `20` | 中期均线周期（约一个月交易日） |
| `MA_LONG` | `60` | 长期均线周期（约一个季度交易日） |
| `MA_TYPE` | `ema` | 均线类型：`ema`（默认）或 `sma` |
| `SLOPE_LOOKBACK` | `5` | 斜率过滤回看交易日数 |
| `USE_SLOPE_FILTER` | `true` | 是否启用 §5.2 斜率过滤 |
| `EPS_PRICE` | `1e-10` | 价格比较容差 |
| `EPS_MA` | `1e-10` | 均线相等比较容差（排列用严格不等式，见 §5） |
| `MTT_HIGH` | `65` | 展示/信号：严格大于视为「中期多头确认」 |
| `MTT_LOW` | `35` | 展示/信号：严格小于视为「中期空头确认」 |
| `MTT_GREY_BULL_LO` / `HI` | `60` / `65` | 多头灰区（含端点）：偏多不确认 |
| `MTT_GREY_BEAR_LO` / `HI` | `35` / `40` | 空头灰区（含端点）：偏空不确认 |
| `NH_NL_WINDOW` | `20` | 诊断用新高/新低窗口（含末日） |
| `MIN_HISTORY_DAYS` | `MA_LONG + SLOPE_LOOKBACK` | 单票最少需要的有效交易日数，默认 `65` |

实现须把上表实现为命名常量；变更须先改本文档。

---

## 3. 输入与样本过滤

### 3.1 输入结构

与 MSI 共用同一类日频 OHLC 输入（字段名对齐，便于同一数据管道）：

| 字段 | 含义 |
|---|---|
| `snapshots` | 按交易日排列；**时间正序**，最后一项为最近交易日 |
| `symbol` | 证券代码 |
| `open` / `high` / `low` / `close_price` | 开高低收，价格须 `> 0` |
| `amount` | 成交额（元），`>= 0`；本规范主分**不使用**成交额 |
| `change_pct` | 涨跌幅（百分点）；本规范主分**不使用**，可留给扩展 |

```
MarketDataInput:
  snapshots: MarketDailySnapshot[]   # 建议覆盖不少于 MIN_HISTORY_DAYS 个交易日

MarketDailySnapshot:
  date: string                       # YYYY-MM-DD
  stocks: StockDailyData[]

StockDailyData:
  symbol, date, open, high, low, close_price, amount, change_pct
```

计算引擎按 **symbol** 重排为「单票收盘价时间序列」，再算均线；不要求每个交易日股票池完全一致（新股/停牌导致缺日则该票可能因历史不足被跳过）。

### 3.2 单票有效条件

与 MSI §4.2 相同：

```
function is_valid_stock(s):
    return
        s.open > 0 and s.high > 0 and s.low > 0 and s.close_price > 0
        and s.high >= s.low
        and s.high >= s.open and s.high >= s.close_price
        and s.low  <= s.open and s.low  <= s.close_price
        and s.amount >= 0
        and finite(s.open, s.high, s.low, s.close_price, s.amount, s.change_pct)
```

每个快照仅保留 `is_valid_stock` 为真的记录。单票序列中，仅使用有效日的 `close_price` 构成 `closes[]`（时间正序）。

### 3.3 历史长度要求

对每只在**末日**出现的 `symbol`：

```
若该股有效 closes 根数 < MIN_HISTORY_DAYS:
    跳过该股（不计入 n_bull / n_bear / n_neutral）
```

即：末日存在但上市不足或停牌过多的票，**不参与**中期趋势统计。

### 3.4 无有效数据

若过滤后没有任何股票能算出 `trend_i ∈ {-1, 0, +1}`（含全部因历史不足被跳过）：

```
mid_trend_score = 50
trend_raw = 0
n_bull = n_bear = n_neutral = 0
诊断类计数 = 0 或中性约定值（见 §7）
```

---

## 4. 均线

对单票 `closes[0..T-1]`（`closes[T-1]` = 末日收盘价），计算末日均线值。

### 4.1 SMA

```
function SMA(closes, P):
    # 使用 closes[T-P .. T-1]
    return mean(closes[T-P .. T-1])
```

### 4.2 EMA（默认）

采用标准收盘 EMA，平滑系数 `α = 2 / (P + 1)`。种子：序列上**第一段**长度为 `P` 的 SMA，再向后迭代至末日。

```
function EMA(closes, P):
    alpha = 2 / (P + 1)
    seed = mean(closes[0 .. P-1])    # 若总长恰为需求窗口，则从更早历史取种子；见下
    ema = seed
    for t in P .. T-1:
        ema = alpha * closes[t] + (1 - alpha) * ema
    return ema                       # 末日 EMA
```

**种子位置**：实现须用该股**全部可用** `closes`（不少于 `MIN_HISTORY_DAYS`）。种子取 `closes[0 .. P-1]` 的 SMA，再从下标 `P` 迭代到 `T-1`。这样 `MA_LONG` 在末日已充分热身。

记：

```
ma_mid  = MA(closes, MA_MID)    # MA_TYPE 为 ema 则 EMA，否则 SMA
ma_long = MA(closes, MA_LONG)
close   = closes[T-1]
```

若需斜率过滤，另算 `SLOPE_LOOKBACK` 日前的中期均线：

```
ma_mid_prev = MA(closes[0 .. T-1-SLOPE_LOOKBACK], MA_MID)
# 即：把序列截断到「SLOPE_LOOKBACK 个交易日之前」的末日，再算当时的 MA_MID
```

---

## 5. 单票中期方向 `trend_i`

### 5.1 基础排列判定

```
function classify_alignment(close, ma_mid, ma_long):
    if close > ma_mid + EPS_MA and ma_mid > ma_long + EPS_MA:
        return +1          # 中期多头
    if close < ma_mid - EPS_MA and ma_mid < ma_long - EPS_MA:
        return -1          # 中期空头
    return 0               # 纠缠 / 过渡
```

说明：

- 使用**严格**大于/小于（外加 `EPS_MA` 容差带），均线粘合或价格骑在均线上视为纠缠。  
- **不要求**三条均线「同时向上发散」的额外幅度条件；发散由可选斜率过滤表达。

### 5.2 斜率过滤（可选，默认开启）

当 `USE_SLOPE_FILTER = true` 时，在基础排列为 `+1` / `-1` 上叠加中期均线方向确认：

```
function apply_slope_filter(alignment, ma_mid, ma_mid_prev):
    if alignment == +1:
        if ma_mid > ma_mid_prev + EPS_MA:
            return +1
        return 0           # 多头排列但 MA20 走平/向下 → 降为纠缠
    if alignment == -1:
        if ma_mid < ma_mid_prev - EPS_MA:
            return -1
        return 0           # 空头排列但 MA20 走平/向上 → 降为纠缠
    return 0
```

```
alignment = classify_alignment(close, ma_mid, ma_long)
if USE_SLOPE_FILTER:
    trend_i = apply_slope_filter(alignment, ma_mid, ma_mid_prev)
else:
    trend_i = alignment
```

| `trend_i` | 含义 |
|---|---|
| `+1` | 中期多头（可计入 `n_bull`） |
| `-1` | 中期空头（可计入 `n_bear`） |
| `0` | 纠缠/过渡/斜率未确认（计入 `n_neutral`，**不进** `trend_raw` 分母） |

---

## 6. 市场中期趋势分 `mid_trend_score`

样本：末日存在、且通过 §3.3 历史长度检查的全部股票。

```
n_bull    = count(trend_i == +1)
n_bear    = count(trend_i == -1)
n_neutral = count(trend_i == 0)
n_classified = n_bull + n_bear

if n_classified == 0:
    trend_raw = 0
else:
    trend_raw = (n_bull - n_bear) / n_classified

mid_trend_score = map(trend_raw)
```

注意：

- `n_neutral` **不出现在分母**，避免大量纠缠把分母放大、把多空差「稀释」成假中性之外的第二种扭曲；纠缠已通过「不投票」体现为分母变小，若几乎全市场纠缠则 `n_classified == 0` → 强制 `50`。  
- 不对 `trend_raw` 做 EWMA：中期结构本身已由均线平滑；再叠短窗 EWMA 会模糊「末日结构」语义。若未来需要「中期分的时间平滑」，须另立字段（如 `mid_trend_score_ewma`），不得静默替换本字段。

---

## 7. 诊断量（不进入主分）

下列字段仅供 API / UI 解释，**不得**加权进 `mid_trend_score`。

### 7.1 家数计数

```
n_bull, n_bear, n_neutral, n_classified
bull_ratio = n_bull / n_classified      # n_classified==0 时约定 0
bear_ratio = n_bear / n_classified
```

### 7.2 相对 MA60 偏离

刻画「中期结构确立后贵不贵」，不决定方向。

```
for each stock with valid ma_long:
    if |ma_long| < EPS_PRICE:
        skip
    else:
        dev_i = (close - ma_long) / ma_long

deviation_median = median(dev_i)     # 无样本则 0
```

### 7.3 20 日新高 / 新低家数比

中期扩散诊断，语义类似 MSI 的 `nh_nl`，但窗口固定为 `NH_NL_WINDOW`（与 MSI 的 `days` 入参无关）。

对末日每只历史足够的股票（至少 `NH_NL_WINDOW` 根有效 K 线）：

```
high_W = max(high over last NH_NL_WINDOW days)
low_W  = min(low  over last NH_NL_WINDOW days)

is_nh = (当日 high >= high_W - EPS_PRICE)    # 含并列新高
is_nl = (当日 low  <= low_W  + EPS_PRICE)

nh_nl_raw = (n_nh - n_nl) / (n_nh + n_nl)   # 分母为 0 → 0
nh_nl_diag = map(nh_nl_raw)
```

---

## 8. 展示分档

由 `mid_trend_score` 映射展示状态（**不含**交易指令）：

```
function mtt_status(score):
    if score > MTT_HIGH:                  return mid_bull_confirmed
    if MTT_GREY_BULL_LO <= score <= MTT_GREY_BULL_HI: return mid_bull_grey
    if score < MTT_LOW:                   return mid_bear_confirmed
    if MTT_GREY_BEAR_LO <= score <= MTT_GREY_BEAR_HI: return mid_bear_grey
    return mid_neutral
```

| 状态 | 条件 | 展示含义 |
|---|---|---|
| `mid_bull_confirmed` | `score > 65` | 中期多头占优（确认） |
| `mid_bull_grey` | `[60, 65]` | 偏多灰区，不作确认 |
| `mid_neutral` | 其余 | 中性 / 纠缠主导 |
| `mid_bear_grey` | `[35, 40]` | 偏空灰区，不作确认 |
| `mid_bear_confirmed` | `score < 35` | 中期空头占优（确认） |

灰区阈值与 MSI 总分灰区对齐，便于产品并列展示时心智一致；**不**与 MSI 信号布尔量自动联立（联立规则若需要，另立组合信号文档）。

---

## 9. 有界性

1. `trend_raw = (n_bull - n_bear) / (n_bull + n_bear)`，在 `n_classified > 0` 时有  
   `-1 ≤ trend_raw ≤ 1`（因为 `|n_bull - n_bear| ≤ n_bull + n_bear`）。  
2. `n_classified == 0` 时强制 `trend_raw = 0`。  
3. `mid_trend_score = map(trend_raw) ∈ [0, 100]`。

---

## 10. API 草案

实现落地时建议独立端点（路径可调整，但字段语义须一致）：

```
GET /api/market/mid-term-trend
```

响应示例：

```json
{
  "value": 72.5,
  "status": "mid_bull_confirmed",
  "trend_raw": 0.45,
  "ma_type": "ema",
  "ma_mid": 20,
  "ma_long": 60,
  "use_slope_filter": true,
  "counts": {
    "bull": 2100,
    "bear": 800,
    "neutral": 1200,
    "classified": 2900
  },
  "diagnostics": {
    "deviation_median": 0.08,
    "nh_nl": 61.0
  }
}
```

| 字段 | 含义 |
|---|---|
| `value` | 即 `mid_trend_score` |
| `status` | §8 分档 |
| `trend_raw` | 映射前原始量 |
| `counts.*` | §7.1 |
| `diagnostics.deviation_median` | §7.2 |
| `diagnostics.nh_nl` | §7.3 的 `nh_nl_diag` |

---

## 11. 代码映射

| 规范内容 | 文件 |
|---|---|
| 常量、均线、`trend_i`、聚合、单测 | `backend/src/market/mid_term.rs` |
| 响应结构体 / `MttStatus` | `backend/src/market/domain.rs` |
| HTTP `GET /api/market/mid-term-trend` | `backend/src/market/routes.rs` |
| Mock 输入 | `backend/src/market/mock_data.rs` |

本文档仍为 SoT；禁止只改代码不改本文档。

---

## 12. 测试要求

1. OHLC 不合法被过滤。  
2. 历史不足 `MIN_HISTORY_DAYS` 的票不计入任何 `n_*`。  
3. 全体 `close > MA20 > MA60` 且斜率向上 → `value` 接近 `100`。  
4. 全体 `close < MA20 < MA60` 且斜率向下 → `value` 接近 `0`。  
5. 全体纠缠（如 `MA20` 与 `MA60` 粘合）→ `value == 50`，`classified == 0`。  
6. 一半多头一半空头、无纠缠 → `value == 50`，`classified > 0`。  
7. `USE_SLOPE_FILTER=true` 时：多头排列但 `MA20` 下行 → 该票计 `neutral`，不得计入 `bull`。  
8. `deviation_median` / `nh_nl` 变化不改变 `value`（诊断隔离）。  
9. `map` 边界：`trend_raw=±1` → `value` 为 `100` / `0`。

---

## 13. 定位与非目标

| 是 | 不是 |
|---|---|
| 中期结构仪表盘（家数口径） | 买卖指令或仓位建议 |
| 与 MSI 并列的慢变量 | MSI 的第 4 层或 Constraint 替换项 |
| `MA20`/`MA60` 结构 + 可选斜率 | 指数点位趋势、主观「波段」标注 |
| 等权全市场 | 按市值/成交额加权的「指数趋势」 |

禁止只改代码不改本文档。

---

## 14. 设计理由摘要

1. **`MA20`/`MA60`**：对应月/季交易日尺度，与 MSI 短窗分离。  
2. **双均线排列 + 收盘确认**：比单看「价上 MA20」更耐震荡；比等待复杂趋势模型更可审计。  
3. **纠缠不进分母**：避免假中性稀释；全市场纠缠时显式 `50`。  
4. **等权家数**：与 MSI 宽度哲学一致，避免权重大票单边决定「中期」。  
5. **斜率默认开**：过滤「排列尚在、中期均线已拐」的假多/假空。  
6. **诊断外置**：偏离与 20 日新高新低解释「贵不贵 / 是否扩散」，但不扭曲方向主分。

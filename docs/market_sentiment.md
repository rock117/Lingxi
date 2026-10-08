# 市场情绪指标（MSI）规范

> **本文档为规范（Source of Truth）。**  
> 凡文中出现的概念、因子、变量、阈值、判定规则，均须给出可实现的公式或伪代码。  
> 实现必须与本文档一致；算法变更须先改本文档再改代码。  
> 实现目录：`backend/src/market/`。

---

## 目录

1. [目标与结构](#1-目标与结构)
   - [1.1 三层架构](#11-三层架构)
   - [1.2 设计原则](#12-设计原则)
   - [1.3 通用映射 `map`](#13-通用映射-map)
   - [1.4 常用运算](#14-常用运算)
2. [规范常量](#2-规范常量)
3. [EWMA（指数加权移动平均）](#3-ewma指数加权移动平均)
4. [输入与样本过滤](#4-输入与样本过滤)
   - [4.1 输入结构](#41-输入结构)
   - [4.2 单票有效条件（伪代码）](#42-单票有效条件伪代码)
   - [4.3 无有效数据](#43-无有效数据)
5. [Direction（方向层）](#5-direction方向层)
   - [5.1 涨跌宽度 `breadth_score`](#51-涨跌宽度-breadth_score)
   - [5.2 资金流向 `money_flow_score`](#52-资金流向-money_flow_score)
   - [5.3 开盘缺口宽度 `gap_breadth_score`](#53-开盘缺口宽度-gap_breadth_score)
   - [5.4 结构确认 `structure_score`](#54-结构确认-structure_score)
   - [5.5 新高 / 新低家数比 `nh_nl_score`](#55-新高--新低家数比-nh_nl_score)
6. [Intensity（强度层）](#6-intensity强度层)
   - [6.1 有方向量能 `directed_volume_score`](#61-有方向量能-directed_volume_score)
   - [6.2 涨跌停压力 `limit_pressure_score`](#62-涨跌停压力-limit_pressure_score仅末日)
7. [Constraint（约束层）](#7-constraint约束层)
   - [7.1 真实区间位置 `range_position_*`](#71-真实区间位置-range_position_)
   - [7.2 日内收盘位置 `intraday_position_score`](#72-日内收盘位置-intraday_position_score仅末日)
   - [7.3 涨跌惯性 `inertia_score`](#73-涨跌惯性-inertia_score)
8. [MSI 总分与展示区间](#8-msi-总分与展示区间)
9. [状态与组合信号](#9-状态与组合信号)
   - [9.1 布尔量](#91-布尔量)
   - [9.2 `VolumeStatus`](#92-volumestatus仅由-volume_ratio)
   - [9.3 `PriceStatus`](#93-pricestatus仅由-range_position_raw)
   - [9.4 信号判定](#94-信号判定顺序固定命中即停)
   - [9.5 标签与建议](#95-标签与建议)
10. [API](#10-api)
    - [10.1 响应字段](#101-响应字段)
    - [10.2 末日诊断计数](#102-末日诊断计数)
11. [代码映射](#11-代码映射)
12. [测试要求](#12-测试要求)
13. [定位](#13-定位)

---

## 1. 目标与结构

MSI 衡量 **A 股短线全市场情绪**，回答三件事：

1. **方向（Direction）**：多数个股、资金、开盘缺口、结构确认、新高新低是否同向？  
2. **强度（Intensity）**：有方向的参与有多用力（量能 × 方向、涨跌停压力）？  
3. **约束（Constraint）**：价格处于 N 日区间何处、日内收在何处、涨跌惯性如何？

### 1.1 三层架构

```
MSI_value = W_DIRECTION * Direction
          + W_INTENSITY * Intensity
          + W_CONSTRAINT * Constraint
```

| 层 | 权重常量 | 内部分量 |
|---|---|---|
| **Direction** | `W_DIRECTION = 0.50` | breadth, money_flow, gap_breadth, structure, nh_nl |
| **Intensity** | `W_INTENSITY = 0.25` | directed_volume, limit_pressure |
| **Constraint** | `W_CONSTRAINT = 0.25` | range_position, intraday_position, inertia |

**说明（Constraint 极性）**：Constraint 各分量与「位置偏高 / 多头惯性」同向计分（越高分越高），**不是**从 MSI 中扣除的惩罚项。高位拥挤风险由组合信号中的 `price_high` 等条件表达。

### 1.2 设计原则

1. 股票 **等权**；**不设成交额门槛**。  
2. 窗口长度 `N`：API 入参 `days` 默认 `5`，并 `clamp` 到 `[3, 30]`；计算引擎信任输入快照条数。  
3. 强度 **带方向**：放量下跌不得抬高 Intensity。  
4. 区间位置使用真实 **high / low**，不用收盘价伪造区间。  
5. 无指数等外部数据时，结构因子用 **等权均涨跌幅** 作「指数感」代理。  
6. 新因子必须可解释、可单测。

### 1.3 通用映射 `map`

多数日因子先得到约落在 `[-1, 1]` 的原始量 `x`（如宽度差、资金差），再映射为展示/合成用的百分制分数：

```
map(x) = 50 + 50 * clamp(x, -1, 1)
```

| 输入 `x` | 输出 | 含义 |
|---|---|---|
| `1` | `100` | 极端偏多 |
| `0` | `50` | 中性 |
| `-1` | `0` | 极端偏空 |

### 1.4 常用运算

| 符号 / 函数 | 含义 |
|---|---|
| `clamp(x, lo, hi)` | 把 `x` 限制在 `[lo, hi]`：小于 `lo` 取 `lo`，大于 `hi` 取 `hi` |
| `mean(xs)` | 算术平均；空集合时本规范中相关分支另有约定（多为 `0` 或中性分） |
| `max` / `min` | 最大值 / 最小值 |
| `ln(x)` | 自然对数；用于把成交额倍率压到可比较的尺度 |
| `tanh(x)` | 双曲正切，值域 `(-1, 1)`，用于把无界或偏大的量软压缩到约 `[-1, 1]`，两端饱和 |
| `finite(x)` | 为有限实数（非 NaN / ±Inf） |
| `count_while` | 自序列起点起，连续满足谓词的元素个数（遇第一个不满足即停） |
| `N` | 输入快照天数（交易日条数），即窗口长度 |
| `末日` / `last` | 时间正序下最后一根快照，表示最近交易日 |
| `*_d` | 某一交易日 `d` 上算出的**日原始量**（映射 / EWMA 之前） |
| `*_score` | 映射到 `[0, 100]` 后的分量分 |
| `*_last` | 末日对应的日原始量（诊断用，未 `map`） |
| `*_raw` | 中间原始量（如 `range_position_raw ∈ [0,1]`、`inertia_raw ∈ [-1,1]`） |

---

## 2. 规范常量

下表为实现中的命名常量；「说明」列解释**用途与取值直觉**。

| 符号 | 值 | 说明 |
|---|---|---|
| `W_DIRECTION` | `0.50` | MSI 中方向层权重（一半由「站哪边」决定） |
| `W_INTENSITY` | `0.25` | MSI 中强度层权重 |
| `W_CONSTRAINT` | `0.25` | MSI 中约束层权重 |
| `W_DIR` | `0.20` | Direction 内五分量等权，各占 20% |
| `W_INT_VOLUME` | `0.70` | Intensity 内：有方向量能权重 |
| `W_INT_LIMIT` | `0.30` | Intensity 内：涨跌停压力权重 |
| `W_CON_RANGE` | `0.45` | Constraint 内：N 日区间位置权重 |
| `W_CON_INTRA` | `0.20` | Constraint 内：日内收盘位置权重 |
| `W_CON_INERTIA` | `0.35` | Constraint 内：涨跌惯性权重 |
| `EWMA_ALPHA` | `0.45` | EWMA 近端权重；越大越看重最近一日（见 §3） |
| `RET_SCALE` | `2.0` | 结构因子中，等权均涨跌幅（**百分点**）除以该尺度后再 `tanh`；约「日均涨跌 2 个百分点」量级对应较强信号 |
| `STREAK_TAU` | `3.0` | 连续天数软饱和尺度；约连涨/连跌 3 日时 `tanh` 已较明显，更长天数增量递减 |
| `MSI_HIGH` | `65` | 信号用：总分严格大于该值视为「多头确认」 |
| `MSI_LOW` | `35` | 信号用：总分严格小于该值视为「空头确认」 |
| `MSI_GREY_BULL_LO` / `HI` | `60` / `65` | 多头灰区（含端点）：偏多但不确认，信号强制中性 |
| `MSI_GREY_BEAR_LO` / `HI` | `35` / `40` | 空头灰区（含端点）：偏空但不确认，信号强制中性 |
| `VOL_HIGH` | `1.5` | 放量阈值：末日成交额 ≥ 窗口日均的 1.5 倍 |
| `VOL_LOW` | `0.5` | 缩量阈值：末日成交额 < 窗口日均的 0.5 倍 |
| `VOL_GREY_HIGH_LO` / `HI` | `1.3` / `1.5` | 放量灰区 `[1.3, 1.5)`：量能暧昧，信号强制中性 |
| `PRICE_HIGH` | `0.8` | 高位：`range_position_raw ≥ 0.8`（靠近 N 日区间上沿） |
| `PRICE_LOW` | `0.2` | 低位：`range_position_raw < 0.2`（靠近 N 日区间下沿） |
| `LIMIT_PCT` | `9.5` | 疑似涨/跌停的涨跌幅阈值（百分点）；启发式，**非**交易所官方涨跌停判定 |
| `EPS_AMOUNT` | `1e-10` | 成交额过小保护，避免除零 |
| `EPS_PRICE` | `1e-10` | 价格区间过窄或浮点比较容差 |

`VolumeStatus` / `PriceStatus` 分档另含展示阈值 `2.0`、`0.3`、`0.6`、`0.4` 等，见 §9。

---

## 3. EWMA（指数加权移动平均）

**全称**：Exponentially Weighted Moving Average。  
**作用**：把多日日序列 `x[0..N-1]` 压成**一个**近端更重要的标量，再交给 `map` 成分数。相对简单算术平均，最近交易日权重更大，更早日期影响按比例衰减。

**参数** `alpha = EWMA_ALPHA = 0.45`：

- 每一天更新时，新值占 `45%`，历史累积占 `55%`。  
- `alpha` 越大，曲线越「跟手」、噪声越大；越小则越平滑、滞后越明显。

输入时间正序：`x[0]` 最旧，`x[last]` 最新。

```
function EWMA(x[], alpha):
    if x 为空: return 0
    s = x[0]
    for t in 1 .. len(x)-1:
        s = alpha * x[t] + (1 - alpha) * s
    return s          # 即 s[last]
```

本规范中，`breadth_d`、`money_flow_d`、`gap_breadth_d`、`structure_d`、`nh_nl_d` 等日序列均先 EWMA，再 `map`。

---

## 4. 输入与样本过滤

计算前先规范化输入：丢掉 OHLC 非法票，保证后续统计可解释。

### 4.1 输入结构

| 字段 | 含义 |
|---|---|
| `snapshots` | 按交易日排列的市场切片；**时间正序**，最后一项为最近交易日 |
| `date` | 交易日，`YYYY-MM-DD` |
| `symbol` | 证券代码 |
| `open` / `high` / `low` / `close_price` | 开、高、低、收；价格须 `> 0` |
| `amount` | 成交额（元），`>= 0`；**无下限过滤** |
| `change_pct` | 相对昨收的涨跌幅，单位为**百分点**（`+2.5` = 涨 2.5%） |

```
MarketDataInput:
  snapshots: MarketDailySnapshot[]

MarketDailySnapshot:
  date: string
  stocks: StockDailyData[]

StockDailyData:
  symbol, date, open, high, low, close_price, amount, change_pct
```

### 4.2 单票有效条件（伪代码）

文字要求：价格为正；最高价不低于开盘/收盘/最低；最低价不高于开盘/收盘；成交额非负；关键数值有限。

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

对每个快照：仅保留 `is_valid_stock` 为真的股票。

### 4.3 无有效数据

若过滤后 `snapshots` 为空，或所有日 `stocks` 皆空，无法刻画情绪，则返回**显式中性**：

```
value = 50
Direction = Intensity = Constraint = 50
各 components 分数类字段 = 50
volume_ratio = 1
range_position_raw = 0.5
signal = neutral（按 §9 由上述输入算出）
diagnostics 计数类 = 0
```

---

## 5. Direction（方向层）

刻画「市场站哪一边」：家数、成交额、开盘缺口、结构确认、新高新低五路共识，等权合成。

```
Direction = W_DIR * (
    breadth_score
  + money_flow_score
  + gap_breadth_score
  + structure_score
  + nh_nl_score
)
```

即五分量各占 `0.20`。各 `*_score ∈ [0, 100]`。

### 5.1 涨跌宽度 `breadth_score`

**含义**：上涨家数相对下跌家数的净优势（忽略平盘）。`breadth_d → +1` 几乎全涨，`→ -1` 几乎全跌，`0` 多空家数相当。

| 变量 | 含义 |
|---|---|
| `up_d` / `down_d` | 当日上涨 / 下跌家数 |
| `breadth_d` | 日宽度原始量，`∈ [-1, 1]` |
| `breadth_score` | 对多日 `breadth_d` 做 EWMA 再 `map` |
| `breadth_last` | 末日 `breadth_d`（诊断） |

```
up_d   = count(stocks where change_pct > 0)
down_d = count(stocks where change_pct < 0)
# 平盘 (change_pct == 0) 不进入分子分母

if up_d + down_d == 0:
    breadth_d = 0
else:
    breadth_d = (up_d - down_d) / (up_d + down_d)

breadth_score = map(EWMA([breadth_d for each day], EWMA_ALPHA))
```

### 5.2 资金流向 `money_flow_score`

**含义**：用成交额加权的多空力量——上涨股成交额与下跌股成交额之差占全日总额的比例。家数宽度看「多少人涨」，资金流向看「钱站哪边」。

| 变量 | 含义 |
|---|---|
| `up_amount_d` / `down_amount_d` | 上涨 / 下跌股成交额合计 |
| `total_amount_d` | 全日成交额（**含平盘**） |
| `money_flow_d` | 日资金原始量，约 `∈ [-1, 1]` |
| `money_flow_score` | EWMA 后 `map` |
| `money_flow_last` | 末日 `money_flow_d` |

```
up_amount_d   = sum(amount where change_pct > 0)
down_amount_d = sum(amount where change_pct < 0)
total_amount_d = sum(all amount)          # 含平盘

if total_amount_d < EPS_AMOUNT:
    money_flow_d = 0
else:
    money_flow_d = (up_amount_d - down_amount_d) / total_amount_d

money_flow_score = map(EWMA([money_flow_d ...], EWMA_ALPHA))
```

### 5.3 开盘缺口宽度 `gap_breadth_score`

**含义**：相对昨收，今日开盘高开/低开的家数净优势，反映隔夜到开盘的情绪投射。平开不计入分母。

| 变量 | 含义 |
|---|---|
| `prev_close` | 前一日同代码收盘价 |
| `gap` | `open - prev_close`；`>0` 高开，`<0` 低开 |
| `gap_up` / `gap_down` | 高开 / 低开家数 |
| `gap_breadth_d` | 日缺口宽度原始量 |
| `gap_breadth_score` | 对缺口日序列 EWMA 后 `map`；序列不足则 `50` |
| `gap_breadth_last` | 序列末日原始量 |

对 `i = 1 .. N-1`（需前一日），当日每只在当日与前一日均存在的股票：

```
prev_close = 前一日同 symbol 的 close_price
if prev_close <= 0: skip
gap = open - prev_close
若 gap > 0 → gap_up++
若 gap < 0 → gap_down++
# gap == 0 不进分母

if gap_up + gap_down == 0:
    gap_breadth_d = 0
else:
    gap_breadth_d = (gap_up - gap_down) / (gap_up + gap_down)
```

```
若 N < 2（缺口序列为空）:
    gap_breadth_score = 50
    gap_breadth_last = 0
否则:
    gap_breadth_score = map(EWMA(gap_breadth 序列, EWMA_ALPHA))
    gap_breadth_last = 序列末日值
```

首日无前收，不进入缺口序列（序列长度可为 `N-1`）。

### 5.4 结构确认 `structure_score`

**含义**：比较「多数个股涨跌方向」（`majority_d = breadth_d`）与「等权平均涨跌幅所代表的整体幅度感」（`index_proxy_d`）。两者同向则互相加强；严重背离（如少数大票拉高均值、多数个股下跌）则互相抵消、分数靠近 50。  
**不是**「结构健康度」：空头同向时分数偏低。

| 变量 | 含义 |
|---|---|
| `majority_d` | 同日 `breadth_d` |
| `mean_ret_d` | 当日有效票 `change_pct` 算术平均（百分点） |
| `index_proxy_d` | `tanh(mean_ret_d / RET_SCALE)`，把均涨跌幅压到约 `(-1,1)` |
| `structure_d` | 二者平均，同向加强、背离趋零 |
| `structure_score` | EWMA 后 `map` |
| `structure_last` | 末日 `structure_d` |

```
majority_d    = breadth_d
mean_ret_d    = mean(change_pct of all valid stocks)
index_proxy_d = tanh(mean_ret_d / RET_SCALE)
structure_d   = (majority_d + index_proxy_d) / 2

structure_score = map(EWMA([structure_d ...], EWMA_ALPHA))
```

若当日无股票：`structure_d = 0`。

### 5.5 新高 / 新低家数比 `nh_nl_score`

**含义**：当日创窗口新高 vs 新低的家数净优势，刻画短线趋势扩散（进攻/溃败面）。窗口含当日，故「新高」= 当日 high 为窗口内 high 的上确界（含并列）。

| 变量 | 含义 |
|---|---|
| `window` | `snapshots[0..=i]`，含当日 |
| `hi` / `lo` | 窗口内该股 high 最大值 / low 最小值 |
| `new_high_count` / `new_low_count` | 创窗口新高 / 新低家数（可同时计入） |
| `nh_nl_d` | `(新高数 - 新低数) / (新高数 + 新低数)` |
| `nh_nl_score` | EWMA 后 `map`；`N < 2` 时为 `50` |
| `nh_nl_last` | 末日 `nh_nl_d` |

对每个 `i = 1 .. N-1`（跳过 `i = 0`，窗口过短）：

```
window = snapshots[0 ..= i]
today  = snapshots[i]

new_high_count = 0
new_low_count  = 0

for each stock s in today.stocks:
    hi = max(high of s.symbol over window)   # 缺日则该日不参与该股极值
    lo = min(low  of s.symbol over window)
    if s.high + EPS_PRICE >= hi: new_high_count++
    if s.low  - EPS_PRICE <= lo: new_low_count++

denom = new_high_count + new_low_count
if denom == 0:
    nh_nl_d = 0
else:
    nh_nl_d = (new_high_count - new_low_count) / denom
```

```
若 N < 2:
    nh_nl_score = 50
    nh_nl_last = 0
    new_high_count = new_low_count = 0
否则:
    nh_nl_score = map(EWMA([nh_nl_d ...], EWMA_ALPHA))
    nh_nl_last / new_high_count / new_low_count = 末日对应值
```

---

## 6. Intensity（强度层）

刻画「打得有多用力」：量能必须乘上方向（放量下跌压低分数），并叠加涨跌停拥挤压力。

```
Intensity = W_INT_VOLUME * directed_volume_score
          + W_INT_LIMIT  * limit_pressure_score
```

### 6.1 有方向量能 `directed_volume_score`

**含义**：末日总成交额相对窗口日均的倍率（`volume_ratio`），经对数+`tanh` 得到量能水位 `level`，再乘末日宽度 `breadth_last`。放量上涨抬高分数；放量下跌拉低分数。

| 变量 | 含义 |
|---|---|
| `daily_total_amount[d]` | 第 `d` 日全样本成交额之和 |
| `avg_amount` | 窗口内日成交额均值 |
| `last_amount` | 末日成交额 |
| `volume_ratio` | `last_amount / avg_amount`；`1` = 与日均持平；`≥1.5` 视为放量（见常量） |
| `level` | 量能水位，`tanh(ln(ratio)/ln(2))`；`ratio=2` 时 `ln` 项为 1，再经 `tanh` |
| `directed_raw` | `level * breadth_last`，有方向的量能原始量 |
| `directed_volume_score` | `map(directed_raw)` |

```
daily_total_amount[d] = sum(amount of stocks on day d)
avg_amount = mean(daily_total_amount[])
last_amount = daily_total_amount[last]

if avg_amount < EPS_AMOUNT:
    volume_ratio = 1
else:
    volume_ratio = last_amount / avg_amount

if volume_ratio <= 0 or not finite(volume_ratio):
    level = -1
else:
    level = tanh( ln(volume_ratio) / ln(2) )

breadth_last = 末日 breadth_d
directed_raw = clamp(level * breadth_last, -1, 1)
directed_volume_score = map(directed_raw)
```

若无快照：`directed_volume_score = 50`，`volume_ratio = 1`。

### 6.2 涨跌停压力 `limit_pressure_score`（仅末日）

**含义**：疑似涨停家数相对跌停家数的净占比，反映情绪极端拥挤（涨停潮 / 跌停潮）。阈值为 `LIMIT_PCT`，**非**官方涨跌停标识。

| 变量 | 含义 |
|---|---|
| `limit_up` / `limit_down` | `change_pct ≥ +LIMIT_PCT` / `≤ -LIMIT_PCT` 的家数 |
| `n` | 末日有效样本数（至少为 1） |
| `limit_raw` | `(涨停数 - 跌停数) / n` |
| `limit_pressure_score` | `map(limit_raw)` |

```
limit_up   = count(change_pct >=  LIMIT_PCT)
limit_down = count(change_pct <= -LIMIT_PCT)
n = max(sample_size, 1)
limit_raw = clamp( (limit_up - limit_down) / n, -1, 1 )
limit_pressure_score = map(limit_raw)
```

---

## 7. Constraint（约束层）

刻画「贵不贵 / 收在哪 / 惯性如何」：N 日真实区间位置、当日阴阳线收盘位置、连续涨跌惯性。分高表示位置偏高或多头惯性偏强（见 §1.1 极性说明）。

```
Constraint = W_CON_RANGE   * range_position_score
           + W_CON_INTRA   * intraday_position_score
           + W_CON_INERTIA * inertia_score
```

### 7.1 真实区间位置 `range_position_*`

**含义**：个股末日收盘价落在近 `N` 日真实最高价与最低价之间的相对位置，再对全市场等权平均。靠近区间上沿 → 偏「贵/拥挤」。

| 变量 | 含义 |
|---|---|
| `high_N` / `low_N` | 窗口内该股最高价的最大值 / 最低价的最小值 |
| `current` | 该股末日收盘价 |
| `pos_i` | 单票位置，`0` 贴下限，`1` 贴上限，`0.5` 中性或区间退化 |
| `range_position_raw` | 全市场 `mean(pos_i)`，`∈ [0, 1]`，供信号 `price_*` 使用 |
| `range_position_score` | `raw * 100`，供 Constraint 加权 |

对输入中出现的每只 `symbol`（在窗口内至少 2 个有效交易日）：

```
high_N  = max(high over days where symbol present)
low_N   = min(low  over days where symbol present)
current = 该股末日 close_price

if |high_N - low_N| < EPS_PRICE:
    pos_i = 0.5
else:
    pos_i = clamp( (current - low_N) / (high_N - low_N), 0, 1 )
```

```
若无任何 pos_i:
    range_position_raw = 0.5
    range_position_score = 50
否则:
    range_position_raw = mean(pos_i)
    range_position_score = range_position_raw * 100
```

### 7.2 日内收盘位置 `intraday_position_score`（仅末日）

**含义**：当日收盘落在当日 high–low 之间的位置（等权平均）。收在最高附近偏强，收在最低附近偏弱。

| 变量 | 含义 |
|---|---|
| `intra_i` | 单票日内位置，`∈ [0, 1]` |
| `intraday_position_score` | `mean(intra_i) * 100` |

```
for each stock s on last day:
    if |s.high - s.low| < EPS_PRICE:
        intra_i = 0.5
    else:
        intra_i = clamp( (s.close_price - s.low) / (s.high - s.low), 0, 1 )

if 无股票:
    intraday_position_score = 50
else:
    intraday_position_score = mean(intra_i) * 100
```

### 7.3 涨跌惯性 `inertia_score`

**含义**：从末日向前看，个股连续上涨/下跌、以及「一直没涨 / 一直没跌」（含平盘）的天数，经软饱和后合成多头惯性与空头惯性，再取市场均值之差。用于表达连板/连跌与阴跌磨损。

| 变量 | 含义 |
|---|---|
| `changes[]` | 该股自末日向前的 `change_pct`（`changes[0]`=末日）；中间缺日则中断 |
| `up_streak` | 连续 `change_pct > 0` 的天数 |
| `down_streak` | 连续 `change_pct < 0` 的天数 |
| `no_up_streak` | 连续 `change_pct ≤ 0`（含平盘的「没涨」） |
| `no_down_streak` | 连续 `change_pct ≥ 0`（含平盘的「没跌」） |
| `soft(k)` | `tanh(k / STREAK_TAU)`，天数越长越接近 1，增量递减 |
| `bull_i` / `bear_i` | 单票多头 / 空头惯性强度 |
| `bull` / `bear` | 全市场均值 |
| `inertia_raw` | `bull - bear` |
| `inertia_score` | `map(inertia_raw)` |
| `avg_*_streak` | 对应 streak 的市场平均天数（诊断） |

样本：末日存在的每只股票。

```
changes[] = 自末日向前的 change_pct（时间倒序：changes[0] = 末日）

up_streak      = count_while from start: change_pct > 0
down_streak    = count_while from start: change_pct < 0
no_up_streak   = count_while from start: change_pct <= 0
no_down_streak = count_while from start: change_pct >= 0

soft(k) = tanh(k / STREAK_TAU)

bull_i = 0.5 * soft(up_streak) + 0.5 * soft(no_down_streak)
bear_i = 0.5 * soft(down_streak) + 0.5 * soft(no_up_streak)
```

```
bull = mean(bull_i), bear = mean(bear_i)
inertia_raw = clamp(bull - bear, -1, 1)
inertia_score = map(inertia_raw)
```

若末日无股票或无数可算：`inertia_score = 50`，平均 streak 诊断为 0。

```
avg_up_streak      = mean(up_streak)
avg_down_streak    = mean(down_streak)
avg_no_up_streak   = mean(no_up_streak)
avg_no_down_streak  = mean(no_down_streak)
```

多头惯性强 → `inertia_score` 偏高；连跌 / 阴跌惯性强 → 偏低。

---

## 8. MSI 总分与展示区间

`value` 为三层加权总分，供展示分档与信号阈值使用；下表分档**不改变**计算公式。

```
value = W_DIRECTION * Direction
      + W_INTENSITY * Intensity
      + W_CONSTRAINT * Constraint
```

| 条件 | 含义（展示用） |
|---|---|
| `value > 65` | 强势多头 |
| `55 < value ≤ 65` | 偏多 |
| `45 ≤ value ≤ 55` | 中性 |
| `35 ≤ value < 45` | 偏空 |
| `value < 35` | 强势空头 |

---

## 9. 状态与组合信号

在总分、量能倍率、区间位置之上做离散标签，供 UI / 策略提示；**灰区优先中性**，避免临界值过度解读。

### 9.1 布尔量

| 变量 | 含义 |
|---|---|
| `msi_high` / `msi_low` | 总分明确多 / 空确认 |
| `msi_grey` | 总分落在多头或空头灰区，信号不作方向结论 |
| `vol_high` / `vol_low` | 放量 / 缩量 |
| `vol_grey_high` | 放量临界灰区 |
| `price_high` / `price_low` | N 日区间高位 / 低位 |

```
msi_high = (value > MSI_HIGH)
msi_low  = (value < MSI_LOW)
msi_grey = (value >= MSI_GREY_BULL_LO and value <= MSI_GREY_BULL_HI)
        or (value >= MSI_GREY_BEAR_LO and value <= MSI_GREY_BEAR_HI)

vol_high      = (volume_ratio >= VOL_HIGH)
vol_low       = (volume_ratio <  VOL_LOW)
vol_grey_high = (volume_ratio >= VOL_GREY_HIGH_LO and volume_ratio < VOL_GREY_HIGH_HI)

price_high = (range_position_raw >= PRICE_HIGH)
price_low  = (range_position_raw <  PRICE_LOW)
```

### 9.2 `VolumeStatus`（仅由 `volume_ratio`）

展示用成交额倍率分档（与信号布尔阈值可重叠，但枚举更细）。

| 条件 | JSON | 含义 |
|---|---|---|
| `volume_ratio >= 2.0` | `extreme_high` | 极端放量 |
| `1.5 <= volume_ratio < 2.0` | `high` | 放量 |
| `0.5 <= volume_ratio < 1.5` | `normal` | 正常 |
| `0.3 <= volume_ratio < 0.5` | `low` | 缩量 |
| `volume_ratio < 0.3` | `extreme_low` | 地量 |

### 9.3 `PriceStatus`（仅由 `range_position_raw`）

展示用 N 日区间位置分档。

| 条件 | JSON | 含义 |
|---|---|---|
| `>= 0.8` | `high_zone` | 高位区 |
| `[0.6, 0.8)` | `upper_mid` | 中上 |
| `[0.4, 0.6)` | `mid_zone` | 中位 |
| `[0.2, 0.4)` | `lower_mid` | 中下 |
| `< 0.2` | `low_zone` | 低位区 |

### 9.4 信号判定（顺序固定，命中即停）

按下列优先级匹配；先命中灰区则直接中性。

```
1. msi_grey or vol_grey_high
       → neutral
2. msi_high and vol_high and not price_high
       → strong_bullish
3. msi_high and vol_high and price_high
       → surge_warning
4. msi_high and vol_low and price_high
       → divergence_warning
5. msi_high and vol_low and not price_high
       → weak_bullish
6. msi_low and vol_high and price_low
       → panic_selling
7. msi_low and vol_low and price_low
       → bottoming
8. msi_low and vol_high and not price_low
       → volume_decline
9. (not msi_high) and (not msi_low) and vol_high and price_low
       → low_volume_accumulation
10. else
       → neutral
```

### 9.5 标签与建议

| `signal_type` | `label` | `advice` |
|---|---|---|
| `strong_bullish` | 强势上涨 | 顺势做多 |
| `surge_warning` | 放量冲高 | 警惕见顶，减仓 |
| `divergence_warning` | 量价背离 | 量价背离，警惕回调 |
| `weak_bullish` | 缩量上涨 | 动能不足，谨慎追高 |
| `low_volume_accumulation` | 低位放量 | 关注反弹信号 |
| `neutral` | 中性观望 | 观望 |
| `panic_selling` | 恐慌抛售 | 关注超跌反弹 |
| `bottoming` | 地量地价 | 关注底部机会 |
| `volume_decline` | 放量下跌 | 风险大，观望 |

`signal` 对象另含：

| 字段 | 含义 |
|---|---|
| `volume_status` | §9.2 分档 |
| `volume_ratio` | 同 `components.volume_ratio` |
| `price_status` | §9.3 分档 |
| `price_position` | 等于 `range_position_raw`（0～1） |

---

## 10. API

```
GET /api/market/sentiment?days=5&use_mock=true
```

| 参数 | 含义 |
|---|---|
| `days` | 窗口交易日数，默认 `5`，`clamp` 到 `[3, 30]` |
| `use_mock` | 默认 `true` 用 Mock；`false` 接真实行情（实现 TODO） |

### 10.1 响应字段

| 字段 | 含义 |
|---|---|
| `value` | MSI 总分 0～100 |
| `layers.direction/intensity/constraint` | 三层分 |
| `components.*` | 各因子分；除 `volume_ratio` 外为 0～100 |
| `components.volume_ratio` | 末日成交额 / 窗口日均 |
| `days` | 实际参与计算的快照天数 |
| `signal` | §9 组合信号 |
| `diagnostics` | 末日计数与原始量，便于核对盘面 |
| `calculated_at` | 计算时间，RFC3339 |

`diagnostics` 主要字段：

| 字段 | 含义 |
|---|---|
| `sample_size` | 末日有效股票数 |
| `up_count` / `down_count` / `flat_count` | 涨 / 跌 / 平家数 |
| `limit_up_count` / `limit_down_count` | 疑似涨停 / 跌停家数 |
| `breadth_last` 等 `*_last` | 对应日因子末日原始量 |
| `new_high_count` / `new_low_count` | 末日窗口新高 / 新低家数 |
| `avg_*_streak` | 各类连续天数的市场平均 |

```json
{
  "value": 52.3,
  "layers": {
    "direction": 51.0,
    "intensity": 48.5,
    "constraint": 58.0
  },
  "components": {
    "breadth": 50.0,
    "money_flow": 49.0,
    "gap_breadth": 54.0,
    "structure": 50.0,
    "nh_nl": 55.0,
    "directed_volume": 47.0,
    "volume_ratio": 1.1,
    "limit_pressure": 52.0,
    "range_position": 60.0,
    "intraday_position": 55.0,
    "inertia": 48.0
  },
  "days": 5,
  "signal": { "...": "见 §9" },
  "diagnostics": {
    "sample_size": 50,
    "up_count": 22,
    "down_count": 24,
    "flat_count": 4,
    "limit_up_count": 1,
    "limit_down_count": 0,
    "breadth_last": -0.04,
    "money_flow_last": -0.02,
    "gap_breadth_last": 0.1,
    "structure_last": 0.02,
    "nh_nl_last": 0.3,
    "new_high_count": 12,
    "new_low_count": 5,
    "avg_up_streak": 1.2,
    "avg_down_streak": 0.8,
    "avg_no_up_streak": 1.0,
    "avg_no_down_streak": 1.5
  },
  "calculated_at": "RFC3339"
}
```

约定：

- `components` 中除 `volume_ratio` 外，分数类字段均为 **0～100**。  
- `signal.price_position` = `range_position_raw`（**0～1**）。  
- `signal.volume_ratio` = `components.volume_ratio`。  
- `*_last` 为映射前的末日原始量。

### 10.2 末日诊断计数

```
sample_size = 末日有效股票数
up_count    = count(change_pct > 0)
down_count  = count(change_pct < 0)
flat_count  = count(change_pct == 0)
limit_up_count   = count(change_pct >=  LIMIT_PCT)
limit_down_count = count(change_pct <= -LIMIT_PCT)
```

---

## 11. 代码映射

| 规范内容 | 文件 |
|---|---|
| 常量、全部计算公式 | `backend/src/market/calculator.rs` |
| 结构体 / 枚举 / 状态分档 | `backend/src/market/domain.rs` |
| Mock OHLC 输入 | `backend/src/market/mock_data.rs` |
| HTTP `days` clamp、路由 | `backend/src/market/routes.rs` |

---

## 12. 测试要求

1. OHLC 不合法被过滤。  
2. 宽度：平盘不进分母。  
3. 缺口：全高开 → `gap_breadth` 偏高。  
4. 放量下跌 → `directed_volume < 50`。  
5. 区间位置使用 high/low（非仅 close）。  
6. 灰区 → `neutral`。  
7. 三层加权：`0.5 / 0.25 / 0.25`。  
8. 结构背离：多数下跌但等权均值被少数大涨拉正 → `structure` 接近中性带。  
9. 末日多数创新高 → `nh_nl` 明显 > 50。  
10. 全市场连续下跌数日 → `inertia < 50`。  
11. 偏多 / 偏空 Mock 端到端。

---

## 13. 定位

短线情绪仪表盘，非交易指令。禁止只改代码不改本文档。

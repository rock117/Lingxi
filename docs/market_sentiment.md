# 市场情绪指标（MSI）规范

> **本文档为规范（Source of Truth）。**  
> 实现必须与本文档一致；算法变更须先改文档再改代码。  
> 实现目录：`backend/src/market/`。

---

## 1. 目标与结构

MSI 衡量 **A 股短线全市场情绪**，回答三件事：

1. **方向**：市场站哪边？  
2. **强度**：共识打得有多用力？  
3. **约束**：是否已拥挤 / 位置是否过高？

### 1.1 三层架构（非平面四子分）

```
MSI = 0.50 × Direction + 0.25 × Intensity + 0.25 × Constraint
```

| 层 | 权重 | 含义 | 内部分量 |
|---|---|---|---|
| **Direction（方向）** | 50% | 票与钱的方向共识 + 开盘预期 | 宽度、资金流向、开盘缺口广度 |
| **Intensity（强度）** | 25% | 有方向的参与强度 | 量价量能、涨跌停压力 |
| **Constraint（约束）** | 25% | 价格拥挤与可追空间 | 真实区间位置、日内收盘位置 |

对外除总分外，**必须**输出三层分与各分量，便于核验「是否贴近真实盘面」。

### 1.2 设计原则

1. 股票 **等权**（不按市值/成交额加权）；**不设成交额门槛**。  
2. 仅用最近 N 天，N ∈ [3, 30]。  
3. 强度必须 **带方向**：放量下跌不得抬高 Intensity。  
4. 位置使用真实 **high/low**，不用收盘价假区间。  
5. 字段：`open/high/low/close/amount/change_pct`。

---

## 2. 规范常量

| 符号 | 值 | 说明 |
|---|---|---|
| `W_DIRECTION` | `0.50` | MSI ← 方向层 |
| `W_INTENSITY` | `0.25` | MSI ← 强度层 |
| `W_CONSTRAINT` | `0.25` | MSI ← 约束层 |
| `W_DIR_BREADTH` | `1/3` | 方向内：宽度 |
| `W_DIR_MONEY` | `1/3` | 方向内：资金 |
| `W_DIR_GAP` | `1/3` | 方向内：缺口 |
| `W_INT_VOLUME` | `0.70` | 强度内：量价量能 |
| `W_INT_LIMIT` | `0.30` | 强度内：涨跌停压力 |
| `W_CON_RANGE` | `0.70` | 约束内：N 日真实区间位置 |
| `W_CON_INTRA` | `0.30` | 约束内：末日日内位置 |
| `EWMA_ALPHA` | `0.45` | 日序列近端加权 |
| `MSI_HIGH` / `MSI_LOW` | `65` / `35` | 信号多空确认 |
| `MSI_GREY_BULL_*` | `[60, 65]` | 多头灰区（含端点）→ neutral |
| `MSI_GREY_BEAR_*` | `[35, 40]` | 空头灰区（含端点）→ neutral |
| `VOL_HIGH` / `VOL_LOW` | `1.5` / `0.5` | 放量 / 缩量 |
| `VOL_GREY_HIGH_*` | `[1.3, 1.5)` | 放量灰区 → neutral |
| `PRICE_HIGH` / `PRICE_LOW` | `0.8` / `0.2` | 高位 / 低位 |
| `LIMIT_PCT` | `9.5` | 疑似涨跌停启发式（非官方） |
| `EPS_AMOUNT` / `EPS_PRICE` | `1e-10` | 数值保护 |

---

## 3. EWMA

时间正序 `x[0]` 最旧：

```
s[0] = x[0]
s[t] = EWMA_ALPHA * x[t] + (1 - EWMA_ALPHA) * s[t-1]
EWMA(x) = s[last]
空序列 → 0
```

---

## 4. 样本过滤

单日股票同时满足才有效：

1. `open > 0`, `high > 0`, `low > 0`, `close_price > 0`  
2. `high >= low`  
3. `high >= open` 且 `high >= close_price`  
4. `low <= open` 且 `low <= close_price`  
5. `amount >= 0`  
6. 上述字段与 `change_pct` 均为有限数  

**无成交额下限。**

无有效数据时：`value=50`，三层与分量均为中性（50 或等价），`volume_ratio=1`，`range_position_raw=0.5`，`signal=neutral`，诊断计数为 0。

---

## 5. Direction（方向层）

各分量先算到 `[0,100]`（50 中性），再合成：

```
Direction = W_DIR_BREADTH * breadth_score
          + W_DIR_MONEY   * money_flow_score
          + W_DIR_GAP     * gap_breadth_score
```

### 5.1 涨跌宽度 `breadth_score`

```
breadth_d = (up - down) / max(up + down, 1)     # 平盘不进分母
breadth_score = 50 + 50 * clamp(EWMA(breadth_d), -1, 1)
```

### 5.2 资金流向 `money_flow_score`

```
money_flow_d = (up_amount - down_amount) / total_amount   # total 含平盘
# total < EPS_AMOUNT → 0
money_flow_score = 50 + 50 * clamp(EWMA(money_flow_d), -1, 1)
```

### 5.3 开盘缺口广度 `gap_breadth_score`

对末日以外的每一日 d（需存在该股前一日收盘价 `prev_close`）：

```
gap_i = open_i - prev_close_i
gap_up   = count(gap_i > 0)
gap_down = count(gap_i < 0)
gap_breadth_d = (gap_up - gap_down) / max(gap_up + gap_down, 1)
```

- 无 `prev_close` 的股票跳过。  
- 若某日有效缺口样本为 0：`gap_breadth_d = 0`。  
- 首日无前收：该日不进入缺口序列（序列长度可为 N-1）。  
- 若缺口序列为空：`gap_breadth_score = 50`。

```
gap_breadth_score = 50 + 50 * clamp(EWMA(gap_breadth_d), -1, 1)
```

---

## 6. Intensity（强度层）

```
Intensity = W_INT_VOLUME * directed_volume_score
          + W_INT_LIMIT  * limit_pressure_score
```

### 6.1 有方向量能 `directed_volume_score`

```
volume_ratio = last_day_total_amount / mean(daily_total_amount)
# avg < EPS → ratio = 1
# ratio <= 0 或非有限 → level = -1
level = tanh(ln(volume_ratio) / ln(2))
breadth_last = 末日 breadth_d
directed_volume_score = 50 + 50 * clamp(level * breadth_last, -1, 1)
```

### 6.2 涨跌停压力 `limit_pressure_score`（末日）

启发式（非交易所官方）：

```
limit_up   = count(change_pct >= LIMIT_PCT)
limit_down = count(change_pct <= -LIMIT_PCT)
n = sample_size
limit_raw = (limit_up - limit_down) / max(n, 1)     # ∈ [-1, 1]
limit_pressure_score = 50 + 50 * limit_raw
```

---

## 7. Constraint（约束层）

```
Constraint = W_CON_RANGE * range_position_score
           + W_CON_INTRA * intraday_position_score
```

位置越高 → 约束分越高 → 表示越「贵/拥挤」（中性 50）。  
组合信号里「高位风险」使用 `range_position_raw ∈ [0,1]`。

### 7.1 真实区间位置（N 日 high/low）

对每只股票（至少 2 个有效交易日）：

```
high_N = max(high over days)
low_N  = min(low over days)
current = 末日 close_price
若 |high_N - low_N| < EPS_PRICE: pos_i = 0.5
否则: pos_i = clamp((current - low_N) / (high_N - low_N), 0, 1)
```

```
range_position_raw = mean(pos_i)          # 无样本 → 0.5
range_position_score = range_position_raw * 100
```

### 7.2 日内收盘位置（末日）

对末日每只有效股票：

```
若 |high - low| < EPS_PRICE: intra_i = 0.5
否则: intra_i = clamp((close_price - low) / (high - low), 0, 1)
intraday_position_score = mean(intra_i) * 100     # 无样本 → 50
```

---

## 8. MSI 与展示区间

```
value = W_DIRECTION * Direction
      + W_INTENSITY * Intensity
      + W_CONSTRAINT * Constraint
```

| 条件 | 含义 |
|---|---|
| `value > 65` | 强势多头 |
| `55 < value ≤ 65` | 偏多 |
| `45 ≤ value ≤ 55` | 中性 |
| `35 ≤ value < 45` | 偏空 |
| `value < 35` | 强势空头 |

---

## 9. 状态与组合信号

### 9.1 `VolumeStatus`（仅 `volume_ratio`）

| 条件 | JSON |
|---|---|
| `>= 2.0` | `extreme_high` |
| `[1.5, 2.0)` | `high` |
| `[0.5, 1.5)` | `normal` |
| `[0.3, 0.5)` | `low` |
| `< 0.3` | `extreme_low` |

### 9.2 `PriceStatus`（仅 `range_position_raw`）

| 条件 | JSON |
|---|---|
| `>= 0.8` | `high_zone` |
| `[0.6, 0.8)` | `upper_mid` |
| `[0.4, 0.6)` | `mid_zone` |
| `[0.2, 0.4)` | `lower_mid` |
| `< 0.2` | `low_zone` |

### 9.3 信号判定（顺序固定）

布尔量：

```
msi_high / msi_low / msi_grey
vol_high / vol_low / vol_grey_high
price_high = range_position_raw >= 0.8
price_low  = range_position_raw < 0.2
```

```
1. msi_grey 或 vol_grey_high → neutral
2. msi_high ∧ vol_high ∧ ¬price_high → strong_bullish
3. msi_high ∧ vol_high ∧ price_high → surge_warning
4. msi_high ∧ vol_low ∧ price_high → divergence_warning
5. msi_high ∧ vol_low ∧ ¬price_high → weak_bullish
6. msi_low ∧ vol_high ∧ price_low → panic_selling
7. msi_low ∧ vol_low ∧ price_low → bottoming
8. msi_low ∧ vol_high ∧ ¬price_low → volume_decline
9. ¬msi_high ∧ ¬msi_low ∧ vol_high ∧ price_low → low_volume_accumulation
10. else → neutral
```

标签 / 建议与旧版相同（强势上涨、放量冲高、量价背离…）。

---

## 10. API

```
GET /api/market/sentiment?days=5&use_mock=true
```

`days` 默认 5，clamp 到 `[3,30]`。

### 响应（关键字段）

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
    "directed_volume": 47.0,
    "volume_ratio": 1.1,
    "limit_pressure": 52.0,
    "range_position": 60.0,
    "intraday_position": 55.0
  },
  "days": 5,
  "signal": { "...": "..." },
  "diagnostics": {
    "sample_size": 50,
    "up_count": 22,
    "down_count": 24,
    "flat_count": 4,
    "limit_up_count": 1,
    "limit_down_count": 0,
    "breadth_last": -0.04,
    "money_flow_last": -0.02,
    "gap_breadth_last": 0.1
  },
  "calculated_at": "..."
}
```

- `components.range_position` / `intraday_position` 为 **0～100 分**。  
- `signal.price_position` 为 **`range_position_raw`（0～1）**。  
- `signal.volume_ratio` 同 `components.volume_ratio`。

---

## 11. 输入 `StockDailyData`

| 字段 | 类型 | 说明 |
|---|---|---|
| `symbol` | string | 代码 |
| `date` | string | `YYYY-MM-DD` |
| `open` | f64 | 开盘价 |
| `high` | f64 | 最高价 |
| `low` | f64 | 最低价 |
| `close_price` | f64 | 收盘价 |
| `amount` | f64 | 成交额（元） |
| `change_pct` | f64 | 涨跌幅 %（相对昨收） |

`snapshots` 时间正序，末日为最近交易日。

---

## 12. 代码映射

| 规范 | 文件 |
|---|---|
| 常量 / 计算 | `calculator.rs` |
| 结构体 / 枚举 | `domain.rs` |
| Mock OHLC | `mock_data.rs` |
| HTTP | `routes.rs` |

---

## 13. 测试要求

1. OHLC 关系不合法被过滤  
2. 宽度平盘不进分母  
3. 缺口：全高开 → `gap_breadth` 偏高  
4. 放量下跌 → `directed_volume < 50` 且不宜抬高 MSI  
5. 区间位置使用 high/low（非仅 close）  
6. 灰区 → neutral  
7. 三层加权：`0.5/0.25/0.25`  
8. 偏多 / 偏空 Mock 端到端  

---

## 14. 定位

MSI 是短线情绪仪表盘，不是交易指令。禁止只改代码不改本文档。

# 市场情绪指标（Market Sentiment Index, MSI）

本文档描述后端 `market` 模块实现的市场情绪指标计算逻辑。

## 概述

MSI 是一个综合性的短期市场情绪指标，基于过去 N 天全市场每只股票的
**涨跌幅**、**成交额**、**收盘价**三个维度数据，计算 4 个子分并加权合成。

### 设计原则

- **等权处理**：所有股票权重相同，不按市值或成交额加权，反映市场整体广度
- **短期分析**：仅分析过去 N 天（3~30 天），不做中长期趋势判断
- **成交量优先**：成交量水平是核心子分，放量/缩量对信号影响最大
- **价格位置辅助**：当前股价在 N 天高低点区间的位置，判断是否在高位/低位

---

## 4 个子分

| 子分 | 权重 | 含义 | 数据来源 |
|---|---|---|---|---|
| 涨跌宽度分 | 20% | N 天涨跌家数比，反映市场参与广度 | 涨跌幅 |
| 资金流向分 | 30% | 上涨股票成交额 vs 下跌股票成交额 | 涨跌幅 + 成交额 |
| **成交量水平分** | **25%** | 最近成交量 vs N 天均量，放量/缩量 | 成交额 |
| **价格位置分** | **25%** | 当前股价在 N 天高低点区间的位置 | 收盘价 |

### 子分 1：涨跌宽度分（Breadth Score，权重 20%）

**原理**：看每天有多少股票在涨、多少在跌，反映市场参与广度。

**单日计算**：

```
up_count_d   = |{ 股票 i : change_pct_{i,d} > 0 }|
down_count_d = |{ 股票 i : change_pct_{i,d} < 0 }|
M_d          = 当天总股票数

breadth_d = (up_count_d - down_count_d) / M_d    # 范围 [-1, 1]
```

**N 天聚合**：

```
breadth_avg = mean(breadth_d, d = 1..N)           # 范围 [-1, 1]

涨跌宽度分 = 50 + 50 × breadth_avg                # 范围 [0, 100]
```

### 子分 2：资金流向分（Money Flow Score，权重 30%）

**原理**：用成交额衡量资金是流向上涨股票还是下跌股票。

**单日计算**：

```
up_turnover_d   = Σ turnover_{i,d}  (所有 change_pct > 0 的股票)
down_turnover_d = Σ turnover_{i,d}  (所有 change_pct < 0 的股票)
total_turnover_d = up_turnover_d + down_turnover_d + flat_turnover_d

money_flow_d = (up_turnover_d - down_turnover_d) / total_turnover_d  # [-1, 1]
```

**N 天聚合**：

```
money_flow_avg = mean(money_flow_d, d = 1..N)

资金流向分 = 50 + 50 × money_flow_avg             # 范围 [0, 100]
```

> 注意：成交额是"被统计的资金量"，不是"加权权重"。
> 它回答"钱在追涨还是杀跌"，而非"给涨跌幅赋权"。

### 子分 3：成交量水平分（Volume Level Score，权重 25%）— 核心

**原理**：当前成交量 vs N 天平均成交量，判断市场是放量还是缩量。
成交量是市场情绪最直接的体现：

- **放量上涨** → 资金积极入场，信号可靠
- **缩量上涨** → 跟随资金不足，信号偏弱
- **放量下跌** → 抛压重，风险大
- **天量** → 可能见顶/见底
- **地量** → 可能见底

**计算**：

```
# N 天每天的全市场总成交额
daily_turnover_d = Σ turnover_{i,d}  (当天所有股票)

# N 天平均日成交额
avg_turnover = mean(daily_turnover_d, d = 1..N)

# 最近一天的成交额
last_turnover = daily_turnover_N

# 成交量比值
volume_ratio = last_turnover / avg_turnover

# 映射到 [0, 100]：ratio=1 → 50，ratio=2 → 100，ratio=0 → 0
成交量水平分 = 50 + 50 × clamp(volume_ratio - 1.0, -1, 1)
```

**成交量状态分类**：

| volume_ratio | 状态 | 含义 |
|---|---|---|
| > 2.0 | 天量（extreme_high） | 极端放量，可能见顶/见底 |
| 1.5 ~ 2.0 | 放量（high） | 资金积极参与 |
| 0.5 ~ 1.5 | 正常（normal） | 成交量平稳 |
| 0.3 ~ 0.5 | 缩量（low） | 交投清淡 |
| < 0.3 | 地量（extreme_low） | 极端缩量，可能见底 |

### 子分 4：价格位置分（Price Position Score，权重 25%）

**原理**：当前股价在 N 天高低点区间的相对位置，判断市场是否在高位或低位。

**单股计算**：

```
high_N = max(close_price_i, 过去 N 天)
low_N  = min(close_price_i, 过去 N 天)
current = 最近一天收盘价

position_i = (current - low_N) / (high_N - low_N)    # 范围 [0, 1]
# 1.0 = 当前价格在 N 天最高点
# 0.0 = 当前价格在 N 天最低点
# 0.5 = 当前价格在 N 天中位
```

**全市场聚合**（等权平均）：

```
avg_position = mean(position_i, 所有股票)

价格位置分 = avg_position × 100                    # 范围 [0, 100]
```

**价格位置状态分类**：

| 位置 | 状态 | 含义 |
|---|---|---|
| > 0.8 | 高位区（high_zone） | 接近 N 天高点，面临阻力 |
| 0.6 ~ 0.8 | 偏高区（upper_mid） | 中上部，趋势偏强 |
| 0.4 ~ 0.6 | 中位区（mid_zone） | 中间位置，方向不明 |
| 0.2 ~ 0.4 | 偏低区（lower_mid） | 中下部，趋势偏弱 |
| < 0.2 | 低位区（low_zone） | 接近 N 天低点，可能有支撑 |

---

## MSI 综合值

```
MSI = 0.20 × 涨跌宽度分
    + 0.30 × 资金流向分
    + 0.25 × 成交量水平分
    + 0.25 × 价格位置分
```

取值范围 `[0, 100]`，`50` 为中性线。

| 区间 | 含义 |
|---|---|
| > 65 | 强势多头 |
| 55 ~ 65 | 偏多 |
| 35 ~ 55 | 中性 |
| 35 ~ 45 | 偏空 |
| < 35 | 强势空头 |

---

## 组合信号（9 种）

基于 MSI 值、成交量比值、价格位置三维组合判定：

| MSI | 成交量 | 价格位置 | 信号类型 | 建议 |
|---|---|---|---|---|
| 高 | 放量 | 中位 | `strong_bullish` 强势上涨 | 顺势做多 |
| 高 | 放量 | 高位 | `surge_warning` 放量冲高 | 警惕见顶，减仓 |
| 高 | 缩量 | 高位 | `divergence_warning` 量价背离 | 警惕回调 |
| 高 | 缩量 | 中位 | `weak_bullish` 缩量上涨 | 动能不足，谨慎追高 |
| 中 | 放量 | 低位 | `low_volume_accumulation` 低位放量 | 关注反弹信号 |
| 中 | 正常 | 中位 | `neutral` 中性观望 | 观望 |
| 低 | 放量 | 低位 | `panic_selling` 恐慌抛售 | 关注超跌反弹 |
| 低 | 缩量 | 低位 | `bottoming` 地量地价 | 关注底部机会 |
| 低 | 放量 | 中/高位 | `volume_decline` 放量下跌 | 风险大，观望 |

### 信号判定逻辑

```
msi_high = MSI > 65
msi_low  = MSI < 35
vol_high = volume_ratio >= 1.5
vol_low  = volume_ratio < 0.5
price_high = price_position >= 0.8
price_low  = price_position < 0.2

if msi_high && vol_high && !price_high → StrongBullish
if msi_high && vol_high && price_high  → SurgeWarning
if msi_high && vol_low && price_high  → DivergenceWarning
if msi_high && vol_low && !price_high → WeakBullish
if msi_low && vol_high && price_low   → PanicSelling
if msi_low && vol_low && price_low    → Bottoming
if msi_low && vol_high && !price_low  → VolumeDecline
if !msi_high && !msi_low && vol_high && price_low → LowVolumeAccumulation
else → Neutral
```

---

## API 接口

### 获取市场情绪指标

```
GET /api/market/sentiment?days=5&use_mock=true
```

**参数**：

| 参数 | 类型 | 默认 | 范围 | 说明 |
|---|---|---|---|---|
| `days` | int | 5 | 3 ~ 30 | 分析天数 N |
| `use_mock` | bool | true | — | 是否使用 Mock 数据 |

**返回示例**：

```json
{
  "value": 49.88,
  "sub_scores": {
    "breadth": 48.8,
    "money_flow": 47.33,
    "volume_level": 54.51,
    "price_position": 49.17
  },
  "days": 5,
  "signal": {
    "signal_type": "neutral",
    "label": "中性观望",
    "advice": "观望",
    "volume_status": "normal",
    "volume_ratio": 1.09,
    "price_status": "mid_zone",
    "price_position": 0.49
  },
  "calculated_at": "2026-09-29T16:19:09.054175500+00:00"
}
```

**字段说明**：

| 字段 | 类型 | 说明 |
|---|---|---|
| `value` | f64 | MSI 综合值 [0, 100] |
| `sub_scores.breadth` | f64 | 涨跌宽度分 [0, 100] |
| `sub_scores.money_flow` | f64 | 资金流向分 [0, 100] |
| `sub_scores.volume_level` | f64 | 成交量水平分 [0, 100] |
| `sub_scores.price_position` | f64 | 价格位置分 [0, 100] |
| `days` | int | 使用的天数 N |
| `signal.signal_type` | enum | 信号类型（9 种之一） |
| `signal.label` | string | 信号中文标签 |
| `signal.advice` | string | 操作建议 |
| `signal.volume_status` | enum | 成交量状态（5 种之一） |
| `signal.volume_ratio` | f64 | 成交量比值（当前 / N 天均量） |
| `signal.price_status` | enum | 价格位置状态（5 种之一） |
| `signal.price_position` | f64 | 价格位置 [0, 1] |
| `calculated_at` | string | 计算时间（ISO 8601） |

---

## 输入数据格式

### StockDailyData（单只股票某天的数据）

| 字段 | 类型 | 说明 |
|---|---|---|
| `symbol` | string | 股票代码 |
| `date` | string | 日期（YYYY-MM-DD） |
| `change_pct` | f64 | 涨跌幅（%，如 +3.5 表示涨 3.5%） |
| `turnover` | f64 | 成交额（元） |
| `close_price` | f64 | 收盘价（元） |

### MarketDataInput（完整输入）

```json
{
  "snapshots": [
    {
      "date": "2026-09-25",
      "stocks": [
        {
          "symbol": "600519",
          "date": "2026-09-25",
          "change_pct": 1.5,
          "turnover": 500000000,
          "close_price": 1680.0
        }
      ]
    }
  ]
}
```

---

## 代码结构

```
backend/src/market/
├── mod.rs          # 模块入口
├── domain.rs       # 领域对象（数据结构定义）
├── calculator.rs   # 计算引擎（4 个子分 + 组合信号）
├── mock_data.rs    # Mock 数据生成器
└── routes.rs       # REST API 路由
```

### 领域对象（domain.rs）

- `StockDailyData` — 单只股票某天的行情数据
- `MarketDailySnapshot` — 某天全市场快照
- `MarketDataInput` — N 天完整输入
- `MsiSubScores` — 4 个子分
- `VolumeStatus` — 成交量状态枚举（5 种）
- `PriceStatus` — 价格位置状态枚举（5 种）
- `SignalType` — 信号类型枚举（9 种）
- `MsiSignal` — 组合信号结果
- `MsiResult` — 最终输出

### 计算引擎（calculator.rs）

- `calc_breadth_score()` — 涨跌宽度分
- `calc_money_flow_score()` — 资金流向分
- `calc_volume_level_score()` — 成交量水平分（返回 score + ratio）
- `calc_price_position_score()` — 价格位置分（返回 score + position）
- `classify_signal()` — 三维组合信号判定
- `build_signal()` — 生成完整信号
- `calculate_msi()` — 完整计算入口

### Mock 数据生成器（mock_data.rs）

- `generate_mock_data()` — 生成 N 天随机数据
- `generate_mock_data_with_bias()` — 带偏置生成（-1.0 极空 ~ +1.0 极多）
- Mock 数据特征：
  - 50 只股票
  - 涨跌幅服从正态分布
  - 成交额对数正态分布
  - 收盘价根据涨跌幅累乘推算

---

## 参数建议

| 参数 | 推荐值 | 说明 |
|---|---|---|
| N（天数） | 5 ~ 10 | 短线情绪；3 天超敏感，10 天较平滑 |
| MSI 高位阈值 | 65 | > 65 视为多头 |
| MSI 低位阈值 | 35 | < 35 视为空头 |
| 放量阈值 | 1.5 | volume_ratio >= 1.5 为放量 |
| 缩量阈值 | 0.5 | volume_ratio < 0.5 为缩量 |
| 天量阈值 | 2.0 | volume_ratio >= 2.0 为天量 |
| 地量阈值 | 0.3 | volume_ratio < 0.3 为地量 |
| 高位阈值 | 0.8 | price_position >= 0.8 为高位 |
| 低位阈值 | 0.2 | price_position < 0.2 为低位 |

---

## 测试覆盖

后端包含 22 个单元测试，覆盖：

- 子分计算：涨跌宽度、资金流向、成交量水平（正常/放量/缩量）、价格位置（高/低/中）
- 信号分类：强势上涨、恐慌抛售、地量地价、量价背离
- 状态分类：成交量状态、价格位置状态
- 端到端：全涨+放量+高位、全跌+缩量+低位
- Mock 数据：生成、偏置、价格累乘

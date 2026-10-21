# 条件选股（已实现）

> 实现：`backend/src/market/screen.rs`、`/api/market/strategies`、`/api/market/screen`。  
> App：策略 Tab 展示目录、组合保存（`condition.kind = screen`）、预览/按条件选股。  
> 策略说明亦见 `docs/strategies.md`。

## 1. 概览

当前内置 **17** 个可组合策略原子。App 可多选，默认 **全部满足（AND）**：

```json
{
  "op": "and",
  "rules": [ /* 一条或多条策略 rule */ ]
}
```

### 原子策略一览

| type_id | 名称 | 默认含义 |
|---|---|---|
| `ma_cross` | 均线金叉 | 短均线上穿长均线 |
| `rsi` | RSI 超卖 | RSI 上穿超卖线 |
| `volume_spike` | 放量上涨 | 成交额放大且上涨 |
| `macd_cross` | MACD 金叉 | DIF 上穿 DEA |
| `bollinger` | 布林带突破 | 自下轨下方回升突破下轨 |
| `price_breakout` | 价格突破 | 收盘突破近 N 日最高 |
| `ma_alignment` | 均线多头排列 | 短>中>长且同步向上 |
| `price_change` | 涨跌幅区间 | 当日涨跌幅落在区间内 |
| `period_return` | 日/周/月涨跌 | 近 N 桶每桶涨跌 + 总涨跌 |
| `income_revenue` | 营业收入 | 当期值(亿元) / 同比% / 环比% |
| `income_parent_np` | 归母净利润 | 当期值(亿元) / 同比% / 环比% |
| `income_deduct_np` | 扣非净利润 | 当期值(亿元) / 同比% / 环比% |
| `income_rd` | 研发费用 | 当期值(亿元) / 同比% / 环比% |
| `cf_operating` | 经营现金流净额 | 当期值(亿元) / 同比% / 环比% |
| `cf_investing` | 投资现金流净额 | 当期值(亿元) / 同比% / 环比% |
| `cf_financing` | 筹资现金流净额 | 当期值(亿元) / 同比% / 环比% |
| `bs_receivable` | 应收账款 | 当期值(亿元) / 同比% / 环比% |

### 公共股票池过滤（引擎内置）

| 条件 | 取值 |
|---|---|
| 当日成交额 | ≥ 3000 万 |
| 涨跌幅 | \|R\| < 9.5% |
| 价格 | open / close > 0 |

结果按当日涨跌幅降序；`top_n` 默认 50（1–200）。K 线回看最多约 **120** 个开市日。

### API

| 方法 | 路径 | 说明 |
|---|---|---|
| GET | `/api/market/strategies` | 策略目录 |
| POST | `/api/market/screen` | 立即选股 |
| GET | `/api/market/screen/:id` | 已保存 screen 条件选股 |

---

## 2. `ma_cross` 均线金叉 / 死叉

| 参数 | 默认 | 说明 |
|---|---|---|
| `fast_period` | 5 | 短期 |
| `slow_period` | 20 | 长期 |
| `ma_type` | `ema` | `sma` / `ema` |
| `direction` | `golden` | `golden` / `death` |

金叉：昨 `fast≤slow`，今 `fast>slow`。死叉相反。

---

## 3. `rsi` RSI 超卖 / 超买

| 参数 | 默认 | 说明 |
|---|---|---|
| `period` | 14 | RSI 周期 |
| `oversold` | 30 | 超卖线 |
| `overbought` | 70 | 超买线 |
| `direction` | `oversold` | `oversold` / `overbought` |

超卖回升：昨 RSI≤超卖线，今 > 超卖线。超买回落对称。Wilder 平滑。

---

## 4. `volume_spike` 放量

| 参数 | 默认 | 说明 |
|---|---|---|
| `period` | 20 | 均额周期（不含当日） |
| `multiplier` | 2.0 | 放大倍数 |
| `price_direction` | `up` | `up` / `down` / `any` |

用日 K **成交额** `amount`。当日额 ≥ 均额 × 倍数，并按价格方向过滤。

---

## 5. `macd_cross` MACD 交叉

| 参数 | 默认 | 说明 |
|---|---|---|
| `fast_period` | 12 | 快线 EMA |
| `slow_period` | 26 | 慢线 EMA |
| `signal_period` | 9 | 信号线 EMA |
| `direction` | `bullish` | `bullish` / `bearish`（亦支持 `both`） |

金叉：DIF 上穿 DEA，且柱状图由负转正。死叉对称。

---

## 6. `bollinger` 布林带突破

| 参数 | 默认 | 说明 |
|---|---|---|
| `period` | 20 | 中轨 SMA 周期 |
| `std_dev` | 2.0 | 标准差倍数 |
| `direction` | `lower` | `lower` / `upper` / `both` |

下轨：昨收 < 昨下轨，今收 ≥ 今下轨。上轨：昨收 < 昨上轨，今收 ≥ 今上轨。

---

## 7. `price_breakout` 价格突破

| 参数 | 默认 | 说明 |
|---|---|---|
| `period` | 20 | 回看高低点周期（不含确认 K） |
| `confirm_bars` | 1 | 连续确认根数 |
| `direction` | `breakout_up` | `breakout_up` / `breakout_down` |

突破阻力：确认区内收盘均 > 回看窗最高价。跌破支撑：均 < 回看窗最低价。

---

## 8. `ma_alignment` 均线多空排列

| 参数 | 默认 | 说明 |
|---|---|---|
| `short_period` | 5 | 短期 |
| `mid_period` | 10 | 中期 |
| `long_period` | 20 | 长期 |
| `ma_type` | `ema` | `sma` / `ema` |
| `direction` | `bullish` | `bullish` / `bearish` |

亦兼容 `periods: [5,10,20]`。多头：短>中>长，且三日线均高于前一日（向上发散）。空头对称。

---

## 9. `price_change` 涨跌幅区间

| 参数 | 默认 | 说明 |
|---|---|---|
| `min_pct` | 2.0 | 最小涨跌幅 % |
| `max_pct` | 7.0 | 最大涨跌幅 % |

当日 `change_pct ∈ [min, max]`（自动纠正 min/max 顺序）。

---

## 10. `period_return` 日/周/月涨跌

在近 **N** 个时间桶上，同时约束每桶涨跌与整段总涨跌。日桶用当日 `change_pct`；周按 ISO 周、月按 `YYYY-MM` 用首尾收盘价算桶涨跌。

| 参数 | 默认 | 说明 |
|---|---|---|
| `timeframe` | `day` | `day` / `week` / `month` |
| `n` | 5 | 回看桶数 |
| `period_mode` | `all_in_range` | `all_in_range` / `all_up` / `all_down` / `majority` / `avg_in_range` |
| `period_min_pct` | 0 | 每桶下限 % |
| `period_max_pct` | 900 | 每桶上限；**≥900 表示不限** |
| `total_min_pct` | 8 | 总涨跌下限 % |
| `total_max_pct` | 900 | 总涨跌上限；**≥900 表示不限** |
| `min_up_count` | 4 | `majority` 时至少几桶落在每桶区间 |
| `include_partial` | `false` | 周/月是否计入未收完的最后一桶 |

- **日 · 总涨跌**：各日涨跌幅连乘折算  
- **周/月 · 总涨跌**：窗口首桶首收 → 末桶末收  

默认语义：近 5 日每日不跌（≥0），合计涨幅 ≥ 8%。

```json
{
  "type": "period_return",
  "timeframe": "day",
  "n": 5,
  "period_mode": "all_in_range",
  "period_min_pct": 0,
  "period_max_pct": 900,
  "total_min_pct": 8,
  "total_max_pct": 900,
  "min_up_count": 4,
  "include_partial": false
}
```

---

## 11. 利润表 / 现金流 / 资产负债财务策略

数据表：`sc_stock_income_statement`（Miana 利润表，指标在 `payload` JSON）。独立原子，可单独组合。

| type_id | 名称 | payload 字段 |
|---|---|---|
| `income_revenue` | 营业收入 | `operatingRevenue`（无则 `totalRevenue`） |
| `income_parent_np` | 归母净利润 | `netProfitToParent` |
| `income_deduct_np` | 扣非净利润 | `netProfitCorrected` |
| `income_rd` | 研发费用 | `researchAndDevelopment` |

每个策略参数相同：

| 度量 `measure` | 含义 |
|---|---|
| `value` | 最新报告期绝对值，阈值单位 **亿元** |
| `yoy` | 同比 %：同 `fiscal_period` 且上年（或约 365 日同类型报告） |
| `qoq` | 环比 %：按 `report_date` 上一份报告（披露口径，非单季拆分） |

| 参数 | 默认 | 说明 |
|---|---|---|
| `measure` | `yoy` | 见上 |
| `min` | 10 | 下限 |
| `max` | 900 | 上限；**≥900 表示不限** |

默认：对应指标同比 ≥ 10%。无财报或缺字段的股票该规则不命中。

```json
{ "type": "income_revenue", "measure": "yoy", "min": 10, "max": 900 }
```

### 现金流量表

数据表：`sc_stock_cashflow_statement`。参数同利润表（`measure` / `min` / `max`）。

| type_id | 名称 | payload 字段 |
|---|---|---|
| `cf_operating` | 经营现金流净额 | `netCashFromOperating` |
| `cf_investing` | 投资现金流净额 | `netCashFromInvesting` |
| `cf_financing` | 筹资现金流净额 | `netCashFromFinancing` |

```json
{ "type": "cf_operating", "measure": "yoy", "min": 10, "max": 900 }
```

### 资产负债表

数据表：`sc_stock_balance_sheet`。参数同上。

| type_id | 名称 | payload 字段 |
|---|---|---|
| `bs_receivable` | 应收账款 | `accountsReceivable` |

```json
{ "type": "bs_receivable", "measure": "yoy", "min": 0, "max": 30 }
```

---

## 12. 组合示例

```json
{
  "op": "and",
  "rules": [
    { "type": "ma_cross", "fast_period": 5, "slow_period": 20, "ma_type": "ema", "direction": "golden" },
    { "type": "volume_spike", "period": 20, "multiplier": 2.0, "price_direction": "up" },
    { "type": "price_change", "min_pct": 2.0, "max_pct": 7.0 }
  ]
}
```

保存：`POST /api/conditions`，`kind: "screen"`，`expression` 为 JSON 字符串。

# A 股全市场财报统计

> 实现：`backend/src/market/fund_stats.rs`、`GET /api/market/fund-stats`。  
> App：更多 → 财报统计。

## 概览

按 **财年 + 期次** 同口径统计全市场，分两类「面」，并汇总各面的同比/环比升降：

| 类型 | 指标 | 分子条件 |
|---|---|---|
| 水平面 | 归母净利赚钱面 | `netProfitToParent` > 0 |
| 水平面 | 经营现金流为正 | `netCashFromOperating` > 0 |
| 增长面 | 归母净利增长面 | 相对上年同口径归母净利增速 > 0 |
| 增长面 | 营收增长面 | 相对上年同口径营收增速 > 0 |

分母均为该期有有效字段的公司数（缺报不计入）。

### 同比 / 环比（百分点）

每个「面」附带：

- **同比** `yoy_delta_pct`：本期占比 − **上年同口径**占比（百分点）
- **环比** `qoq_delta_pct`：本期占比 − **上一报告期**占比（百分点）

上一报告期：`Q1` ← 上年 `FY`，`H1` ← `Q1`，`Q3` ← `H1`，`FY` ← `Q3`。

`summary` 汇总四项面中同比/环比上升、下降、持平的个数（|Δ| ≤ 0.05pct 视为持平）。

> 注：环比比较的是「面」本身（市场广度），不是把累计报表拆成单季后再算公司级环比增速。

## 期次

| id | 说明 | 库内 `fiscal_period` 兼容 |
|---|---|---|
| `Q1` | 一季报 | Q1, 1 |
| `H1` | 中报 | H1, Q2, HY, SEMI, S1 |
| `Q3` | 三季报 | Q3, 3 |
| `FY` | 年报 | FY, Q4, A, ANNUAL, Y, YEAR |

## API

| 方法 | 路径 | 说明 |
|---|---|---|
| GET | `/api/market/fund-stats/meta` | 期次列表、默认年/期、可选年份 |
| GET | `/api/market/fund-stats?year=&period=` | 统计结果；缺省则用最新可用 |

### 响应要点

```json
{
  "year": 2024,
  "period": "FY",
  "period_label": "年报",
  "available_years": [2024, 2023, 2022],
  "yoy_base": { "year": 2023, "period": "FY", "period_label": "年报" },
  "qoq_base": { "year": 2024, "period": "Q3", "period_label": "三季报" },
  "net_profit": {
    "sample": 4800, "positive": 3800, "non_positive": 1000,
    "positive_pct": 79.2, "yoy_delta_pct": 2.1, "qoq_delta_pct": -0.5
  },
  "net_profit_yoy": { "sample": 4500, "positive": 2700, "non_positive": 1800, "positive_pct": 60.0, "yoy_delta_pct": 1.2, "qoq_delta_pct": 0.8 },
  "revenue_yoy": { "sample": 4500, "positive": 2800, "non_positive": 1700, "positive_pct": 62.2, "yoy_delta_pct": -1.0, "qoq_delta_pct": 0.3 },
  "operating_cf": { "sample": 4700, "positive": 3300, "non_positive": 1400, "positive_pct": 70.2, "yoy_delta_pct": 0.5, "qoq_delta_pct": -1.2 },
  "summary": { "yoy_up": 3, "yoy_down": 1, "yoy_flat": 0, "qoq_up": 2, "qoq_down": 2, "qoq_flat": 0 },
  "note": "…"
}
```

数据表：`sc_stock_income_statement`、`sc_stock_cashflow_statement`。

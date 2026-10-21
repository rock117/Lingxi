# A 股全市场财报统计

> 实现：`backend/src/market/fund_stats.rs`、`GET /api/market/fund-stats`。  
> App：底部「财报」Tab。

## 概览

按 **财年 + 期次** 同口径统计全市场：

| 指标 | 含义 | 分子条件 |
|---|---|---|
| 归母净利赚钱面 | 赚钱公司占比 | `netProfitToParent` > 0 |
| 营收增长面 | 营收同比改善占比 | 相对上年同口径营收增速 > 0 |
| 经营现金流为正 | 经营 CF 净额为正占比 | `netCashFromOperating` > 0 |

分母均为该期有有效字段的公司数（缺报不计入）。

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
  "net_profit": { "sample": 4800, "positive": 3800, "non_positive": 1000, "positive_pct": 79.2 },
  "revenue_yoy": { "sample": 4500, "positive": 2800, "non_positive": 1700, "positive_pct": 62.2 },
  "operating_cf": { "sample": 4700, "positive": 3300, "non_positive": 1400, "positive_pct": 70.2 },
  "note": "…"
}
```

数据表：`sc_stock_income_statement`、`sc_stock_cashflow_statement`。

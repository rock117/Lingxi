# 宽基 ETF 份额 / 市值（按组）

> 实现：`backend/src/market/etf_share_groups.rs`、`GET /api/market/etf-share-groups`。  
> App：更多 → 宽基ETF份额。  
> 数据表：`sc_etf_share_d1`（采集见 `docs/exchange_etf_share_api.md`）；市值估算对齐 `sc_fund_kline_d1_bfq.close`。

## 目标

按四个宽基组展示场内份额与估算市值的时间序列；图表只看组合计，表格可展开组内单只（按当前指标降序）。

| `index_group` | 展示名 |
|---------------|--------|
| `hs300` | 沪深300 |
| `sz50` | 上证50 |
| `kc50` | 科创50 |
| `cyb` | 创业板指 |

## 指标

| 指标 | 含义 | 算法 |
|------|------|------|
| 份额 | 组/只场内份额 | `SUM(share_units)`；展示常用 **亿份** |
| 市值 | 估算规模 | `SUM(share_units × 当日收盘)`（无收盘价的标的不计入该日市值）；展示 **亿元** |

份额为存量；周/月粒度取该桶**最后一个有数交易日**的合计，禁止多日加总。

## 粒度与区间

| 参数 | 取值 |
|------|------|
| `granularity` | `day` / `week` / `month` |
| `range` | `1m` / `3m` / `6m` / `ytd` / `1y` |

## UI

- 开关：`日|周|月`、`份额|市值`、`图表|表格`
- 图表：四组折线；点图例或表格组行可高亮
- 表格：组汇总（最新 + 区间 Δ）；展开单只，按当前指标降序

## API

`GET /api/market/etf-share-groups?granularity=day&range=3m`

一次返回份额与市值序列，客户端切换指标不重拉。

### 响应要点

```json
{
  "as_of": "2026-11-21",
  "range": "3m",
  "granularity": "day",
  "start_date": "2026-08-21",
  "note": "市值=份额×收盘估算；缺K线标的不计入当日市值。数据多为T+1。",
  "groups": [
    {
      "id": "hs300",
      "label": "沪深300",
      "latest_share": 1.23e11,
      "latest_mkt": 4.56e11,
      "delta_share": 1.0e9,
      "delta_share_pct": 0.8,
      "delta_mkt": 2.0e9,
      "delta_mkt_pct": 0.4,
      "series": [{ "date": "2026-08-21", "share": 1.2e11, "mkt": 4.5e11 }],
      "members": [
        {
          "symbol": "510300.SH",
          "name": "沪深300ETF华泰柏瑞",
          "share": 5e10,
          "mkt": 2e11,
          "delta_share": 1e8,
          "delta_share_pct": 0.2,
          "delta_mkt": 5e8,
          "delta_mkt_pct": 0.25
        }
      ]
    }
  ]
}
```

`members` 按 `as_of` 市值降序（客户端切份额时可按份额重排）。数值单位为 **份 / 元**（原始），App 再格式化为亿份/亿元。

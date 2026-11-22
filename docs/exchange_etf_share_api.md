# 交易所 ETF 基金份额 API

> 用途：日频采集 **场内 ETF 基金份额**（申赎规模代理），不依赖东财 / 高积分 Tushare。  
> 参考实现口径与 [AKShare](https://github.com/akfamily/akshare) 同源（可直接 HTTP，不必用 Python）。  
> 本文仅覆盖「份额 / 规模」；行情仍用现有 Miana / 自建 K 线。  
> App 按组展示：[`etf_share_groups.md`](./etf_share_groups.md)。

## 概览

| 市场 | 接口形态 | 历史 | 建议用法 |
|------|----------|------|----------|
| **上交所** | JSON（`commonQuery.do`） | 按日改 `STAT_DATE` | 按交易日循环回补 / 日更 |
| **深交所** | XLSX（`ShowReport`） | `txtStart`～`txtEnd`，单次约 ≤6 个月 | 分段回补 / 日更 |

共性：

- 免费、无 token；需合理 UA，上交所 **必须带 Referer**。
- 数据多为 **清算后** 披露，通常 **T+1 上午** 才有上一交易日。
- 口径一般为 **场内份额**；与部分 App「总份额」（含场外）可能不一致，做趋势对比时统一用交易所即可。
- 库内尚无对应表；落库时建议自建日频表（见文末）。

---

## 1. 上交所 ETF 基金份额

### 1.1 页面

https://www.sse.com.cn/assortment/fund/etf/list/scale/

### 1.2 请求

| 项 | 值 |
|----|----|
| Method | `GET` |
| URL | `https://query.sse.com.cn/commonQuery.do` |

**Query 参数**

| 参数 | 必填 | 说明 |
|------|------|------|
| `sqlId` | 是 | 固定 `COMMON_SSE_ZQPZ_ETFZL_XXPL_ETFGM_SEARCH_L` |
| `STAT_DATE` | 是 | 统计日，`yyyy-MM-dd`（换日即取该日截面） |
| `isPagination` | 建议 | `true` |
| `pageHelp.pageSize` | 建议 | `10000`（一次拉全） |
| `pageHelp.pageNo` | 建议 | `1` |
| `pageHelp.beginPage` | 建议 | `1` |
| `pageHelp.endPage` | 建议 | `1` |
| `pageHelp.cacheSize` | 建议 | `1` |

**Headers（必填）**

```http
Referer: https://www.sse.com.cn/
User-Agent: Mozilla/5.0 …
```

### 1.3 响应

JSON。份额列表在 `result[]`。

| 字段 | 类型 | 说明 |
|------|------|------|
| `STAT_DATE` | string | 统计日期 `yyyy-MM-dd` |
| `SEC_CODE` | string | 基金代码（6 位，如 `510300`） |
| `SEC_NAME` | string | 简称 |
| `ETF_TYPE` | string | ETF 类型（如单市 / 跨市等） |
| `TOT_VOL` | string/number | **基金份额，单位：万份** |
| `NUM` | string | 序号 |

换算：

```text
份额(份) = TOT_VOL × 10000
```

### 1.4 示例

```http
GET https://query.sse.com.cn/commonQuery.do?isPagination=true&pageHelp.pageSize=10000&pageHelp.pageNo=1&pageHelp.beginPage=1&pageHelp.cacheSize=1&pageHelp.endPage=1&sqlId=COMMON_SSE_ZQPZ_ETFZL_XXPL_ETFGM_SEARCH_L&STAT_DATE=2025-01-15
Referer: https://www.sse.com.cn/
```

`result[0]` 示例（字段以实盘为准）：

```json
{
  "STAT_DATE": "2025-01-15",
  "ETF_TYPE": "单市",
  "SEC_CODE": "510010",
  "NUM": "1",
  "SEC_NAME": "治理ETF",
  "TOT_VOL": "20152.44"
}
```

### 1.5 历史数据

- **支持历史**：改 `STAT_DATE` 即可取过去交易日截面（已用 `2025-01-15` 验证有数据）。
- **无日期区间参数**：要长历史 → 按 `sc_trade_calendar`（或等价交易日历）逐日请求。
- 非交易日 / 未披露日：`result` 可能为空，落库时跳过即可。

### 1.6 AKShare 对应

`ak.fund_etf_scale_sse(date="20250115")`  
（内部会把 `TOT_VOL` ×10000 写成「份」。）

---

## 2. 深交所 ETF 基金规模（日频，推荐）

### 2.1 页面

https://www.szse.cn/market/fund/volume/etf/index.html

### 2.2 请求

| 项 | 值 |
|----|----|
| Method | `GET` |
| URL | `https://www.szse.cn/api/report/ShowReport` |
| 响应 | **XLSX 二进制**（不是 JSON） |

**Query 参数**

| 参数 | 必填 | 说明 |
|------|------|------|
| `SHOWTYPE` | 是 | `xlsx` |
| `CATALOGID` | 是 | `scsj_fund_jjgm` |
| `TABKEY` | 是 | `tab1` |
| `txtStart` | 是 | 开始日 `yyyy-MM-dd` |
| `txtEnd` | 是 | 结束日 `yyyy-MM-dd` |
| `jjlb` | 是 | `ETF`（亦可 `LOF` / `不动产基金`） |
| `random` | 建议 | 任意随机数，防缓存 |

**Headers**

```http
Referer: https://www.szse.cn/market/fund/volume/etf/index.html
User-Agent: Mozilla/5.0 …
Host: www.szse.cn
```

### 2.3 响应（Excel 列）

| 列名 | 说明 |
|------|------|
| `日期` | 统计日 |
| `基金代码` | 6 位代码（读表时注意浮点，建议 zfill） |
| `基金简称` | 简称 |
| `基金规模(份)` | **基金份额，单位：份** |

示例区间 `2025-01-02`～`2025-01-03` 可正常返回多行（含货币 ETF 等，按业务过滤即可）。

### 2.4 历史数据与限制

- **支持历史区间**：一次可拉多日。
- **单次跨度约 ≤ 6 个月**：再长常返回空表（仅表头）；回补请按 ≤6 个月分段。
- 长历史：`[start, start+6m)` 循环，直到目标 `end`。

### 2.5 AKShare 对应

`ak.fund_scale_daily_szse(start_date="20250102", end_date="20250103", symbol="ETF")`

---

## 3. 深交所「当前列表」份额（仅最新，不推荐做历史）

仅作对照，**不要**当历史源。

| 项 | 值 |
|----|----|
| 页面 | https://fund.szse.cn/marketdata/fundslist/index.html |
| URL | `https://fund.szse.cn/api/report/ShowReport` |
| 参数 | `SHOWTYPE=xlsx`，`CATALOGID=1000_lf`，`TABKEY=tab1` |
| Referer | `https://fund.szse.cn/marketdata/fundslist/index.html` |
| 份额列 | `当前规模(份)` |
| 历史 | **无日期参数**，只有当前截面 |

AKShare：`ak.fund_etf_scale_szse()`。

---

## 4. 与其它数据源对比

| 源 | 份额历史 | 备注 |
|----|----------|------|
| **上交所 / 深交所（本文）** | 有 | 免费；深交所分段 ≤6 个月 |
| Miana `/api/fund/v2/sort` | 无官方历史 | 仅实时 `totShr`/`mktCap`，可自采截面 |
| 东财 `fund_etf_spot_em` | 无长历史 | 「最新份额」；易限流 |
| Tushare `etf_share_size` | 有 | 文档要求较高积分（如 8000） |
| Tushare `fund_share` | 有 | 积分门槛相对低，口径需自行核对 |

---

## 5. 采集与落库建议

### 5.1 日更

1. 取上一交易日 `D`（`sc_trade_calendar`）。
2. 上交所：`STAT_DATE=D` → 全市场截面。
3. 深交所：`txtStart=txtEnd=D`，`jjlb=ETF`。
4. 代码统一为 `xxxxxx.SH` / `xxxxxx.SZ`，份额统一为 **份**（注意上交所万份换算）。
5. Upsert 主键：`(symbol, trade_date)`。

### 5.2 回补

- 沪：交易日循环。
- 深：按最多约 6 个月一段循环。
- 限速：建议间隔数百毫秒～1s，失败重试；勿高频打满。

### 5.3 表示例（尚未建表）

```sql
-- 建议表名示例，实际迁移放 SecuritiesCollector
CREATE TABLE IF NOT EXISTS sc_etf_share_d1 (
    symbol       TEXT NOT NULL,          -- 510300.SH / 159919.SZ
    trade_date   DATE NOT NULL,
    share_units  NUMERIC(24, 4) NOT NULL, -- 份
    etf_type     TEXT,                   -- 可选：上交所 ETF_TYPE
    source       TEXT NOT NULL,          -- sse / szse
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (symbol, trade_date)
);
```

规模（元）可用 `share_units × 净值或收盘价` 另算；净值可来自现有基金行情，不必强求本接口提供。

---

## 6. 注意

- 接口为交易所网页同源，**非正式 OpenAPI**；`sqlId` / `CATALOGID` 若变更需对照官网页面与 AKShare 更新。
- 货币 ETF、债券 ETF 等也会出现在列表中；若只要股票宽基，按下文「观察白名单」或 `sc_fund` 名称过滤。
- 遵守合理访问频率；生产采集优先放 Collector 侧，与 Lingxi 读库解耦。

---

## 7. 核心宽基 ETF 观察白名单

> 来源：`sc_fund`（`is_active` + `fund_type` 含 ETF）。  
> 口径：**宽基被动**——名称匹配对应指数，且不含增强/指增、价值/成长/红利/ESG/现金流等主题变体。  
> 用途：份额日更时可只落这些代码，或全市场入库后再按本表过滤。名单会随新产品增减，以库内最新为准。

### 7.1 沪深300（25）

| 代码 | 名称 |
|------|------|
| 159300.SZ | 沪深300ETF富国 |
| 159330.SZ | 沪深300ETF东财 |
| 159393.SZ | 沪深300ETF万家 |
| 159673.SZ | 沪深300ETF鹏华 |
| 159919.SZ | 沪深300ETF嘉实 |
| 159925.SZ | 沪深300ETF南方 |
| 510300.SH | 沪深300ETF华泰柏瑞 |
| 510310.SH | 沪深300ETF易方达 |
| 510320.SH | 沪深300ETF中金 |
| 510330.SH | 沪深300ETF华夏 |
| 510350.SH | 沪深300ETF工银 |
| 510360.SH | 沪深300ETF广发 |
| 510370.SH | 沪深300ETF兴业 |
| 510380.SH | 沪深300ETF国寿 |
| 510390.SH | 沪深300ETF平安 |
| 515130.SH | 沪深300ETF博时 |
| 515310.SH | 沪深300ETF汇添富 |
| 515330.SH | 沪深300ETF天弘 |
| 515350.SH | 沪深300ETF民生加银 |
| 515360.SH | 沪深300ETF方正富邦 |
| 515380.SH | 沪深300ETF泰康 |
| 515390.SH | 沪深300ETF华安 |
| 515660.SH | 沪深300ETF国联安 |
| 561930.SH | 沪深300ETF招商 |
| 563520.SH | 沪深300ETF永赢 |

国家队观察常优先：`510300` / `510310` / `510330` / `159919`。

### 7.2 上证50（11）

| 代码 | 名称 |
|------|------|
| 510050.SH | 上证50ETF华夏 |
| 510100.SH | 上证50ETF易方达 |
| 510190.SH | 上证50ETF华安 |
| 510600.SH | 上证50ETF申万菱信 |
| 510680.SH | 上证50ETF万家 |
| 510710.SH | 上证50ETF博时 |
| 510800.SH | 上证50ETF建信 |
| 510850.SH | 上证50ETF工银 |
| 510950.SH | 上证50ETF广发 |
| 530000.SH | 上证50ETF天弘 |
| 530050.SH | 上证50ETF东财 |

国家队观察常优先：`510050`。

### 7.3 科创50（17）

| 代码 | 名称 |
|------|------|
| 588000.SH | 科创50ETF华夏 |
| 588040.SH | 科创50ETF鹏华 |
| 588050.SH | 科创50ETF工银 |
| 588060.SH | 科创50ETF广发 |
| 588080.SH | 科创50ETF易方达 |
| 588090.SH | 科创50ETF华泰柏瑞 |
| 588150.SH | 科创50ETF南方 |
| 588180.SH | 科创50ETF国联安 |
| 588280.SH | 科创50ETF华安 |
| 588720.SH | 科创50ETF中银 |
| 588840.SH | 科创50ETF万家 |
| 588870.SH | 科创50ETF汇添富 |
| 588940.SH | 科创50ETF富国 |
| 588950.SH | 科创50ETF景顺 |
| 589150.SH | 科创50ETF平安 |
| 589360.SH | 科创50ETF国泰 |
| 589850.SH | 科创50ETF东财 |

国家队观察常优先：`588080` / `588000`。

### 7.4 创业板指（14）

仅名称形态为「创业板ETF+管理人」、跟踪创业板指的产品；不含创业板50/200/综指，以及软件/算力/新能源等主题。

| 代码 | 名称 |
|------|------|
| 159205.SZ | 创业板ETF东财 |
| 159247.SZ | 创业板ETF汇添富 |
| 159810.SZ | 创业板ETF浦银 |
| 159821.SZ | 创业板ETF中银证券 |
| 159908.SZ | 创业板ETF博时 |
| 159915.SZ | 创业板ETF易方达 |
| 159948.SZ | 创业板ETF南方 |
| 159952.SZ | 创业板ETF广发 |
| 159956.SZ | 创业板ETF建信 |
| 159957.SZ | 创业板ETF华夏 |
| 159958.SZ | 创业板ETF工银 |
| 159964.SZ | 创业板ETF平安 |
| 159971.SZ | 创业板ETF富国 |
| 159977.SZ | 创业板ETF天弘 |

国家队观察常优先：`159915` 等头部流动性标的。

### 7.5 代码速查（67）

```text
# 沪深300
159300.SZ 159330.SZ 159393.SZ 159673.SZ 159919.SZ 159925.SZ
510300.SH 510310.SH 510320.SH 510330.SH 510350.SH 510360.SH
510370.SH 510380.SH 510390.SH 515130.SH 515310.SH 515330.SH
515350.SH 515360.SH 515380.SH 515390.SH 515660.SH 561930.SH 563520.SH

# 上证50
510050.SH 510100.SH 510190.SH 510600.SH 510680.SH 510710.SH
510800.SH 510850.SH 510950.SH 530000.SH 530050.SH

# 科创50
588000.SH 588040.SH 588050.SH 588060.SH 588080.SH 588090.SH
588150.SH 588180.SH 588280.SH 588720.SH 588840.SH 588870.SH
588940.SH 588950.SH 589150.SH 589360.SH 589850.SH

# 创业板指
159205.SZ 159247.SZ 159810.SZ 159821.SZ 159908.SZ 159915.SZ
159948.SZ 159952.SZ 159956.SZ 159957.SZ 159958.SZ 159964.SZ
159971.SZ 159977.SZ
```

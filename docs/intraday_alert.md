# 日内 TopN 推送（趋势 / 超跌）

> **本文档为规范（Source of Truth）。**  
> 实现：`backend/src/market/intraday_alert.rs`、`intraday_push.rs`。  
> 仅使用真实行情（库内日 K + 盘中 Miana），无 Mock。

---

## 1. 目标

交易时段内每 **5 分钟** 各算一类 Top10，**分别推送两条通知**：

| 类型 | kind | 含义 |
|---|---|---|
| **上涨趋势** | `intraday_alert` | 相对大盘强势 + 五日结构 + 买点/量价比，偏续涨 |
| **超跌反弹** | `intraday_oversold` | **多日回撤为主** + 当日杀跌为辅，排名越前越「跌无可跌」、反弹概率相对更大 |

两类独立筛选，允许同一标的同时出现在两份列表（极少见）。

---

## 2. 调度

| 项 | 值 |
|---|---|
| 间隔 | 300 秒 |
| 时段 | 仅开市日连续竞价窗内重算、落库并推送；非交易时段不推 |
| 数据 | `load_market_window_hybrid(db, 6, …)`：近 5 个开市日 + 当日实时 |
| 原始行情 | `market_realtime_snapshot`（`(symbol, quote_ts)`） |
| 缓存 | `market_result_cache`：`kind=intraday_alert` 与 `kind=intraday_oversold`，主键 `(kind, date)` |
| 通道 | 各一条 `PushMessage { type: "notification", … }`；按各自 `condition_id` 与上一条 `payload` 去重 |

---

## 3. 共用常量

| 符号 | 值 | 含义 |
|---|---|---|
| `AMOUNT_MIN` | `3e7` | 成交额下限（元） |
| `PRESELECT_K` | `150` | 初筛上限 |
| `LIMIT_PCT` | `9.5` | 近涨停/近跌停近似阈值（买不进） |
| `SCORE_MIN` | `55` | 上涨趋势 Top10 最低分 |
| `SCORE_MIN_OVERSOLD` | `50` | 超跌 Top10 最低分 |
| `DELTA_MIN` | `1.2` | 相对大盘最少超额/弱势（百分点） |
| `RS_CAP` | `5.0` | RS 映射封顶 |
| `VP_LO / VP_HI` | `1.5e7 / 5e8` | 量价比 log 映射界 |
| `W_A / W_B / W_C` | `0.35 / 0.30 / 0.35` | 趋势：相对强度 / 五日 / 买点 |
| `WO_A / WO_B / WO_C` | `0.50 / 0.30 / 0.20` | 超跌：回撤 / 当日弱势 / 杀跌量能 |
| `DD_MIN / DD_DEEP` | `-8 / -28` | 超跌最少回撤；满分回撤深度 |
| `OVERSOLD_HIST_MIN` | `3` | 超跌最少已收盘日 K 根数 |

大盘 \(R_{mkt}\)：有效样本等权均涨跌幅（**不含** \(R_i \ge LIMIT_PCT\) 或 \(R_i \le -LIMIT_PCT\)）。

基础有效票：成交额 ≥ `AMOUNT_MIN`、OHLC 有效、价 ≥ 2、涨跌幅有限。

---

## 4. 上涨趋势（`intraday_alert`）

### 4.1 过滤

\(R_i > 0\)、\(R_i < LIMIT_PCT\)、\(RS_i = R_i - R_{mkt} \ge DELTA_MIN\)；按 \(RS\) 取前 `PRESELECT_K`。

### 4.2 得分（0–100）

\[
score = 0.35\,A + 0.30\,B + 0.35\,C
\]

**A — 相对强度**

\[
A = 100 \cdot \mathrm{clip}\big((RS_i - DELTA_MIN) / (RS_CAP - DELTA_MIN),\,0,\,1\big)
\]

\(pos=(P-L)/(H-L)>0.7\) 时 \(A \mathrel{+}= 5\)（上限 100）。

**B — 近 5 日 K**（已收盘，不含今日）

| 子项 | 权重 | 要点 |
|---|---|---|
| B1 五日收益 | 0.35 | \(R_{5d}\) ∈ `[2,12]` 满分 |
| B2 站上 MA5 | 0.25 | |
| B3 波动 | 0.20 | 振幅适中 |
| B4 量能 | 0.15 | 末日额/前均 ∈ `[1.1,2.5]` |
| B5 阳线抬高 | 0.05 | |

缺 5 日：\(B=40\)。

**C — 买点 + 量价比**

\[
VP_i = \mathrm{amount}_i / \max(R_i,\varepsilon),\quad
C5 = 100\cdot\mathrm{clip}\frac{\ln VP_i-\ln VP_{LO}}{\ln VP_{HI}-\ln VP_{LO}}
\]

| 子项 | 权重 | 要点 |
|---|---|---|
| C1 剩余空间 | 0.20 | \(h=LIMIT_PCT-R_i\)；\(C1=100\cdot\mathrm{clip}((h-0.5)/5)\) |
| C2 回流区 | 0.20 | \(pos\in[0.35,0.65]\)→100；邻域 70；\(pos>0.9\)→35；其余 50 |
| C3 回吐 | 0.15 | \(giveback=(H-P)/P_{prev}\times100\)；&lt;1→100；&lt;3 线性降；否则 30 |
| C4 绝对流动性 | 0.10 | \(C4=100\cdot\mathrm{clip}((\ln amount-\ln AMOUNT_MIN)/3)\) |
| **C5 量价比** | **0.35** | 同上 \(C5\)；同涨幅成交额越大越好 |

排序：`score` ↓，`RS` ↓，`amount` ↓；取前 10 且 \(score\ge SCORE_MIN\)。

推送标题：`上涨趋势 Top{N}`；系统条件名「上涨趋势股」。

---

## 5. 超跌反弹（`intraday_oversold`）

多日回撤为主、当日杀跌为辅；**越超跌（回撤越深）得分越高**。

### 5.1 回撤定义

取近最多 5 个已收盘日的最高价 \(H_{peak}=\max high\)，今日收盘（实时价）\(P\)：

\[
DD_i = (P / H_{peak} - 1)\times 100
\quad\text{（百分点，通常为负）}
\]

### 5.2 过滤

- 当日 \(R_i < 0\) 且 \(R_i > -LIMIT_PCT\)（下跌但非近跌停）  
- 至少 3 根历史日 K，且 \(DD_i \le DD_{MIN}\)（默认 \(DD_{MIN}=-8\)，至少回撤 8%）  
- 按 \(|DD|\) 取前 `PRESELECT_K`

### 5.3 得分

\[
score = 0.50\,A + 0.30\,B + 0.20\,C
\]

**A — 多日回撤深度（主，0.50）**

\(DD\) 从 \(DD_{MIN}=-8\) 到 \(DD_{DEEP}=-28\) 线性映射到 0–100（更深 → 更高）：

\[
A = 100 \cdot \mathrm{clip}\big((DD_{MIN} - DD_i) / (DD_{MIN} - DD_{DEEP}),\,0,\,1\big)
\]

**B — 当日弱势（辅，0.30）**

\[
RS_i = R_i - R_{mkt}
\]

\[
B_{rs} = 100 \cdot \mathrm{clip}\big((-RS_i - DELTA_MIN) / RS_CAP,\,0,\,1\big)
\]

日内位置 \(B_{pos}\)（贴日低更好，已反抽降权）：

| \(pos\) | \(B_{pos}\) |
|---|---|
| \(\le 0.35\) | 100 |
| \(\le 0.55\) | 70 |
| \(\le 0.70\) | 45 |
| \(> 0.70\) | 25 |

\[
B = 0.75\,B_{rs} + 0.25\,B_{pos}
\]

**C — 杀跌量能（0.20）**

下跌日量价比 \(VP^{down}=\mathrm{amount}/\max(|R_i|,\varepsilon)\)，log 映射同趋势侧 \(VP_{LO}/VP_{HI}\)（放量杀跌更像出清）。

排序：`score` ↓，`|DD|` ↓，`amount` ↓；取前 10 且 \(score\ge SCORE_MIN_OVERSOLD\)。

推送标题：`超跌反弹 Top{N}`；系统条件名「超跌反弹股」。  
`items[].drawdown_pct` 写入 \(DD_i\)；`reason` 含 `DD=…`。

常量：`DD_MIN=-8`，`DD_DEEP=-28`。

---

## 6. 推送载荷

每条通知：

- `payload.kind`：`intraday_alert` 或 `intraday_oversold`  
- `payload`：`date` / `r_mkt` / `sample_size` / `candidate_size` / `items[]`  
- App：解析两类 payload；默认收起前 3 名，展开全部并标已读  

---

## 7. API（调试）

`GET /api/market/intraday-alerts` → `{ "trend": …, "oversold": … }`（即时计算，不推送）。

---

## 8. 定位

筛选工具，**非投资建议**。不保证续涨或反弹。

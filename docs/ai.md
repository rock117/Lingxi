# Lingxi AI 能力设计

> **本文档为设计规范（Source of Truth）。** 算法/边界变更先改文档再改代码。  
> 状态：**Backend `AiClient` + 个股分析 HTTP；App「分析个股」页已接入。**  
> 关联：`docs/screen.md`、`docs/fund_stats.md`、`docs/market_sentiment.md`、`docs/mid_term_trend.md`。

---

## 1. 当前范围

| 项 | 现状 |
|---|---|
| 调用方 | Backend 进程内（`AiClient`）+ App 经市场 API |
| App UI | **个股详情 →「AI 分析个股」** |
| 对外 HTTP | **不提供** 通用 `/api/ai/*`；个股分析走 `GET /api/market/stock/analyze` |
| 实现 | `backend/src/ai/`、`backend/src/market/stock_analyze.rs`、`AppConfig` AI 配置 |

原则：

1. **AI 不算行情事实** — K 线与成交明细由引擎/东财拉齐后作为 JSON 上下文塞进 prompt。  
2. **可降级** — `AI_API_KEY` 为空则 `AiClient::enabled() == false`，接口仍返回 K 线与明细摘要。  
3. **密钥只在服务端**。  
4. **不替代** MSI / MTT / screen / fund_stats 等确定性逻辑。

---

## 2. 配置（默认 DeepSeek）

官网：https://api-docs.deepseek.com/  
格式：与 OpenAI Chat Completions 兼容。

**配置位置（与 `DATABASE_URL` / `MIANA_*` 相同）：**

1. 仓库根目录 `.env`（参考 `.env.example`）  
2. `script/deploy_backend.py` 部署时写入远端 systemd `Environment=`  
3. 本地直接 `cargo run` 时需自行导出同名环境变量，或先 `set`/`export` 再启动（backend 不自动读 `.env` 文件）

| 环境变量 | 默认 | 说明 |
|---|---|---|
| `AI_API_BASE` | `https://api.deepseek.com` | 也可写 `https://api.deepseek.com/v1`（官方两者均可） |
| `AI_API_KEY` | 空 | DeepSeek 控制台申请的 Key；为空则 AI 关闭 |
| `AI_MODEL` | `deepseek-flash` | 官方当前主推；高质量可用 `deepseek-v4-pro` |
| `AI_TIMEOUT_SECS` | `30` | HTTP 超时秒数 |

请求路径由客户端拼接为：`{AI_API_BASE}/chat/completions`。请求 body 带 `"thinking": {"type": "disabled"}`。

---

## 3. 模块与内部 API

```
backend/src/ai/
  mod.rs       导出 AiClient
  client.rs    OpenAI 兼容 chat + explain / draft_screen
  prompts.rs   系统提示模板（含 stock_analyze_system）

backend/src/market/
  stock_analyze.rs   近5日K + 成交明细 + AI
  eastmoney.rs       东财 details/get
```

启动时：`AiClient::from_config(&cfg)` → `rocket.manage(ai)`；`EastmoneyClient` 同样 manage。

### 3.1 `AiClient`

| 方法 | 说明 |
|---|---|
| `enabled() -> bool` | 是否已配置 Key |
| `chat(system, user) -> Result<String>` | 通用补全，返回 assistant 文本 |
| `explain(kind, payload_json) -> Result<ExplainResult>` | 解释引擎结果 JSON（短中文） |
| `draft_screen(user_prompt) -> Result<ScreenDraft>` | 自然语言 → expression 草稿 |

---

## 4. 个股 AI 分析

### 4.1 HTTP

`GET /api/market/stock/analyze?symbol=600519.SH`

响应字段（节选）：

| 字段 | 说明 |
|---|---|
| `trade_date` | 成交明细所用交易日（非开市日则为最近开市日） |
| `detail_source` | `live` = 东财拉取并写入库；`db` = 拉取失败后用库内快照 |
| `detail_last_time` | 明细最新一笔时间，如 `2026-10-24 14:59:57`；无则 null |
| `detail_tick_count` | 笔数 |
| `detail_updated_at` | 库写入/快照时间 |
| `bars` | 近约 5 个交易日日 K |
| `analysis` | AI 正文；未启用或失败时为说明文案 |
| `ai_enabled` / `ai_model` | 是否启用及模型名 |

### 4.2 成交明细

- 数据源：东财 `push2.eastmoney.com` **details/get**（见 SecuritiesCollector `eastmoney-apis.md` §1.4）；`pos=-50000` 尽量拉全天。  
- 每行：`时间,价格,手数,方向(1买/2卖/4中性),笔序`；入库 `detail` 为同格式字符串 JSON 数组。  
- 交易日：`sc_trade_calendar` 中 `is_open` 且 `≤` 上海今日的最近一日。  
- 拉取成功：upsert `sc_stock_trade_detail`（PK `(symbol, trade_date)`，`detail` JSONB 字符串数组）。  
- 拉取失败或为空：读该日库数据；再无则读该票最近一日明细。  
- 送给 AI 的是**摘要**（买卖手数、VWAP、高低、大单、尾盘样本），不是全量 tick。

### 4.3 App

- 个股详情页按钮「AI 分析个股」→ `StockAnalyzeScreen`。  
- 页面展示：**最新一笔时间**、来源（实时/库）、近 5 日 K、AI 正文；支持「重新分析」。

---

## 5. 其它能力

### 5.1 解释 `explain`

输入：引擎已算好的 JSON。输出约 3～5 句中文。暂无专用 App 入口。

### 5.2 选股草稿 `draft_screen`

自然语言 → expression JSON（带 catalog + 校验）。**暂无 App 草稿流**。

---

## 6. 以后可选

| 项 | 说明 |
|---|---|
| `POST /api/ai/explain` 等 | 对 App 开放通用 AI 时再加 |
| App「解释一下 / AI 写策略」 | UI 另开需求 |
| Tool calling 多轮 | 首期仍是「先算数再 chat」 |

---

## 7. 与引擎文档边界

| 文档 | AI 关系 |
|---|---|
| `screen.md` | `draft_screen` 只能组合已实现原子 |
| `fund_stats.md` / 情绪 / MTT | `explain` 只解释其输出，不改公式 |
| 本页个股分析 | 只解释已拉取的 K 线与明细摘要 |

---

## 8. 一句话

> **引擎/东财出事实，AiClient 写人话；个股分析经 `/api/market/stock/analyze` 对 App 开放，密钥仍只在服务端。**

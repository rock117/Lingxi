# Lingxi AI 能力设计

> **本文档为设计规范（Source of Truth）。** 算法/边界变更先改文档再改代码。  
> 状态：**Backend `AiClient` + 个股分析（事实 API + 分维 AI）；App「分析」Tab 已接入。**  
> 关联：`docs/screen.md`、`docs/fund_stats.md`、`docs/market_sentiment.md`、`docs/mid_term_trend.md`、`docs/api_auth.md`（对外 API Key，与本文件的 `AI_API_KEY` 不同）。

---

## 1. 当前范围

| 项 | 现状 |
|---|---|
| 调用方 | Backend 进程内（`AiClient`）+ App 经市场 API |
| App UI | 首页「分析」Tab；策略选股结果点股票进分析页 |
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
  prompts.rs   加载/拼装系统提示（个股四维优先读配置文件）

backend/config/
  ai_prompts.toml    个股四维 system 提示词（盘面/短线/基本面/财报；可改后重启）
                     框架参考公开 equity / A-share Agent Skills，并约束「只吃 JSON、短文、禁买卖指令」

backend/src/market/
  stock_analyze.rs   近5日K + 成交明细 + AI
  eastmoney.rs       东财 details/get
  tencent.rs         腾讯当日分笔（东财失败回退）
```

启动时：`AiClient::from_config(&cfg)` → `rocket.manage(ai)`；`EastmoneyClient` 同样 manage。  
提示词查找顺序：`AI_PROMPTS_PATH` → `config/ai_prompts.toml`（相对 cwd）→ `backend/config/…` → `<exe_dir>/config/…`；均失败则用内置默认。

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

**事实（快）：** `GET /api/market/stock/analyze?symbol=600519.SH`  
只拉 K/明细/估值/财报并算确定性标签；各维 `ai.status=pending`（或 AI 关闭时为 `disabled`），**不调大模型**。

**分维 AI（懒加载）：** `GET /api/market/stock/analyze/ai?symbol=…&dim=tape|short_term|fundamentals|statements&force=0`  
- 复用进程内事实缓存（约 3 分钟）；AI 结果缓存约 30 分钟（同 symbol+trade_date+dim）。  
- `force=1` 跳过 AI 缓存重生成。  
- 返回 `{ dim, ai: { status, text, bias, confidence, flags }, cached, … }`。

事实响应字段（节选）：

| 字段 | 说明 |
|---|---|
| `trade_date` / `detail_*` | 成交明细交易日与来源 |
| `quote` / `bars` | 报价与近 5 日 K |
| `detail_summary` | 买卖比、VWAP、分时段、大单跟价、`labels` |
| `ai` / `short_term` / `fundamentals` / `statements` | 各维事实 + `ai` 结构；顶栏 `analysis` 与盘面 `ai.text` 同步（兼容） |
| `ai_enabled` / `ai_model` | 是否配置了上游 Key |

`DimAi`：`status`=`pending|ready|disabled|error`；`bias`=`bullish|neutral|bearish|unknown`；`confidence`=`high|medium|low|unknown`；`flags` 短标签。

模型须输出 JSON（见 `backend/config/ai_prompts.toml`）；解析失败时降级为纯文本 + `bias=unknown`。

### 4.2 成交明细与确定性指标

- 数据源顺序：**东财** → **腾讯** → 库（只读，本接口不写库）。  
- 交易日：`sc_trade_calendar` 最近开市日。  
- 盘面摘要含：买卖占比、价相对 VWAP、开盘/尾盘买卖、大单净额、大单后约 5 笔跟价%、`labels`。  
- 短线：MA 排列、价距 MA%、近 20 日高低、量能标签。  
- 基本面：`ocf_vs_np`、资产负债率、`quality_flags`。  
- 财报：`trend_flags`（营收/净利环比、现金含量等）。

### 4.3 App

- 首页 Tab **「分析」**：先拉事实，**切到某维 Tab 再请求该维 AI**；可「重新生成」。  
- UI：偏多/偏空色标、置信度、flags 芯片；完整报价随列表滚动。  
- 搜索：`GET /api/market/stocks/search`。

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
| 行业/历史估值分位 | 给基本面 AI 估值锚（需库内同业数据） |
| `POST /api/ai/explain` 等 | 对 App 开放通用 AI 时再加 |
| Tool calling 多轮 | 仍保持「先算数再 chat」 |

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

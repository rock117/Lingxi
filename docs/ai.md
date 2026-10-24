# Lingxi AI 能力设计

> **本文档为设计规范（Source of Truth）。** 算法/边界变更先改文档再改代码。  
> 状态：**Backend 内部能力已落地（可被后台代码调用）**；**暂无 App UI、暂不对外暴露 `/api/ai/*`。**  
> 关联：`docs/screen.md`、`docs/fund_stats.md`、`docs/market_sentiment.md`、`docs/mid_term_trend.md`。

---

## 1. 当前范围

| 项 | 现状 |
|---|---|
| 调用方 | **仅 backend 进程内**（推送文案、内部任务、后续脚本等） |
| App UI | **不做** |
| 对外 HTTP | **不提供** `/api/ai/*`（以后若开放再补路由） |
| 实现 | `backend/src/ai/` + `AppConfig` 中的 AI 配置 |

原则不变：

1. **AI 不算行情事实** — 数字由引擎算好后作为上下文塞进 prompt。  
2. **可降级** — `AI_API_KEY` 为空则 `AiClient::enabled() == false`，调用方自行跳过。  
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

请求路径由客户端拼接为：`{AI_API_BASE}/chat/completions`，例如：

```bash
curl https://api.deepseek.com/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer ${AI_API_KEY}" \
  -d '{
        "model": "deepseek-flash",
        "messages": [
          {"role": "system", "content": "You are a helpful assistant."},
          {"role": "user", "content": "Hello!"}
        ],
        "stream": false,
        "thinking": {"type": "disabled"}
      }'
```

### DeepSeek 注意点

1. **鉴权**：`Authorization: Bearer <key>`（与 OpenAI 相同）。  
2. **模型名**：用 `deepseek-flash` / `deepseek-v4-pro`（旧名 `deepseek-chat` 若仍可用，以官网为准）。  
3. **Thinking**：官网默认开启 thinking；本项目内部调用在 body 里带 `"thinking": {"type": "disabled"}`，避免解释/JSON 草稿变慢、变贵。需要深度推理时再在代码里打开。  
4. Thinking 开启时，`temperature` 等参数可能被忽略；最终答案仍在 `choices[0].message.content`（思维链在 `reasoning_content`，本客户端当前只读 `content`）。

---

## 3. 模块与内部 API

```
backend/src/ai/
  mod.rs       导出 AiClient
  client.rs    OpenAI 兼容 chat + explain / draft_screen
  prompts.rs   系统提示模板
```

启动时：`AiClient::from_config(&cfg)` → `rocket.manage(ai)`。  
其它模块：`ai: &State<AiClient>` 或持有 `AiClient` 克隆后调用。

### 3.1 `AiClient`

| 方法 | 说明 |
|---|---|
| `enabled() -> bool` | 是否已配置 Key |
| `chat(system, user) -> Result<String>` | 通用补全，返回 assistant 文本 |
| `explain(kind, payload_json) -> Result<ExplainResult>` | 解释引擎结果 JSON（短中文） |
| `draft_screen(user_prompt) -> Result<ScreenDraft>` | 自然语言 → expression 草稿（带 catalog 约束 + 校验） |

`kind` 建议取值：`screen_result` / `sentiment` / `fund_stats` / `mid_term` / `intraday_alert`（自由字符串，写入 system 提示）。

### 3.2 调用约定（后台代码）

```rust
if !ai.enabled() {
    // 跳过或用模板文案
    return Ok(());
}
let text = ai.explain("fund_stats", &serde_json::to_string(&stats)?).await?;
// 使用 text.text
```

- 传入的 `payload_json` 应为 **摘要**（统计字段、TopN 简表），避免整表行情。  
- 失败时返回 `Err`，调用方决定降级，**不要**让 AI 失败拖垮选股/推送主路径。

---

## 4. 能力说明

### 4.1 解释 `explain`（优先给内部用）

输入：引擎已算好的 JSON 字符串。  
输出：`ExplainResult { text, model }`，约 3～5 句中文。  
用途示例：盘后简报、日志旁注、通知正文润色。

### 4.2 选股草稿 `draft_screen`

输入：自然语言。  
输出：`ScreenDraft { expression, explanation, warnings }`。  
服务端会：

1. 把精简 `strategy_catalog()` 塞进 system prompt  
2. 要求模型只输出 JSON  
3. 用 `screen::parse_expression` 校验；非法 type/参数写入 `warnings` 或直接 `Err`

**暂无 App 草稿流**；可供后台批测、管理脚本或未来 HTTP 复用同一函数。

---

## 5. 以后可选（未做）

| 项 | 说明 |
|---|---|
| `POST /api/ai/explain` 等 | 对 App 开放时再加 routes，内部仍调同一 `AiClient` |
| App「解释一下 / AI 写策略」 | UI 另开需求 |
| Tool calling 多轮 | 首期用「调用方先算数 → 再 explain」即可；复杂编排再加 tools |
| 按用户限流 | 目前仅内部调用，靠超时与调用方节流 |

---

## 6. 与引擎文档边界

| 文档 | AI 关系 |
|---|---|
| `screen.md` | `draft_screen` 只能组合已实现原子 |
| `fund_stats.md` / 情绪 / MTT | `explain` 只解释其输出，不改公式 |

---

## 7. 一句话

> **Backend 自用：引擎算事实，AiClient 写人话；App 与公网 AI API 暂不接入。**

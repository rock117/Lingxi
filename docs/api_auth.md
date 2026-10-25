# Lingxi API 认证与限流设计

> **本文档为设计规范（Source of Truth）。** 实现前以本文为准；落地后同步改状态行。  
> 状态：**设计中，代码未接入。** 当前 backend 直出公网、路由无鉴权。  
> 背景：无 nginx / API 网关，认证与限流均在 **Rocket 进程内**完成。

---

## 1. 目标与非目标

### 目标

1. 挡住公网扫描与白嫖（尤其 `stock/analyze` 的 AI + 外部行情成本）。  
2. 保护写接口（条件 CRUD、通知删除等），避免数据被污染。  
3. 改动面小：单租户、个人 App 自用，不引入完整账号体系。

### 非目标（现阶段）

- 多用户注册 / 登录 / Refresh Token / RBAC  
- 按用户隔离数据（当前即为单租户库）  
- 依赖额外反向代理才能鉴权（代理可选，非前置条件）  
- 指望 APK 内藏 Key「绝对不可逆向」（防的是扫端口滥用，不是防破解；靠轮换 + 限流）

---

## 2. 推荐方案：共享 API Key + 进程内限流

```
App (OkHttp Interceptor)
  Authorization: Bearer <LINGXI_API_KEY>
        │
        ▼
Rocket Request Guard
  1. 校验 Bearer 与环境变量一致（常量时间比较）
  2. 按 IP（及/或 Key）滑动窗口限流 → 超限 429
  3. 通过后进入业务 handler
```

无独立网关时，鉴权与限流都放在 Rust 内即可。

---

## 3. 配置

| 变量 | 默认 | 说明 |
|---|---|---|
| `LINGXI_API_KEY` | 空 | 服务端与 App 共享的长随机串（建议 ≥32 字节）；**空则业务路由应拒绝或启动强告警** |
| （可选）`API_RATE_LIMIT_PER_MIN` | `120` | 全局限流：每 IP 每分钟 |
| （可选）`API_ANALYZE_RATE_LIMIT_PER_MIN` | `6` | 仅 `stock/analyze`：每 IP 每分钟 |

配置位置与现有环境变量一致：

1. 仓库根目录 `.env` / `.env.example`  
2. `script/deploy_backend.py` → systemd `Environment=`  
3. App：设置页与 `ServerConfig` 持久化（**勿把生产 Key 写死进仓库或 APK 源码**）

与 `AI_API_KEY`（调 DeepSeek）区分：`LINGXI_API_KEY` 只保护 **Lingxi 对外 HTTP/WS**。

---

## 4. 服务端设计（Rocket）

### 4.1 Request Guard

- 类型名建议：`ApiAuth`（实现 `FromRequest`）。  
- 读取 `Authorization: Bearer <token>`；缺失、格式错误、与 `LINGXI_API_KEY` 不等 → **401**。  
- 比较使用常量时间算法（如 `subtle`），降低时序探测风险。  
- 路由写法示例：handler 增加参数 `_auth: ApiAuth`，无需改业务逻辑本体。

### 4.2 路由分层

| 类别 | 路径（示例） | 鉴权 | 限流 |
|---|---|---|---|
| 探活 | `GET /api/health` | **可不鉴权** | 宽松或仅防爆破 |
| 读行情 / 搜索 / 选股 / 策略 / 财报统计等 | `/api/market/*`（除下项） | 要 | 全局窗口 |
| 个股 AI 分析 | `GET /api/market/stock/analyze` | 要 | **更严**（单独配额） |
| 条件 / 通知写接口 | `/api/conditions/*`、`/api/notifications/*` | 要 | 全局窗口 |
| WebSocket | `/ws`（以实际路径为准） | 要 | 连接数 / 握手频率 |

写接口与 `analyze` **必须**鉴权。只读行情也可统一鉴权，避免被人当免费代理。

### 4.3 WebSocket 带 Key

浏览器/部分客户端握手不便自定义头时，二选一（实现时定一种并写进本文）：

1. 查询参数：`?token=<LINGXI_API_KEY>`（日志需脱敏，勿打全量 token）  
2. 或子协议 / 首条鉴权消息（更啰嗦，一般个人 App 用 1 即可）

HTTP 与 WS **共用同一 Key**。

### 4.4 进程内限流

- 结构：滑动窗口或固定窗口，`DashMap`（或等价）以 **客户端 IP** 为键；可选再按 Key 计数。  
- 超限响应：**429**，正文简短说明；可带 `Retry-After`。  
- `stock/analyze` 使用独立、更小的配额（防刷 DeepSeek / 东财腾讯）。  
- 取 IP：优先 `X-Forwarded-For` 最左段（仅当你将来前面有可信反代时启用）；当前直出则用连接对端地址，避免客户端伪造头绕过。

### 4.5 CORS

- 已允许 `Authorization` 头则保持。  
- 原生 App 不依赖 CORS；若仅自用，可逐步收紧 `allowed_origins`（调试来源白名单）。  
- `allow_credentials` 与 `AllowedOrigins::all()` 长期并存不理想，收紧 origin 时一并整理。

### 4.6 TLS

- 公网暴露时 Bearer 在明文 HTTP 上可被窃听 → **应上 TLS**（本机证书、Caddy、或 Cloudflare 橙云等）。  
- TLS 可在外部终止；**鉴权仍在 Rocket**。文档不强制某一种部署，但公网明文视为不可接受。

---

## 5. App 设计

1. `ServerConfig`（或设置页）增加 `apiKey`，与 `baseUrl` 一并存储。  
2. `ApiClient` 的 OkHttp `Interceptor`：若 Key 非空，附加  
   `Authorization: Bearer <apiKey>`。  
3. WS 客户端握手带同一 token（与 §4.3 选定方式一致）。  
4. 收到 **401**：提示检查设置中的 API Key；**429**：提示稍后重试。  
5. 个人分发：Key 本地配置；设备丢失或泄漏时，只改服务端 `LINGXI_API_KEY` 并更新 App 设置即可轮换。

---

## 6. 错误与可观测性

| HTTP | 含义 |
|---|---|
| 401 | 未带或错误 Key |
| 429 | 触发限流 |
| 503 | （可选）`LINGXI_API_KEY` 未配置且策略为「拒绝服务」时 |

日志：

- 记录鉴权失败次数、限流命中（带 request_id、IP 哈希或截断 IP）。  
- **禁止**把完整 `LINGXI_API_KEY` / Bearer 写入日志。

---

## 7. 落地顺序

1. 环境变量 `LINGXI_API_KEY` + `ApiAuth` 守卫挂到除 health 外的路由。  
2. App 设置页 + OkHttp Interceptor；WS 同步带 token。  
3. 全局限流 + `analyze` 单独限流。  
4. 公网 TLS；按需收紧 CORS / 加反代。  
5. （以后多用户）守卫改为验 JWT；共享 Key 留给脚本/机器账号。`ApiAuth` 调用点可复用。

---

## 8. 与现有文档关系

| 文档 | 关系 |
|---|---|
| `docs/ai.md` | `AI_API_KEY` 为上游模型密钥；本文件的 `LINGXI_API_KEY` 保护对外入口。analyze 限流与 AI 成本直接相关。 |
| `script/deploy_backend.py` | 落地时增加 systemd `LINGXI_API_KEY=`（及可选限流变量）。 |
| 各 `docs/*` API 节 | 实现后在「鉴权」处注明：需 `Authorization: Bearer`。 |

---

## 9. 验收清单（实现后勾选）

- [ ] 无 Key 访问业务 API → 401  
- [ ] 错误 Key → 401  
- [ ] 正确 Key → 200（业务正常）  
- [ ] health 无 Key 仍可用（若按本文白名单）  
- [ ] analyze 超配额 → 429，其它接口仍可能可用  
- [ ] App 配置 Key 后全链路（含 WS）可用  
- [ ] 日志无明文 Key  

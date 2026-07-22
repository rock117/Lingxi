# Lingxi (灵犀)

一个炒股条件推送应用：当自定义的交易信号或新闻事件条件满足时，实时推送通知到手机 App。

详细设计与实现计划见 [PLAN.md](./PLAN.md)。

## 项目结构

```
Lingxi/
├── PLAN.md      # 详细实现计划
├── backend/     # Rust 后端（Rocket + SeaORM + SQLite + WebSocket）
└── app/         # Flutter App（Android）
```

## 架构概览

```
┌─────────────────┐         HTTP REST (CRUD)         ┌──────────────────┐
│  Flutter App    │ ◄──────────────────────────────► │  Rust Backend    │
│  (Android)      │                                  │  (Rocket+tokio)  │
│                 │       WebSocket (push)           │                  │
│                 │ ◄─────────────────────────────── │  ┌────────────┐  │
└─────────────────┘                                  │  │ Mock Engine│  │
                                                     │  │ (周期触发) │  │
                                                     │  └────────────┘  │
                                                     │  SQLite (SeaORM) │
                                                     └──────────────────┘
```

- **后台 API**：抓取/接收数据（当前为 Mock 假数据），根据用户配置的条件进行匹配，
  满足时通过 WebSocket 推送给已连接的客户端，并持久化到 SQLite。
- **App**：新建/管理条件（交易指令 或 新闻事件），保存到后台；接收并展示推送的通知。

## 技术栈

### 后端 (`backend/`)
- [Rocket](https://rocket.rs/) 0.5 + `rocket_ws` — Web 框架 + WebSocket
- [SeaORM](https://www.sea-ql.org/SeaORM/) 2.0 — ORM（SQLite 后端）
- [sqlx-cli](https://github.com/launchbadge/sqlx) — 数据库 migration 管理（纯 SQL 文件）
- `tokio` — 异步运行时

### App (`app/`)
- Flutter（Android）
- `dio` — HTTP 客户端
- `web_socket_channel` — WebSocket 客户端
- `provider` — 状态管理
- `shared_preferences` — 服务器配置本地持久化

## 快速开始

### 后端

```bash
cd backend

# 安装 sqlx-cli（首次）
cargo install sqlx-cli --no-default-features --features sqlite,rustls

# 执行数据库 migration
sqlx migrate run --database-url "sqlite://data.db?mode=rwc" --source migrations

# 启动服务（默认监听 0.0.0.0:8000）
cargo run
```

验证服务：

```bash
curl http://127.0.0.1:8000/api/health
```

### App

```bash
cd app
flutter pub get
flutter run
```

首次启动后，在设置页填写后端服务器地址与端口（局域网内运行的后端 IP + 8000）。

## API 一览

| 方法 | 路径 | 说明 |
|---|---|---|
| GET | `/api/health` | 健康检查 |
| POST | `/api/conditions` | 新建条件 |
| GET | `/api/conditions` | 列出所有条件 |
| GET | `/api/conditions/:id` | 获取单个条件 |
| PUT | `/api/conditions/:id` | 更新条件 |
| DELETE | `/api/conditions/:id` | 删除条件 |
| GET | `/api/notifications` | 列出通知（`?unread=true` 过滤未读） |
| POST | `/api/notifications/:id/read` | 标记通知已读 |
| WS | `/ws` | WebSocket 推送通道 |

更多细节（数据模型、条件表达式格式等）见 [PLAN.md](./PLAN.md)。

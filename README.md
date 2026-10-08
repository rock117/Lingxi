# Lingxi (灵犀)

一个炒股条件推送应用：当自定义的交易信号或新闻事件条件满足时，实时推送通知到手机 App。

详细设计与实现计划见 [PLAN.md](./PLAN.md)。

## 目录

- [项目结构](#项目结构)
- [架构概览](#架构概览)
- [技术栈](#技术栈)
  - [后端 (`backend/`)](#后端-backend)
  - [App (`app/`)](#app-app)
- [快速开始](#快速开始)
  - [后端](#后端)
  - [App](#app)
  - [部署后端到远端](#部署后端到远端)
- [API 一览](#api-一览)

## 项目结构

```
Lingxi/
├── PLAN.md      # 详细实现计划
├── backend/     # Rust 后端（Rocket + SeaORM + SQLite + WebSocket）
└── app/         # Kotlin Android App（Jetpack Compose）
```

## 架构概览

```
┌─────────────────┐         HTTP REST (CRUD)         ┌──────────────────┐
│  Kotlin App     │ ◄──────────────────────────────► │  Rust Backend    │
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
- Kotlin + Jetpack Compose
- ViewModel + StateFlow — 状态管理
- DataStore Preferences — 服务器配置本地持久化
- Navigation Compose — 页面导航
- Retrofit / OkHttp / WebSocket — 接口已定义，**当前 UI 使用 Mock 数据**（见代码中 TODO）

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

用 Android Studio 打开 `app/` 目录，或用本机已安装的 Gradle 命令行装到手机：

```bat
cd app
install-debug.bat
```

PowerShell（可跟日志）：

```powershell
cd app
powershell -ExecutionPolicy Bypass -File .\install-debug.ps1 -Logcat
```

脚本默认使用：

- Gradle：`C:\rock\coding\tool\gradle-8.10-bin\gradle-8.10`
- JDK 17：`C:\Program Files\Eclipse Adoptium\jdk-17.0.12.7-hotspot`
- SDK：`%LOCALAPPDATA%\Android\Sdk`

路径不对时改脚本参数或文件内变量即可。手机需开启 USB 调试，`adb devices` 显示 `device`。

依赖仓库已配置国内镜像（`settings.gradle.kts`：阿里云 / 腾讯云 / 华为云优先，官方兜底）；`gradlew` 的 Gradle 发行包用腾讯云。首次拉 Compose 等大包仍可能较慢，之后有本地缓存会快很多。若仍超时，可用本机 Gradle + `install-debug.bat`。

当前 App 为 **Mock 模式**：条件/通知在内存中读写，不请求后端。  
设置页默认服务器地址在**构建时**从仓库根目录 `.env` 注入（`app_server_host`，缺省则用 `ssh_server`）；用户改过设置后以 DataStore 为准。本地调试可把 `.env` 改成 `10.0.2.2` 或设置页手动改。

### 部署后端到远端

1. 在仓库根目录配置 `.env`（可参考 `.env.example`）：`ssh_server` / `ssh_user` / `ssh_pwd` 等。  
2. 安装依赖：`pip install paramiko python-dotenv`  
3. 执行：

```bash
python script/deploy_backend.py
```

脚本会：上传 `backend/` → 远端 `cargo build --release` → 安装并重启 `lingxi-backend` systemd 服务（默认目录见 `deploy_remote_dir`，端口见 `app_http_port`）。

机内健康检查（SSH 上）：

```bash
curl http://127.0.0.1:8000/api/health
# 期望 {"status":"ok"}
```

**公网访问**：需在云厂商安全组放行 TCP `app_http_port`（默认 `8000`）。未放行时手机/外网连不上，但本机 `systemctl status lingxi-backend` 仍可为 active。

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

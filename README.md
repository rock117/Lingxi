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
  - [PostgreSQL（Docker）](#postgresqldocker)
  - [后端](#后端)
  - [App](#app)
  - [部署后端到远端](#部署后端到远端)
  - [远端 systemd 服务](#远端-systemd-服务)
- [API 一览](#api-一览)

## 项目结构

```
Lingxi/
├── PLAN.md              # 详细实现计划
├── docker-compose.yml   # 本地/服务器 PostgreSQL
├── script/              # 部署等脚本（deploy_backend.py）
├── backend/             # Rust 后端（Rocket + SeaORM + PostgreSQL + WebSocket）
└── app/                 # Kotlin Android App（Jetpack Compose）
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
                                                     │ PostgreSQL       │
                                                     └──────────────────┘
```

- **后台 API**：抓取/接收数据（当前为 Mock 假数据），根据用户配置的条件进行匹配，
  满足时通过 WebSocket 推送给已连接的客户端，并持久化到 PostgreSQL。
- **App**：新建/管理条件（交易指令 或 新闻事件），保存到后台；接收并展示推送的通知。

## 技术栈

### 后端 (`backend/`)
- [Rocket](https://rocket.rs/) 0.5 + `rocket_ws` — Web 框架 + WebSocket
- [SeaORM](https://www.sea-ql.org/SeaORM/) 2.0 — ORM（PostgreSQL）
- [Docker Compose](./docker-compose.yml) — 本地 / 服务器 Postgres 16
- `tokio` — 异步运行时

### App (`app/`)
- Kotlin + Jetpack Compose
- ViewModel + StateFlow — 状态管理
- DataStore Preferences — 服务器配置本地持久化
- Navigation Compose — 页面导航
- Retrofit / OkHttp / WebSocket — 接口已定义，**当前 UI 使用 Mock 数据**（见代码中 TODO）

## 快速开始

### PostgreSQL（Docker）

仓库根目录：

```bash
docker compose up -d
docker compose ps
```

默认账号见 `.env.example`：`lingxi` / `lingxi`，库名 `lingxi`，端口 `5432`。  
连接串：`postgres://lingxi:lingxi@127.0.0.1:5432/lingxi`（写入 `.env` 的 `DATABASE_URL`）。

数据目录由 `.env` 的 `POSTGRES_DATA_DIR` 指定（宿主机路径，可为绝对路径），挂载到容器内 `/var/lib/postgresql/data`。  
例：`POSTGRES_DATA_DIR=C:/rock/data/lingxi-postgres`。`docker compose down` 不会删除该目录。

### 后端

```bash
# 先确保 Postgres 已起来
docker compose up -d

cd backend
# 默认 DATABASE_URL 指向本机 Docker Postgres；启动时自动建表
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

1. 在仓库根目录配置 `.env`（可参考 `.env.example`），至少包括：
   - `ssh_server` / `ssh_port` / `ssh_user` / `ssh_pwd`
   - `deploy_remote_dir`：远端源码编译目录（默认 `/home/rock/project/lingxi`）
   - `deploy_app_dir`：远端运行目录（默认 `/home/rock/apps/lingxi`）
   - `deploy_database_url`：写入 systemd 的数据库连接串（可与本地 `DATABASE_URL` 不同）
   - `app_http_port`：服务端口（默认 `8000`）
2. 安装依赖：`pip install paramiko python-dotenv`
3. 执行：

```bash
python script/deploy_backend.py
```

**默认步骤：**

1. 读取 `.env`  
2. SSH 连接服务器  
3. 上传本地 `backend/` 到 `deploy_remote_dir`（尊重 `.gitignore`）  
4. 远端 `cargo build --release`  
5. 将二进制安装到 `deploy_app_dir/lingxi-backend`  
6. 写入/覆盖 systemd 单元 `lingxi-backend.service` 并 `restart`  
7. 请求 `http://127.0.0.1:<port>/api/health` 做健康检查  

已有同名服务时**不会新建第二个服务**，只覆盖同一 unit 并重启；会有短暂中断。

**常用参数：**

| 参数 | 含义 |
|---|---|
| `--dry-run` | 只列出将上传的文件，不操作服务器 |
| `--skip-build` | 只上传，不编译 |
| `--skip-restart` | 上传+编译，不改 systemd |
| `--service-only` | 不上传/编译，只按 `deploy_app_dir` 安装并重启服务 |
| `--remote-dir` / `--app-dir` | 覆盖编译目录 / 运行目录 |

机内健康检查（SSH 上）：

```bash
curl http://127.0.0.1:8000/api/health
# 期望 {"status":"ok"}
```

**公网访问**：需在云厂商安全组放行 TCP `app_http_port`（默认 `8000`）。

### 远端 systemd 服务

部署脚本安装的服务名：**`lingxi-backend`**。

| 项 | 默认值 |
|---|---|
| 单元文件 | `/etc/systemd/system/lingxi-backend.service` |
| 二进制 | `/home/rock/apps/lingxi/lingxi-backend`（即 `deploy_app_dir`） |
| 工作目录 | `/home/rock/apps/lingxi` |
| 监听 | `0.0.0.0:8000`（`HOST` / `PORT`） |
| 数据库 | `deploy_database_url` 写入的 `DATABASE_URL` |

在服务器上管理（需 root / sudo）：

```bash
# 启动 / 停止 / 重启
sudo systemctl start lingxi-backend
sudo systemctl stop lingxi-backend
sudo systemctl restart lingxi-backend

# 状态
sudo systemctl status lingxi-backend

# 是否开机自启
sudo systemctl enable lingxi-backend
sudo systemctl disable lingxi-backend

# 实时日志
sudo journalctl -u lingxi-backend -f

# 最近日志
sudo journalctl -u lingxi-backend -n 100 --no-pager
```

改 unit 或环境变量后需：

```bash
sudo systemctl daemon-reload
sudo systemctl restart lingxi-backend
```

也可在本机只刷新服务配置：

```bash
python script/deploy_backend.py --service-only
```

**注意：** 不要同时手动 `cargo run` / 前台跑二进制与 systemd 抢同一端口（默认 `8000`）。调试时先 `sudo systemctl stop lingxi-backend`。

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

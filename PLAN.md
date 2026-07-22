# Lingxi 炒股条件推送应用 - 实现计划

## 概述

一个 Rust + Flutter 的炒股条件推送应用。Rust 后端 (Rocket + SeaORM + SQLite + WebSocket)
提供 REST API 管理条件和通知，并通过 WebSocket 长连接向 Flutter App 实时推送消息；
内置 Mock 数据引擎周期性触发条件并推送。Flutter App 负责服务器配置、新建/管理条件、
接收并展示推送消息。

## 架构

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

## 技术栈

### 后端 (Rust)
- `rocket` 0.5 + `rocket_ws` — Web 框架 + WebSocket
- `sea-orm` — ORM（SQLite 后端）
- `tokio` — 异步运行时
- `serde` / `serde_json` — 序列化
- `tracing` — 日志
- `rocket_cors` — CORS 中间件

### App (Flutter)
- `dio` — HTTP 客户端
- `web_socket_channel` — WebSocket
- `provider` — 状态管理
- `shared_preferences` — 服务器配置持久化

## 项目结构

```
Lingxi/
├── PLAN.md                       # 本文档
├── AGENTS.md                     # 构建/运行命令（阶段7生成）
├── backend/                      # Rust 后端
│   ├── Cargo.toml
│   ├── migration/                # SeaORM 数据库迁移
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       └── m20250101_000001_create_tables.rs
│   ├── entity/                   # SeaORM 实体
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── condition.rs
│   │       ├── notification.rs
│   │       └── mod.rs
│   └── src/
│       ├── main.rs
│       ├── config.rs
│       ├── db.rs
│       ├── error.rs
│       ├── routes/
│       │   ├── mod.rs
│       │   ├── condition.rs
│       │   ├── notification.rs
│       │   └── health.rs
│       ├── ws/
│       │   ├── mod.rs
│       │   └── hub.rs
│       └── engine/
│           ├── mod.rs
│           ├── mock.rs
│           └── matcher.rs
│
└── app/                          # Flutter App
    ├── pubspec.yaml
    └── lib/
        ├── main.dart
        ├── app.dart
        ├── config.dart
        ├── models/
        │   ├── condition.dart
        │   └── notification.dart
        ├── services/
        │   ├── config_store.dart
        │   ├── api_service.dart
        │   └── ws_service.dart
        ├── providers/
        │   ├── settings_provider.dart
        │   ├── condition_provider.dart
        │   └── notification_provider.dart
        ├── screens/
        │   ├── home_screen.dart
        │   ├── condition_list_screen.dart
        │   ├── condition_form_screen.dart
        │   ├── notification_list_screen.dart
        │   └── settings_screen.dart
        └── widgets/
```

## 数据模型 (SQLite)

```sql
CREATE TABLE condition (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT NOT NULL,
    kind        TEXT NOT NULL,                    -- 'trade' | 'news'
    symbol      TEXT,                             -- 股票代码（news 可空）
    expression  TEXT NOT NULL,                    -- JSON 配置
    enabled     BOOLEAN NOT NULL DEFAULT TRUE,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE TABLE notification (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    condition_id INTEGER NOT NULL,
    title        TEXT NOT NULL,
    body         TEXT NOT NULL,
    payload      TEXT,
    read         BOOLEAN NOT NULL DEFAULT FALSE,
    created_at   TEXT NOT NULL,
    FOREIGN KEY (condition_id) REFERENCES condition(id) ON DELETE CASCADE
);
```

## API 设计

### REST

| 方法 | 路径 | 说明 |
|---|---|---|
| GET  | `/api/health` | 健康检查（App 测试连接用） |
| POST | `/api/conditions` | 新建条件 |
| GET  | `/api/conditions` | 列出所有条件 |
| GET  | `/api/conditions/:id` | 获取单个条件 |
| PUT  | `/api/conditions/:id` | 更新条件 |
| DELETE | `/api/conditions/:id` | 删除条件 |
| GET  | `/api/notifications` | 列出通知（`?unread=true` 分页） |
| POST | `/api/notifications/:id/read` | 标记已读 |

### WebSocket

- 路径：`/ws`（与 HTTP 同端口）
- 服务端推送 JSON：
```json
{
  "type": "notification",
  "data": {
    "id": 123,
    "condition_id": 1,
    "title": "AAPL 触发买入信号",
    "body": "MA5 上穿 MA20",
    "payload": {},
    "created_at": "2026-07-21T10:30:00Z"
  }
}
```

## 后端关键模块

### WebSocket Hub (`ws/hub.rs`)
- `tokio::sync::broadcast` 通道维护所有连接
- 引擎触发时 `send`，所有订阅者收到

### Mock Engine (`engine/mock.rs`)
- 周期任务（默认 10 秒）
- 读取 `enabled` 条件，用假数据匹配
- 匹配成功 → 写 `notification` → 广播

### 条件匹配 (`engine/matcher.rs`)
- 第一版：`expression` 为 JSON，如 `{"type":"price_drop","threshold":0.05}`
- 简单随机/阈值判断，后续可扩展为脚本/DSL

## Flutter App 设计

### 服务器设置
- 字段：服务器地址、HTTP 端口、WebSocket 端口、是否 TLS、WebSocket 路径
- `shared_preferences` 持久化
- 首次启动引导填写，提供「测试连接」按钮（调 `/api/health`）
- 设置变更后触发 WebSocket 重连

### 页面
1. **首页**：底部 Tab 切换「条件」「通知」，AppBar 齿轮图标进设置
2. **条件列表**：显示所有条件，可启用/禁用、删除
3. **新建/编辑条件表单**：名称、类型、股票代码、表达式参数
4. **通知列表**：推送历史，未读标红，点击标记已读
5. **设置页**：服务器配置 + 测试连接

### 状态管理
- `Provider` + `ChangeNotifier`
- `SettingsProvider` / `ConditionProvider` / `NotificationProvider`

### WebSocket 服务
- App 启动后建立连接，断线自动重连
- 收到消息 → 加入 `NotificationProvider` → UI 刷新

## 实现阶段

### 阶段 1：后端骨架
1. 初始化 Cargo workspace + crate 结构
2. 配置 SeaORM + SQLite，写迁移建表
3. 生成实体
4. 搭建 Rocket，加 `/api/health`，跑通数据库连接

### 阶段 2：条件 CRUD
5. 实现 `/api/conditions` 全套 CRUD
6. 统一错误处理 + JSON 响应封装
7. curl 验证

### 阶段 3：WebSocket + Mock 引擎
8. 实现 WebSocket Hub（broadcast）
9. 实现 `/ws` 路由
10. 实现 Mock Engine 周期任务
11. wscat 验证推送

### 阶段 4：通知查询接口
12. 实现 `/api/notifications` 列表 + 标记已读

### 阶段 5：Flutter App 骨架 + 服务器设置
13. `flutter create app`，配置依赖
14. `ConfigStore` + `SettingsProvider` + `SettingsScreen`
15. `ApiService` / `WsService` 从配置动态构造 URL
16. 首次启动引导 + 测试连接
17. 条件列表 + 表单页

### 阶段 6：Flutter App 通知
18. 通知列表页
19. 接入 WebSocket 实时刷新
20. 未读标记 + UI 状态

### 阶段 7：联调 + 完善
21. 真机联调
22. 写 `AGENTS.md`
23. 可选：`flutter_local_notifications` 系统通知

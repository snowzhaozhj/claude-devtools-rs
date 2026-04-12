## Context

`cdt-api::ipc` 已定义 `DataApi` trait（20 个 async 方法）+ `ApiError` + `PushEvent`。`http` module 是空 stub。spec 定义 7 个 Requirement 覆盖路由、搜索、SSE、错误处理、端口绑定。

## Goals / Non-Goals

**Goals:**
- axum Router 骨架：路由组织、handler 函数签名
- `AppState`：持有 `Arc<dyn DataApi>` + `broadcast::Sender<PushEvent>`
- SSE 端点：`/api/events`
- `ApiError` → axum `IntoResponse` 转换
- Server 启动函数：bind + graceful shutdown
- 路由覆盖 spec 的所有端点分组

**Non-Goals:**
- `DataApi` trait 的具体实现（`LocalDataApi`）→ 由 `cdt-cli` 组装
- Updater 端点 → UI 层决策
- CORS / auth 中间件 → 后续增强

## Decisions

### D1: Module 结构

```
cdt-api/src/http/
├── mod.rs         # pub use + router builder
├── state.rs       # AppState
├── routes.rs      # 路由注册
├── handlers.rs    # 通用 handler 辅助
└── sse.rs         # SSE 事件端点
```

### D2: Handler 模式

所有 handler 委托给 `State<AppState>.api: Arc<dyn DataApi>`。handler 只负责：
1. 提取参数（Path / Query / Json）
2. 调用 `api.xxx()`
3. 转换 `Result<T, ApiError>` → `impl IntoResponse`

### D3: 错误响应

`ApiError` 实现 `IntoResponse`：
- `ValidationError` → 400
- `NotFound` → 404
- `Internal` → 500
- `SshError` → 502

### D4: SSE

`/api/events` 返回 `Sse<impl Stream<Item = Event>>`。用 `broadcast::Receiver<PushEvent>` 做多客户端分发。

## Risks / Trade-offs

- **[Trade-off] axum 版本锁定** → 选 axum 0.8（最新稳定），workspace 首次引入
- **[Trade-off] handler 暂不实现 body** → 路由骨架 + handler 签名到位，body 委托 `DataApi` trait（trait 的实现不在此 port）

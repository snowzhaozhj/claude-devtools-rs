## 1. 依赖 + 脚手架

- [x] 1.1 在 workspace `Cargo.toml` 添加 `axum` 和 `tower-http` 依赖；在 `cdt-api/Cargo.toml` 添加
- [x] 1.2 建立 `cdt-api/src/http/` module 结构：`mod.rs`、`state.rs`、`routes.rs`、`sse.rs`
- [x] 1.3 `cargo build -p cdt-api` 确认编译通过

## 2. AppState + 错误转换

- [x] 2.1 在 `state.rs` 定义 `AppState { api: Arc<dyn DataApi>, events_tx: broadcast::Sender<PushEvent> }`
- [x] 2.2 为 `ApiError` 实现 axum `IntoResponse`：`ValidationError`→400, `NotFound`→404, `Internal`→500, `SshError`→502
- [x] 2.3 单元测试：各错误码映射

## 3. 路由注册

- [x] 3.1 在 `routes.rs` 实现 `build_router(state: AppState) -> Router`：按 spec 分组注册路由
  - `/api/projects` GET
  - `/api/projects/:project_id/sessions` GET
  - `/api/sessions/:session_id` GET
  - `/api/search` POST
  - `/api/config` GET / PATCH
  - `/api/notifications` GET / POST
  - `/api/ssh/connect` POST / `/api/ssh/disconnect` POST / `/api/ssh/resolve-host` GET
  - `/api/contexts` GET / POST
  - `/api/validate/path` POST
  - `/api/claude-md` GET
  - `/api/events` GET (SSE)
- [x] 3.2 每个 handler 提取参数 → 委托 `state.api.xxx()` → 返回 JSON 或 error

## 4. SSE 事件端点

- [x] 4.1 在 `sse.rs` 实现 `/api/events` handler：`broadcast::Receiver<PushEvent>` → `Sse<impl Stream>`
- [x] 4.2 单元测试：`PushEvent` 序列化为 SSE data

## 5. Server 启动

- [x] 5.1 在 `mod.rs` 实现 `start_server(state, port) -> Result<()>`：bind + serve + graceful shutdown
- [x] 5.2 端口冲突时返回明确错误（spec: "SHALL NOT switch ports silently"）

## 6. lib.rs 导出 + 集成

- [x] 6.1 在 `lib.rs` 导出 `http` module 的公开 API
- [x] 6.2 `cargo clippy --workspace --all-targets -- -D warnings`
- [x] 6.3 `cargo fmt --all`
- [x] 6.4 `cargo test -p cdt-api`

## 7. 文档 + 收尾

- [x] 7.1 更新根 `CLAUDE.md` 的 Capability→crate map：`http-data-api` → `done ✓`
- [x] 7.2 `openspec validate port-http-data-api --strict`

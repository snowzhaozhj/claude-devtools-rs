## Why

`http-data-api` 是最后一个 capability（13/13）。它在 `/api` 前缀下暴露 HTTP 端点，镜像 `ipc-data-api` 的 `DataApi` trait surface。TS 侧有 12 个路由文件 + SSE 事件端点。

Rust 选 `axum` 作 HTTP 框架（workspace 尚未引入，本 port 添加）。路由 handler 委托给 `DataApi` trait，实现与传输解耦。

## What Changes

- 在 `cdt-api::http` module 实现 axum router：
  - 路由注册：projects / sessions / search / config / notifications / ssh / validation / utility
  - SSE 事件端点：`/api/events`（broadcast channel → SSE stream）
  - 错误处理中间件：`ApiError` → HTTP 状态码 + JSON body
  - Server 启动：bind 到配置端口 + graceful shutdown
- 新增 workspace 依赖：`axum`、`tower`、`tower-http`

## Capabilities

### New Capabilities
（无）

### Modified Capabilities
- `http-data-api`：Rust port 增强错误处理——lookup failure 返回 404（而非 TS 的 200 + null），tracked as spec delta

## Impact

- **代码**：`crates/cdt-api/src/http/` 从空 module 扩展为 axum server
- **依赖**：新增 `axum`、`tower-http`（cors）
- **下游**：`cdt-cli` 可启动 HTTP server

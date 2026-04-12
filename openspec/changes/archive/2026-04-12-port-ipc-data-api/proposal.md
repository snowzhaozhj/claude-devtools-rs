## Why

`ipc-data-api` 是第 12 个 capability（倒数第 2）。它是消费层——所有底层 capability（parse/analyze/discover/config/ssh）的数据通过这个 API facade 暴露给 UI。TS 侧有 13 个 IPC handler 文件 + preload bridge，~3600 行。

Rust 端选择 **trait-based facade**（不绑 Electron IPC），UI 技术栈未定前只定义 trait surface，后续 `port-http-data-api` 用 axum 实现 HTTP 绑定。

## What Changes

- 在 `cdt-api::ipc` module 定义 `DataApi` trait：覆盖 spec 的 8 个 Requirement
- 定义请求/响应类型（query params + result structs）
- 定义 `ApiError` 结构化错误（error code + message）
- 定义 push 事件枚举（`PushEvent`：file change / notification / ssh status）
- 实现 `LocalDataApi`：组装底层 crate 调用，作为默认实现

## Capabilities

### New Capabilities
（无）

### Modified Capabilities
（无——按原 spec 实现）

## Impact

- **代码**：`crates/cdt-api/src/ipc/` 从空 module 扩展为 trait + types + impl
- **依赖**：新增 `async-trait`；已有所有底层 crate 依赖
- **下游**：`port-http-data-api` 将 wrap 这个 trait 为 axum routes

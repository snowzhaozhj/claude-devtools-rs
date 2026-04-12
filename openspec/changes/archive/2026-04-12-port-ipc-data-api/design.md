## Context

`cdt-api` 依赖全部底层 crate。当前是 stub。spec 定义 8 个 Requirement，对应 TS 侧 13 个 handler 文件。

Rust 不用 Electron IPC，选 trait facade + 默认 struct 实现。HTTP 绑定留给 `port-http-data-api`。

## Goals / Non-Goals

**Goals:**
- `DataApi` async trait：项目/会话查询、搜索、配置、通知、SSH、context、文件验证、辅助读取
- 请求/响应类型 + `ApiError` 结构化错误
- `PushEvent` 枚举
- `LocalDataApi` 默认实现（组装底层 crate 调用）
- 单元测试覆盖 spec scenario

**Non-Goals:**
- HTTP server / axum routes → `port-http-data-api`
- Updater progress 事件 → UI 层决策
- SSE 推送 → HTTP 层

## Decisions

### D1: Module 结构

```
cdt-api/src/ipc/
├── mod.rs         # pub use
├── types.rs       # 请求/响应类型
├── error.rs       # ApiError
├── events.rs      # PushEvent 枚举
├── traits.rs      # DataApi trait 定义
└── local.rs       # LocalDataApi 实现
```

### D2: Trait 方法分组

按 spec 的 8 个 Requirement 分组，每组 2-5 个方法。总约 20 个方法。

### D3: `LocalDataApi` 依赖注入

`LocalDataApi` 持有各底层 manager 的引用/`Arc`：
- `ProjectScanner`（`cdt-discover`）
- `ConfigManager`（`cdt-config`）
- `NotificationManager`（`cdt-config`）
- `SshConnectionManager`（`cdt-ssh`）

测试时可 mock 或用 real 实例 + tempdir。

### D4: 分页

`PaginatedRequest { page_size, cursor }` + `PaginatedResponse<T> { items, next_cursor, total }`。cursor 为 opaque string（encode offset）。

## Risks / Trade-offs

- **[Trade-off] Trait 方法较多（~20）** → 但保持了与 spec 的 1:1 映射
- **[Risk] `LocalDataApi` 需要真实文件系统做集成测试** → 单元测试用 mock 数据，集成测试留给 CLI

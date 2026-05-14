## Why

当前 Rust 端口的会话列表虽然返回分页响应，但后端仍依赖全局/全量发现与排序后再切片；随着历史会话数增长，列表首屏加载会持续变慢。原版 `claude-devtools` 的实际优化路径是按项目 cursor 分页、首屏轻量加载、按需补齐 pinned sessions，本 change 先对齐该保守方案，避免引入持久索引或新的流式近似语义。

## What Changes

- 修改 `list_sessions` 的行为契约：默认作为 project-scoped cursor pagination 入口，面向当前项目/当前 worktree 分页返回会话列表。
- 列表首屏默认请求 `pageSize = 20`，响应保留 `nextCursor` / `hasMore` 语义，不要求同步返回精确 total。
- 列表路径采用 light metadata：同步响应只包含可由文件名/mtime 等轻量信息得到的字段，深度 metadata 通过后台扫描当前页或可见窗口补齐。
- 修改 Sidebar/Command Palette 的完整历史加载契约：默认 Sidebar 不再为了列表首屏同步拉取完整会话历史；需要完整历史的搜索入口 SHALL 明确逐页加载或使用专用搜索能力。
- Pinned/hidden sessions SHALL 通过按 `sessionId` 补拉/合并机制保障可见性，不依赖第一页刚好包含这些 session。
- Dashboard 项目卡片 SHALL NOT 为了展示项目概览触发所有项目的会话列表加载。
- 不引入持久索引、SQLite、跨启动缓存，也不在本 change 中设计新的流式近似首屏协议。

## Capabilities

### New Capabilities

- 无。

### Modified Capabilities

- `ipc-data-api`: 修改 `list_sessions` 分页语义、total 语义、light metadata 与后台扫描范围要求，并补充按 session ids 补拉列表项的契约。
- `sidebar-navigation`: 修改 Sidebar 会话列表加载策略，从默认完整历史加载改为对齐原版的 project-scoped cursor pagination；补充 pinned/hidden 补齐与 Dashboard 不触发全项目 sessions 加载的 UI 契约。

## Impact

- 后端：`crates/cdt-api` 的 `LocalDataApi::list_sessions` / `list_sessions_sync`、分页 response 类型、metadata 扫描调度；必要时扩展 `DataApi` trait 以支持按 session ids 补拉。
- 发现层：`crates/cdt-discover::ProjectScanner` 的 project-scoped paginated listing 语义与测试。
- 前端：`ui/src/lib/api.ts`、Sidebar store/component、Command Palette 搜索入口、Dashboard 项目概览加载路径、mock IPC fixtures。
- 测试：IPC contract、session metadata stream、project scanner pagination、Sidebar/Command Palette user story。
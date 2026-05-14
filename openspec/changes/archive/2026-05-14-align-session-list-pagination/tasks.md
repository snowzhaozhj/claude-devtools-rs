## 1. cdt-api / cdt-discover 后端分页契约

- [x] 1.1 调整 `PaginatedResponse` / `list_sessions` 相关 contract test，明确 `nextCursor` 驱动分页且 UI 不依赖精确 `total`
- [x] 1.2 实现或收窄 `ProjectScanner` 的 project-scoped paginated listing，确保 `pageSize = 0` 返回 validation error
- [x] 1.3 调整 `LocalDataApi::list_sessions`，同步返回 light `SessionSummary`，后台 metadata 扫描只覆盖本次响应页
- [x] 1.4 新增按 `sessionId` 批量获取 light `SessionSummary` 的 API/IPC/HTTP 能力，用于 pinned/hidden 补齐
- [x] 1.5 补充 `session_metadata_stream` / `ipc_contract` / `project_scanner` 测试，覆盖当前页 metadata 推送、cursor 翻页、by-id 补拉与 pageSize=0 拒绝

## 2. ui Sidebar / Dashboard / Command Palette

- [x] 2.1 调整前端 API 类型与 mock IPC，支持 by-id summary fetch 与新的分页依赖方式
- [x] 2.2 修改 Sidebar 会话加载：首屏 `pageSize = 20`，滚动使用 `nextCursor` 加载更多，按 `sessionId` 去重合并
- [x] 2.3 实现 pinned/hidden ids 的按需补拉与合并，保持既有 Pin/Hide 视觉和过滤行为
- [x] 2.4 确保 Dashboard 项目概览不触发所有项目 `list_sessions` 加载
- [x] 2.5 调整 Command Palette 全历史搜索路径，避免依赖 Sidebar 首屏完整 sessions 数组
- [x] 2.6 补充 Vitest/Playwright 覆盖首屏 20 条、滚动翻页、pinned 旧会话补齐、Dashboard 不预取 sessions

## 3. 验证与收尾

- [x] 3.1 运行 `cargo fmt --all`
- [x] 3.2 运行 `cargo clippy --workspace --all-targets -- -D warnings`
- [x] 3.3 运行相关 Rust 测试：`cargo test -p cdt-api` 与 `cargo test -p cdt-discover`
- [x] 3.4 运行前端检查与测试：`npm run check --prefix ui`，以及相关 UI 单测/e2e
- [x] 3.5 运行 `openspec validate align-session-list-pagination --strict`
- [x] 3.6 完成后让 codex 做实现二审，并在 archive 前确认 spec scenarios 均有测试覆盖
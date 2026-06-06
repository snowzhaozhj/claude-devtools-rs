## Why

AI 助手（session-insights skill / MCP）消费 CLI 输出时，99% 的 context tokens 浪费在噪音上。实测：6 个错误 → `--filter errors_only` 返回 135KB JSON（有用信息 < 1KB，信噪比 0.4%）；65 chunks 的结构概览 → `--content omit` 返回 241KB。根因是 `cdt-query` 层缺少 chunk 与聚合之间的「item 级展平查询」——只能拿整个 chunk 或 session 级统计，没有"每条工具调用一行"或"每个 chunk 一行概览"的中间粒度。

## What Changes

- **`cdt-query` 新增 `extract` 模块**：提供 `extract_overview` / `extract_tool_executions` / `extract_errors` 三个纯函数，将 `&[Chunk]` 展平为扁平条目序列（`ChunkOverviewEntry` / `ToolExecEntry`）
- **统一 error message 提取**：新增 `extract_error_summary(te) -> Option<String>`，按优先级从 `errorMessage` → exit code → stderr 截断提取有意义的错误信息，修复 `sessions errors` 的 `(no message)` 问题
- **CLI 新增 `--extract errors|tools|overview`**：在 `sessions detail` 的现有管道（filter → grep → range/tail）末端增加展平输出分支，默认 text 格式、支持 `--format json` 输出扁平 JSON array
- **废弃 `ErrorEntry`**：`cdt-query::engine::ErrorEntry` 被 `ToolExecEntry` 替代，`get_session_errors()` 标 `#[deprecated]` 内部委托 `extract_errors()`
- **更新 session-insights skill**：用原生 `--extract` 命令替代 python pipe patterns

## Capabilities

### New Capabilities

（无新 capability——extract 是 cli-output 现有 capability 的扩展）

### Modified Capabilities

- `cli-output`：新增 `--extract` flag 的行为契约（三种模式、与现有 filter/window 组合、text/JSON 输出格式）

## Impact

- `crates/cdt-query/src/extract.rs`：新增模块（3 struct + 4 函数）
- `crates/cdt-query/src/engine.rs`：`ErrorEntry` 标废弃、`get_session_errors()` 委托
- `crates/cdt-query/src/lib.rs`：导出 extract 模块
- `crates/cdt-cli/src/main.rs`：新增 `--extract` 参数 + 分发逻辑
- `crates/cdt-cli/src/view.rs`：新增 extract 结果的 text/JSON 格式化
- `crates/cdt-cli/src/mcp/mod.rs`：MCP `get_session_errors` 迁移到 extract（可延后）
- `crates/cdt-cli/assets/skills/session-insights/SKILL.md`：更新 patterns
- 不涉及 `cdt-core` / `cdt-analyze` / `cdt-api` / 前端

## Why

用户问"昨天做了什么"时，AI 必须对每个会话做 `--content full --range M:N` 碎片化读取才能理解会话内容（实测：25 次 CLI 调用、$5、37 分钟、3 次用户纠正）。根因是 `SessionSummary` 只有 `title`（首条用户消息），缺乏会话活动的结构化摘要，AI 无法在 list 阶段就掌握每个会话"做了什么"。

## What Changes

- `SessionMetadata` 扫描时新增 7 个字段提取：`user_intents`（用户消息首行序列）、`last_active`、`duration_ms`、`total_cost`、`tool_error_count`、`files_touched`、`git_summary`
- `SessionSummary` struct 同步新增对应字段，CLI `sessions list` / MCP `list_sessions` / SSE `SessionMetadataUpdate` 三路自动暴露
- `MetadataCacheEntry` 扩展以缓存新字段
- CLI `--json` 可用字段列表更新
- SKILL `session-insights` 新增日报场景路径

## Capabilities

### New Capabilities

（无）

### Modified Capabilities

- `ipc-data-api`：`SessionSummary` struct 新增可选字段，影响 `list_sessions` / `list_sessions_cross_project` 返回值
- `mcp-server`：`list_sessions` tool 返回值自动包含新字段（serde 透传）
- `cli-output`：`sessions list --json` 可用字段列表扩展

## Impact

- `crates/cdt-api/src/ipc/session_metadata.rs`：扫描循环扩展 + MetadataCacheEntry 扩展
- `crates/cdt-api/src/ipc/types.rs`：`SessionSummary` struct
- `crates/cdt-api/src/ipc/events.rs`：`SessionMetadataUpdate` event
- `crates/cdt-api/src/ipc/local.rs`：metadata → summary 映射
- `crates/cdt-cli/src/main.rs`：`list_available_fields()`
- `crates/cdt-cli/assets/skills/session-insights/SKILL.md`：日报场景
- MCP 层无需代码改动（serde 自动透传）

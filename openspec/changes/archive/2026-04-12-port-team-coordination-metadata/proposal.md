## Why

`team-coordination-metadata` 是第 10 个 capability。`cdt-analyze` 已为它预留了空 `team` module、`cdt-core` 定义了 `TeamMeta`/`Process.team`/`SubagentCandidate`。本 port 实现 teammate 消息检测、team 工具摘要、`Process.team` 富化，并完成前序 port 留下的两个尾巴：`filter_resolved_tasks` 端到端接入 `build_chunks`、context-tracking 的 `teammate_message` display item。

## What Changes

- 在 `cdt-analyze::team` module 实现：
  - `is_teammate_message`：正则检测 `<teammate-message teammate_id="...">` 标签
  - `parse_teammate_attrs`：提取 `teammate_id`、`color`、`summary`
  - `format_team_tool_summary`：7 种 team 工具的专用摘要格式
  - `extract_team_meta_from_task`：从 Task call input 提取 `TeamMeta`
- 修改 `build_chunks`：调用 `filter_resolved_tasks` 过滤已 resolve 的 Task（接尾 port-tool-execution-linking 的 TODO）
- 修改 `build_chunks`：用 `is_teammate_message` guard 排除 teammate 消息产生 `UserChunk`
- 在 context aggregator 补齐 `teammate_message` display item 路径

## Capabilities

### New Capabilities

（无）

### Modified Capabilities

- `team-coordination-metadata`：无 spec 行为变更，仅实现

## Impact

- **代码**：`crates/cdt-analyze/src/team/` 从空 module 扩展为完整实现；`chunk/builder.rs` 增加 teammate guard 和 filter 接入
- **依赖**：`regex`（teammate tag 检测）——`cdt-analyze` 当前无 regex 依赖，需添加
- **无新 crate 依赖**：`TeamMeta`/`Process`/`SubagentCandidate` 已在 `cdt-core`

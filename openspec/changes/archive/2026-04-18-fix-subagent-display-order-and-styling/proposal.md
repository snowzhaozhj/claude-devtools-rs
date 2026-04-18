## Why

Rust 版 SessionDetail 中 subagent 卡片被**集中堆到 AIChunk 末尾**，而原版是按 Task `tool_use` 时序**就地交错**在 tool call 序列里展示。同时 badge 用 `EXPLORE` 这种 `subagent_type` 大写值（应固定为 `TASK`）、模型名显示 `haiku-4-5`（应对齐原版 `parseModelString` 的 `haiku4.5`），Task tool call 与 subagent 卡片同时显示造成重复。根因在 `cdt-analyze::attach_subagents_to_chunks` 把 `SubagentSpawn` step `push` 到 `semantic_steps` 末尾，而没有插入到匹配 Task `ToolExecution` step 之后。

## What Changes

- 后端 `cdt-analyze::chunk::builder::attach_subagents_to_chunks`：`SubagentSpawn` step 必须 insert 到该 subagent 对应 Task `ToolExecution` step 之后，而不是 append 到末尾
- 前端 `ui/src/lib/displayItemBuilder.ts`：新增 `taskIdsWithSubagents` 过滤，`tool_execution` step 若为 Task 且有对应 subagent 则跳过（对齐原版 TS 行为）
- 前端 `ui/src/components/SubagentCard.svelte`：
  - badge label 固定为 `TASK`（非团队 / 非特定 subagent_type 时），展开 Meta 行 Type 字段仍显示原 `subagent_type` 值
  - 模型名显示沿用 `parseModelString` 对齐原版（`haiku-4-5-20251001` → `haiku4.5`），保留 family 色
  - Execution Trace header 加 Terminal 图标
- `ToolExecution` 须向前端透出 `tool_name`（若已有则复用）以便 displayItemBuilder 识别 Task

## Capabilities

### New Capabilities
（无）

### Modified Capabilities
- `chunk-building`: `SubagentSpawn` semantic step 的插入位置从 "追加到 `semantic_steps` 末尾" 调整为 "插入到匹配 Task `ToolExecution` step 之后"，保证下游 UI 能按时序就地渲染

## Impact

- 后端：`crates/cdt-analyze/src/chunk/builder.rs`（`attach_subagents_to_chunks`），新增 1-2 个单测覆盖 "SubagentSpawn 紧随 Task ToolExecution"
- 前端：`ui/src/lib/displayItemBuilder.ts`（Task 跳过逻辑）、`ui/src/components/SubagentCard.svelte`（样式 + 模型名 + 徽章）
- 原版参考：`../claude-devtools/src/renderer/utils/displayItemBuilder.ts`（行 104-173）、`SubagentItem.tsx`（行 74-76、303-326）
- 无破坏性 API 变更；`DataApi` trait 不改；`ToolExecution.tool_name` 已存在（`cdt-core/chunk.rs`）
- 属于 `openspec/followups.md` 的 UI 对齐修复，未单列 impl-bug 条目

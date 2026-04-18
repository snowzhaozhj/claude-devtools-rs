## Context

SessionDetail 对 AIChunk 的渲染管线为：后端 `build_chunks_with_subagents` 产出 `AIChunk`（含 `semantic_steps`、`tool_executions`、`subagents`），前端 `displayItemBuilder.ts::buildDisplayItems` 把三者按 `semantic_steps` 顺序 flatten 成 `DisplayItem[]`，在 `SessionDetail.svelte` 遍历渲染；其中 `SubagentCard.svelte` 负责 subagent 卡片展示。

当前实现缺陷：
1. `cdt-analyze::attach_subagents_to_chunks` 在 `chunk-building` 主循环产出 `semantic_steps` **之后**才 append `SubagentSpawn`，所以 subagent_spawn step 总在末尾 → 前端按序遍历时集中渲染到末尾。
2. 原版 TS 为避免 "Task tool call + subagent 卡片" 重复展示，`displayItemBuilder.buildDisplayItems` 用 `taskIdsWithSubagents` 过滤掉带 subagent 的 Task tool；Rust 版 TS 未实现。
3. `SubagentCard` badge 直接把 `subagent_type` 当标签（大写样式显示为 `EXPLORE` 等）；原版固定 `TASK` 标签，把 `subagent_type` 放进展开后的 Meta 行。
4. 模型名直接 `replace("claude-", "").replace(/-\d{8}$/, "")` → `haiku-4-5`；原版 `parseModelString` 会折叠成 `haiku4.5` 并附 family 色。

## Goals / Non-Goals

**Goals**
- 后端 `semantic_steps` 中 `SubagentSpawn` MUST 插入在与之对应的 Task `ToolExecution` step 之后（紧邻）。
- 前端 `displayItemBuilder` 跳过带 subagent 的 Task tool_execution。
- `SubagentCard` 顶层 badge 固定 `TASK`（team 成员仍显示成员名），`subagent_type` 移到展开 Meta 行 Type 字段。
- 模型名显示对齐原版 `parseModelString`（`haiku-4-5-*` → `haiku4.5`），并保留 family 色。
- Execution Trace header 加 Terminal 图标（对齐原版视觉）。

**Non-Goals**
- 不改 `SubagentCard` 的展开三层布局（Meta / Context Usage / Execution Trace）。
- 不动 `tool-execution-linking`、`team-coordination-metadata` 的数据层解析逻辑；只改 `chunk-building` 插入位置。
- 不扩 IPC 协议/类型；`ToolExecution.tool_name` 已经透传。
- 不引入 `parseModelString` 的完整 family 色谱；只补足 Rust 版缺失的"压缩连字符 + 去日期 suffix"行为。

## Decisions

### Decision 1：`SubagentSpawn` 插入位置

在 `attach_subagents_to_chunks` 中，不 `push` 到 `semantic_steps` 末尾，而是找到 `tool_use_id == rt.task_use_id` 的 `SemanticStep::ToolExecution` 位置，调用 `Vec::insert(idx + 1, SubagentSpawn{..})`。

**替代方案**：在前端 `displayItemBuilder` 手动重排。被否——后端是语义真相源，UI 不该再做时序修复；且前端做需要同样的 `taskUseId → subagentSessionId` 匹配信息，会重复后端已有的 `ResolvedTask` 数据。

### Decision 2：多 subagent 同一 Task 的顺序

若多个 subagent 都对应同一个 Task tool（理论上不会发生，`resolve_subagents` 1:1 匹配），按 `resolved` 列表顺序连续插入在 Task step 之后；未找到匹配 Task step 时退化为 append 到末尾（保持旧行为，避免抛弃）。

### Decision 3：前端跳过 Task 的条件

`displayItemBuilder.ts` 构建 `taskIdsWithSubagents = new Set(chunk.subagents.map(s => s.parentTaskId).filter(Boolean))`；遍历 `semantic_steps` 碰到 `tool_execution` 且 `exec.toolName === "Task"` 且 `taskIdsWithSubagents.has(exec.toolUseId)` 时跳过。

**备注**：`SubagentProcess.parentTaskId` 在 `cdt-core/src/process.rs` 已有且通过 IPC 透传到 `ui/src/lib/api.ts`。

### Decision 4：badge 策略

- 有 team → 显示 `process.team.memberName`（已有）。
- 其他情况 → 统一显示 `TASK`；`subagent_type` 不再作为顶层 badge。
- 展开 Meta 行 Type 字段 SHALL 显示 `process.subagentType ?? (team ? "Team" : "Task")`（已有）——无需改动。
- 颜色：`getSubagentTypeColorSet(subagentType, configs)` 逻辑保留用于圆点 / 徽章色，但文字固定 `TASK`。

### Decision 5：模型名压缩

新增 `ui/src/lib/modelParser.ts`（若不存在则建）导出 `parseModelString(raw): { family, name }`：
- 去 `claude-` 前缀、去 `-YYYYMMDD` 日期 suffix
- 去 `-` 分隔的 family/version，用 `.` 连接版本号：`haiku-4-5` → `haiku4.5`、`opus-4-7` → `opus4.7`
- 返回 `{ family: "haiku" | "sonnet" | "opus" | "unknown", name }`

`SubagentCard` 从 `parseModelString` 取 `name` 显示，`family` 可供后续配色扩展（当前不强制用）。

## Risks / Trade-offs

- **未匹配 Task step 的退化路径**：若 `ResolvedTask.task_use_id` 查不到对应 `ToolExecution`（不可能但保兜底），`SubagentSpawn` 仍 push 到末尾——日志一条 `tracing::warn!` 用于排查。
- **跨 chunk 的 subagent**：当前 `attach_subagents_to_chunks` 假定 subagent 属于同一个 `AIChunk`，新实现不改变这一前提。
- **测试覆盖**：`cdt-analyze` 需新增 `subagent_spawn_inserted_after_task_step` 单测；前端 `displayItemBuilder` 可补最小单测（pure function，无 DOM）。

## Migration Plan

无数据迁移。UI 侧顺序变化即时生效；不影响现有 session 文件。

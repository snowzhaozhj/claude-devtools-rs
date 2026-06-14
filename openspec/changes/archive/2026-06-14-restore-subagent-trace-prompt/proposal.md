## Why

SubAgent 的 ExecutionTrace（展开 subagent 卡片后显示的执行轨迹）丢失了**第一条 input**——父会话给该 subagent 的 prompt。这条 prompt 是理解 subagent 在干什么的关键上下文，用户展开 trace 却只看到 thinking / tool / output，看不到"它被要求做什么"。根因是前端 `buildDisplayItemsFromChunks` 一刀切跳过所有非 AI chunk，把承载 prompt 的 `UserChunk` 整个丢弃。原版 TS 对此有专门的 `subagent_input` DisplayItem，port 时丢了这个类型。

## What Changes

- `buildDisplayItemsFromChunks`（`ui/src/lib/displayItemBuilder.ts`）：不再无差别跳过非 AI chunk。`UserChunk` 提取文本、清洗后非空时产出一个 `user_message` DisplayItem（复用已有类型）；slash UserChunk（`extractSlashInfo` 命中）跳过——避免与下个 AIChunk 的 slash item 重复渲染；`SystemChunk` / `CompactChunk` 仍跳过。
- `ExecutionTrace.svelte`：新增 `user_message` item 渲染分支（BaseItem + User 图标 + prose body），与主视图 `SessionDetail` 的 user_message 渲染样式一致。
- 影响的 capability 行为：subagent ExecutionTrace 展开时显示的 DisplayItem 流类型枚举，从 `thinking / tool / output / 嵌套 subagent` 扩展为额外包含父会话给 subagent 的 prompt（user_message）。

## Capabilities

### New Capabilities
<!-- 无新增 capability -->

### Modified Capabilities
- `session-display`: "Subagent 内联展开 ExecutionTrace" Requirement 下的 "Execution Trace 内独立展开" Scenario——展开时显示的 DisplayItem 流 SHALL 额外包含父会话给 subagent 的输入（user prompt）。

## Impact

- 改动文件：`ui/src/lib/displayItemBuilder.ts`（`buildDisplayItemsFromChunks` 逻辑 + 一个文本提取 helper）、`ui/src/components/ExecutionTrace.svelte`（渲染分支）。
- `buildDisplayItemsFromChunks` 调用方仅 `SubagentCard.svelte` 与 `WorkflowCard.svelte`，两者都经 `ExecutionTrace.svelte` 渲染——主会话视图（`SessionDetail` 直接遍历 `detail.chunks` 渲染 UserChunk 气泡）不经此函数，零影响。
- 无后端 / IPC / serde 改动；无新依赖。
- 测试：新增 `buildDisplayItemsFromChunks` 的 vitest 单测（含 UserChunk → user_message、slash UserChunk 跳过、tool_result-only 不产 UserChunk 三个 case）。

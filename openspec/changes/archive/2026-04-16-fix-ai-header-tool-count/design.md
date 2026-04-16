## Context

原版 TS 的 AI chunk 渲染走 `enhanceAIGroup()` → `buildDisplayItems()` → `buildSummary()` → `DisplayItemList` 管道。Rust 版缺少这个中间层，直接用 raw chunk fields 渲染，导致计数和展示与原版不一致。

后端数据已经正确：`get_session_detail` 调用 `build_chunks_with_subagents`，已执行 `filter_resolved_tasks`。问题纯在前端展示层。

## Goals / Non-Goals

**Goals:**
- 前端补充 `displayItemBuilder` 中间层，对齐原版的展示项构建逻辑
- Header summary 由 `displayItems` 统计生成，消除计数偏差
- 展开列表渲染全部 display item 类型（thinking / tool / output / subagent / slash）
- Subagent 用独立卡片组件渲染

**Non-Goals:**
- 不改后端 chunk 构建逻辑（已正确）
- 不实现原版的 `lastOutputDetector`（Rust 版当前把最后一段 text 放在 `ai-body` 始终可见，这个行为可以保留）
- 不实现 `toolLinkingEngine`（原版在前端再做一次 tool linking；Rust 版后端已完成 `pair_tool_executions`，前端直接用 `toolExecutions` 字段）
- 不实现 teammate_message / compact_boundary / subagent_input 类型（当前无对应数据源）

## Decisions

### 1. DisplayItem 从后端 chunk 数据直接构建，不重复 tool linking

原版前端在 `buildDisplayItems` 中调用 `linkToolCallsToResults` 做二次关联。Rust 版后端已经在 `pair_tool_executions` 中完成了 tool linking，结果存在 `AIChunk.toolExecutions` 中。前端直接用这个字段，不再二次关联。

`buildDisplayItems` 的数据源：
- `chunk.semanticSteps`：遍历顺序就是时间线顺序
- `chunk.toolExecutions`：按 `toolUseId` 查找对应的完整 execution 信息
- `chunk.subagents`：按 `sessionId` 查找 subagent Process 信息
- `chunk.slashCommands`：slash 命令列表

### 2. output（text 消息）在展开列表和 ai-body 中的处理

原版把 `text` 和 `thinking` 都放在展开列表（`DisplayItemList`）中；最后一段 text 由 `LastOutputDisplay` 在展开列表外始终显示。

Rust 版当前把 `text` 和 `thinking` 放在 `ai-body`（始终可见），展开列表只有 tool 和 subagent。

**决策**：对齐原版——`text` 和 `thinking` 移入展开列表。`ai-body` 区域改为只放最后一段 text（始终可见的 last output）。这样展开后能看到完整的消息序列。

### 3. SubagentCard 组件范围

原版 `SubagentItem.tsx` 支持展开显示嵌套的 subagent 消息列表和执行 trace。当前 Rust 版后端 `Process` 类型没有 `messages` 字段（只有 `sessionId`、`rootTaskDescription`、`spawnTs`、`endTs`、`metrics`、`team`）。

**决策**：SubagentCard 第一版只展示元信息（任务描述、执行时长、metrics）+ 点击可导航到 subagent session 的 tab。不展示嵌套消息列表（留后续 change）。

## Risks / Trade-offs

- **[Risk] semanticSteps 顺序依赖** → `extract_semantic_steps` 按 response 顺序 + content block 顺序产出 steps，本身就是时间线顺序。只要不重排就能对齐原版。
- **[Risk] 最后一段 text 的检测** → 简单取 `semanticSteps` 中最后一个 `kind === "text"` 的 step 作为 last output，在展开列表中跳过它（对齐原版 `findLastOutput` 的逻辑）。
- **[Trade-off] 不实现 teammate_message** → 当前后端没有 teammate message 的提取管道。等 team-coordination-metadata 的 UI 层 port 时再补。

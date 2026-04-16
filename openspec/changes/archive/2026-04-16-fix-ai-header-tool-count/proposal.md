## Why

AI chunk 的渲染层缺少与原版对齐的 `displayItemBuilder` 中间层。原版通过 `enhanceAIGroup()` → `buildDisplayItems()` → `buildSummary()` 管道将 raw AIGroup 数据转换为统一的 `AIGroupDisplayItem[]` 列表（含 thinking / tool / output / subagent / slash 等 7 种类型），再由 `DisplayItemList` 组件统一渲染。

Rust 版直接从 raw chunk fields 计数和渲染，导致三个用户可见问题：
1. **Header 计数偏少**：`buildAiGroupSummary` 从 `toolExecutions.length` / `semanticSteps` 计数，缺少原版 `displayItems` 的关联和去重逻辑
2. **展开后看不到 message**：原版的 `output` type display item 在展开列表中渲染为 `TextItem`；Rust 版把 `text` step 放在始终可见的 `ai-body` 区域，展开列表中只有 tool 和 subagent
3. **Subagent 渲染为 tool call 样式**：用 `BaseItem`（工具卡片样式），缺少原版 `SubagentItem` 的独立卡片（含任务描述、嵌套消息列表、执行时长等）

## What Changes

- 前端新增 `displayItemBuilder.ts`——从 `AIChunk` 的 `semanticSteps` + `toolExecutions` + `subagents` + `responses` 构建统一的 `DisplayItem[]`
- 用 `buildSummary(displayItems)` 替换 `buildAiGroupSummary(chunk)` 生成 header 文本
- 重构 `SessionDetail.svelte` 的 AI chunk 渲染：用 `DisplayItem[]` 驱动展开列表，包含 thinking / tool / output / subagent / slash 全部类型
- 新增 `SubagentCard.svelte` 组件——独立的 subagent 卡片样式（对齐原版 `SubagentItem.tsx`）

## Capabilities

### New Capabilities

- `ai-chunk-display`：AI chunk 的展示项构建、summary 生成和渲染逻辑

### Modified Capabilities

无。这是纯前端展示层改动，不影响后端 spec。

## Impact

- `ui/src/lib/displayItemBuilder.ts`（新建）：`buildDisplayItems()` + `buildSummary()`
- `ui/src/lib/toolHelpers.ts`：移除 `buildAiGroupSummary()`
- `ui/src/routes/SessionDetail.svelte`：AI chunk 展开内容改为 `DisplayItem[]` 驱动
- `ui/src/lib/components/SubagentCard.svelte`（新建）：subagent 独立卡片组件
- `openspec/followups.md`：标记 chunk-building impl-bug "Task tool 过滤未在 AIChunk 构建阶段生效" 已在 UI 层修复

## Context

对比原版截图，Rust 版 UI 在以下方面与原版有差距，需逐项对齐。

原版关键实现参考：
- `AIChatGroup.tsx`：AI header 统计由 `buildSummary(displayItems)` 生成
- `displaySummary.ts`：统计 thinking/tools/messages/subagents/slashes 五类
- `LinkedToolItem.tsx`：工具统一用 lucide `Wrench` 图标
- `ThinkingItem.tsx`/`TextItem.tsx`/`SubagentItem.tsx`：各用 `Brain`/`MessageSquare`/`Bot`
- `SubagentItem.tsx`：独立卡片，显示 type badge + 描述 + model + 状态 + 时长
- `SystemChatGroup.tsx`：Terminal 图标 + 左对齐 + pre 格式

## Goals / Non-Goals

**Goals:**
- AI header 统计信息对齐原版格式
- 语义步骤图标对齐原版 lucide 图标
- Subagent 展示从空占位改为有意义的卡片
- System 消息样式对齐原版

**Non-Goals:**
- 不引入完整 lucide 图标库（用内联 SVG path 即可，体积更小）
- 不实现 subagent execution trace 嵌套展开（后续补充）
- 不实现 subagent context usage 详细统计（后续补充）
- 不改后端

## Decisions

### D1: SVG 图标方案

不引入 lucide-svelte 依赖。新建 `ui/src/lib/icons.ts`，导出关键图标的 SVG path 字符串（Wrench、Brain、MessageSquare、Bot、Terminal、ChevronRight），在组件中用 `<svg>` 内联渲染。对齐原版视觉，零运行时依赖。

### D2: AI header 统计

在 `toolHelpers.ts` 中新增 `buildAiGroupSummary(chunk: AIChunk): string`，统计 semanticSteps 中各类型数量，格式："N tool calls, M messages, K subagents"。空项不显示。

### D3: Subagent 卡片

对齐原版 `SubagentItem.tsx` 的核心信息：
- Header：Bot 图标 + "Subagent" badge + type（如有）+ 描述截断 60 字 + 状态（ongoing/done）+ 时长
- 数据：从 `AIChunk.subagents` 数组取（后端已序列化）

### D4: System 消息

对齐原版 `SystemChatGroup.tsx`：
- Terminal 图标 + "System" 标签 + 时间戳
- 内容用 `<pre>` 保持格式
- 左对齐（当前居中），宽度限制 85%

## Risks / Trade-offs

- [Subagent 数据可能不完整] → `AIChunk.subagents` 当前是 `unknown[]`，需要确认实际数据结构，不够就做降级显示

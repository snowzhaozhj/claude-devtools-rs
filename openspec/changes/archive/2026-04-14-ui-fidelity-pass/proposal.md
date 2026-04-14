## Why

Rust 版 UI 与原版在信息丰富度和视觉精致度上存在明显差距。当前处于重构/移植阶段，需要严格对齐原版的 UI 表现，包括图标系统、统计信息、工具展示样式和 subagent 展示。

## What Changes

- **AI header 统计信息**：对齐原版 `buildSummary()` 的格式，显示 "N tool calls, M messages, K subagents" 等完整统计
- **语义步骤图标**：对齐原版 lucide-react 图标——工具用扳手(Wrench)、thinking 用大脑(Brain)、文本用消息气泡(MessageSquare)、subagent 用机器人(Bot)
- **Subagent 展示**：从空占位改为独立卡片（描述、model、状态、时长），对齐原版 `SubagentItem.tsx`
- **System 消息样式**：对齐原版 `SystemChatGroup.tsx`（Terminal 图标 + pre 格式输出 + 左对齐）

## Capabilities

### New Capabilities

（无）

### Modified Capabilities

（无）

## Impact

- `ui/src/routes/SessionDetail.svelte`：AI header 统计 + 图标 + subagent 渲染 + system 消息样式
- `ui/src/lib/toolHelpers.ts`：新增 `buildSummary()` 统计函数
- `ui/src/components/BaseItem.svelte`：icon 从字符串改为支持 SVG
- 无后端改动

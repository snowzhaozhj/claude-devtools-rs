## 1. SVG 图标系统

- [x] 1.1 新建 `ui/src/lib/icons.ts`，导出 Wrench/Brain/MessageSquare/Bot/Terminal/ChevronRight 的 SVG path 常量
- [x] 1.2 `BaseItem.svelte` 的 `icon` prop 从 `string`（emoji）改为支持 SVG path 渲染

## 2. AI header 统计信息

- [x] 2.1 `toolHelpers.ts` 新增 `buildAiGroupSummary(chunk: AIChunk): string`，统计 tool calls / messages / subagents / thinking 数量
- [x] 2.2 `SessionDetail.svelte` AI header 区域替换当前的 tool count toggle 为完整统计文本

## 3. 语义步骤图标

- [x] 3.1 Tool 项的图标从 "T" 改为 Wrench SVG
- [x] 3.2 Thinking 项的图标从 "*" 改为 Brain SVG
- [x] 3.3 Subagent 项的图标改为 Bot SVG

## 4. Subagent 展示

- [x] 4.1 确认 `AIChunk.subagents` 的实际数据结构（Process: sessionId, rootTaskDescription, spawnTs, endTs, metrics, team）
- [x] 4.2 在 `api.ts` 中定义 SubagentProcess 类型
- [x] 4.3 `SessionDetail.svelte` 中 subagent_spawn 步骤从空占位改为显示描述 + 时长

## 5. System 消息样式

- [x] 5.1 System 消息改为左对齐 + Terminal 图标 + pre 格式内容，对齐原版 `SystemChatGroup.tsx`

## 6. 验证

- [x] 6.1 `npm run check --prefix ui` 类型检查通过
- [ ] 6.2 `cargo fmt --all`
- [ ] 6.3 `cargo tauri dev` 视觉验证

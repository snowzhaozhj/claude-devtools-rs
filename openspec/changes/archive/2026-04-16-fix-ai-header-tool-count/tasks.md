## 1. DisplayItem 类型定义与构建函数

- [x] 1.1 在 `ui/src/lib/displayItemBuilder.ts` 中定义 `DisplayItem` 类型（thinking / tool / output / subagent / slash 五种 variant）和 `buildDisplayItems(chunk)` 函数
- [x] 1.2 实现 last output 检测：取 semanticSteps 中最后一个 `kind === "text"` 的 step，在 displayItems 中跳过它
- [x] 1.3 实现 `buildSummary(items)` 函数：统计各类型数量，生成 header summary 字符串（顺序：tool → slash → message → subagent → thinking）

## 2. SubagentCard 组件

- [x] 2.1 新建 `ui/src/components/SubagentCard.svelte`：显示任务描述、执行时长、team 信息，独立卡片样式（区别于 BaseItem 的工具样式）
- [x] 2.2 点击 SubagentCard 导航到 subagent session tab（调用 tabStore 的 openTab）

## 3. SessionDetail 渲染重构

- [x] 3.1 重构 `SessionDetail.svelte` AI chunk 渲染：展开区域改为遍历 `buildDisplayItems(chunk)` 返回的 `DisplayItem[]`，按类型分发渲染（tool → BaseItem+ToolViewer, thinking → BaseItem+markdown, output → prose div, subagent → SubagentCard, slash → BaseItem）
- [x] 3.2 `ai-body` 区域改为只渲染 last output（最后一段 text，始终可见）
- [x] 3.3 移除 `toolHelpers.ts` 中的 `buildAiGroupSummary`，header summary 改用 `buildSummary(displayItems)`

## 4. 验证与清理

- [x] 4.1 `npm run check --prefix ui`（svelte-check + tsc 通过）
- [x] 4.2 `cargo tauri dev` 启动应用，验证：编译成功、应用正常启动（PID 6786）
- [x] 4.3 更新 `openspec/followups.md`：标记 chunk-building "Task tool 过滤未在 AIChunk 构建阶段生效" 在 UI 层已修复

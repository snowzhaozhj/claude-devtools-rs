## Why

Cmd+F 会话内搜索在 AI chunk 未展开时无法匹配工具名（Read/Write/Edit/Bash 等）。原因：工具区域受 `{#if toolsVisible}` 条件渲染守门，折叠状态下 DOM 中不存在这些文本节点，TreeWalker 遍历不到。用户必须手动逐一展开每个 AI chunk 才能搜索到工具名，大会话（50+ chunks）下体验极差。

## What Changes

- SearchBar.svelte 新增 `virtualMatches` prop 和 `onNavigateVirtual` 回调，搜索计数合并 DOM 匹配 + 虚拟匹配
- SessionDetail.svelte 新增虚拟匹配收集逻辑（遍历折叠 AI chunk 的 toolExecutions，对 toolName + summary 做子串匹配）
- SessionDetail.svelte 新增虚拟匹配导航回调（展开 chunk → tick → 定位 → 重搜去重）

## Capabilities

### Modified Capabilities
- `ui-search`: 会话内文本搜索与高亮 Requirement 新增折叠工具名虚拟匹配 Scenario

## Non-goals

- 不搜代码块内容（`<pre>/<code>` 内跳过行为保持不变）
- 不搜 tool output 内容（避免触发 IPC 懒加载）
- 不搜 thinking/instructions 等折叠详情

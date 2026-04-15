## Why

SessionDetail 的内容展示存在两个明显短板：
1. **Edit 工具**只显示 REMOVED/ADDED 两个独立块，无法直观看到行级变更——原版使用 LCS diff 算法生成统一视图
2. **Mermaid 图表**在 markdown 中只显示为代码块，无法渲染为可视化图表——原版动态加载 mermaid 库渲染 SVG

## What Changes

### DiffViewer（LCS 行级 diff）
- 新增 `DiffViewer.svelte`：LCS 最长公共子序列算法，生成 added/removed/context 三类行，双列行号，+/- 前缀，统一 diff 视图
- Header 显示文件名、语言标签、+N/-N 统计
- 改造 `EditToolViewer.svelte`：用 DiffViewer 替换原有简单展示

### Mermaid 图表渲染
- 安装 `mermaid` 依赖（动态 import，不影响初始加载）
- 修改 `render.ts`：mermaid 代码块输出占位 div + base64 编码源码
- 新增 `mermaid.ts`：动态加载 mermaid 库 + DOM 后处理渲染
- SessionDetail 添加 `$effect` 在内容变化后触发 mermaid 渲染
- Code/Diagram 切换按钮 + 渲染失败降级到代码视图
- 自动适配深色/浅色主题

### ExecutionTrace（现状说明）
当前 AI chunk 的工具执行已通过 BaseItem + 专用 Viewer 渲染，等效于主 AI group 的 execution trace。原版的深层 ExecutionTrace（subagent 内部执行链）需要后端 SubagentProcess 携带 messages 数据，本批不涉及后端改动，留给后续迭代。

## Capabilities

### Modified Capabilities
- **session-display**：新增 DiffViewer 和 Mermaid 渲染相关 Scenarios

## Impact

- **前端文件**：新增 `DiffViewer.svelte`、`mermaid.ts`；改造 `EditToolViewer.svelte`、`render.ts`、`SessionDetail.svelte`、`app.css`
- **依赖**：新增 `mermaid`（动态 import）
- **后端**：无改动

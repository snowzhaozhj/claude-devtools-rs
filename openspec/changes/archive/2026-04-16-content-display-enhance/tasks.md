## 1. DiffViewer 组件

- [x] 1.1 创建 `DiffViewer.svelte`：LCS 矩阵计算 + 回溯生成 DiffLine[]（added/removed/context），双列行号（oldNum/newNum）
- [x] 1.2 Header：pencil 图标 + 文件名 + 语言标签 + +N/-N 统计
- [x] 1.3 行渲染：行号 gutter（40px×2）+ 前缀（+/-/空格）+ 内容，added/removed 行背景色区分
- [x] 1.4 改造 EditToolViewer 使用 DiffViewer 替换原有 REMOVED/ADDED 分块展示

## 2. Mermaid 图表渲染

- [x] 2.1 `npm install mermaid`
- [x] 2.2 修改 `render.ts` code renderer：`lang === "mermaid"` 时输出 `.mermaid-block` 占位 div + base64 编码源码
- [x] 2.3 创建 `mermaid.ts`：`processMermaidBlocks(container)` 动态 import mermaid → 渲染 SVG → Code/Diagram 切换 → 错误降级
- [x] 2.4 SessionDetail 添加 `$effect` 在 detail+conversationEl 变化后调用 `processMermaidBlocks`
- [x] 2.5 `app.css` 添加 mermaid 相关样式（toolbar/toggle/svg/error）

## 3. 验证

- [x] 3.1 `npm run check --prefix ui` 类型检查通过（0 错误）

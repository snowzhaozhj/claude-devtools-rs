## 1. UI 渲染策略

- [x] 1.1 对比原版 Read/Write/Edit 工具查看器实现，确认需要移植的轻量渲染行为与不复刻的差异。
- [x] 1.2 调整 ReadToolViewer，使较大输出展开时避免逐行同步调用重型 `highlightCode`。
- [x] 1.3 调整 WriteToolViewer，使较大内容展开时避免逐行同步调用重型 `highlightCode`。
- [x] 1.4 调整 DiffViewer/EditToolViewer，使 diff 行保留统一 diff 语义但不执行逐行重型语法高亮。
- [x] 1.5 检查 SessionDetail 展开状态与输出缓存路径，减少与单个工具无关的派生重算，并为 omitted Read 首次展开增加稳定加载占位。

## 2. 测试与验证

- [x] 2.1 为大文本 Read/Write/Edit 渲染策略增加 Vitest 覆盖或等价性能回归测试。
- [x] 2.2 运行 `npm run check --prefix ui`。
- [x] 2.3 运行相关前端单测。
- [x] 2.4 用 mock 浏览器或 Tauri dev 手动验证较大 Read/Edit/Write 工具展开体感。
- [x] 2.5 运行 `openspec validate session-tool-detail-render-perf --strict`。

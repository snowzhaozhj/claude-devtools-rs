## 1. 依赖安装

- [x] 1.1 `npm install marked highlight.js dompurify` + 对应 `@types/` 包
- [x] 1.2 确认 `npm run check` 通过

## 2. 渲染工具模块

- [x] 2.1 创建 `ui/src/lib/render.ts`：导出 `renderMarkdown(text: string): string`（marked + DOMPurify）
- [x] 2.2 导出 `highlightCode(code: string, lang?: string): string`（highlight.js，默认 json）
- [x] 2.3 配置 highlight.js 按需加载语言：json、bash、typescript、rust、python

## 3. SessionDetail 集成

- [x] 3.1 AI text 步骤：`step.text` 改用 `{@html renderMarkdown(step.text)}`，外层加 `.markdown-body` class
- [x] 3.2 AI thinking 步骤：同上改用 markdown 渲染
- [x] 3.3 Tool input：JSON 用 `{@html highlightCode(inputStr, 'json')}`
- [x] 3.4 Tool output：同上用 `highlightCode`，error output 保留红色边框

## 4. 样式

- [x] 4.1 添加 `.markdown-body` 样式（标题、列表、行内代码、代码块），与 Tokyo Night 主题一致
- [x] 4.2 引入 highlight.js 的 `github-dark` 主题 CSS（在 script 中 import）
- [x] 4.3 确认代码块在折叠/展开时渲染正确（`{@html}` 每次展开重新渲染）

## 5. 验证

- [x] 5.1 `npm run check` 通过
- [x] 5.2 `cargo tauri dev` 启动成功

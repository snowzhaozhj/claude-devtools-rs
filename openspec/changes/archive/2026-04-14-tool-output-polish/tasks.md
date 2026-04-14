## 1. CSS 变量调整

- [x] 1.1 `app.css` 中调整 `--code-bg` 为 `#efeee9`、`--code-border` 为 `#d5d3cf`，增强代码块与背景的对比度
- [x] 1.2 同步调整 `--prose-pre-bg` 和 `--prose-pre-border` 保持一致

## 2. OutputBlock 通用组件

- [x] 2.1 新建 `ui/src/components/OutputBlock.svelte`，封装代码块渲染逻辑（语法高亮、错误状态、折叠/展开）
- [x] 2.2 Props 设计：`code: string`、`lang?: string`、`isError?: boolean`、`collapsedMaxHeight?: number`（默认 225px ≈ 15 行）
- [x] 2.3 折叠逻辑：`$effect` 中检测 `scrollHeight > clientHeight`，超出则显示底部遮罩 + "展开全部（N 行）"按钮
- [x] 2.4 水平滚动：`white-space: pre; overflow-x: auto` 替代 `pre-wrap`

## 3. 迁移各 Tool Viewer

- [x] 3.1 `DefaultToolViewer.svelte` 迁移到 OutputBlock（input + output 两处）
- [x] 3.2 `BashToolViewer.svelte` 迁移 output 区域到 OutputBlock
- [x] 3.3 `ReadToolViewer.svelte` 修复 white-space: pre（保持行号布局）
- [x] 3.4 `WriteToolViewer.svelte` 修复 white-space: pre
- [x] 3.5 `EditToolViewer.svelte` 修复 diff 区域 white-space: pre

## 4. 验证

- [x] 4.1 `npm run check --prefix ui` 类型检查通过
- [ ] 4.2 `cargo tauri dev` 启动后视觉验证：代码块边界清晰、长 output 可折叠/展开、宽内容可水平滚动

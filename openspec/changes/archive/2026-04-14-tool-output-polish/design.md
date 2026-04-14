## Context

当前 Tool Viewer 的代码块存在三个视觉问题：

1. `--code-bg: #f4f3f0` 与外层 `--color-surface: #f9f9f7` 色差极小，代码块边界模糊。原版 TS 用 `#f0efed` + `border: #d5d3cf`，对比度更高
2. 长 output 使用 2000 字符截断，但实际阅读以行为单位——2000 字符可能只有 5 行（长 JSON key），也可能有 80 行（短日志），体验不一致
3. `white-space: pre-wrap` 强制断行，长路径/URL 被折到下一行，丢失对齐和可读性

## Goals / Non-Goals

**Goals:**
- 代码块与周围内容有清晰的视觉边界
- 长 output 默认折叠到合理高度（行数驱动），用户可展开
- 宽内容保持原始格式，水平滚动查看
- 各 Tool Viewer 的 output 样式统一，减少重复代码

**Non-Goals:**
- 不改数据层或 Tauri IPC
- 不引入虚拟滚动（output 长度有 max-height 限制，无需）
- 不改 prose 区域的 markdown 代码块样式（这次只改 Tool Viewer 内的代码块）
- 不改配色方案的整体风格（仍然是 Soft Charcoal 暖灰）

## Decisions

### D1: 代码块背景色加深

`--code-bg` 从 `#f4f3f0` 调整为 `#efeee9`，与 `--color-surface: #f9f9f7` 的亮度差从 ~2 增大到 ~5。`--code-border` 从 `#e0deda` 调整为 `#d5d3cf`（与原版一致）。不加阴影——原版也不用阴影，保持扁平风格。

**替代方案**：加 `box-shadow` → 拒绝，与 Soft Charcoal 扁平风格冲突。

### D2: 基于行数的折叠

output 区域默认显示前 15 行（≈225px），超出部分隐藏，底部显示"展开全部（N 行）"按钮。点击后展开到 `max-height: 600px`（仍有 overflow-y scroll 兜底）。

实现：在各 Viewer 的 `<pre>` 外包一层 `.output-collapsible`，用 JS 计算 `scrollHeight > clientHeight` 来决定是否显示展开按钮，避免预先切割文本（保留完整内容以便 Cmd+F 搜索）。

**替代方案**：保持字符截断 → 拒绝，字符数与可视行数不成正比。

### D3: 水平滚动

代码块 `white-space` 从 `pre-wrap` 改为 `pre`，配合 `overflow-x: auto`。这样长行保持原始格式，用户水平滚动查看。

**替代方案**：保持 `pre-wrap` → 拒绝，用户明确反馈长路径断行影响可读性。

### D4: 抽取 OutputBlock 组件

新建 `ui/src/components/OutputBlock.svelte`，封装代码块的通用逻辑：
- Props: `code: string`, `lang?: string`, `isError?: boolean`, `maxLines?: number`
- 内含折叠/展开逻辑、语法高亮、错误状态样式
- 各 Tool Viewer 统一使用，消除 5 处重复的 `<pre class="code-block">` 模板和样式

## Risks / Trade-offs

- [水平滚动 vs 手机/窄窗口] → 桌面应用，窗口宽度可控，风险低。`overflow-x: auto` 只在需要时出现滚动条
- [JS 行数计算的 SSR 兼容性] → Tauri 只有 client-side 渲染，无 SSR 问题
- [折叠状态下 Cmd+F 能否搜到被折叠内容] → 内容始终在 DOM 中（`overflow: hidden`，非 `display: none`），浏览器搜索可以找到

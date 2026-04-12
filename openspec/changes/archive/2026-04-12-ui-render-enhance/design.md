## Context

SessionDetail.svelte 当前用 `<div>` 和 `<pre>` 渲染所有文本。需要引入两个渲染能力：markdown → HTML 和 JSON → 高亮 HTML。

## Goals / Non-Goals

**Goals:**
- Markdown 渲染 AI text/thinking 步骤
- JSON 语法高亮 tool input/output
- 保持 Tokyo Night 暗色主题一致性
- 代码块内的内容也有语法高亮

**Non-Goals:**
- 不做图片/LaTeX 渲染
- 不做 markdown 编辑能力
- 不改后端

## Decisions

### 1. Markdown 库选择

**选择**：`marked`（~40KB gzipped）

**理由**：轻量、零依赖、纯解析不带 DOM 操作，适合 Svelte 的 `{@html}` 渲染。`markdown-it` 功能更多但体积更大且不需要插件生态。`snarkdown` 太简陋不支持代码块。

### 2. 代码高亮库选择

**选择**：`highlight.js`（按需加载语言包）

**理由**：成熟稳定，支持按需注册语言（只加载 `json`、`bash`、`typescript`、`rust`、`python`）。`shiki` 效果更好但需要 WASM 加载，在 Tauri webview 里可能有兼容性问题。`Prism` 也可以但 highlight.js 的 API 更简洁。

### 3. XSS 防护

**选择**：对 `marked` 输出做 sanitize，用 `DOMPurify`。

**理由**：AI 输出和 tool output 可能包含恶意 HTML（用户 session 数据不可信）。`marked` 默认不转义 HTML，必须 sanitize。`DOMPurify` 是业界标准（~7KB gzipped）。

### 4. 集成方式

**选择**：创建 `ui/src/lib/render.ts` 工具模块，导出 `renderMarkdown(text)` 和 `highlightJson(code)` 两个纯函数，SessionDetail.svelte 通过 `{@html}` 调用。

**理由**：渲染逻辑与组件分离，便于测试和复用。

### 5. 样式

**选择**：highlight.js 使用内置 `github-dark` 主题（接近 Tokyo Night）。Markdown 渲染的样式在 SessionDetail.svelte 的 `<style>` 中用 `:global()` 选择器限定作用域。

## Risks / Trade-offs

- **bundle 体积增加**：~50KB gzipped（marked + highlight.js 核心 + 5 语言包 + DOMPurify）→ 对桌面应用影响可忽略
- **`{@html}` XSS**：必须经过 DOMPurify，不能跳过

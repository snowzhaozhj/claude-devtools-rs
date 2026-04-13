## Why

当前 Svelte 版 UI 使用 Tokyo Night 蓝调配色、线性页面导航、统一代码块渲染，与原版 React/Electron 版本在视觉精致度上差距明显。原版已经形成了成熟的设计语言（Soft Charcoal 色板、Sidebar 持久化布局、BaseItem 统一交互模式、专用 Tool Viewer），直接对齐可以大幅提升用户体验，避免重新设计的成本。

## What Changes

- **布局重构**：从三页线性导航（ProjectList → SessionList → SessionDetail）改为 Sidebar + Main 双栏持久化布局。Sidebar 左侧常驻项目选择器和会话列表，Main 区域显示会话详情。
- **配色体系切换**：从 Tokyo Night 蓝调（`#1a1b26` 背景）切换到原版 Soft Charcoal 色板（`#141416` 背景、Zinc 系灰色层次、`rgba(255,255,255,0.05)` 微妙边框）。建立完整的 CSS 变量体系。
- **组件架构升级**：提取 `BaseItem` 可展开项组件（icon + label + summary + tokens + status dot + chevron），所有 AI chunk、tool、thinking 项统一复用。
- **Tool 专用 Viewer**：为 Read（带行号 + 文件名头部）、Edit（diff 红绿对比）、Write（文件内容预览）、Bash（命令高亮）提供专用渲染组件。
- **视觉细节打磨**：hover 状态、过渡动画、StatusDot、ChevronRight 旋转、代码块边框与背景分离等。
- **清理 Vite 模板遗留**：移除 `app.css` 中未使用的 hero/counter/next-steps 等样式。

## Capabilities

### New Capabilities

（无——纯 UI 层改动，不涉及数据层 capability）

### Modified Capabilities

（无——不改变任何 spec 级行为要求）

## Impact

- **前端文件**：`ui/src/` 下几乎所有 `.svelte` 和 `.css` 文件将被修改或新增
- **新增组件**：`BaseItem.svelte`、`StatusDot.svelte`、`ReadToolViewer.svelte`、`EditToolViewer.svelte`、`WriteToolViewer.svelte`、`BashToolViewer.svelte`、`DefaultToolViewer.svelte`、`Sidebar.svelte`
- **Rust/Tauri 层**：不涉及任何改动
- **依赖**：不引入新的 npm 依赖（继续使用 marked + highlight.js + dompurify）

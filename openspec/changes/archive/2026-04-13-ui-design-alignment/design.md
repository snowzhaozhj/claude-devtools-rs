## Context

当前 Svelte 5 前端有 4 个页面组件（`App.svelte`、`ProjectList.svelte`、`SessionList.svelte`、`SessionDetail.svelte`）+ 2 个库文件（`api.ts`、`render.ts`）+ 1 个全局样式（`app.css`，含大量 Vite 模板遗留）。全部样式硬编码在组件 `<style>` 块中，无 CSS 变量体系。

原版 React/Electron 版有完整的设计系统：~60 个 CSS 变量、Sidebar + TabBar + PaneContainer 三层布局、BaseItem 统一交互组件、5 种 Tool Viewer 专用渲染。

## Goals / Non-Goals

**Goals:**
- 建立与原版一致的 CSS 变量体系（Soft Charcoal 色板）
- 实现 Sidebar + Main 双栏持久化布局
- 提取 `BaseItem` 可复用组件，统一所有可展开项的交互模式
- 为 Read/Edit/Write/Bash 提供专用 Tool Viewer
- 清理 Vite 模板遗留样式

**Non-Goals:**
- Tab 系统（多会话并行）——后续迭代
- Context Panel（右侧上下文面板）——后续迭代
- 虚拟滚动（大会话性能）——后续迭代
- 搜索/过滤功能——后续迭代
- Light 主题——后续迭代
- 可拖拽 Sidebar 宽度调整——后续迭代

## Decisions

### 1. CSS 变量体系：直接复用原版变量名

**选择**：从原版 `index.css` 提取核心 CSS 变量子集（约 40 个），保持相同变量名。

**替代方案**：自定义变量名 → 拒绝，因为后续对齐更多特性时还得改名。

**核心变量分组**：
```
Surface:     --color-surface, --color-surface-raised, --color-surface-sidebar
Text:        --color-text, --color-text-secondary, --color-text-muted
Border:      --color-border, --color-border-subtle, --color-border-emphasis
Tool items:  --tool-item-name, --tool-item-summary, --tool-item-muted, --tool-item-hover-bg
Code:        --code-bg, --code-border, --code-line-number, --code-filename
Syntax:      --syntax-string, --syntax-comment, --syntax-number, --syntax-keyword, --syntax-type, --syntax-function
Prose:       --prose-heading, --prose-body, --prose-link, --prose-code-bg, --prose-code-text
Thinking:    --thinking-bg, --thinking-border, --thinking-text
Diff:        --diff-added-bg, --diff-added-text, --diff-removed-bg, --diff-removed-text
Tool result: --tool-result-success-bg, --tool-result-error-bg, --tool-result-error-text
```

### 2. 布局方案：Sidebar 组件内含项目选择器 + 会话列表

**选择**：`App.svelte` 改为 flex 水平双栏。左侧 `Sidebar.svelte`（固定 280px）包含项目下拉选择器 + 会话列表。右侧 Main 区域显示 `SessionDetail`（或空状态）。

**替代方案**：保持线性导航但加面包屑 → 拒绝，与原版差距仍大，且 Sidebar 是后续 Tab 系统的基础。

**Sidebar 内部结构**：
```
Sidebar (280px, --color-surface-sidebar)
├── SidebarHeader
│   ├── 应用标题 "Claude DevTools"
│   └── 项目选择器（下拉或列表切换）
└── SessionList（滚动区域）
    └── 按日期分组的会话条目
```

### 3. 组件架构：BaseItem + 专用 Viewer 分离

**选择**：提取 `BaseItem.svelte` 作为统一的可展开项容器（header + slot），各 Viewer 通过 slot 嵌入。

**BaseItem header 布局**（与原版对齐）：
```
[Icon 16px] [Label font-medium] [-] [Summary truncate] ... [Token badge] [StatusDot] [Duration] [Chevron ▸/▾]
```

**Viewer 组件**：
- `ReadToolViewer.svelte`：文件头（图标+文件名+语言badge+复制按钮）+ 带行号的代码块
- `EditToolViewer.svelte`：文件路径 + old_string（红色 diff）+ new_string（绿色 diff）
- `WriteToolViewer.svelte`：文件路径 + 文件内容预览
- `BashToolViewer.svelte`：命令显示 + 输出（保留折叠逻辑）
- `DefaultToolViewer.svelte`：通用 INPUT/OUTPUT 代码块（当前逻辑的提取）

### 4. 文件组织：components 目录

**选择**：新增 `ui/src/components/` 目录。

```
ui/src/
├── components/
│   ├── Sidebar.svelte
│   ├── SidebarHeader.svelte
│   ├── BaseItem.svelte
│   ├── StatusDot.svelte
│   └── tool-viewers/
│       ├── ReadToolViewer.svelte
│       ├── EditToolViewer.svelte
│       ├── WriteToolViewer.svelte
│       ├── BashToolViewer.svelte
│       └── DefaultToolViewer.svelte
├── routes/
│   └── SessionDetail.svelte  (大幅简化，提取 viewer 后)
├── lib/
│   ├── api.ts
│   ├── render.ts
│   └── toolHelpers.ts  (新增：tool summary 生成、状态判断等)
├── App.svelte  (重构为双栏布局)
└── app.css     (重写为 CSS 变量体系)
```

### 5. Thinking 块渲染：从 `<details>` 改为 BaseItem

**选择**：Thinking 块不再用 HTML `<details>`，改为通过 `BaseItem`（icon=🧠, label="Thinking"）统一渲染，与原版一致。

### 6. AI Chunk 渲染：从折叠 bar 改为 flat list

**选择**：原版不把整个 AI chunk 折叠为一行，而是把 AI response 的每个 semantic step 作为独立的 BaseItem 平铺展示（thinking / text / tool 各自独立展开）。当前版本的「点击 AI bar 展开」模式改为直接平铺。

**替代方案**：保持当前的 AI chunk 折叠模式 → 拒绝，与原版交互模式差异大。

**结果**：User message 直接显示内容；AI response 的 steps 逐个平铺为 BaseItem 行。

## Risks / Trade-offs

- **大范围文件改动**：几乎所有前端文件都会改。风险是中间状态难以验证 → 缓解：按 task 分步，每步 `cargo tauri dev` 验证。
- **AI chunk 平铺可能导致长会话视觉碎片化** → 缓解：通过 User/AI 消息间的分隔线和缩进保持层次感。后续虚拟滚动可进一步优化。
- **组件拆分增加文件数** → 可接受的代价，换来每个组件职责清晰、可独立测试。

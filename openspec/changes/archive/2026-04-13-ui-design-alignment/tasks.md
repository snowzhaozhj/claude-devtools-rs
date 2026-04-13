## 1. CSS 变量体系 + 清理遗留样式

- [x] 1.1 重写 `app.css`：移除 Vite 模板遗留样式（hero/counter/next-steps/ticks 等），建立原版 Soft Charcoal CSS 变量体系（surface/text/border/code/syntax/prose/thinking/diff/tool-result 分组，约 40 个变量）
- [x] 1.2 验证：`npm run check` 通过，`cargo tauri dev` 启动后基础样式正常

## 2. 布局重构：Sidebar + Main 双栏

- [x] 2.1 新建 `ui/src/components/Sidebar.svelte`：包含项目选择器（下拉）+ 会话列表（滚动），固定宽度 280px，使用 `--color-surface-sidebar` 背景
- [x] 2.2 新建 `ui/src/components/SidebarHeader.svelte`：应用标题 + 项目下拉选择器
- [x] 2.3 重构 `App.svelte`：从线性导航改为 flex 水平双栏（Sidebar + Main），移除 header/back-btn，路由状态简化为「选中项目+选中会话」驱动
- [x] 2.4 调整 `SessionDetail.svelte`：移除外层 padding（由 Main 区域控制），适配新布局
- [x] 2.5 验证：`npm run check` 通过，`cargo tauri dev` 双栏布局正常，项目切换→会话列表→会话详情流程通畅

## 3. BaseItem 组件 + StatusDot

- [x] 3.1 新建 `ui/src/components/StatusDot.svelte`：小圆点指示器（ok=绿/error=红/pending=灰/orphaned=灰），6px 圆形
- [x] 3.2 新建 `ui/src/components/BaseItem.svelte`：统一的可展开项容器——header 行（icon + label + separator + summary + spacer + token badge + status dot + duration + chevron）+ slot 展开内容（左侧 2px 边框缩进）。hover 背景 `--tool-item-hover-bg`，chevron 旋转动画 90deg
- [x] 3.3 验证：创建简单测试场景确认 BaseItem 展开/折叠、hover、chevron 动画正常

## 4. Tool 专用 Viewer

- [x] 4.1 新建 `ui/src/lib/toolHelpers.ts`：提取 tool summary 生成（`getToolSummary`）、tool status 判断（`getToolStatus`）、文件扩展名→语言映射等辅助函数
- [x] 4.2 新建 `ui/src/components/tool-viewers/DefaultToolViewer.svelte`：通用 INPUT/OUTPUT 代码块（从 SessionDetail 提取当前逻辑），带折叠输出、错误高亮
- [x] 4.3 新建 `ui/src/components/tool-viewers/ReadToolViewer.svelte`：文件头部（文件图标 + 文件名 + 语言 badge + 复制按钮）+ 带行号的代码块 + 语法高亮
- [x] 4.4 新建 `ui/src/components/tool-viewers/EditToolViewer.svelte`：文件路径 + old_string（`--diff-removed-bg/text` 红色背景）+ new_string（`--diff-added-bg/text` 绿色背景）diff 对比
- [x] 4.5 新建 `ui/src/components/tool-viewers/WriteToolViewer.svelte`：文件路径 + 文件内容预览代码块
- [x] 4.6 新建 `ui/src/components/tool-viewers/BashToolViewer.svelte`：命令显示（单独样式）+ 输出代码块（保留折叠逻辑）
- [x] 4.7 验证：`npm run check` 通过

## 5. SessionDetail 重构：集成 BaseItem + Viewer

- [x] 5.1 重构 AI chunk 渲染：从「点击 bar 展开整个 chunk」改为 semantic steps 平铺，每个 step 用 BaseItem 渲染（thinking: 🧠 图标紫色、text: 💬 图标、tool_execution: 🔧 图标）
- [x] 5.2 Tool execution 渲染改为 BaseItem + 专用 Viewer 路由（Read→ReadToolViewer, Edit→EditToolViewer, Write→WriteToolViewer, Bash→BashToolViewer, 其它→DefaultToolViewer）
- [x] 5.3 User chunk 样式对齐原版：使用 `--chat-user-bg/text/border` 变量，气泡式卡片
- [x] 5.4 System/Compact chunk 样式对齐
- [x] 5.5 Prose 样式迁移到 CSS 变量（`--prose-heading/body/link/code-bg` 等）
- [x] 5.6 验证：`npm run check` 通过，`cargo tauri dev` 会话详情渲染与原版视觉基本对齐

## 6. 视觉细节打磨

- [x] 6.1 代码块样式对齐：`--code-bg` 背景、`--code-border` 边框、`word-break: break-all` 改为 `overflow-x: auto`
- [x] 6.2 Sidebar 会话列表条目样式：hover 效果、选中态高亮、时间格式
- [x] 6.3 整体间距/圆角/字体大小微调，确保与原版视觉一致
- [x] 6.4 最终验证：`npm run check` 通过，`cargo tauri dev` 全流程视觉验收

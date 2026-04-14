## 1. Sidebar 会话过滤

- [x] 1.1 `Sidebar.svelte` 在 `session-count-bar` 中嵌入搜索输入框，绑定 `filterQuery` 状态
- [x] 1.2 用 `$derived` 计算 `filteredSessions`：对 `session.title` 做 case-insensitive includes 过滤
- [x] 1.3 `dateGroups` 改为基于 `filteredSessions` 而非 `sessions`，搜索结果为空时显示"无匹配会话"

## 2. 搜索高亮工具函数

- [x] 2.1 新建 `ui/src/lib/searchHighlight.ts`，实现 `highlightMatches(container, query): number`（TreeWalker 遍历文本节点，包裹 `<mark>`）
- [x] 2.2 实现 `clearHighlights(container): void`（移除所有 `<mark>` 恢复原始文本）
- [x] 2.3 实现 `scrollToMatch(container, index): void`（设置当前匹配项样式 + `scrollIntoView`）

## 3. SearchBar 组件

- [x] 3.1 新建 `ui/src/components/SearchBar.svelte`，包含搜索输入框 + 结果计数（"N of M"）+ 上下导航按钮 + 关闭按钮
- [x] 3.2 实现 300ms debounce 搜索触发，Enter 下一个、Shift+Enter 上一个、Esc 关闭
- [x] 3.3 `app.css` 中添加 `mark` 高亮样式（匹配项背景色 + 当前项区分色）

## 4. 集成到 SessionDetail

- [x] 4.1 `SessionDetail.svelte` 添加搜索状态管理（searchVisible）和 Cmd+F 快捷键监听
- [x] 4.2 在 top-bar 下方条件渲染 SearchBar，传入 `.conversation` 容器引用
- [x] 4.3 切换 session 时自动关闭搜索栏并清除高亮

## 5. 验证

- [x] 5.1 `npm run check --prefix ui` 类型检查通过
- [ ] 5.2 `cargo tauri dev` 视觉验证：Sidebar 过滤 + Cmd+F 搜索 + 高亮导航

## Context

当前 Sidebar 会话列表无搜索/过滤功能，会话多时难以定位。Session 详情页也没有内容搜索——原版有自定义 Cmd+F 搜索栏（SearchBar + 高亮 + 导航），需要移植到 Svelte。

原版搜索的关键特征：
- SearchBar 固定在详情页顶部，Cmd+F 打开、Esc 关闭
- 300ms debounce 后执行纯前端 indexOf 搜索
- 匹配项用 `<mark>` 标签高亮，当前项样式区别于其他匹配
- Enter/Shift+Enter 导航上下匹配项，自动滚动到视口
- 显示 "N of M" 结果计数

## Goals / Non-Goals

**Goals:**
- Sidebar 搜索框实时过滤会话列表（按标题匹配）
- Session 内 Cmd+F 搜索（全文匹配、高亮、导航）
- 对齐原版交互模式

**Non-Goals:**
- 不接后端 `SessionSearcher`（前端过滤已足够，会话数量不大）
- 不做正则搜索、模糊匹配
- 不改 Tauri IPC 层

## Decisions

### D1: Sidebar 过滤 — 前端 filter

在 `session-count-bar` 区域嵌入搜索输入框，实时过滤 `sessions` 数组（对 `session.title` 做 case-insensitive includes）。无 debounce（纯内存过滤，无需）。清空搜索框恢复完整列表。

### D2: Session 内搜索 — DOM TreeWalker

原版用 React Children 递归给元素加 `<mark>`。Svelte 中内容通过 `{@html}` 注入，无法操作元素树。

方案：用 DOM `TreeWalker` 在渲染后的 `.conversation` 容器中搜索文本节点，将匹配文本包裹在 `<mark>` 元素中。搜索变更时先清除所有 `<mark>` 再重新标记。

优点：不修改渲染管线，不影响 markdown/高亮逻辑，与原版效果一致。

### D3: SearchBar 组件

新建 `ui/src/components/SearchBar.svelte`：
- Props: `visible: boolean`、`onClose: () => void`、`containerEl: HTMLElement`（搜索的 DOM 容器）
- 内部管理搜索状态：query、currentIndex、totalMatches
- 300ms debounce 触发搜索
- Enter → 下一个、Shift+Enter → 上一个、Esc → 关闭
- 当前匹配项 `scrollIntoView({ block: "center" })`

### D4: 搜索高亮工具

新建 `ui/src/lib/searchHighlight.ts`：
- `highlightMatches(container: HTMLElement, query: string): number` — 返回匹配数
- `clearHighlights(container: HTMLElement): void` — 移除所有 `<mark>`
- `scrollToMatch(container: HTMLElement, index: number): void` — 滚动到第 N 个匹配

## Risks / Trade-offs

- [DOM 操作 vs `{@html}` 内容更新冲突] → 搜索时 conversation 内容已稳定（不会实时追加），冲突风险低。切换 session 时自动关闭搜索栏
- [大量 `<mark>` 节点性能] → 原版也是全量标记，session 文本量不足以造成性能问题

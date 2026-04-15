## 1. Tab Store 核心

- [x] 1.1 创建 `ui/src/lib/tabStore.svelte.ts`：定义 `Tab` 接口、模块级 `$state`（`tabs: Tab[]`、`activeTabId: string | null`）、导出 `openTab`/`closeTab`/`setActiveTab`/`getActiveTab` 函数
- [x] 1.2 实现 per-tab UI 状态：`Map<string, TabUIState>`（`expandedChunks`、`expandedItems`、`searchVisible`、`contextPanelVisible`、`scrollTop`），导出 `getTabUIState`/`saveTabUIState` 函数
- [x] 1.3 实现 per-tab session 数据缓存：`Map<string, SessionDetail>`，`openTab` 时若缓存存在直接复用，否则标记待加载；`closeTab` 时清理缓存

## 2. TabBar 组件

- [x] 2.1 创建 `ui/src/components/TabBar.svelte`：水平标签条，渲染 `tabs` 列表，高亮 `activeTabId`，每个 tab 显示 label + 关闭按钮（X）
- [x] 2.2 TabBar 样式：高度 36px，Soft Charcoal 配色对齐（`--color-bg-tertiary` 背景、`--color-border` 分隔线、active tab 用 `--color-bg-primary`），tab 项水平滚动，文本截断省略号

## 3. App 布局改造

- [x] 3.1 改造 `App.svelte`：移除 `selectedSessionId` 单一状态，改为从 `tabStore` 读取 `activeTab`；Main 区域改为 TabBar + SessionDetail 垂直布局
- [x] 3.2 Sidebar 集成：`onSelectSession` 回调改为调用 `tabStore.openTab()`，Sidebar 高亮从 `selectedSessionId` 改为 `activeTab?.sessionId`

## 4. SessionDetail 状态外提

- [x] 4.1 SessionDetail 改造：`expandedChunks`/`expandedItems`/`searchVisible`/`contextPanelVisible` 改为从 tabStore 的 per-tab UI 状态读写，组件本身不再持有这些 `$state`
- [x] 4.2 滚动位置保存/恢复：tab 切换时保存当前 `conversationEl.scrollTop` 到 tabStore，激活新 tab 时恢复
- [x] 4.3 Session 数据加载：SessionDetail 的 `onMount`/`$effect` 改为先检查 tabStore 缓存，有则直接用，无则调用 API 加载并存入缓存

## 5. 验证

- [x] 5.1 `npm run check --prefix ui` 类型检查通过
- [x] 5.2 `cargo tauri dev` 启动，功能验证：打开多个 session tab → 切换 → 关闭 → 展开状态独立 → 滚动位置恢复

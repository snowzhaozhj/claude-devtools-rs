# ui/ — Svelte 5 + Vite 前端

仅在 Claude 读写 `ui/**` 下的文件时由 Claude Code 自动加载（子目录 CLAUDE.md on-demand 机制）。跨域共识在根 `CLAUDE.md`。

## 架构与布局

- **chrome 三层**：`UnifiedTitleBar`（顶部 44 px，macOS 内部 80 px 让位 traffic light，含 ProjectSwitcher / UpdateStatusPill / RosettaStatusIcon / 通知 / 设置）+ Sidebar（200~500 px 可拖拽）+ TabBar（pane 内独立，仅 tab 列表，**不再**含通知/设置/traffic-light padding）+ Main。
- **页面**：SessionDetail、SettingsView、NotificationsView、DashboardView（项目卡片网格，替代空状态）。Tab 类型 4 种：session / settings / notifications / memory（后三者单例）。
- **核心组件**：BaseItem、StatusDot、OutputBlock、SearchBar（Cmd+F）、CommandPalette（Cmd+K）、ContextPanel（Category/Ranked + DirectoryTree）、DiffViewer（LCS 行级 diff）、SessionContextMenu（右键 5 项）、Tool Viewer（Read/Edit/Write/Bash/Default）、UpdatePopover、ProjectSwitcher。
- **图标**：`ui/src/lib/icons.ts` 导出 lucide 风格 SVG path 常量，BaseItem 通过 `svgIcon` prop 渲染。

## 状态与主题

- `tabStore.svelte.ts` 管 tabs / activeTabId / per-tab UI 状态 / session 缓存 / notificationUnreadCount。`sidebarStore.svelte.ts` 管 sidebar 宽度 + per-project Pin/Hide（内存级）。Settings / Notifications 状态各自组件内管。
- 主题：`app.css` `:root` 浅色 + `[data-theme="dark"]` 深色 + `@media prefers-color-scheme` 跟随系统。`lib/theme.ts::applyTheme()` 设置 `data-theme`，App 启动从 config 读。

## 数据流（前端侧）

- **session 元数据**：后端 emit `session-metadata-update`，前端 `listen()` 按 sessionId in-place patch（不要替换 SessionSummary 实例引用，会触发整行 DOM 重建）。
- **通知**：后端 emit `notification-update`，前端 `listen()` 立即刷 badge + TabBar 每 30 秒轮询 unreadCount 兜底。
- **file-change 节流链**：后端 `cdt-watch::FileWatcher` debounce 100 ms；前端 `ui/src/lib/fileChangeStore::dedupeRefresh` 仅合并 in-flight 期间的并发调用，**不做时间节流**。高频写 JSONL 会每几百 ms re-render——如需降频加 250 ms cooldown 或 trailing debounce。

## Svelte 5 陷阱（high frequency 全部踩过）

- **`$effect` 自动追踪 + props 顶层初始化要 untrack**：`$effect` 中读取的所有响应式变量自动成为依赖；**模块顶层用 props 取初始值**（如 `let uiState = getTabUIState(tabId)` / `` const key = `session-detail-${tabId}` ``）也会触发 `state_referenced_locally` warning。两类场景都用 `import { untrack } from "svelte"` + `untrack(() => variable)`。typical case：tabId 在组件生命周期内不变（切 tab 走 destroy/recreate），但 Svelte 5 仍要求显式声明。
- **`<button>` 嵌套禁止**：浏览器会修复 DOM 结构导致 Svelte 假设失效。用 `<span role="button" tabindex="-1">` 替代。
- **`{@const}` 位置限制**：只能是 `{#if}` / `{:else}` / `{#each}` / `{#snippet}` / `<Component>` 的直接子级，不能放在 `<div>` 等 HTML 元素内。在块开头集中声明。
- **`cache = source.field` + `cache ?? source.field` 兜底反模式**：写完缓存 cache 永远不为 null，`??` fallback 永远不生效——`source` 替换（props 实例换 / 父刷新）后 UI 卡在旧 cache 值。**只在"真正派生出新值"时写 cache**（如 IPC 返回新结构），其它分支让 cache 保持 null 让 fallback 自动消费实时 source。例：`SubagentCard.svelte::ensureMessages` rollback / IPC 失败路径不写 `messagesLocal`。codex 三轮 CR 重复发现。
- **`$effect` 订阅 `$derived` 派生值的去重**：effect 只在派生**值**变化时重跑——派生内的依赖某 state 变了但派生输出（`===`）不变时，effect 不重跑。props 实例替换但派生指纹未变 → effect 不动 → 任何缓存了 props 字段的本地 state 会永久遮蔽 props 替换。检查清单：派生指纹包含所有"内容变化但 props ref 也可能变"的维度，或干脆不缓存让 props 字段直接派生消费。
- **`{@attach}` 挂副作用**：DOM 元素需要副作用 + cleanup（ResizeObserver / IntersectionObserver / scroll listener 等）时用 `{@attach (el) => { ...setup; return () => cleanup; }}`，比 `bind:this + onMount + onDestroy` 三段式更内聚。例见 `Sidebar.svelte::session-list` 挂 ResizeObserver。
- **scoped CSS root attribute 必须 `:global()`**：写 `[data-theme="dark"] .my-class` 会被 svelte-check 报 `css_unused_selector`——root html 的 `data-theme` 不在组件 scoped 范围。改 `:global([data-theme="dark"]) .my-class`。
- **`content-visibility: auto` 父级 throttle 子树 CSS animation**：浏览器把离屏子树的 layout/paint/animation 跳过，spinner / ping / sweep / shimmer 离屏 + 回到视口会"半天才转一下"。修法：含持续 animation 的父容器 SHALL 退出 contain，如 `class:msg-row-contained={!hasAnimation}`。已发：#121 OngoingBanner spinner / #122 OngoingBanner ping+sweep。

## Settings 与 config 修改

- **乐观更新模式**：config 修改不能依赖 `updateConfig` 返回值刷新 UI，应先乐观更新本地 `$state`，异步调 API，失败时回滚（重新 `getConfig`）。

## 渲染依赖与高亮

- **依赖**：`marked`（markdown→HTML）+ `highlight.js`（按需加载语言）+ `dompurify`（XSS 防护）+ `mermaid`（图表，动态 import）。highlight.js 不引入预制主题 CSS，用 `app.css` 自定义 Soft Charcoal token 颜色。
- **hljs token 颜色单点维护**：`.hljs-*` token 颜色 SHALL 写在 `ui/src/app.css` 全局（与 `--syntax-*` CSS 变量绑定，浅/深主题自动切换）；组件内**不要**再写 `.<component> :global(.hljs-*) { color: ... }` 局部覆盖。历史散在 5 处导致 DiffViewer 漏写、Edit 工具行 +/- 背景下 token 无色。
- **`getLanguageFromPath`**（`ui/src/lib/toolHelpers.ts`）优先级链路：精确特殊名（Dockerfile/Makefile）→ ext 真映射 → 前缀兜底（Dockerfile.dev）→ text；改动时不要破坏顺序，否则 `Jenkinsfile.kts`（Kotlin DSL）会被错认 groovy。

## 列表 / 详情自动刷新反闪烁三原则

1. **`{#each}` 必须带稳定 key**（AIChunk 用 `responses[0].uuid`，UserChunk/System/Compact 用 `uuid`，SessionSummary 用 `sessionId`），否则 file-change 刷新时整段 DOM 重建 + mermaid/highlight.js 重跑。
2. **`loadX(..., silent = false)` 加 silent 参数**：file-change handler 传 `silent=true` 保留旧列表直到新数据到达，**不要**经过"加载中..."中间态。
3. **状态指示器嵌入已有 slot**（如 `<OngoingBanner>` 替代最后 AIChunk 的 `lastOutput`，对齐原版 `LastOutputDisplay.tsx::isLastGroup && isSessionOngoing` 语义），**不要**作为独立节点追加到流尾部——显隐切换时 scrollHeight 跳变引发贴底滚动视觉抖动。

## PaneView 与 SessionDetail

- **PaneView `{#key}` 复合 `tabId@sessionId`**：单用 `activeTab.id` 在 `openOrReplaceTab`（保留 tabId 仅换 sessionId）路径下不触发 SessionDetail destroy/recreate，导致详情页只换标题不刷新。复合 key 后旧实例 destroy 时 `SessionDetail.onDestroy` SHALL 用 `getTabSessionId(tabId) === sessionId` guard 防止把旧 session 的 expanded/scroll 状态写回 tabUIStates 污染新 session（tabId 不变 → 新实例直接读到旧 state）。

## 与原版对齐

- 前端文本清洗逻辑移植自 `../claude-devtools/src/shared/utils/contentSanitizer.ts`（`sanitizeDisplayContent`）。扩展 UI 功能时优先查原版 `src/renderer/` 和 `src/shared/` 对应实现，**直接移植不要造轮子**。
- 视觉规范 / 重写组件视觉级任务 SHALL 先 invoke `impeccable` skill 拿 PRODUCT.md + DESIGN.md 设计禁令（side-stripe ban / hero-metric / glass / 渐变文字等）。

## 开发命令

- `pnpm --dir ui run check` 必须从项目根目录跑，从 `src-tauri/` 目录跑会找不到 `package.json`。
- 浏览器直接访问 `localhost:5173` 会报 `invoke` undefined——必须通过 `cargo tauri dev` 的窗口测试，或用 `pnpm --dir ui run dev` + 浏览器访问 `?mock=1&fixture=...`。
- worktree rebase 后若 origin/main 加新 ui 依赖（典型 `tauri-plugin-opener`），跑 `pnpm --dir ui install` 重装（pnpm hardlink + global store，lockfile 未变近瞬时；变了也只下差量）。

## 浏览器调试入口

不开 Tauri 窗口调 UI：`pnpm --dir ui run dev` → `http://localhost:5173/?mock=1&fixture=multi-project-rich`。fixture 有 `empty` / `single-project` / `multi-project-rich` 三种，详见 `ui/src/lib/__fixtures__/`。**仅 dev 启用**，production bundle 完全不含 mockIPC（vite DCE 验证见 `tauriMock.bundle.test.ts`）。

## 测试基础设施陷阱

- **vitest `globals: false` 是硬约束**：设 `globals: true` 后 Playwright runner 报「test.describe was called in a file imported by configuration file」——vitest 通过 vite plugin 注入全局 `test`/`describe`，污染 Playwright transform 链。`ui/vitest.config.ts` MUST 保持 `globals: false`，vitest 测试显式 `import { test, describe, expect } from 'vitest'`。
- **production bundle DCE 整块包**：消除 mockIPC chunk 必须 `if (import.meta.env.DEV) { ... await import('./lib/tauriMock'); ... }` 整块包，**不能**用 `if (!DEV) return; ...` 早期 return——后者 vite 仍把 dynamic import chunk 输出到 dist。验证：`rm -rf ui/dist && pnpm --dir ui run build && ls ui/dist/assets | grep -iE "mock|fixture"` 应空。
- **bundle test 强制 `NODE_ENV=production`**：`tauriMock.bundle.test.ts` 调 `execSync('pnpm run build', { env: { ...process.env, NODE_ENV: 'production' } })`——否则 vitest 父进程 `NODE_ENV=test` 传染给子进程，DCE 失效。默认 skip，本地手动用 `RUN_BUNDLE_TESTS=1 pnpm --dir ui run test:unit`。
- **vite optimizer cache 多 spec 跑后污染 Playwright**：连续 `pnpm exec playwright test` 后再跑可能报「test.describe in config」假错；`rm -rf ui/node_modules/.vite ui/node_modules/.cache` 清掉即恢复。CI 上 `reuseExistingServer=false` 不受影响。
- **Playwright 绕过 UI 直接调 store**：`TabBar` 仅在 `pane.tabs.length > 0` 时渲染——空状态点不到「设置」/「通知」title 按钮。`main.ts` dev-only 暴露 `window.__cdtTest = { openSettingsTab, openNotificationsTab, openTab, setActiveTab }`，spec 用 `page.evaluate(() => window.__cdtTest.openSettingsTab())` 绕过 sidebar virtualization 时序 flake。production bundle 由 `if (DEV)` 块 DCE，不暴露。
- **`window.__cdtTest.openTab(sessionId, projectId, label)` 参数顺序**：sessionId 在前、projectId 在后；反了会报 `Cannot read properties of undefined (reading 'length')`。
- **vitest 测 svelte store 模块级 `$state`**：模块级 `$state` 跨 vitest test 不 reset（每个 test 都拿同一个模块实例）。两种处理：(a) 渐进 assertion 不 reset（推荐）；(b) `vi.resetModules()` + dynamic import 强制重载（复杂）。
- **Playwright `reuseExistingServer: true` 本地缓存 fixture 内存 state**：fixture module 的 `fx.config` 等 mutable state 在 vite dev server 进程内存里——上一轮 e2e 改完 config 后下一轮跑会拿到串用 state。**改 fixture 后 SHALL `pkill -f vite` 强制 fresh webServer**，或临时 `reuseExistingServer: false` 跑一遍。CI 上 `process.env.CI` 自动 fresh。
- **mockIPC 注入 `__TAURI_INTERNALS__`，UI 代码不能用它判 mock vs real**：`tauriMock.ts::setupMockIPC` 内部的 mockWindows + mockIPC 会注入 `__TAURI_INTERNALS__`，导致用 `if ('__TAURI_INTERNALS__' in window)` 做运行时分支的 UI 代码在 `?mock=1` 浏览器调试模式走错路径。架构原则：plugin 命令的 mock 走 `tauriMock.ts::buildHandler` 加 case（如 `'plugin:opener|open_url'` → `window.open`），UI 代码统一调真 plugin API，IPC 层负责分流。

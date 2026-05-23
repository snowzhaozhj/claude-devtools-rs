## 1. 通用浮层组件 `AppContextMenu.svelte`

- [x] 1.1 新建 `ui/src/lib/components/AppContextMenu.svelte`，定义 props 类型 `ContextMenuItem = { label, icon?, action, disabled?, danger?, separator? }` 与 `Props = { x, y, items, onClose, predeAtFirstItem? }`
- [x] 1.2 渲染 `role="menu"` `aria-orientation="vertical"` 容器，items map 到 `role="menuitem"`（separator 渲染 `<div class="cm-sep" role="separator">`），视觉沿用现 `SessionContextMenu` 的 token（bg / border / radius / shadow / padding / font）
- [x] 1.3 实现 viewport 边界 clamp：`$derived(Math.min(x, window.innerWidth - MENU_WIDTH - 8))` / 同向 y
- [x] 1.4 实现键盘 ↑↓ Enter Esc + Tab focus trap：`activeIndex` `$state` + 方向键循环（**经过** `aria-disabled`、仅跳过 `role="separator"`）+ Enter / Space 在 enabled item 触发 action 关菜单 + Enter / Space 在 `aria-disabled` item no-op + Esc 关闭并 focus trigger
- [x] 1.4a 菜单打开 SHALL 立即将 focus 移到第一个非 separator menuitem（不区分鼠标 / 键盘触发）
- [x] 1.4b 鼠标 hover 同步 `activeIndex` 到 hover 项，避免键盘与鼠标 focus 分裂
- [x] 1.4c disabled item 用 `aria-disabled="true"` 而**非**原生 `disabled` 属性，保留 a11y 树可达性
- [x] 1.5 实现关闭触发：document `mousedown` 外点 / Esc / window `blur` / 任意祖先 `scroll` / window `resize`，全部在 `onMount` 注册、`onDestroy` 清理
- [x] 1.6 实现 disabled / danger / separator / icon 渲染分支
- [x] 1.7 实现 action feedback 机制：`item.action?.feedback?: { label: string; durationMs?: number }` 字段（沿用现 `SessionContextMenu::copyText` 的 600ms 模式），命中时 item label 切换为 feedback label 后关闭

## 2. Svelte action `use:contextMenu`

- [x] 2.1 新建 `ui/src/lib/contextMenu.svelte.ts`，导出 `contextMenu(node, provider)` action 与类型 `ContextMenuProvider = ContextMenuItem[] | (e: MouseEvent | KeyboardEvent) => ContextMenuItem[]`
- [x] 2.2 action 内挂 `oncontextmenu`：调用 `e.preventDefault()` + `e.stopPropagation()` + 通过 Svelte 5 `mount()` API 把 `AppContextMenu` portal 到 `document.body` 末尾（**不**作为 trigger 子节点 inline 渲染——避免 overflow clipping / z-index stacking 问题，详 design.md D7）
- [x] 2.2a action 内部维护 `menuInstance: ReturnType<typeof mount> | null` 引用；新右键触发时 SHALL 先 unmount 旧 instance 再 mount 新 instance，确保 body 末尾同时仅有 ≤ 1 个菜单 instance
- [x] 2.3 action 内挂 `mousedown`（smart-select 防护）：`e.button === 2` 且 `window.getSelection()?.toString().length === 0` 时 `preventDefault`
- [x] 2.4 action 内挂键盘 `keydown` 监听 Menu 键（`e.key === "ContextMenu"`）/ Shift+F10：触发 `contextmenu` 事件并定位到 trigger bbox 中心
- [x] 2.5 action update 钩子：provider 变化时刷新内部引用（不重挂 listener）
- [x] 2.6 action destroy 钩子：移除所有 listener；调 `unmount(menuInstance)` 兜底清理任何残余菜单 DOM（防 trigger 元素被 Svelte 移除时菜单残留）

## 3. 全局兜底 `installGlobalContextMenuFallback`

- [x] 3.1 在 `ui/src/lib/contextMenu.svelte.ts` 内导出 `installGlobalContextMenuFallback()`，使用 module-level `boolean` flag 保证幂等
- [x] 3.2 内部注册 `window.addEventListener('contextmenu', handler, { capture: false })`
- [x] 3.3 handler 逻辑：`e.target.closest('input, textarea, [contenteditable="true"], [data-allow-native-context]')` 命中即 return；`e.defaultPrevented` 为 true 即 return；其余情况 `e.preventDefault()`
- [x] 3.4 在 `ui/src/main.ts` 启动序列内调用 `installGlobalContextMenuFallback()`，确保在 Svelte mount 之前

## 4. 重构 `SessionContextMenu` / `TabContextMenu` 复用 AppContextMenu

- [x] 4.1 修改 `ui/src/components/SessionContextMenu.svelte`：内部改用 `<AppContextMenu items={...} ...>` 渲染，自身只负责把 props（sessionId / isPinned / canSplit / onOpen* / onToggle*）映射成 `ContextMenuItem[]`；保留外部 API 兼容
- [x] 4.2 修改 `ui/src/components/TabContextMenu.svelte`：同上模式，items 映射 closeTab / closeOthers / 等
- [x] 4.3 修改 `ui/src/components/Sidebar.svelte::1020`：把 `oncontextmenu={(e) => onContextMenu(e, session)}` 改用 `use:contextMenu={(e) => buildSessionItems(session)}` 形式，删除 `ctxMenu = $state(...)` 状态；外部消费者现有 `<SessionContextMenu>` 渲染逻辑改为由 action 内部 portal mount
- [x] 4.4 修改 `ui/src/components/TabBar.svelte::120`：同上，改用 `use:contextMenu`
- [x] 4.5 验证 sidebar / tab 两路右键菜单 visual / 行为与重构前一致（手测 + 已有 vitest / playwright 测试）

## 5. CSS 兜底 `Sidebar.svelte::.session-item`

- [x] 5.1 在 `Sidebar.svelte::1512` `.session-item` CSS 块加 `user-select: none; -webkit-user-select: none`（双保险，独立修截图同款 bug）
- [x] 5.2 验证手测：在 sidebar 会话项的 worktree chip 文字上右键，无任何文字被自动选中

## 6. 测试

- [x] 6.1 vitest 单测 `contextMenu.svelte.ts::installGlobalContextMenuFallback` 三态决策（白名单元素放行 / `defaultPrevented` 跳过 / 兜底 preventDefault）
- [x] 6.2 vitest 单测 `use:contextMenu` smart-select 防护（mock `window.getSelection`，验证有/无选区分支）
- [x] 6.3 vitest 单测 `AppContextMenu` 键盘 ↑↓ Enter Esc 行为（jsdom DOM 事件 + activeIndex assertion）：覆盖 (a) 打开后 focus 进第一项；(b) ↑↓ 经过 `aria-disabled` 不跳过；(c) Enter / Space 在 enabled 触发 + 在 `aria-disabled` no-op；(d) 鼠标 hover 同步 activeIndex
- [x] 6.3a vitest 单测 portal 行为：mount 后 `document.body.lastElementChild` 是菜单根节点；新右键替换 instance 后 body 末尾仍 ≤ 1 个菜单；unmount 后 body 不留菜单 DOM
- [x] 6.3b vitest 单测 `installGlobalContextMenuFallback` HMR 幂等：重复调用时 window 上 contextmenu listener 数量保持 1（用 `addEventListener` spy）
- [x] 6.4 Playwright e2e：右键 Sidebar 会话项 → 看到 app 菜单 + 无 smart-select；右键空白处 → 无菜单；右键 `<input>` → 浏览器原生菜单（用 `page.evaluate` 检测 dialog/menu 存在性）；选中会话项后按 Menu 键（或 Shift+F10）→ 同样的 app 菜单弹出在元素 bbox 中心
- [x] 6.5 现有 sidebar / tab 右键菜单 e2e 测试 SHALL 保持通过（回归保护）

## 7. 自验

- [x] 7.1 `pnpm --dir ui run check` 通过（svelte-check 0 errors）
- [ ] 7.2 `cargo tauri dev` 启动桌面 app 手测：(a) 任意空白区右键无 Reload / Look Up 菜单；(b) sidebar 会话项右键弹 app 菜单且文字不被选中；(c) TabBar 标签右键弹 app 菜单；(d) `<input>` 右键弹浏览器输入菜单；(e) 选中文本右键不弹任何菜单（Phase 2 才加文本菜单，Phase 1 内符合预期）；(f) 键盘 Menu 键在 sidebar 会话项上触发同款菜单
- [ ] 7.3 dark 主题手测：菜单 token 在深色主题下视觉合规（border / shadow 在 dark surface 下足够分离）

## 8. DESIGN.md 同步

- [x] 8.1 archive 前跑 `/impeccable extract`，把 `AppContextMenu` 沉淀为 DESIGN.md `## 5. Components::### Context menu` 子节
- [x] 8.2 在 DESIGN.md `## 2. Colors::### Named Rules` 增补 **The App Owns the Right-Click Rule.** 命名规则（详见 design.md::Visual Contract::DESIGN.md delta plan）

## 9. 二审

- [x] 9.1 codex design 二审（按 `.claude/rules/codex-usage.md` 第 3 节，本 change 命中"UI 重构 / 跨 surface" + 状态机/键盘 a11y）：propose 完成后立即调
- [x] 9.2 codex 反馈应对：design / spec / tasks 三处同步修正再 re-validate

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex PR 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

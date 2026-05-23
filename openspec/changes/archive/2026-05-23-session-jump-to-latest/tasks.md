# tasks

## 1. spec delta + design 落地

- [ ] 1.1 `proposal.md` / `design.md` / `tasks.md` / `specs/session-display/spec.md` 落地
- [ ] 1.2 `openspec validate session-jump-to-latest --strict` 通过
- [ ] 1.3 codex design 二审通过（含新 Named Rule + spec scenario，按 `.claude/rules/codex-usage.md` 第 3 节强制）

## 2. apply：UI 实现（SessionDetail.svelte）

- [ ] 2.1 `ui/src/lib/icons.ts` 新增 `iconChevronsDown`（lucide path）
- [ ] 2.2 `ui/src/lib/platform.ts` 确认有 `isMac()` helper（不存在则补，仅这一个改动 + 同 PR 落）
- [ ] 2.3 `SessionDetail.svelte` 新增常量 `JUMP_THRESHOLD = 300` 与 `PROG_SCROLL_FALLBACK_MS = 1500`
- [ ] 2.4 `SessionDetail.svelte` 新增 `isFar` derived（`scrollTop + clientHeight < scrollHeight - JUMP_THRESHOLD`）+ scroll 监听 rAF 节流（passive listener）
- [ ] 2.5 `SessionDetail.svelte` programmatic-scroll 状态机（D-V4）：
  - [ ] 2.5.1 `isProgrammaticScroll: $state` + `progScrollTimer: number | null`
  - [ ] 2.5.2 `startProgrammaticScroll()`：set flag = true + clear 旧 timer + setTimeout 兜底
  - [ ] 2.5.3 `stopProgrammaticScroll()`：clear flag + clearTimeout
  - [ ] 2.5.4 `scrollend` 事件监听（主终止条件）
  - [ ] 2.5.5 scroll listener 内距底 ≤ 16px 兜底提前 stopProgrammaticScroll
  - [ ] 2.5.6 `wheel` / `touchmove` 监听（passive）→ stopProgrammaticScroll
  - [ ] 2.5.7 handleKeydown 内"非本快捷键 keydown" → stopProgrammaticScroll
- [ ] 2.6 `SessionDetail.svelte` 新增 `scrollToLatest()`：检测 `prefers-reduced-motion: reduce` 切 behavior `'auto' | 'smooth'`，调 `conversationEl.scrollTo({ top: scrollHeight, behavior })`，调用 `startProgrammaticScroll()`
- [ ] 2.7 `SessionDetail.svelte::handleKeydown` 扩展（D-V3）：
  - [ ] 2.7.1 input/textarea/contenteditable focused → return 不拦
  - [ ] 2.7.2 **active pane guard**（codex P1#2 修法）：`getActiveTabId() !== tabId` → return 不拦（多 pane 多 SessionDetail mount 场景仅 focused pane 拦截）
  - [ ] 2.7.3 mac `⌘+ArrowDown` / 非 mac `Ctrl+End` 拦截，preventDefault + 调 scrollToLatest
- [ ] 2.8 `SessionDetail.svelte` 渲染浮动按钮：inline `<button>` 形态，含 `aria-label` + 平台分流 `title`
- [ ] 2.9 ContextPanel 让位（D-V Surface Decision，codex P2#3 修法）：
  - [ ] 2.9.1 `app.css` 全局 CSS var `--session-jump-button-right`：默认 `16px`
  - [ ] 2.9.2 SessionDetail 在 `contextPanelVisible` 切 conversation 容器 class `.has-context-panel`，CSS 内 `.has-context-panel { --session-jump-button-right: calc(min(320px, 50vw) + 16px); }`
  - [ ] 2.9.3 按钮 CSS `right: var(--session-jump-button-right)`
- [ ] 2.10 CSS：默认 / hover / focus-visible / pressed / programmatic-scrolling-suppressed 五态 + dark theme + reduced-motion 降级
- [ ] 2.11 **cleanup**（codex P2#4 修法）—— `{@attach}` 绑 conversation 容器，cleanup 函数中：
  - [ ] 2.11.1 `removeEventListener('scroll', ...)` + `removeEventListener('scrollend', ...)` + `removeEventListener('wheel', ...)` + `removeEventListener('touchmove', ...)`
  - [ ] 2.11.2 `cancelAnimationFrame(rAF id)`
  - [ ] 2.11.3 `clearTimeout(progScrollTimer)`
  - [ ] 2.11.4 keydown listener 仍走 `document.addEventListener` + `onDestroy` `removeEventListener`（与既有 `Cmd+F` 模式对齐）
- [ ] 2.12 看代码量决定是否抽 `JumpToLatestButton.svelte`（按 D-V6：≥ 80 行 inline 就抽）

## 3. apply：DESIGN.md delta 同 PR 落地

- [ ] 3.1 `DESIGN.md` Section 5 末尾新增 `### Floating affordances` 子节（贴 `design.md::Visual Contract::DESIGN.md delta plan` 内容）
- [ ] 3.2 `DESIGN.md` Section 5 末尾新增 Named Rule `**The Floating Is Affordance, Not Decoration Rule.**`

## 4. apply：测试

- [ ] 4.1 `ui/src/routes/SessionDetail.test.svelte.ts`（或扩展既有）：
  - [ ] 4.1.1 距底 ≤ 300px 不显示按钮
  - [ ] 4.1.2 距底 > 300px 显示按钮
  - [ ] 4.1.3 点击按钮触发 `scrollTo` 调用（断言入参 `top = scrollHeight, behavior = 'smooth'`）
  - [ ] 4.1.4 programmatic-scroll 期间不重显（scroll 事件触发不让按钮重新可见，直到 scrollend 或用户输入）
  - [ ] 4.1.5 scrollend 事件清除 isProgrammaticScroll
  - [ ] 4.1.6 wheel / touchmove 事件清除 isProgrammaticScroll（用户打断 smooth scroll）
  - [ ] 4.1.7 重复点击按钮 clearTimeout 旧 fallback timer，新 scroll 不被旧 timer 提前清
  - [ ] 4.1.8 距底 ≤ 16px 兜底清除 isProgrammaticScroll
- [ ] 4.2 `ui/tests/e2e/session-jump-to-latest.spec.ts`（新建 / 复用 fixture 长会话）：
  - [ ] 4.2.1 长会话浏览器渲染下用户上滚 → 按钮浮现
  - [ ] 4.2.2 点击按钮回底（assertion 距底 ≤ 16px）
  - [ ] 4.2.3 mac 端模拟 `Cmd+ArrowDown` 触发跳底（page.evaluate 注入 `Object.defineProperty(navigator, 'platform', { value: 'MacIntel' })` 或用 page.keyboard.press 跨平台测）
  - [ ] 4.2.4 Win/Linux `Ctrl+End` 触发跳底
  - [ ] 4.2.5 SearchBar focused 时按 `Cmd+↓` 不拦截（光标行为不变，conversation 不滚）
  - [ ] 4.2.6 reduced-motion emulate 下进出立即 + scroll behavior auto
  - [ ] 4.2.7 多 pane 场景仅 focused pane 内 SessionDetail 拦截快捷键（split pane 后 focus 在右 pane → 按 `⌘+↓` 仅右 pane 滚到底，左 pane 视口位置不变）
  - [ ] 4.2.8 ContextPanel 打开时按钮 right offset 视觉回归（截图对比 / `getBoundingClientRect().right` 与 conversation right 边距）
- [ ] 4.3 `pnpm --dir ui run check`（svelte-check 0 errors）
- [ ] 4.4 `pnpm --dir ui exec playwright test session-jump-to-latest.spec.ts` 全绿
- [ ] 4.5 `just test-ui-unit` 全绿

## 5. apply：手动桌面 smoke

- [ ] 5.1 `just dev` 启动 Tauri 桌面端 + 长会话 fixture（数千 chunk）
- [ ] 5.2 三态 smoke：上滚 → 按钮浮现 / 点击回底 / `⌘+↓` 回底（mac）
- [ ] 5.3 ContextPanel 打开时按钮让位检查
- [ ] 5.4 浅色 + 深色 + system 三主题视觉回看
- [ ] 5.5 reduced-motion 系统设置开启下行为正确

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

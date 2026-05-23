## 1. tabStore schema 改动（ui）

- [x] 1.1 `ui/src/lib/tabStore.svelte.ts::TabUIState` interface：删除 `scrollTop: number` 字段，新增 `atBottom: boolean` / `anchorChunkId: string | null` / `anchorOffsetPx: number`
- [x] 1.2 `getTabUIState` 默认值更新：`atBottom: false` / `anchorChunkId: null` / `anchorOffsetPx: 0`
- [x] 1.3 grep `ui/src` + `ui/tests` 范围内对 `TabUIState.scrollTop` / `uiState.scrollTop` / `getTabUIState(...).scrollTop` / `saveTabUIState({ scrollTop })` 的硬编码消费，全部迁移到新三件套或在 1.4 / 3.x 中重写

## 2. SessionDetail 锚点法实现（ui）

- [x] 2.1 `ui/src/routes/SessionDetail.svelte`：删除 `latestScrollTop` 变量；新增 `latestAnchor: { atBottom: boolean; anchorChunkId: string | null; anchorOffsetPx: number }` 由 scroll listener 同步维护
- [x] 2.2 实现 `captureScrollAnchor()` 纯函数（按 `design.md::D2` 算法）
- [x] 2.3 实现 `isAtBottom(el): boolean` helper（共享给 `refreshDetail` 与 `captureScrollAnchor`），替换原 `scrollTop + clientHeight >= scrollHeight - 16` 散落判断
- [x] 2.4 实现 `restoreScrollAnchor(state)`：分支 `atBottom` / `anchorChunkId` 命中 / 兜底 console.warn 三路
- [x] 2.5 实现 `startBottomPin()` 状态机：`MutationObserver(conversationEl, { subtree: true, attributes: true, attributeFilter: ['data-rendered'], childList: true, characterData: true })` + scroll listener + 200 ms 稳定 timer + 5 s 上限超时；返回 cleanup
- [x] 2.5.1 cleanup 调用契约：SessionDetail 持有 `currentBottomPinCleanup: (() => void) | null`；自终止路径内置 cleanup 后置 null；新一轮 `restoreScrollAnchor` 启动 pin 前 SHALL 强制收敛上一轮；`onDestroy` SHALL 兜底调 cleanup 防 MutationObserver / timer 泄漏
- [x] 2.6 `onMount` cached path（`detail = cached` 后 `await tick()`）与非 cached path（getSessionDetail 返回后 `await tick()` 已存在于 try/finally 后）二者 await tick 之后**均**调 `restoreScrollAnchor(uiState)` 替代原 `if (conversationEl && uiState.scrollTop > 0) scrollTop = uiState.scrollTop`；非 cached path 即使 IPC 失败（error 路径）也不调 restore（loading 状态 conversationEl 未 mount）
- [x] 2.7 `onDestroy`：保留 `getTabSessionId(tabId) === sessionId` guard；调用 `saveTabUIState(tabId, { ...latestAnchor, ...其他字段 })`
- [x] 2.8 chunk 渲染处统一注入 `data-chunk-id={chunk.chunkId}`：UserChunk / AIChunk / SystemMessage / Compact 四个分支顶层 `<div>`（key 已是稳定 chunkId，不需新 ID schema）

## 3. 测试覆盖（ui）

- [x] 3.1 `ui/src/components/SessionDetail.test.svelte.ts`：新增 vitest 用例覆盖 `captureScrollAnchor()` 选锚点逻辑（mock conversation rect + 多 chunk rect，验证选第一个 `bottom > containerTop` 的 chunk）
- [x] 3.2 vitest 覆盖 `isAtBottom` 阈值（distanceFromBottom = 0/16/17 三个边界）
- [x] 3.3 vitest 覆盖 `restoreScrollAnchor` 兜底降级（anchorChunkId 在 DOM 内找不到 → console.warn + scrollTop 不变）
- [x] 3.4 `ui/tests/e2e/tab-scroll-preserve.spec.ts` 重写为三场景，**所有 anchorChunkId / atBottom 字段断言均直接读 `tabStore` 状态**（不再硬编码 `scrollTop`）：
  - (a) 滚到底部切走切回仍在底部（assert `distanceFromBottom <= 16`）
  - (b) 滚到中间位置切走切回相同 chunk 视口顶偏差 ≤ 50 px（assert `anchor.getBoundingClientRect().top - container.getBoundingClientRect().top` 与切走时差 ≤ 50）
  - (c) `anchorChunkId` 失效降级（手动 `saveTabUIState(tabId, { ..., anchorChunkId: 'nonexistent' })` 后切回，assert 视口在顶 + console.warn 命中）
  - 不再设"反转锚点法"伪测试（手工 mutation 不可在 CI 自动化）；回归保护交给 vitest 单测覆盖 `captureScrollAnchor` 行为

## 4. 收尾清理

- [x] 4.1 PR 描述写明 PR #223 known limitation（"如需精确恢复，可在恢复后用 ResizeObserver 监听 sH 变化重设 scrollTop——独立 PR 处理"）已由本 PR 关闭——SessionDetail.svelte 内本身没有 known limitation 注释，无需 grep 删除
- [x] 4.2 `openspec/followups.md` 若有"PR #223 滚动恢复 / tab-management scroll restore"相关条目 → 标 ✅ 附 commit hash；当前已确认无该条目，archive 时再 verify 一次
- [x] 4.3 `just preflight` 全绿（fmt / lint / 全 vitest / Playwright e2e / spec validate / archive check / IPC sync）

## N. 发布

- [x] N.1 push 分支 + 开 PR（`fix/tab-scroll-restore-anchor`）
- [ ] N.2 wait-ci 全绿（与 N.3 并行启动）
- [ ] N.3 codex 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

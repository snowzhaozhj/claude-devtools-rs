## 1. UI 修改（capability: sidebar-navigation）

- [x] 1.1 新增 `ui/src/lib/sessionMerge.ts`：含 `mergeSessions(prev, next, sort)` 与 `mergeSilentMetadata(prev, next)`（从 `Sidebar.svelte` 提取，保持函数签名与行为完全一致），并新增 `applySilentRefresh(prev, prevCursor, firstPageItems): { sessions, nextCursor }`——把"silent 合并尾部 + 保留 cursor"两件事封装为单一可单测的纯函数
- [x] 1.2 `ui/src/components/Sidebar.svelte`：删掉组件内私有 `mergeSessions` / `mergeSilentMetadata`，改 import 自 `../lib/sessionMerge`
- [x] 1.3 `ui/src/components/Sidebar.svelte::loadSessions` silent 分支：调 `applySilentRefresh(sessions, sessionsNextCursor, result.items)` 得到 `{ sessions: fresh, nextCursor }`，最后赋值 `sessions = fresh + reconcile; sessionsNextCursor = nextCursor`；非 silent 分支保持原行为（`fresh = result.items; sessionsNextCursor = result.nextCursor`）
- [x] 1.4 验证 `refreshDeferredSessions`（line 285）与 file-change `scheduleRefresh` callback 都走同一 silent 路径——两类入口同时获得修复，无需额外改动

## 2. UI 测试（vitest）

- [x] 2.1 新增 `ui/src/lib/sessionMerge.test.ts`，纯单测覆盖 spec delta 三个新 Scenario + 一个回归 Scenario（无需 mockIPC）：
  - `applySilentRefresh` 后 prev 中超出第一页的尾部 sessions 全部保留（`result.sessions.length ≥ prev.length`，对应 Scenario "silent 刷新保留尾部已翻页 sessions"）
  - `applySilentRefresh` 后 `result.nextCursor === prevCursor`，与 `firstPageItems.nextCursor` 无关（对应 Scenario "silent 刷新不重置分页 cursor"）
  - `applySilentRefresh` 后 prev 中已 patch 元数据（title 非 null）的 session 在合并后保留元数据，新骨架的 `title=null` 不会覆盖（既有 Scenario "file-change silent 刷新保留已获取元数据" 回归）
  - `mergeSessions` 排序稳定：构造 prev 中 timestamp 相同的两条 session，合并后顺序保持
  - `applySilentRefresh` 后 prev 中所有 sessionId（含 prev 后部分页内容）SHALL 仍存在于 `result.sessions`，覆盖 Scenario "silent 刷新不丢失任何 prev sessionId"
- [x] 2.2 e2e 覆盖（可选）：若 Playwright 已有 sidebar 翻页 fixture，加一条 spec 用 `window.__cdtTest` API 模拟 silent refresh，断言 sidebar 计数不跳变；判断不便加 e2e 时跳过本步骤

## 3. preflight 与验证

- [x] 3.1 `npm run check --prefix ui`（svelte-check + tsc）
- [x] 3.2 `npm run test:unit --prefix ui -- --run`（vitest 新单测全绿）
- [x] 3.3 `openspec validate sidebar-silent-refresh-preserves-pagination --strict`
- [x] 3.4 `just preflight`（fmt + lint + test + spec-validate）
- [ ] 3.5 手动 verify：`just dev` 起桌面应用 → 选一个会话多的项目 → 翻页到 60 条 → 等 file-change 触发或在 Claude 会话里发消息触发后端推送 → 观察 sidebar 计数 `60/60` 不跳变（merge 后由用户在主分支跑，worktree 内不便）

## 4. codex 二审 + commit + PR + archive

- [x] 4.1 design + spec delta 阶段 codex 二审（CLAUDE.md 行为契约改动硬约束）
- [x] 4.2 实现 commit，PR 描述列改动 + 测试覆盖 + 性能数据
- [x] 4.3 实现 commit push 后 codex 二审（默认）；修完所有发现的问题后第二轮 codex 验证
- [x] 4.4 archive commit 作为 PR 最后一个 commit：`openspec archive sidebar-silent-refresh-preserves-pagination -y`
- [ ] 4.5 merge PR

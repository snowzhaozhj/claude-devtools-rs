## Context

`Sidebar.svelte::loadSessions(projectId, silent: boolean)` 是当前项目会话列表的统一加载入口，有两类调用路径：

- **非 silent**：用户切 project（`$effect` 监听 `selectedProjectId`）；首次进入应用。语义：替换式加载第一页。
- **silent**：file-change 推送（活跃会话每秒可多次）、用户点击"有更新"按钮。语义按 spec line 333 "file-change silent 刷新保留已获取元数据"是"保留 metadata"，但**未明确**是否保留尾部已翻页 sessions。

当前实现：

```svelte
async function loadSessions(projectId: string, silent = false) {
  // ...
  const result: PaginatedResponse<SessionSummary> = await listSessions(projectId, SESSION_PAGE_SIZE);
  // ...
  let fresh = silent ? mergeSilentMetadata(sessions, result.items) : result.items;
  fresh = await reconcilePinnedAndHidden(projectId, fresh);
  // ...
  sessions = fresh;
  sessionsNextCursor = result.nextCursor;  // ← silent 路径也重置
}
```

`mergeSilentMetadata(prev, next)` 实现是 `next.map(...)`——返回数组的长度就是 `next.length`（第一页 20 条）。silent 路径下 `sessions` 被截短，`sessionsNextCursor` 被回退到第一页 cursor。

用户感知：
1. 计数 `{visibleSessions.length}/{totalSessions}` 翻页深时频繁跳变（60 → 20 → 40 → 60）
2. scrollTop 已超出截短列表的 scrollHeight，浏览器钳制 scrollTop 或 sessions 列表锚定的会话消失，视觉锚点错位
3. 点击"有更新"也走同一路径，"跳转的锚点不对"是同一根因的另一表现

性能背景：`listSessions` IPC 走异步骨架协议（list_sessions 返回 title=null 的骨架，后台 task 通过 `subscribe_session_metadata` 推送 patch），第一页 20 条骨架结果体积很小（~2KB），合并 60 条 sessions 是纯前端操作。

## Goals / Non-Goals

**Goals:**
- silent 刷新不改变 `sessions.length`（除非真正有新增 session）
- silent 刷新不改变 `sessionsNextCursor`，用户已翻到的分页位置保留
- silent 刷新仍按 spec line 333 保留 prev 已 patch 元数据
- 复用现有 `mergeSessions` helper，不引入新合并逻辑
- 性能：silent 路径不引入可感延迟（N=60 量级 O(N log N) sort）

**Non-Goals:**
- 不改非 silent 路径行为（切 project / 首次加载仍替换）
- 不改 file-change 节流链（`fileChangeStore::scheduleRefresh` 的 250ms trailing debounce 不动）
- 不处理"后端真的删了一条 session"导致的尾部幽灵 session（已存在的边缘场景，超出本 change 范围）
- 不改 `session-metadata-update` 增量 patch 通道（与 silent refresh 是两条独立通路）
- 不引入"点击有更新自动 scrollTo(0)"行为（spec line 541 只说"再刷新第一页"，不说滚动；保留 scrollTop 是 spec line 365 已规定的）

## Decisions

### D1: silent 路径复用 `mergeSessions(prev, next, sort=true)` 而非另写

**候选方案：**

- **A. 复用 `mergeSessions(prev, next, true)`**（采用）：
  - 内部已调 `mergeSilentMetadata([old], [item])[0]` 处理"silent 拉到骨架但 prev 已 patch"
  - `merged = [...prev]` 初始化保留尾部
  - `sort=true` 处理 prev 中元数据更新后位置变化（Y 因新消息时间戳变新 → 排到前 20）
  - 单一合并入口，silent 与 `loadMoreSessions` 共用合并逻辑

- **B. 新加 silent-only helper `mergeSilentSessions`**：
  - 与 A 行为一致但语义更"显式"
  - 多一个 helper 维护成本，且与 `mergeSessions` 99% 重复

- **C. 只 patch metadata 不动列表**：
  - 严格按 spec line 333 字面"merge 元数据"，silent 刷新不增加新 session
  - 但 silent 的本意之一是"用户在浏览历史时如果有新 session 出现，回顶后能看到"——spec line 540 "Sidebar SHALL 显示有更新提示，直到用户回到顶部或点击提示后再刷新第一页"暗示 silent 应该拉新页（至少第一页内有新 session 时）

**取舍：** A 最干净。`mergeSessions` 已经处理三类合并：(1) prev 中存在 → metadata 保留 + 位置可能变；(2) prev 中不存在 → 追加；(3) prev 尾部超出 next 范围 → 保留。三类正好覆盖 silent 需求。

### D2: silent 路径不重置 `sessionsNextCursor`

**候选方案：**

- **A. silent 路径完全不动 `sessionsNextCursor`**（采用）：保留用户已翻到的位置；下一次 `loadMoreSessions` 仍用 prev cursor 拉"用户未看过的下一页"。
- **B. silent 路径用 `result.nextCursor`**（当前 bug 行为）：cursor 回退到第一页之后；用户滚动会重新加载第二页（其实就是已经在 sessions 数组里的 session 重复拉一遍）→ `mergeSessions` 去重后 sessions.length 不变，但白白浪费一次 IPC + 一次合并。
- **C. silent 路径取 `max(prev cursor, result.nextCursor)`**：复杂且无意义——cursor 是 opaque token，不可比较。

**取舍：** A 简单且正确。"silent 刷新 = 已知列表的元数据更新 + 可能新增的最前面 sessions"，不应该影响"下一页"的概念。

### D3: 不在 silent 完成后 scroll-to-top

**候选方案：**

- **A. 不自动滚动**（采用，符合 spec line 365 "scrollTop 不重置"）
- **B. silent 刷新后 sessionListEl.scrollTo({ top: 0 })**：与 spec line 365 冲突；用户在 browsingHistory 状态下点"有更新"也不期望被强制拽回顶部（spec line 541 只说"刷新第一页"）。

**取舍：** A。如果将来需要"点有更新 = scrollTo(0) + refresh"的产品语义，应当再走一次 openspec 改 spec line 541 + 365，不在本 change 里塞。

### D5: 抽 `applySilentRefresh(prev, prevCursor, firstPageItems)` 纯函数 helper（codex design 二审补丁）

**起因：** 初版 tasks.md 计划用纯 `mergeSessions` 单测覆盖"cursor 不重置"。codex 二审指出：cursor 决策（赋值在 `Sidebar.svelte::loadSessions` 内部）与 `mergeSessions` 解耦，单测无法触达。

**候选方案：**

- **A. 抽 `applySilentRefresh(prev, prevCursor, firstPage): { sessions, nextCursor }`**（采用）：把 silent 路径的"合并尾部 + 保留 prev cursor"封装为单一纯函数；单测直接构造 prev/cursor/firstPage 验证返回值
- **B. 加 mockIPC 级 loadSessions 集成测试**：能测但触达不到边界（mockIPC 受 vite optimizer cache 污染历史，比纯单测脆）
- **C. 把 silent 决策保留在 Sidebar.svelte 内部**：单测覆盖不到关键约束 = 留风险

**取舍：** A。helper 是 5 行纯函数，单测覆盖 100% 决策路径。同时把组件内私有 `mergeSessions` / `mergeSilentMetadata` 一并提取到 `ui/src/lib/sessionMerge.ts`，单测同时覆盖既有合并语义的回归。

### D4: 不动 `mergeSilentMetadata` helper

**候选方案：**

- **A. 保留 `mergeSilentMetadata`**（采用）：它仍然被 `mergeSessions` 内部调用（line 205），承担"用 prev 元数据填充 next 骨架"的单一职责。silent 路径直接调 `mergeSilentMetadata` 的调用点被 `mergeSessions` 取代，但 helper 本身保留，避免改动扩散。
- **B. 删除 `mergeSilentMetadata` 内联到 `mergeSessions`**：增加一处改动，无功能收益。

**取舍：** A。最小改动面。

## Risks / Trade-offs

- **Risk**：后端 list_sessions 返回的第一页结果与 prev sessions 的"前 20 条"sessionId 集合可能差异（因为 prev 的元数据 patch 使某些 session timestamp 变化）。`mergeSessions` 用 `sort=true` 按 timestamp 排序，最终列表顺序与 prev 可能略有变化。
  - **Mitigation**：spec line 547 "Sidebar SHALL NOT 因追加新页而重新排序已加载历史列表" 针对的是"加载更多"路径（`loadMoreSessions`，sort=false），与 silent 路径是两个 Requirement，行为独立。silent 路径 sort 是为了"prev 第 30 位的 session 因新消息冒到前 20" 的场景——这是用户期望的实时刷新效果。

- **Risk**：silent 拉到的第一页可能不包含 prev 中已存在的 sessionId 也不属于 reconcilePinnedAndHidden 范围的 session（边缘场景：用户清了某条 session，后端 list_sessions 不再返回它）。当前 `mergeSessions(prev, next)` 保留 prev 尾部，导致已删除的 session 仍显示。
  - **Mitigation**：这是已存在的边缘场景（用户主动删 session 后 sidebar 不实时去除），不在本 change 范围。spec 也未规定 silent 刷新需做 prev 清理。下次用户切 project / 应用重启自然消失。

- **Trade-off**：silent 路径多一次 `Map` 构造（O(N)）+ 一次 `Array.sort`（O(N log N)，N=60 约 60×log₂60 ≈ 350 比较）。在 250ms 节流窗口内不连发，每次刷新百微秒级，无可感延迟。

## Migration Plan

无破坏性 / 无数据迁移。代码改动 ~5 行 + 1 个新测试文件。回滚即恢复 silent 替换路径。

## Why

PR-A（合并 commit `908aee9`）落地了 sidebar 会话列表区块的纯视觉重排（meta 行 grid、worktreeName 行内 label、删 branch / cwd 字段、Memory entry 紧凑、pin icon 简化、date label 加计数、加载更多按钮三态）。但当时识别为「行为契约级」的两块改动留给本 PR-B 走 openspec 路径：

1. **Worktree filter 形态**：当前 sidebar-navigation spec D6 写明「顶部渲染 worktree filter dropdown」，多 wt group 时是个孤立的 macOS 风格 select，与 sidebar 整体「调试工作台」语言脱节；用户切 worktree 要"打开 → 选 → 关闭"两步交互，且只能看到当前选中值，不能扫到所有 wt 名。chip cluster 风格能让所有 wt 一眼可见 + 一次点击切换。
2. **Session count 显示口径**：当前实现 `Sidebar.svelte:822` 仅渲染 `{visibleSessions.length}` 单数字（"5"），用户读不出含义；hover tooltip 才显 `可见 5 / 总 127`。spec 现状写 `{visibleSessions.length}/{totalSessions}` 但 visibleSessions 受 filter / hide 影响，单数字脱离 hide/search 上下文表达不准。本 PR 改成「默认单数字 = 当前 scope 总量；搜索激活 = 命中数」双态，hover tooltip 暴露完整含义；分页进度由 PR-A 已落地的底部 `▼ 加载更多 · 剩 N 条` 按钮承担——用户不感知客户端分页内部状态，顶部 count 不必表达。

## What Changes

- **MODIFIED** Requirement: `Worktree filter dropdown for multi-worktree group` —— 改名为 `Worktree filter chip cluster for multi-worktree group`；filter 控件从 `Dropdown.svelte size="sm"` 改为新建子组件 `WorktreeChipCluster.svelte`：横向 flex，每个 wt 一个 chip，"全部" chip 在最前默认选中，chip 数量过多时容器横向滚动；chip 视觉语言与 PR-A meta 行 wt label 同色族（mono + muted），选中态用 `--color-surface-overlay` + `border-emphasis` 表达（沿用 PR #146 已固化的"持久选中是 quiet"规则）。
- **MODIFIED** Requirement: `会话总数显示口径` —— 从「显示 `{visibleSessions.length}/{totalSessions}` 单一形式 + 现状代码退化为单数字」改为 **双态显示 + 跟 filter scope 走**：
  - 默认（无搜索）：单数字 `{scopeTotal}`，`scopeTotal` 跟 worktree filter 走（ALL → `selectedGroup.totalSessions`；具体 wt → `wt.sessions.length`）；分页加载进度由 sidebar 底部 `▼ 加载更多 · 剩 N 条` 按钮承载（PR-A 已落地），顶部 count 不再表达分页（用户不感知客户端分页内部状态）
  - 搜索激活：`{matchCount} 匹配`（filterQuery 非空时聚焦命中数）
  - hover tooltip：基础一层 `总 127`；`hiddenCount > 0` 时追加 `· 5 已隐藏`，`hiddenCount === 0` 时仅显一层
- 新建 `ui/src/lib/components/WorktreeChipCluster.svelte` 子组件（chip cluster + horizontal scroll + 单选状态机）
- `Sidebar.svelte` 替换 `Dropdown` 引用为 `WorktreeChipCluster`，并改 `session-count-num` 渲染口径

## Capabilities

### New Capabilities
（无）

### Modified Capabilities
- `sidebar-navigation`: 修改两个既有 Requirement
  - `Worktree filter dropdown for multi-worktree group` → 控件形态从 dropdown 改为 chip cluster + 标题更名
  - `会话总数显示口径` → 显示改为双态（默认单数字 `{scopeTotal}` / 搜索 `{matchCount} 匹配`）+ tooltip 单层 + 条件 hidden

## Impact

**前端**：
- 新建 `ui/src/lib/components/WorktreeChipCluster.svelte`（~80 行 Svelte 5 + CSS）
- `ui/src/components/Sidebar.svelte` 改 ~30 行：删 Dropdown 引用、引入 WorktreeChipCluster、改 session-count-num 渲染逻辑（双态 `{scopeTotal}` / `{matchCount} 匹配`）、加 `scopeTotal` derived（按 worktreeFilter 派生 group total / wt total）
- 视觉验收依赖 mock fixture `multi-project-rich`（已含多 wt group `mock-rich-rust` + 两 wt `rust-port`/`feat-x`）

**测试**：
- 新增 vitest 单测：`WorktreeChipCluster.test.svelte.ts`（chip 渲染顺序 + 单选切换 + 滚动行为）
- 更新 `Sidebar.test.svelte.ts`：count 双态断言（默认 `{scopeTotal}` / 搜索 `{matchCount} 匹配` / tooltip 单层 + 条件 hidden）+ 不再 mock Dropdown
- 更新 e2e `worktree-filter.spec.ts`（已存在的多 wt filter test）：从 dropdown selector 改为 chip cluster selector
- e2e `sidebar-collapse-and-branch.spec.ts`（PR-A 已更新）保持不变

**IPC / 后端**：无改动。filter 切换仍走既有 `list_group_sessions(groupId, pageSize, cursor)`，cursor 构造逻辑（worktreeFilter → buildFilterCursor）保持。`scopeTotal` 是纯前端派生（来自 `list_repository_groups` 已返回的 `RepositoryGroup.totalSessions` / `Worktree.sessions.length`），不涉及 IPC 字段新增。

**Spec fidelity**：本 change 同时把 `Sidebar.svelte:822` 单数字 `{visibleSessions.length}` 与 spec line 664 之间的脱节修正——之前 spec 写 `{visible}/{total}` 但实现退化到单数字，本 change 用 `{scopeTotal}` / `{matchCount} 匹配` 双态取代两边旧描述。

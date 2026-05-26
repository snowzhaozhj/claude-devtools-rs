## Why

`sidebar-navigation` 已积累到 36 个 Requirement / 168 个 Scenario，既包含项目导航、会话列表、会话操作，也混入 Tab 状态归属与 Worktree filter 等边界语义；当前按历史实现增量排列，reviewer 很难按用户行为找到契约 owner。issue #303 的阶段 3 需要先把 Sidebar 主 spec 重组为用户视角结构，并把明确属于 Tab 生命周期 / Tab identity 的 Scenario 迁到 `tab-management`，为后续 PR 继续拆分大 capability 降低风险。

## What Changes

- archive 同 commit 内重写 `sidebar-navigation` Purpose，使其从用户价值视角描述 Sidebar 负责的导航、列表、操作、形态和性能边界，不引入新的 SHALL/MUST 行为约束。
- archive 同 commit 内将 `sidebar-navigation` 的保留 Requirement 按用户行为重新分组排序：项目导航、会话列表、会话操作、列表性能、Worktree 多 group 切换、Sidebar 形态。OpenSpec active delta 仅表达可验证的跨 cap owner 迁移。
- 保持现有 Scenario 行为契约 100% 不变：每个保留或迁移的 Scenario 的 WHEN / THEN / AND / OR / NOT 子句字符级保持。
- 从 `sidebar-navigation` 处理 4 个 Tab owner 候选 Scenario：3 个迁入 `tab-management`，1 个不新增而由 `tab-management` 既有 `无 active tab 时 Sidebar 无高亮` Scenario 覆盖。
- 本 PR 不在 active delta 中批量清理 Scenario 标题；若 archive 同 commit 调整标题，仅限不改 Scenario 子句的用户视角命名。
- 不引入新 UI、代码、IPC 字段、Tauri command 或后端行为。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `sidebar-navigation`: active delta 移除归属 Tab 生命周期 / Tab identity 的 Scenario；archive 同 commit 将主 spec Purpose 与 Requirement 顺序改为用户行为视角，保留 Sidebar owner 行为契约。
- `tab-management`: 接收 Sidebar 点击打开 tab、高亮跟随 focused pane activeTab、session tab 使用 worktree id 打开 detail 的 Scenario，并用既有无 active tab 高亮 Scenario 覆盖同义行为，成为这些 Tab 状态行为的唯一 owner。

## Impact

- 仅影响 OpenSpec 文档：`openspec/changes/reorganize-sidebar-navigation/**`，archive 后同步到 `openspec/specs/sidebar-navigation/spec.md` 与 `openspec/specs/tab-management/spec.md`。
- 不改 Rust / Svelte / Tauri 代码，不改测试，不改依赖，不改 IPC / HTTP / SSE 协议。
- 需要同步 spec-purity baseline 中 `sidebar-navigation` 与 `tab-management` 的计数变化，确保 ratchet 不因重组误报。

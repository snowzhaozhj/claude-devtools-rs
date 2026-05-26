## Why

`sidebar-navigation` 已积累到 36 个 Requirement / 168 个 Scenario，既包含项目导航、会话列表、会话操作，也混入 Tab 状态归属与 Worktree filter 等边界语义；当前按历史实现增量排列，reviewer 很难按用户行为找到契约 owner。issue #303 的阶段 3 需要先明确 Sidebar 与 Tab 的 owner 边界，并把明确属于 Tab 生命周期 / Tab identity 的 Scenario 迁到 `tab-management`，为后续 PR 继续拆分大 capability 降低风险。

## What Changes

- archive 同 commit 内重写 `sidebar-navigation` Purpose，使其从用户价值视角描述 Sidebar 负责的导航、列表、操作、形态和性能边界，不引入新的 SHALL/MUST 行为约束。
- 将用户行为分组作为设计审计索引记录；archive 后不手工重排 `sidebar-navigation` 主 spec Requirement。
- 保持现有 Scenario 行为契约 100% 不变：迁入 `tab-management` 的 Scenario 允许把实现术语改写为用户可观察行为，但不改变 WHEN / THEN / AND 的语义。
- 从 `sidebar-navigation` 处理 4 个 Tab owner 候选 Scenario：3 个迁入 `tab-management`，1 个不新增而由 `tab-management` 既有 `无 active tab 时 Sidebar 无高亮` Scenario 覆盖。
- Scenario 标题调整必须通过 active delta 表达；archive 同 commit 的主 spec 直改仅限 Purpose 元描述。
- 不引入新 UI、代码、IPC 字段、Tauri command 或后端行为。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `sidebar-navigation`: active delta 移除归属 Tab 生命周期 / Tab identity 的 Scenario；archive 同 commit 仅在必要时更新主 spec Purpose，保留 Sidebar owner 行为契约。
- `tab-management`: 接收 Sidebar 点击打开标签页、高亮跟随当前窗格活跃标签页、会话详情按所属工作树加载的 Scenario，并用既有无 active tab 高亮 Scenario 覆盖同义行为，成为这些 Tab 状态行为的唯一 owner。

## Impact

- 仅影响 OpenSpec 文档：`openspec/changes/reorganize-sidebar-navigation/**`，archive 后同步到 `openspec/specs/sidebar-navigation/spec.md` 与 `openspec/specs/tab-management/spec.md`。
- 不改 Rust / Svelte / Tauri 代码，不改测试，不改依赖，不改 IPC / HTTP / SSE 协议。
- 需要同步 spec-purity baseline 中 `sidebar-navigation` 与 `tab-management` 的计数变化，确保 ratchet 不因重组误报。

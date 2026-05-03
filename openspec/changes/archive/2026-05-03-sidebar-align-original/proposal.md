## Why

本端口 Sidebar 在两处明显落后于 Electron 原版：(1) 看不到当前工作所在的 git 分支，原版 SidebarHeader Row 2 的 worktree 选择器可一眼看到分支名；(2) 不支持收起，原版有 `Cmd+B` 快捷键和 PanelLeft 按钮把 sidebar 整体折叠，让 chat 区域占满全宽。这两个高频可见性差距让从原版迁移过来的用户体验断层。本 change 一次性把它们补齐：加 git 分支只读栏 + 折叠/展开机制。

## What Changes

**git 分支只读栏**：
- `cdt-api::SessionSummary` 新增字段 `git_branch: Option<String>`（IPC camelCase: `gitBranch`），骨架返回时为 `None`
- `cdt-api::SessionMetadataUpdate` 同步加 `git_branch`，复用现有 `session-metadata-update` broadcast 通道一并推送
- 后端 metadata scan 在解析 session JSONL 时取**最后一条**非空 `git_branch`（与原版 `sessionExporter.ts` 取值方式一致）
- `sidebar-navigation` 加 Requirement：SidebarHeader 在项目名按钮下方渲染 git 分支栏，优先显示 `activeSessionId` 对应 session 的 `gitBranch`，无 active 时退回 `sessions[0]` 的 `gitBranch`，两者均为 `None` 时该栏 SHALL NOT 渲染
- contract test 同步：`cdt-api/tests/ipc_contract.rs` 加 `gitBranch` 字段断言

**侧栏折叠/展开**：
- SidebarHeader 加折叠按钮（`PanelLeft` icon，置于项目名按钮右侧），点击切换 `isSidebarCollapsed` 状态
- 折叠时 sidebar 整体不渲染（宽度为 0），展开按钮 SHALL 出现在 TabBar 最左侧（同 icon），点击恢复
- 全局 `Cmd+B` / `Ctrl+B` 快捷键 SHALL 切换折叠状态
- 折叠状态 SHALL 持久化到 `sidebarStore.svelte.ts`（仅内存，与 sidebar 宽度同维度；不入 backend config）
- `sidebar-navigation` 加 Requirement 描述折叠交互

## Capabilities

### New Capabilities

（无）

### Modified Capabilities

- `ipc-data-api`: `SessionSummary` 与 `SessionMetadataUpdate` 字段扩展（新增 `git_branch`）
- `sidebar-navigation`: 新增"项目 git 分支只读栏"与"侧栏折叠/展开"两个 Requirement

## Impact

- 后端：`crates/cdt-api/src/ipc/types.rs`（字段）、`crates/cdt-api/src/ipc/local.rs`（metadata scan 取值 + broadcast payload）
- 测试：`crates/cdt-api/tests/ipc_contract.rs`（contract 断言）、单测覆盖"取最后一条非空 `git_branch`"
- 前端 store：`ui/src/lib/sidebarStore.svelte.ts`（加 `isSidebarCollapsed` 状态 + toggle）
- 前端组件：`ui/src/components/SidebarHeader.svelte`（git 分支栏 + 折叠按钮）、`ui/src/components/Sidebar.svelte`（透传 `activeSessionId`、根据 collapsed 控制可见性）、`ui/src/components/TabBar.svelte`（折叠态展开按钮）、`ui/src/App.svelte`（Cmd+B 快捷键 + 顶层布局响应折叠）、`ui/src/lib/icons.ts`（新增 `GIT_BRANCH` / `PANEL_LEFT` icon）
- 前端类型：`ui/src/lib/api.ts`（`SessionSummary` / `SessionMetadataUpdate` interface 加字段）
- Fixtures：`ui/src/lib/__fixtures__/*.ts` 按新字段补值
- 不影响：骨架快速加载（`gitBranch` 走异步 patch）；`list_sessions` IPC 命名 / 调用语义；`Tauri invoke_handler!` / `EXPECTED_TAURI_COMMANDS` 列表

## Why

(1) 上一轮 sidebar 改动（change `sidebar-align-original`）的 messageCount 仍**高于原版**——经查 `cdt-api/src/ipc/session_metadata.rs::extract_session_metadata` 计数时只过滤 `category != User` 与 `is_meta`，未排除"内容只是 `tool_result` block 的 user 消息"。原版 `claude-devtools/src/main/types/messages.ts::isParsedUserChunkMessage` 要求 user 消息**必须含至少一个 text 或 image block**，且 cdt-parse 的 hard noise 检测在无 text block 时 short-circuit 返回（`extract_user_text` 没拿到 text 就放弃分类），导致纯 tool_result 的 user 行最终归到 `MessageCategory::User`。每次 AI 调一个工具就有一条这样的"假 user 消息"被算进计数。

(2) 上一轮把 git 分支显示放在 SidebarHeader 项目名下方一栏，**基于错误假设**——`gitBranch` 是 per-session 属性（同一 project 的不同 session 可能在不同 branch，因为用户中途切了），不是 per-project。原版用 worktree 选择器表达跨 branch 切换；本端口不做 worktree 切换，单栏只能反映 active 一条，跨 session 切换才看得到差异——视觉位置和语义错位。本 change 把 branch 显示从 SidebarHeader 移到每条 SessionItem 行内，与计数 / 时间共占第二行 meta line。

## What Changes

**messageCount 算法对齐**：
- `cdt-api/src/ipc/session_metadata.rs` 引入 `is_user_chunk_message(msg) -> bool` 函数，对齐原版 `isParsedUserChunkMessage`：要求 `MessageContent::Blocks` 至少含一个 text 或 image block（排除纯 tool_result-only user 消息）；`MessageContent::Text` 直接通过（hard noise / interrupt 已被 cdt-parse 分类剥离）
- `extract_session_metadata` 计数判断从 `category == User && !is_meta` 改为 `is_user_chunk_message(msg)`
- 新增单测覆盖："纯 tool_result-only user 行不计入"、"text+tool_result 混合 block user 行计入"、"image-only user 行计入"

**git 分支显示位置移动**：
- `cdt-api/src/ipc/types.rs` 的 `SessionSummary.gitBranch` 字段保留（per-session 仍需要）；后端取值规则不变
- `Sidebar.svelte` SessionItem 第二行 meta（messageCount + time）末尾追加 `· <git-icon>{branch}` chip（branch 为 null 时不渲染，不留空位）
- `SidebarHeader.svelte` 去掉 `branch-row`、删除 `sessions` / `activeSessionId` 两个 props（保留 `onToggleCollapsed`）
- `Sidebar.svelte` 不再透传 `sessions` / `activeSessionId` 给 SidebarHeader
- e2e `sidebar-collapse-and-branch.spec.ts` 第三个 test "git 分支栏渲染 active session 的 gitBranch" 改为断言 SessionItem 行内可见 branch 文本

## Capabilities

### New Capabilities

（无）

### Modified Capabilities

- `sidebar-navigation`:
  - MODIFIED `会话项展示`：消息计数语义明确为"过滤后的真实 user-chunk 消息数"（不含 tool_result-only 行）；元数据显示新增 git 分支 chip（如有）
  - REMOVED `项目 git 分支只读栏`：分支信息迁移到 SessionItem 行内；migration 注解为"该信息已迁移到每条会话项的元数据行"

## Impact

- 后端：`crates/cdt-api/src/ipc/session_metadata.rs`（计数算法 + 单测）
- 前端组件：`ui/src/components/Sidebar.svelte`（SessionItem meta 行加 branch chip）、`ui/src/components/SidebarHeader.svelte`（去掉 branch-row + props 减回去）、`ui/src/App.svelte`（透传简化）
- 测试：`ui/tests/e2e/sidebar-collapse-and-branch.spec.ts`（分支断言迁移）
- 不影响：`SessionSummary` IPC 字段（`gitBranch` 保留）、`SessionMetadataUpdate`、`list_sessions` 协议、Tauri command 列表、骨架快速加载、折叠/展开行为

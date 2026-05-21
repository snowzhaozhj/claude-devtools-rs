## Why

PR #183 合并 composite project 后用户痛点暴露：同一个 git repo 的多个 worktree（含 `.claude/worktrees/<name>/` 子目录的活跃 worktree、被 prune 后只剩 JSONL 的历史 worktree）和**主仓子目录里跑 claude 产生的不同 cwd**（如 `crates/`、`src-tauri/`），在 `~/.claude/projects/` 下各自编码成独立目录，被 `WorktreeGrouper` 正确合并到一个 `RepositoryGroup` 后，UI `ProjectSwitcher` 仍然按 worktree 维度 accordion 展开 N 条平铺，且 `is_main_worktree=true` 的判定只看 `.git` 是否目录，导致主仓 cwd 与子目录 cwd 都被命名为 "main" 撞名。同时 PR #183 给每条 session 加的行尾 cwd 全路径标签视觉噪音明显。用户心智里"`claude-devtools-rs` 就是一个项目"，需要把项目入口从 worktree 维度统一到 git repo 维度，session 列表按 mtime 全局合并。

## What Changes

- **BREAKING**：`Project.is_main_worktree` 语义从"`path` 自身 `.git` 是目录"修正为"`path` 是 git working tree 根目录"——区分"主 working tree 根"与"主 working tree 子目录"两种 walk-up 都找到同一 `.git` 但语义不同的情况
- **新增**：`Project.cwd_relative_to_repo_root: Option<String>`，描述 cwd 相对 repo 根的子路径（如 `crates`、`.claude/worktrees/feat-x`），UI 用作 chip 展示；repo 根本身为 `None`
- **新增 IPC**：`list_group_sessions(group_id, page_size, cursor)`——后端按 group 内各 worktree session mtime 做 **k-way merge 流式分页**，返回 `Vec<SessionSummary>` + opaque `next_cursor`；server 无状态，cursor 自描述每个 worktree 的指针位置
- **重构**：`ProjectScanner` 的 `read_semaphore` 从每实例独立改为 `Arc<Semaphore>` 由 `LocalDataApi` 注入共享，避免每次 IPC 新建 scanner 让 head-read 并发上限从 64 击穿到 19 × 64
- **UI 简化**：`ProjectSwitcher` 删除 worktree accordion 渲染分支，每个 `RepositoryGroup` 占一行（单 worktree 项目保持原扁平显示）；选中 group 后 sidebar 显示该 group 所有 worktree 合并、按 mtime 全局降序的 session 列表
- **UI 新增**：sidebar 顶部 worktree filter 下拉（默认"全部"，options 为 group 内 distinct cwd 列表 + branch chip + cwd 相对路径），切换时通过 `list_group_sessions` 的 cursor 重置重新分页
- **UI 删除**：`Sidebar.svelte:836-839` 的 session 行尾 cwd 全路径 label（PR #183 引入），改为右侧 chip：分支名（带 git icon）+ 当 `cwd_relative_to_repo_root` 非 None 时附加 `…/<lastTwoSegs>`；保留 `SessionDetail.svelte` 顶部 cwd badge 不变
- **状态机收敛**：`selectedProjectId` 在 sidebar / store / SSE patch / pin / hide 持久化 / cache key 的语义从 worktree id 统一为 group id（单 worktree 项目时 group id == worktree id == project id，无影响；多 worktree group 时所有 key 切换为 group id）
- 后端 `active_scans` per-key cancel 的 key 形态从 `project_id` 扩展为 `(group_id, page_cursor_hash)`，切页 / 切 group / 切 worktree filter 时取消正在跑的旧拉取

## Capabilities

### New Capabilities
（无）

### Modified Capabilities
- `project-discovery`：`is_main_worktree` 语义修正 + 新增 `cwd_relative_to_repo_root` 字段，`RepositoryGroup` 暴露 `distinct_cwds_per_worktree` 供 UI filter；移除 `infer_parent_repo_from_worktree_path` 对 main 子目录 cwd 的误判路径
- `ipc-data-api`：新增 `list_group_sessions` IPC（含 cursor / k-way merge 分页契约）+ `SessionSummary` 增加 `worktreeId` / `worktreeName` / `cwdRelativeToRepoRoot` 字段；`ProjectScanner` 构造改为接受共享 `Arc<Semaphore>`；移除 PR #183 引入的 session 列表行尾 cwd 全路径展示约束
- `sidebar-navigation`：`ProjectSwitcher` 项目入口从 worktree 维度简化为 group 维度；session 列表合并 + worktree filter 下拉 + 行尾 chip 展示规则；`selectedProjectId` 语义切换为 group id 的收敛 checklist（SSE filter / pin / hide / cache key / filter state 五处）

## Impact

- **代码**：
  - `crates/cdt-core/src/project.rs` —— `Project` / `Worktree` / `RepositoryGroup` 类型字段调整
  - `crates/cdt-discover/src/worktree_grouper.rs` + `project_scanner.rs` —— `is_main_worktree` 语义修正、`cwd_relative_to_repo_root` 计算、共享 semaphore 注入
  - `crates/cdt-api/src/ipc/local.rs` + `traits.rs` + `types.rs` —— 新 IPC `list_group_sessions` + cursor 编码 + active_scans key 改造
  - `crates/cdt-api/tests/ipc_contract.rs` —— 新增 IPC 字段 round-trip
  - `ui/src/components/ProjectSwitcher.svelte` —— 删 accordion 分支
  - `ui/src/components/Sidebar.svelte` —— 删行尾 cwd label、加 worktree filter、改 selectedProjectId 收敛
  - `ui/src/lib/projectDataStore.svelte.ts` + `sidebarStore.svelte.ts` —— group id 语义切换 + filter state
  - `ui/src/lib/api.ts` —— `listGroupSessions` 客户端
- **性能预算**（详 `.claude/rules/perf.md`）：
  - `list_group_sessions` 单页预期 ~20-30ms（N readdir + heap k-way merge），N=19 量级
  - 内存峰值 ~200 KB（N × avg_sessions × 骨架字节）
  - DOM 节点：删 cwd label 后**减少**
  - 不引入新跨进程依赖
- **依赖**：无新增 crate
- **配置 / 持久化**：`pinned_sessions` / `hidden_sessions` 的 key 仍是 session id，不受影响；若有以 `project_id` 为 key 的持久化（filter state、`active_scans` 中间态等）按 group id 写入，但 group id 等同主 worktree 的 project id 字符串，向后兼容
- **现有 PR #183 字段**：`Session.cwd` 保留（SessionDetail badge / worktree filter / 行尾 chip 都消费）；删除的只是 `Sidebar.svelte:836-839` 行尾全路径 label 的展示路径

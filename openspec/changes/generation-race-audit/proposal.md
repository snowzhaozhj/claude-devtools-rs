## Why

PR #198 (`project-scanner-memoize`) 的 codex 三轮 verify 报了 2 个 sub-window race（followups.md L305-319），列入 coverage-gap 不在该 PR scope 修：

1. **Race 1：`switch_context` bump-first 顺序导致的 generation snapshot race**
   `switch_context`（`crates/cdt-api/src/ipc/local.rs:3309`）当前**先** `context_generation.fetch_add(1, SeqCst)` **再** `await ssh_mgr.switch_context(target)`。bump 与 ssh_mgr state mutation 之间存在 await window：
   - T0：`switch_context` 内 `_ops` 锁拿到 → bump gen N→N+1 → 进入 `ssh_mgr.switch_context` await
   - T1：并发 `list_repository_groups` 进入：pre snapshot = N+1（已 bump），内部 `active_fs_and_policy().await` 拿 OLD ctx（ssh_mgr.active_context_id() 仍返旧），scan + grouper 跑完
   - T2：`list_repository_groups` post = N+1 == pre → **校验通过** → 调 `refresh_worktree_meta_cache(&old_groups)` → 全局 flat-keyed `worktree_meta_cache` 被旧 ctx 数据 clear-and-rebuild
   - T3：用户在 NEW ctx 调 `list_sessions` → `apply_worktree_meta` 查到 OLD ctx 的 mapping（worktree_id 巧合相同时）或 None（worktree_id 不重合时）→ UI 拿到错乱 worktreeName/groupId 或 null fallback

2. **Race 2：`build_group_session_page` 用旧 groups + 新 fs 混用**
   `build_group_session_page`（`crates/cdt-api/src/ipc/local.rs:574-584`）当前先调 `list_repository_groups().await` 拿 groups，再独立 `active_fs_and_context_strict().await` 拿 (fs, projects_dir, ctx)：两次 await 间可被 ssh switch / reconfigure 跨过。`list_repository_groups` 即便 safe-degrade 返回 OLD ctx 的 groups，下方 (fs, ctx) 已是 NEW ctx → scanner 用 OLD worktree_id 在 NEW fs 上扫 → 返回空页（UI 看到空 group）。

两个 issue 都属"一次错乱、下次 IPC 自然修复"性质（不是 silent data corruption），但**用户可观察**——SSH 切 ctx 期间一次 list 看到错乱 worktree meta 或空 group page，会让"切换后立刻看一眼新 host 数据"的常见动作失效。本 change 把这两个 sub-window race 的修法收敛为统一约束写进 spec，并加自动化回归屏障。

历史背景：PR #198 与之前几轮 codex 修订（H2 / H2-R / H2-R-2 / 第四轮 Blocker / 第五轮 Blocker）已经把 in-flight scan task broadcast 路径上的 generation 校验补全，**这两个 race 残留在 IPC handler 层**——具体是 `list_repository_groups::refresh_worktree_meta_cache` 写入路径 + `build_group_session_page` 跨两次 active_fs await 的快照拼接路径。

## What Changes

- **ipc-data-api**: 修改 Requirement `SessionSummary 增加 worktree 元信息字段`：把 `worktree_meta_cache` 刷新约束从"`list_repository_groups` 调用后立即填充 + grouper 重跑时整体替换"加强为"刷新前 SHALL 在 `ssh_watcher_ops` 锁下做 (current_ctx == captured_ctx) **AND** (current_generation == captured_generation) **双重**校验，任一 mismatch 时 skip refresh（safe degrade）"。新增 Scenario 覆盖 "switch_context 期间并发 list_repository_groups 不污染全局 cache" / "同 host 快速 disconnect+reconnect 期间 generation bump 触发 skip refresh" / "reconfigure_claude_root 改 Local projects_dir 期间 list_repository_groups 不污染 cache"。
- **ipc-data-api**: 修改 Requirement `Expose group session listing via k-way merge pagination`：把"groups 与 fs/ctx 来源"约束从隐含变显式——`build_group_session_page` SHALL 通过单次内部 helper（`list_repository_groups_inner`）一次性拿 (groups, fs, projects_dir, ctx, captured_generation) 同源**五元组**，MUST NOT 各自独立 `await` 两次 active 抽样；额外新增"后台 metadata scan task spawn 前 SHALL 在 `ssh_watcher_ops` 锁内做 (ctx + generation) 二次校验，mismatch 时返页面骨架但 SHALL NOT spawn 任何 metadata scan task"约束；新增 Scenario 覆盖"snapshot consistency"与"spawn-time skip"两类边角。
- **ssh-remote-context**: 修改 Requirement `Reconnect lifecycle preserves SFTP session integrity`：固化 bump-first 顺序保留（不改 5 处 mutate 入口顺序避免破坏 in-flight scan invalidation 契约），同时新增 invariant"刷新派生 cache 写入 SHALL 用 (captured ctx + captured generation) 与 ssh_mgr 当前 (active + generation) 双重校验"。

## Impact

- Affected specs: `ipc-data-api` / `ssh-remote-context`
- Affected code:
  - `crates/cdt-api/src/ipc/local.rs`
    - 抽 `list_repository_groups_inner(&self) -> Result<(Vec<RepositoryGroup>, Arc<dyn FS>, PathBuf, ContextId), ApiError>`：把已有 `list_repository_groups` 的内部主体提到 inner，inner 返回 `(groups, fs, projects_dir, ctx)` 同源快照
    - `list_repository_groups` 退化为薄 wrapper：调 inner，refresh_worktree_meta_cache 路径加 `ssh_watcher_ops` 锁 + ctx-equality 校验
    - `build_group_session_page` 改用 inner，复用 (fs, projects_dir, ctx)，删掉 line 584 的 `active_fs_and_context_strict().await` 第二次抽样
  - `crates/cdt-api/tests/`
    - 新增 `local_generation_race.rs`：两个 invariant 测试覆盖 Race 1 / Race 2
  - `openspec/followups.md`：把 `### [coverage-gap] context_generation 模式 sub-window race` 段标 ✅ 并指向本 change
- BREAKING: 否（IPC 字段不变；行为变化是"safe degrade 时 cache 不 refresh"，原本 race 路径下 cache refresh 是 bug 不是契约）

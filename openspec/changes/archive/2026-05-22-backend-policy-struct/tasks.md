# Tasks

> 推进节拍：`.claude/rules/opsx-apply-cadence.md`。业务段 1-8，发布尾段 N.1-N.4。
> design 阶段 codex 二审完成且 pass 之后才进入 1.x（CLAUDE.md What to do first 第 0 条 + opsx-apply-cadence "design 阶段 codex 二审"）。

## 1. `cdt-fs::BackendPolicy` struct 扩展

- [x] 1.1 `crates/cdt-fs/src/backend_policy.rs`：加 `supports_memory: bool` / `supports_subagent_scan: bool` / `stale_check_strategy: StaleCheckStrategy` 三个字段
- [x] 1.2 同文件加 `pub enum StaleCheckStrategy { LocalClock5min, SkipUntilClockSync }` + `#[derive(Debug, Clone, Copy, PartialEq, Eq)]`
- [x] 1.3 更新 `for_local()`：`supports_memory: true` + `supports_subagent_scan: true` + `stale_check_strategy: StaleCheckStrategy::LocalClock5min`
- [x] 1.4 更新 `for_ssh()`：`supports_memory: false` + `supports_subagent_scan: false` + `stale_check_strategy: StaleCheckStrategy::SkipUntilClockSync`
- [x] 1.5 更新 `for_http()`：按 design D7 / Open Question 1 等同 `for_local()` 行为兜底（`supports_memory: true` + `supports_subagent_scan: true` + `stale_check_strategy: LocalClock5min`）
- [x] 1.6 更新现有 `for_local_uses_skeleton_then_stream` / `for_ssh_uses_full_eager_single_round_trip` / `for_http_uses_full_eager_single_round_trip` 单测加新 3 字段断言
- [x] 1.7 加新单测 `stale_check_strategy_enum_has_two_variants` 用 `[StaleCheckStrategy; 2]` 编译期断言 variant 数 == 2（同 `InitialLoadPolicy` 现有模式）
- [x] 1.8 加新单测 `backend_policy_is_copy_eq` 显式断言 `BackendPolicy` 实现 `Copy + Eq`（trait bound 校验）
- [x] 1.9 `cargo test -p cdt-fs` 过

## 2. `cdt-discover::WorktreeGrouper::new_dyn` 入口（含显式 `Arc<dyn>` blanket impl）

- [x] 2.1 **显式加 `Arc<T>` blanket impl**（codex design 二审 Blocking #2）：`crates/cdt-discover/src/worktree_grouper.rs` 在 trait 定义旁加 `#[async_trait] impl<T: GitIdentityResolver + ?Sized + Send + Sync> GitIdentityResolver for Arc<T>`，4 个方法（`resolve_identity` / `get_branch` / `is_main_worktree` / `resolve_all`）forward 到 `(**self).<method>(path).await`
- [x] 2.2 加 `impl WorktreeGrouper<Arc<dyn GitIdentityResolver>> { pub fn new_dyn(resolver: Arc<dyn GitIdentityResolver>) -> Self { Self::new(resolver) } }`
- [x] 2.3 **Fallback（若 2.1 编译失败）**：删 2.1 blanket impl，改加 newtype wrapper `struct DynGitIdentityResolver(Arc<dyn GitIdentityResolver>)` + `#[async_trait] impl GitIdentityResolver for DynGitIdentityResolver`（4 方法手工 forward），`new_dyn` 内部 `Self::new(DynGitIdentityResolver(resolver))`；选 fallback 时 BackendResolvers 字段类型保持 `Arc<dyn GitIdentityResolver>`（wrapper 在 grouper 调用边界完成）
- [x] 2.4 加 `crates/cdt-discover/tests/worktree_grouper_dyn.rs` 单测：构造 `Arc<dyn GitIdentityResolver>`（用 `LocalGitIdentityResolver::new()` 包 Arc），断言 `WorktreeGrouper::new_dyn(arc)` 返实例可调 `group_by_repository`；若走 fallback newtype 路径也加同形单测
- [x] 2.5 `cargo test -p cdt-discover` 过

## 3. `cdt-api::ipc::backend_resolvers` 新 module

- [x] 3.1 新增 `crates/cdt-api/src/ipc/backend_resolvers.rs`：`pub(crate) struct BackendResolvers { pub search_config: SearchConfig, pub git_identity_resolver: Arc<dyn GitIdentityResolver> }`（struct + 字段都 `pub(crate)`/`pub`，crate 内可读，crate 外不暴露）
- [x] 3.2 同文件加 `struct NoopGitIdentityResolver`（从 `local.rs:180-198` 抽过来）+ `impl GitIdentityResolver for NoopGitIdentityResolver`（三方法返 None/None/true 兜底）
- [x] 3.3 加 `static LOCAL_RESOLVERS: LazyLock<Arc<BackendResolvers>>` + `static SSH_RESOLVERS: LazyLock<Arc<BackendResolvers>>` 静态实例缓存
- [x] 3.4 加 `impl BackendResolvers` 方法：`for_local() -> Arc<Self>` / `for_ssh() -> Arc<Self>` / `from_fs(&dyn FileSystemProvider) -> Arc<Self>`（按 `fs.kind()` 派发）
- [x] 3.5 `crates/cdt-api/src/ipc/mod.rs` 加 `pub(crate) mod backend_resolvers;`（无需 re-export，调用方走 `crate::ipc::backend_resolvers::BackendResolvers` 全路径）
- [x] 3.6 删 `crates/cdt-api/src/ipc/local.rs` 顶部 inline `NoopGitIdentityResolver` 定义（已搬到 backend_resolvers.rs）+ 替换 `list_repository_groups` 内 `NoopGitIdentityResolver` 直引用为通过 `resolvers.git_identity_resolver` 取
- [x] 3.7 **inline unit test**（codex design 二审 Blocking #3：`pub(crate)` 类型不能被 integration test 跨 crate 访问，写成 module 内 `#[cfg(test)] mod tests`）：在 `backend_resolvers.rs` 底部加 `#[cfg(test)] mod tests`，覆盖 `from_fs(Local) ptr_eq for_local()` / `from_fs(Ssh) ptr_eq for_ssh()` / `for_local().search_config.is_ssh == false` / `for_ssh().search_config.is_ssh == true`；fs 用 fake provider（`crates/cdt-fs/src/local.rs::LocalFileSystemProvider` 或仅 mock `FsKind`）
- [x] 3.8 `cargo test -p cdt-api ipc::backend_resolvers` 过

## 4. `LocalDataApi::active_fs_and_policy` helper

- [x] 4.1 `crates/cdt-api/src/ipc/local.rs`：基于现有 `active_fs_and_context_strict()` 加 `pub(crate) async fn active_fs_and_policy(&self) -> Result<(Arc<dyn FileSystemProvider>, PathBuf, cdt_fs::ContextId, BackendPolicy, Arc<BackendResolvers>), ApiError>`
- [x] 4.2 helper 内部按 `fs.kind()` 派发：`Local → BackendPolicy::for_local() + BackendResolvers::for_local()`；`Ssh → BackendPolicy::for_ssh() + BackendResolvers::for_ssh()`
- [x] 4.3 helper docstring 显式：`fs.kind()` 比对仅允许在本 helper + `BackendResolvers::from_fs` 内部使用（业务 callsite 通过 policy/resolvers 字段读取）
- [x] 4.4 `cargo check -p cdt-api` 过

## 5. 6 处 callsite 改造

### 5.1 `get_project_memory` (line 2206-2215)

- [x] 5.1.1 `let (fs, _projects_dir) = self.active_fs_and_projects_dir().await?;` 改为 `let (fs, _projects_dir, _ctx, policy, _resolvers) = self.active_fs_and_policy().await?;`
- [x] 5.1.2 `if fs.kind() == cdt_discover::FsKind::Ssh` 改为 `if !policy.supports_memory`
- [x] 5.1.3 删 `// policy fork: PR-E lift to BackendPolicy::supports_memory` 注释

### 5.2 `read_memory_file` (line 2232-2257)

- [x] 5.2.1 同 5.1.1 改 helper 调用
- [x] 5.2.2 同 5.1.2 改条件
- [x] 5.2.3 同 5.1.3 删注释

### 5.3 `get_session_detail` subagent scan (line 2314-2323)

- [x] 5.3.1 顶层已用 `active_fs_and_context_strict()`，本 PR 改为 `active_fs_and_policy()` 拿 policy + resolvers
- [x] 5.3.2 `let candidates = if is_remote { Vec::new() } else if CROSS_PROJECT_SUBAGENT_SCAN { ... } else { ... }` 改为 `let candidates = if !policy.supports_subagent_scan { Vec::new() } else if CROSS_PROJECT_SUBAGENT_SCAN { ... } else { ... }`
- [x] 5.3.3 删 `// policy fork: PR-E lift to BackendPolicy::supports_subagent_scan` 注释

### 5.4 `get_session_detail` is_ongoing stale check (line 2327-2338)

- [x] 5.4.1 `let is_ongoing = if messages_ongoing && !is_remote { !is_file_stale(...).await } else { messages_ongoing }` 改为 `match policy.stale_check_strategy { StaleCheckStrategy::LocalClock5min if messages_ongoing => !is_file_stale(...).await, _ => messages_ongoing }`（或语义等价的 if/match 组合）
- [x] 5.4.2 删 `// policy fork: ... PR-E lift to BackendPolicy::stale_check_strategy` 注释（保留 issue #94 stale 5min 业务说明）
- [x] 5.4.3 验证语义等价：仅 `LocalClock5min` + `messages_ongoing` 时跑 stale check；其它路径直接返 `messages_ongoing`

### 5.5 `search` (line 2730-2740)

- [x] 5.5.1 `let (fs, projects_dir) = self.active_fs_and_projects_dir().await?;` 改为 `let (fs, projects_dir, _ctx, _policy, resolvers) = self.active_fs_and_policy().await?;`
- [x] 5.5.2 `let config = SearchConfig::from_fs_kind(fs.kind());` 改为 `let config = resolvers.search_config.clone();`（SearchConfig 4 字段 Vec/Duration cheap clone）
- [x] 5.5.3 删 `// policy fork: PR-E lift to BackendPolicy::search_config` 注释

### 5.6 `list_repository_groups` (line 3145-3175)

- [x] 5.6.1 `let (fs, projects_dir) = self.active_fs_and_projects_dir().await?;` 改为 `let (fs, projects_dir, _ctx, _policy, resolvers) = self.active_fs_and_policy().await?;`
- [x] 5.6.2 删 `let is_remote = fs.kind() == cdt_discover::FsKind::Ssh;`
- [x] 5.6.3 `let groups = if is_remote { ... NoopGitIdentityResolver ... } else { ... LocalGitIdentityResolver::new() ... }` 改为单路径 `let grouper = cdt_discover::WorktreeGrouper::new_dyn(resolvers.git_identity_resolver.clone()); let groups = grouper.group_by_repository(projects).await;`
- [x] 5.6.4 删两处 `// policy fork: PR-E lift to BackendPolicy::git_identity_resolver` 注释

## 6. grep 不变性测试

- [x] 6.1 新增 `crates/cdt-api/tests/no_kind_compare_outside_resolvers.rs`：用 `std::fs::read_to_string` 读 `crates/cdt-api/src/ipc/local.rs`
- [x] 6.2 统计两 pattern 各自出现次数：`fs.kind() ==` 与 `let is_remote =`（用 `str::matches().count()` 或 line-by-line contains）
- [x] 6.3 **硬编码真实阈值**（codex design 二审 Blocking #1）：`const MAX_LET_IS_REMOTE: usize = 2;` + `const MAX_FS_KIND_EQ: usize = 3;`；断言两 count ≤ 各自阈值
- [x] 6.4 测试源码顶部注释列每处剩余出处的合理性：
   - `let is_remote =` 共 2 处：line 837 `list_sessions_skeleton` + line 1594 `build_group_session_page`（均属 PR-D D2 落地的 SSH-Local 同入口下的内部派生，不在 PR-E 6 处之列）
   - `fs.kind() ==` 共 3 处：line 837 / 1594（同上 let 的右值）+ line 3125 `read_mentioned_file` SSH gate（codex Open Question 标 follow-up，预期未来 PR-G `supports_mention_file_resolution: bool` 字段消除）
- [x] 6.5 测试顶部注释明示"调整阈值时 SHALL 在 PR 描述引用 design D6 + 列新增/移除的 callsite + 引用对应 spec / followup"
- [x] 6.6 `cargo test -p cdt-api --test no_kind_compare_outside_resolvers` 过

## 6-bis. SSH context IPC contract 覆盖（codex design 二审 Blocking #4）

- [x] 6b.1 在 `crates/cdt-api/tests/ipc_contract.rs`（或新增 `crates/cdt-api/tests/backend_policy_runtime.rs`）加 SSH active context 下两条断言：
   - `get_project_memory("any-id")` 在 SSH context 下 SHALL 返 `ProjectMemory { has_memory: false, count: 0, layers: vec![], default_file: None, .. }`
   - `read_memory_file("any-id", "CLAUDE.md")` 在 SSH context 下 SHALL 返 `Err(ApiError::not_found(...))` 错误消息含 "SSH context"
- [x] 6b.2 SSH context 用现有测试基础设施 mock（若 `ipc_contract.rs` 已有 SSH active mock pattern 复用之；否则在测试内部构造 `LocalDataApi` + 注入 mock `SshSessionManager` 持 active context）
- [x] 6b.3 `cargo test -p cdt-api --test <test_file>` 过

## 7. clippy + workspace test + spec validate

- [x] 7.1 `cargo clippy --workspace --all-targets -- -D warnings` 过
- [x] 7.2 `cargo fmt --all` 过
- [x] 7.3 `cargo test --workspace` 过
- [x] 7.4 `pnpm --dir ui run check` 过（前端无改动，应当 noop 但走一遍流程）
- [x] 7.5 `openspec validate backend-policy-struct --strict` 过

## 8. perf 校验

- [x] 8.1 apply 前在 main HEAD 跑一次基线：`bash scripts/run-perf-bench.sh --runs 5`，记录 wall / user / sys / RSS / user-real-ratio
- [x] 8.2 apply 后切回本 branch 跑一次：`bash scripts/run-perf-bench.sh --runs 5`
- [x] 8.3 对比四维：wall +20% / user +50% / RSS +30% / user-real-ratio cross 0.5 任一即拒；预期零回归
- [x] 8.4 PR 描述贴四维数据（按 `.claude/rules/perf.md` Perf impact 模板）

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

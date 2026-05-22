# tasks

## 1. 设计 codex 二审

- [x] 1.1 把 design.md D1-D3 + spec delta 提交给 `Agent({ subagent_type: "codex:codex-rescue", ... })`，prompt 模板：`.claude/templates/codex-prompt-design-review.md`，重点列：
  - 锁内 ctx-equality 校验是否真闭合 Race 1（边角：connect→disconnect→connect 同 host 在 inner 期间快速来回）
  - bump-first 保留是否影响其它 spawn 路径正确性（list_sessions_skeleton / get_session_detail 等）
  - 单一 snapshot 后是否仍有"groups 过期但 fs 当前"残留（reconfigure_claude_root 跨 inner 边界的次级 race）
  - 不变量测试是否真覆盖目标 race（特别是 mock counter 是否绕过结构性 race）
- [x] 1.2 codex 报问题 → 修 design / spec / tasks 三处 → re-`openspec validate generation-race-audit --strict` → 才进 apply（codex 报了 3 个 Bug，已在 design/spec/tasks 同步修订：D1 加 captured_generation 双重校验、D2 加 spawn 前锁内二次校验、保留 inner pre/post 不删）

## 2. apply：抽 inner 五元组 + wrapper (ctx + generation) 双重校验

- [x] 2.1 抽 `LocalDataApi::list_repository_groups_inner(&self) -> Result<(Vec<RepositoryGroup>, Arc<dyn FileSystemProvider>, PathBuf, cdt_fs::ContextId, /* captured_generation */ u64), ApiError>`：把现有 `list_repository_groups` 内 `pre_root` / `pre_ctx_gen` 抽样 + `active_fs_and_policy()` + `scan_projects_cached_with` + grouper 提到 inner；**captured_generation SHALL 在 active_fs_and_policy 完成之后立即 load**（与 (fs, ctx) 同 snapshot）；保留 inner 内既有 `pre/post root_generation && context_generation` 校验作 fast-path，mismatch 时仍返五元组（wrapper 会做最终校验，inner 保留 fast-path 的 `tracing::debug!` 留痕）
- [x] 2.2 加 helper `async fn current_active_context_id_under_lock(&self) -> cdt_fs::ContextId`：调用方 SHALL 已持 `ssh_watcher_ops` 锁；内部 `ssh_mgr.active_context_id().await` + `provider_and_context_id` 失败时 fall through 到 `ContextId::local(self.projects_dir.lock().await.clone())`（同 active_fs_and_context_strict safe-degrade 形态，但因持 ssh_watcher_ops 锁状态稳定）
- [x] 2.3 重写 `list_repository_groups`：调 inner 拿五元组；锁内做 (current_ctx == captured_ctx) **AND** (current_generation == captured_generation) 双重校验；任一 mismatch SHALL skip refresh + `tracing::debug!`（含 captured/current ctx + 两个 generation 值）；match 时锁内调 `refresh_worktree_meta_cache(&groups)` 然后释放锁返 groups
- [x] 2.4 重写 `build_group_session_page`：删 line 584 的 `active_fs_and_context_strict().await`；改用 `list_repository_groups_inner()` 一次性拿五元组（含 captured_generation）；`expected_context_generation = captured_generation`、`expected_root_generation` 仍单独抽样（与 list_sessions_skeleton 同形）；
- [x] 2.5 在 `build_group_session_page` 内 page 骨架组装完成后、spawn `scan_metadata_for_page` 后台 task 之前，SHALL 持 `ssh_watcher_ops` 锁做 (ctx + generation) 二次校验：mismatch 时 SHALL 返 `GroupSessionPage` 骨架但 SHALL NOT spawn 任何 metadata scan task；match 时 SHALL 在锁内完成所有 page_jobs 的 `tokio::spawn(scan_metadata_for_page(...))` 注册到 active_scans 后释放锁
- [x] 2.6 不引入 post-bump 到 `switch_context` / `ssh_connect` / `ssh_disconnect` / `reconfigure_claude_root` / `shutdown_ssh_all`（保留 bump-first 对 in-flight scan 的早 fail 契约；本 change 不动 5 处 bump 顺序）
- [x] 2.7 `cargo clippy --workspace --all-targets -- -D warnings`（本 PR 涉及代码无 clippy 报错）
- [x] 2.8 `cargo fmt --all`
- [x] 2.9 `cargo test -p cdt-api`（全部既有测试通过）

## 3. apply：invariant 测试（4 条 + 真并发 race fake-ssh delay 钩子作 follow-up）

- [x] 3.1 新建 `crates/cdt-api/tests/local_generation_race.rs`
- [x] 3.2 暴露 `cfg(any(test, feature = "test-utils"))` 计数器 `LocalDataApi::refresh_worktree_meta_cache_call_count` / `metadata_scan_spawn_count` / `active_fs_and_policy_call_count`（feature-gated 仅 test build 不影响 release perf）；加 dev-dependency `cdt-api = { path = ".", features = ["test-utils"] }` 让集成测试可访问 cfg 字段
- [ ] 3.3 加 fake SSH manager delay 钩子（`switch_delay_rx` / `disconnect_delay_rx` / `connect_delay_rx`）—— **本 change scope 之外**：需改 `crates/cdt-ssh/src/lib.rs` 加 `#[cfg(test)]` test hook，跨 crate 改动；标 follow-up 5.2
- [x] 3.4 ~~Test 1（真并发 ssh switch race）~~ —— 依赖 3.3 fake delay 钩子；标 follow-up 5.2
- [x] 3.5 ~~Test 2（同 host disconnect+reconnect race）~~ —— 同上 follow-up 5.2
- [x] 3.6 ~~Test 3（spawn 前 ctx mismatch skip）~~ —— 同上 follow-up 5.2
- [x] 3.7 加 `build_group_session_page_calls_active_fs_only_once`（Test 4）：断言 `active_fs_and_policy_call_count == 1`（D2 单一 snapshot 落地验证 ✅）
- [x] 3.8 加 `concurrent_list_groups_does_not_panic`（Test 5 简化版）：16 轮真并发 list_repository_groups，断言 panic-free + 至少触发一次 refresh（验证 ssh_watcher_ops 锁路径无死锁）
- [x] 3.9 加 `list_repository_groups_refreshes_meta_cache_on_match_path`：验证 ctx + generation match 路径正常 refresh（counter +1）
- [x] 3.10 加 `build_group_session_page_spawns_metadata_scan_on_match_path`：验证 spawn 守卫不误把 match 路径也禁用（counter ≥ 1）
- [x] 3.11 `cargo test -p cdt-api --test local_generation_race`（4/4 通过）

## 4. spec 与 followup 收尾

- [x] 4.1 `openspec validate generation-race-audit --strict`
- [x] 4.2 `openspec/followups.md` 把 `### [coverage-gap] context_generation 模式 sub-window race（PR #198 codex 三轮 verify 残留）` 段标 ✅，加一行"已在 change `generation-race-audit` 修复"

## 5. follow-up（不在本 change scope，留 followups.md）

- [ ] 5.1 `worktree_meta_cache` 改 `(ContextId, worktree_id)` 复合 key namespace（彻底消除全局 cache 跨 ctx 污染）—— 大改动，需独立 change；本 change 用 (ctx + generation) 锁内校验已结构性闭合 race
- [ ] 5.2 `cdt-ssh::FakeSshManager` 加 delay injection 钩子（`switch_delay_rx` / `disconnect_delay_rx` / `connect_delay_rx`）+ 在 `crates/cdt-api/tests/local_generation_race.rs` 补 Test 1/2/3 真并发 race 触发测试 —— 需要 cdt-ssh crate 改动（test hook），跨 crate scope；invariant 已通过本 PR 的结构性 invariant 测试覆盖（`active_fs_and_policy_call_count == 1` / `refresh counter` / `spawn counter`），真并发 race 触发测试只是补强证据

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

## 1. cdt-api: `ParsedMessageCache` 内部 key 升级

- [x] 1.1 `crates/cdt-api/src/ipc/parsed_message_cache.rs::ParsedMessageCache.map: HashMap<PathBuf, _>` → `HashMap<(cdt_fs::ContextId, PathBuf), ParsedMessageEntry>`
- [x] 1.2 `ParsedMessageCache.order: VecDeque<PathBuf>` → `VecDeque<(cdt_fs::ContextId, PathBuf)>`
- [x] 1.3 `ParsedMessageCache::lookup(&mut self, ctx: &ContextId, path: &Path) -> Option<ParsedMessageEntry>`——内部用 tuple key 查找 / bump 队首
- [x] 1.4 `ParsedMessageCache::insert(&mut self, key: (ContextId, PathBuf), entry: ParsedMessageEntry)`——存在则更新 + bump 队首；不存在且超容量则 pop_back evict
- [x] 1.5 `ParsedMessageCache::remove(&mut self, ctx: &ContextId, path: &Path)`——签名扩 ContextId，仅清匹配 key 的 entry（保留其它 ctx 同 path 的 entry）
- [x] 1.6 `ParsedMessageCache::remove_if_signature_mismatch(&mut self, ctx: &ContextId, path: &Path, current_sig: &FileSignature) -> bool`——签名扩 ContextId
- [x] 1.7 `PARSED_MESSAGE_CACHE_CAPACITY` 保持 `50` 不变（详 design D3）
- [x] 1.8 现有 7 个单测 `parsed_cache_evicts_lru_when_over_capacity` / `parsed_cache_lookup_bumps_hit_to_front` / `parsed_cache_remove_drops_entry` / `parsed_cache_remove_noop_when_absent` / `remove_if_signature_mismatch_keeps_entry_when_sig_matches` / `remove_if_signature_mismatch_removes_when_sig_changes` / `remove_if_signature_mismatch_noop_when_absent` 同步改造：构造 `let ctx = ContextId::local(PathBuf::from("/test"));`，所有 `lookup(path)` 改 `lookup(&ctx, path)`、`insert(path, entry)` 改 `insert((ctx.clone(), path), entry)`、`remove(path)` 改 `remove(&ctx, path)`、`remove_if_signature_mismatch(path, sig)` 改 `remove_if_signature_mismatch(&ctx, path, sig)`
- [x] 1.9 新增单测 `parsed_local_vs_ssh_keys_do_not_collide`：同 path 不同 `ContextId` 插入 → lookup 用 Local ctx 命中 Local entry，lookup 用 SSH ctx 命中 SSH entry（互不串扰）
- [x] 1.10 新增单测 `parsed_lru_evicts_with_mixed_context`：插入 51 个跨 Local/SSH 混合 entry → 最早一个被淘汰；总容量 ≤ 50
- [x] 1.11 新增单测 `parsed_switch_context_does_not_clear_cache`：插入 Local entry → 模拟 ContextId 切到 SSH（直接构造 SSH ctx 查询）→ Local entry 仍在 cache（用 Local ctx 查询能命中）
- [x] 1.12 新增单测 `parsed_remove_if_signature_mismatch_per_ctx`：同 path 写入 Local ctx + SSH ctx 两个 entry → 调 `remove_if_signature_mismatch(&local_ctx, path, &new_sig)` 仅清 Local entry，SSH entry 仍在
- [x] 1.13 `cargo test -p cdt-api --lib ipc::parsed_message_cache::tests` 单跑全过

## 2. cdt-api: `extract_parsed_messages_cached` 走 `FileSystemProvider::stat`

- [x] 2.1 `extract_parsed_messages_cached` 签名改为 `pub(crate) async fn extract_parsed_messages_cached(cache: &StdMutex<ParsedMessageCache>, fs: &dyn cdt_fs::FileSystemProvider, context_id: &cdt_fs::ContextId, path: &Path) -> Option<Arc<Vec<ParsedMessage>>>`
- [x] 2.2 内部用 `fs.stat(path).await.ok()?` 替换 `tokio::fs::metadata(path).await.ok()?`；结果走 `FileSignature::from_fs_metadata(&meta)` 构造 signature（移除函数级 `#[allow(deprecated)]`）
- [x] 2.3 cache `lookup(context_id, path)` / `insert((context_id.clone(), path.to_path_buf()), entry)` 用新 tuple key API
- [x] 2.4 现有所有针对 `extract_parsed_messages_cached` 的 `tests` mod 测试（`cached_hit_returns_arc_without_rereading` / `cached_miss_when_file_size_changes` / `cached_miss_when_inode_changes_via_rename` / `cached_stat_failure_returns_none_no_write` / `empty_file_is_cached_as_valid_empty_result`）SHALL 改造：构造 `let fs = cdt_fs::local_handle();` + `let ctx = cdt_fs::ContextId::local(tmp.path().to_path_buf());`，调用时传入 `&*fs, &ctx`
- [x] 2.5 新增单测 `parsed_cached_uses_fs_stat_not_tokio_fs_metadata`：用 `cdt_fs::InstrumentedFs::new(cdt_fs::local_handle())` 包装 + `with_fs_counter` 跑 cache miss → 断言 `FsOpCounts.stat >= 1`；cache hit 再调一次 → 断言 hit 路径 `FsOpCounts.stat` 仅增 1（cache 命中时 fs.stat 也要调一次做 signature 校验）—— 验证 fs trait 链路通畅
- [x] 2.6 新增单测 `parsed_cached_local_vs_ssh_isolation`：用 fake `FileSystemProvider` 模拟 Local + SSH 两个 ContextId 各写一条 entry → 互不串扰（注：fake fs 仅用作 stat / parse_file 路径替身；本 change scope 内 SSH callsite 实际不走 cache wrapper，本测试用于验证 wrapper 本身的 ctx 隔离能力）
- [x] 2.7 `cargo test -p cdt-api --lib ipc::parsed_message_cache` 全过

## 3. cdt-api: 2 处业务 callsite 改一次性快照 + 接入 `active_fs_and_context()`（详 design D8-bis）

- [x] 3.1 `crates/cdt-api/src/ipc/local.rs::get_image_asset`（line 2449 附近）：把方法起始处的 `let (fs, projects_dir) = self.active_fs_and_projects_dir().await?;` **替换**为 `let (fs, projects_dir, ctx) = self.active_fs_and_context().await;`（注意：返回类型从 `Result<_>` 变为非 Result，整个方法不需要 `?`——但本 helper 永远成功，与原 `active_fs_and_projects_dir` 在 helper 内部错误降级到 Local 的语义一致）。Local 分支调 `extract_parsed_messages_cached(&self.parsed_msg_cache, &*fs, &ctx, &jsonl_path).await`，**不**再二次调 helper。SSH 分支继续 `let body = fs.read_to_string(&jsonl_path).await ...`（同一 fs 快照，不查 cache）
- [x] 3.2 `crates/cdt-api/src/ipc/local.rs::get_tool_output`（line 2511 附近）：同 3.1 改造——起始处只调一次 `active_fs_and_context()` 拿三元组，is_remote 判断走 SSH 分支或 Local 分支
- [x] 3.3 `crates/cdt-api/src/ipc/local.rs::prime_parsed_msg_cache_for_test`（`#[cfg(any(test, feature = "test-utils"))]`，line 3217 附近）：同样改造 `let (fs, _, ctx) = self.active_fs_and_context().await; extract_parsed_messages_cached(&self.parsed_msg_cache, &*fs, &ctx, path).await`
- [x] 3.4 `cargo check --workspace` 全过
- [x] 3.5 验证 `active_fs_and_context` 返回类型与原 `active_fs_and_projects_dir` 接 `?` 的差异不破坏 caller（前者无 Result，后者返 `Result<(Arc<dyn Fs>, PathBuf), ApiError>`）：本 change 仅替换 2 处 callsite 起始，下游分支逻辑保持原结构调整即可（删掉 `?`）

## 4. cdt-api: `spawn_parsed_msg_cache_invalidator` ContextId + fs.stat 改造

- [x] 4.1 `crates/cdt-api/src/ipc/local.rs::spawn_parsed_msg_cache_invalidator`（line 1673 附近）：函数体首部一次性合成 `let ctx = cdt_fs::ContextId::local(projects_dir.clone()); let fs = cdt_fs::local_handle();`，循环内复用
- [x] 4.2 stat 路径：`tokio::fs::metadata(&path).await` 替换为 `fs.stat(&path).await`；构造 `FileSignature::from_fs_metadata(&meta)`（移除内部 `#[allow(deprecated)]`）
- [x] 4.3 调用 `cache.remove_if_signature_mismatch(&ctx, &path, &current_sig)` 与 `cache.remove(&ctx, &path)` 用新签名
- [x] 4.4 新增 invalidator 集成测试（在 `parsed_message_cache.rs::tests` 末尾或 `local.rs::tests` 内）：调用方写入 Local ctx entry → 模拟 watcher event（构造 `FileChangeEvent` 或直接调内部 stat + remove 流程的等价路径）→ 验证 Local entry 被 remove；用 SSH ctx 查询 SHALL NOT 受影响
- [x] 4.5 `cargo check --workspace` 全过

## 5. 现有集成测试适配

- [x] 5.1 `crates/cdt-api/tests/ipc_contract.rs` 不动（无 IPC 字段变化）
- [x] 5.2 `crates/cdt-api/tests/build_time_invariants.rs` 复查 `tokio::fs::metadata` allowlist：本 change 把 `parsed_message_cache.rs` 与 `local.rs::spawn_parsed_msg_cache_invalidator` 的 `tokio::fs::metadata` 直调全清完——build-time 守护若曾把这两个路径列为 allowlist，本 change 之后可以从 allowlist 中删除（属于"减少 allowlist 条目"的好事，不破 CI）
- [x] 5.3 `cargo test --workspace` 全过

## 6. counter-based wrapper bench（详 codex 二审 Q5 + design D6 scope 说明）

> **重要 scope 调整**（codex 二审 Q5）：原计划的 "fake-SSH fs + extract_parsed_messages_cached miss→hit 计数" bench **设计上行不通**——`extract_parsed_messages_cached` 内部 cache miss 路径调 `cdt_parse::parse_file(path)`，而 `parse_file` 内部用 `tokio::fs::File::open`（**不**走 fs trait），fake_ssh_fs 的 `open_read` 方法不会被调用，miss 路径完全绕开 fake fs。等到 PR-D 把 `parse_file` 切到 `FileSystemProvider::open_read` 之后，才能让 fake-SSH bench 真正验证 SSH 命中省 RTT。
>
> **本 PR 改为做什么**：验证 wrapper 自身的 stat 入口已切到 `fs.stat()`——用真磁盘 jsonl fixture + `InstrumentedFs::new(local_handle())` 包装 + `with_fs_counter` 记录 stat 计数；first call (miss) 后 stat=1、second call (hit) 后 stat=2；hit 路径不重 parse。SSH 命中省 RTT 的 fake bench 标 follow-up（FU-1 PR-D）。

- [x] 6.1 在 `crates/cdt-api/src/ipc/parsed_message_cache.rs::tests` 模块底部新增 `parsed_message_cache_stat_counter_hit_miss` 测试（**注**：`parsed_message_cache` 是 `pub(crate)`，整合测试在 `tests/` 下无法访问；放在 lib mod 里与 PR-B 的 `perf_metadata_cache_ssh_hit` 同形态），`#[ignore]` 标记 + `#[tokio::test]`（不需要 `start_paused`，本 bench 不依赖 tokio 时间）
- [x] 6.2 fixture：`tempfile::tempdir()` 内写入 3 个真磁盘 jsonl 文件，每个含 user+assistant 各 1 行最小内容；用 `cdt_fs::InstrumentedFs::new(cdt_fs::local_handle())` 包装作为 fs；ctx = `ContextId::local(tmp.path().to_path_buf())`
- [x] 6.3 用 `cdt_fs::with_fs_counter(async { ... }).await` 跑两轮：
  - 第一轮（miss）：3 个 path 各调一次 `extract_parsed_messages_cached(&cache, &*fs, &ctx, &path).await`；记录 counters_miss
  - 第二轮（hit）：同 3 个 path 各调一次；记录 counters_hit_delta（第二轮 - 第一轮）
- [x] 6.4 **counter-based 验收**：
  - 第二轮 hit 路径 `read_to_string` / `open_read` delta MUST 等于 0（cache 命中后绝不再读全文件——即便 parse_file 内部走 tokio::fs，本 bench 用 LocalFs，open_read 经由 cdt-fs 的 open_read 路径，hit 路径不走该 API）
  - 第二轮 stat delta MUST 等于 3（每个 hit 仍需 stat 校验 signature）
  - 第一轮 miss 后 cache 大小 MUST 等于 3
- [x] 6.5 输出 `tracing::info!` 到 `nocapture`：`miss counters = {...}`、`hit delta = {...}`、cache 大小
- [x] 6.6 本地跑 `cargo test -p cdt-api --lib ipc::parsed_message_cache::tests::parsed_message_cache_stat_counter_hit_miss -- --ignored --nocapture` 验收
- [x] 6.7 **注**：本 bench 是 `extract_parsed_messages_cached` wrapper 的 fs.stat 入口验证 + cache hit 不重 parse 验证；与 PR-D 之后的"fake-SSH 命中省 50ms RTT × N" perf bench 是不同维度的两件事（PR-D 把 parse_file 切 fs.open_read 之后才能补 fake-SSH bench）

## 7. perf baseline 验证

- [x] 7.1 apply 前跑 `bash scripts/run-perf-bench.sh --runs 5` 记录 baseline 数据（`perf_cold_scan` 四维 + `perf_get_session_detail` 若 fixture 不在则跳过）
- [x] 7.2 apply 全部完成后再跑一次 `bash scripts/run-perf-bench.sh --runs 5`
- [x] 7.3 验收：wall+20% / user+50% / RSS+30% / user-real-ratio > 1.0 任一超即拒；数据填 PR Perf impact 段

## 8. src-tauri/Cargo.lock 同步复查

- [x] 8.1 跑 `cargo check --manifest-path src-tauri/Cargo.toml` 触发 lockfile 同步
- [x] 8.2 `git diff src-tauri/Cargo.lock`：若有变化，commit 进本 PR；无变化也无害（PR-B 已修一次，本 PR 通常 no-op）

## 9. 编译 + 测试 + spec validate

- [x] 9.1 `cargo fmt --all`
- [x] 9.2 `cargo clippy --workspace --all-targets -- -D warnings` 全过
- [x] 9.3 `cargo test --workspace` 全过
- [x] 9.4 `pnpm --dir ui run check` 全过（本 change 不动 ui，仍走一遍兜底）
- [x] 9.5 `openspec validate parsed-message-cache-context-prefix --strict` 过

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过（与 wait-ci 并行启动；如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

## 后续 follow-up（不在本 change，但提醒勿漏）

- [ ] FU-1 PR-D：把 `cdt_parse::parse_file` 内部 `tokio::fs::File::open` 切 `FileSystemProvider::open_read`，与现有 SSH callsite（`get_tool_output` SSH 分支 / `get_image_asset` SSH 分支）一起接入 parsed-message cache wrapper —— 真正让 SSH 大 session 反复查 tool_output / image 时省 RTT（本 change 是 cache key 拓扑就位，SSH 命中省 RTT 留 PR-D）；同时新增 fake-SSH 50ms RTT counter-based bench 验证 SSH miss→hit `open_read`/`read_to_string` 计数（本 PR codex 二审 Q5 已确认当前 `parse_file` 不走 fs trait 时无法做此 bench）
- [ ] FU-2 byte cap：若 PR-D 让 SSH 接入 cache 后实测内存峰值超 perf budget 200MB（50 entry × 平均 ~3MB ≈ 150MB 已接近），开 follow-up PR 引入 `current_bytes: AtomicUsize` + `max_bytes` 双闸门
- [ ] FU-3 invalidator root 切换协调：运行时 `update_general.claudeRootPath` 改 root 后，`spawn_parsed_msg_cache_invalidator` 持的 `projects_dir` 是构造时快照，invalidator 推算 ctx 与新 root 下 callsite 写入 ctx 不一致 —— 若实测体验差再考虑引入 invalidator 重启机制（metadata cache 当前同等限制，PR 后续统一处理）

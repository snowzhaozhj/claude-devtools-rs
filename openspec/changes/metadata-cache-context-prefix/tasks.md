## 1. cdt-ssh: `SshSessionResources` + `SshSessionManager` 扩展

- [x] 1.1 `crates/cdt-ssh/src/session.rs::SshSessionResources` 加 `host_signature: cdt_fs::HostSignature` 字段
- [x] 1.2 `connect_inner` 在 `resolve_host_via_ssh_g` 拿到 `ResolvedHost` 后立即 `let input: SshConfigDigestInput = (&resolved).into(); let host_signature = cdt_fs::HostSignature::from_ssh_config_fields(&input);`，存入构造的 `SshSessionResources`
- [x] 1.3 `SshSessionManager::context_id(&self, context_id: &str) -> Option<cdt_fs::ContextId>` 新增——`sessions.lock().await.get(context_id).map(|r| ContextId::ssh(r.host_signature.clone(), r.remote_home.clone()))`
- [x] 1.4 `insert_test_context` 加可选 `host_signature: Option<cdt_fs::HostSignature>` 参数；缺省时按 `(host, port, user)` 拼接做 fake digest 构造（用 `SshConfigDigestInput` + `from_ssh_config_fields` 跑真算法，不直接造 raw bytes，避免 fake 与真路径行为分叉）
- [x] 1.5 单测覆盖 `ssh-remote-context` spec delta scenarios：(a) connect 路径自动存 `host_signature`（用 fake `SshFileSystemProvider::fake()`，断言 resources 字段非零 digest）；(b) `context_id(&str)` 返回 `Some(ContextId)`，digest 等于 resolved 计算结果；(c) 未注册 context 返回 `None`；(d) 同 host reconnect 后 `ContextId` `==`（两次走 ssh -G 成功路径）；(e) **degraded fallback safe miss**：mock 一次 `ResolvedHost { proxyjump: Some(...), proxycommand: None, hostkeyalias: None }`（ssh -G 路径）与一次 `ResolvedHost { proxyjump: None, proxycommand: None, hostkeyalias: None }`（fallback 路径）→ digest A != digest B → ContextId 不等
- [x] 1.6 `cargo test -p cdt-ssh` 全过

## 2. cdt-api: `MetadataCache` 内部 key 升级

- [x] 2.1 `crates/cdt-api/src/ipc/session_metadata.rs::METADATA_CACHE_CAPACITY` 改 `200` → `2000`
- [x] 2.2 `MetadataCache.map: HashMap<PathBuf, _>` → `HashMap<(cdt_fs::ContextId, PathBuf), MetadataCacheEntry>`
- [x] 2.3 `MetadataCache.order: VecDeque<PathBuf>` → `VecDeque<(cdt_fs::ContextId, PathBuf)>`
- [x] 2.4 `MetadataCache::lookup(&mut self, ctx: &ContextId, path: &Path) -> Option<MetadataCacheEntry>`——内部用 tuple key 查找 / bump 队首
- [x] 2.5 `MetadataCache::insert(&mut self, key: (ContextId, PathBuf), entry: MetadataCacheEntry)`——存在则更新 + bump 队首；不存在且超容量则 pop_back evict
- [x] 2.6 测试 fixture 适配：`dummy_entry` 不变；所有 `cache.lookup(path)` 测试调用改为 `cache.lookup(&ctx, path)`；所有 `cache.insert(path, entry)` 改为 `cache.insert((ctx, path), entry)`
- [x] 2.7 新增单测 `local_vs_ssh_keys_do_not_collide`：同 path 不同 `ContextId` 插入 → lookup 用 Local ctx 命中 Local entry，lookup 用 SSH ctx 命中 SSH entry（互不串扰）
- [x] 2.8 新增单测 `lru_capacity_2000_evicts_lru_with_mixed_context`：插入 2001 个跨 Local/SSH 混合 entry → 最早一个被淘汰；总容量 ≤ 2000
- [x] 2.9 新增单测 `switch_context_does_not_clear_cache`：插入 Local entry → 模拟 ContextId 切到 SSH（直接构造 SSH ctx 查询）→ Local entry 仍在 cache（用 Local ctx 查询能命中）
- [x] 2.10 `cargo test -p cdt-api --lib session_metadata::tests` 单跑全过

## 3. cdt-api: `extract_session_metadata_cached` / `try_lookup_cached_metadata` 走 `FileSystemProvider::stat`

- [x] 3.1 `extract_session_metadata_cached` 签名改为 `pub(crate) async fn extract_session_metadata_cached(cache: &StdMutex<MetadataCache>, fs: &dyn cdt_fs::FileSystemProvider, context_id: &cdt_fs::ContextId, path: &Path) -> SessionMetadata`
- [x] 3.2 内部用 `fs.stat(path).await` 替换 `tokio::fs::metadata(path).await`；结果走 `FileSignature::from_fs_metadata(&meta)` 构造 signature（移除函数级 `#[allow(deprecated)]`）
- [x] 3.3 cache `lookup(context_id, path)` / `insert((context_id.clone(), path.to_path_buf()), entry)` 用新 tuple key API
- [x] 3.4 `try_lookup_cached_metadata` 同样签名扩展 + 内部改造；返回 `Option<SessionMetadata>` 语义不变
- [x] 3.5 现有所有针对 `extract_session_metadata_cached` 的 `tests` mod 测试（`cached_hit_returns_cached_metadata_without_rereading` / `cached_miss_when_file_size_changes` / `cached_miss_when_inode_changes_via_rename` / `cached_stat_failure_falls_through_no_write` / `cached_hit_synthesizes_is_ongoing_with_fresh_stale_check` / `cache_hit_returns_legacy_title_without_recomputing` / `cache_miss_after_signature_change_uses_new_algorithm`）SHALL 改造：构造 `let fs = cdt_fs::local_handle();` + `let ctx = cdt_fs::ContextId::local(tmp.path().to_path_buf());`，调用时传入 `&*fs, &ctx`
- [x] 3.6 新增单测 `cached_uses_fs_stat_not_tokio_fs_metadata`：用 `cdt_fs::InstrumentedFs` 包装 `LocalFileSystemProvider` + `with_fs_counter` 跑 cache miss → 断言 `FsOpCounts.stat >= 1`；cache hit 再调一次 → 断言 hit 路径 `FsOpCounts.stat` 仅增 1（cache 命中时 fs.stat 也要调一次做 signature 校验）—— 验证 fs trait 链路通畅
- [x] 3.7 新增单测 `cached_local_vs_ssh_isolation`：用 fake `FileSystemProvider` 模拟 Local + SSH 两个 ContextId 各写一条 entry → 互不串扰
- [x] 3.8 spec delta `ipc-data-api` Scenario "Local 与 SSH 同字面 path 不串扰" / "不同 SSH host 之间不串扰" 各有对应单测

## 4. cdt-api: `LocalDataApi.active_fs_and_context` 就地合成 + 6 处 callsite

> 调整（codex 二审 D2 + D3 Blocking）：**不**加 `self.fs` 字段（死字段），**不**加 `self.current_context_id` 字段（disconnect/connect 中间态会让 fs/ctx 不一致）；fs + ctx 在 `active_fs_and_context()` 内就地合成，确保来自同一快照。`switch_context` / `ssh_connect` / `ssh_disconnect` 无需触 cache 相关状态。

- [x] 4.1 新增 inherent 方法 `async fn active_fs_and_context(&self) -> (Arc<dyn FileSystemProvider>, PathBuf, cdt_fs::ContextId)`——内部 `if let Some(ssh_id) = self.ssh_mgr.active_context_id().await { if let Some(provider) = self.ssh_mgr.provider(&ssh_id).await { let remote_home = provider.remote_home().to_path_buf(); let ctx = self.ssh_mgr.context_id(&ssh_id).await.unwrap_or_else(|| ContextId::local(remote_home.clone())); return (Arc::new(provider), remote_home, ctx); } } let projects_dir = self.projects_dir.lock().await.clone(); (cdt_fs::local_handle(), projects_dir.clone(), ContextId::local(projects_dir))`
- [x] 4.2 现有 `active_fs_and_projects_dir` **保留** `pub(crate)` 兼容签名——内部转调 `active_fs_and_context` 并丢弃 ctx；本 change 不改其它 callsite，只改 metadata cache 6 处
- [x] 4.3 改造 6 处 cache callsite（`local.rs:822` / `local.rs:890` / `local.rs:1397` / `local.rs:1681` / `local.rs:1756` / `local.rs:1820`）：每处都 `let (fs, _projects_dir, ctx) = self.active_fs_and_context().await;`，然后 `extract_session_metadata_cached(&self.metadata_cache, &*fs, &ctx, &path).await` / `try_lookup_cached_metadata(&self.metadata_cache, &*fs, &ctx, &path).await`
- [x] 4.4 `scan_metadata_for_page` 入参签名扩 `fs: Arc<dyn FileSystemProvider>` + `context_id: ContextId`（spawn 异步任务内 move 进去），callsite 同步传入
- [x] 4.5 `switch_context` / `ssh_connect` / `ssh_disconnect` 三个方法 **不**做 cache 相关改动（其它行为保持现状；无需更新任何 LocalDataApi 字段）
- [x] 4.6 `cargo check --workspace` 全过
- [x] 4.7 新增 LocalDataApi 集成测试 `active_fs_and_context_returns_consistent_snapshot`：(a) 默认 Local 状态 → `(LocalFs, ContextId::local(...))`；(b) 注入 fake SSH context → `(SshFs, ContextId::ssh(...))`；(c) 模拟 active=Some 但 provider 已 disconnect（race window）→ 安全降级到 Local `(LocalFs, ContextId::local(...))`，**不**返回 `(LocalFs, ContextId::ssh(...))` 混合
- [x] 4.8 新增 LocalDataApi 集成测试 `ssh_disconnect_does_not_clear_cache`：写入 SSH ctx entry → `ssh_disconnect` → 用同 SSH ctx lookup 仍命中（直接构造同 ContextId 查询；cache wrapper 已被改造）

## 5. 现有集成测试适配

- [x] 5.1 `crates/cdt-api/tests/http_list_sessions_cache_hit_inline.rs` 若直接用 `MetadataCache` 公开 API 则适配新 tuple key；若仅通过 `LocalDataApi` IPC 间接使用则不需改
- [x] 5.2 `crates/cdt-api/tests/build_time_invariants.rs` 若 grep `tokio::fs::metadata` 不在 allowlist 内的 cache 路径，SHALL 把改造后的 session_metadata.rs 仍可通过（fs.stat 替换后该路径直调 0 次 tokio::fs::metadata，反而更干净）
- [x] 5.3 `crates/cdt-api/tests/ipc_contract.rs` 不动（无 IPC 字段变化）
- [x] 5.4 `cargo test --workspace` 全过

## 6. fake-SSH perf bench（counter-based assertion，详 design EXTRA-4 修正）

- [x] 6.1 新增 `crates/cdt-api/tests/perf_metadata_cache_ssh_hit.rs`，`#[ignore]` 标记 + `#[tokio::test(flavor = "current_thread", start_paused = true)]`
- [x] 6.2 实现 `FakeSshFs { latency: Duration, files: HashMap<PathBuf, (u64, SystemTime, String /* content */)> }` 直接实现 `FileSystemProvider`（**不**包装 `LocalFileSystemProvider`，避免真磁盘 I/O）——`kind()` 返 `FsKind::Ssh`；每个 trait 方法首先 `tokio::time::sleep(latency).await` 模拟 50ms RTT，然后从内存 HashMap 返响应
- [x] 6.3 fixture：构造 `Vec<(PathBuf, String)>` 500 个 fake path + 最小 jsonl 内容（user+assistant 两行）；mtime 用 `SystemTime::UNIX_EPOCH + Duration::from_secs(now-1000+i)` 让每个 path 独立；`FakeSshFs` latency = 50ms
- [x] 6.4 用 `ContextId::ssh(fake_host_signature(), PathBuf::from("/fake/ssh/home"))` 走 `extract_session_metadata_cached`；用 `cdt_fs::InstrumentedFs::new(fake_ssh_fs)` 包装 + `with_fs_counter(async { ...第一轮 500 次 miss... }).await` 记录 miss counters；clone counter 后再 `with_fs_counter(async { ...第二轮 500 次 hit... }).await` 记录 hit counters
- [x] 6.5 **counter-based 验收**：`assert!(hit.open_read == 0 && hit.read_to_string == 0)`（cache 命中后绝不再读全文件）；`assert!(hit.stat == 500)`（每个 hit 仍需 stat 校验 signature）；miss counters 仅用于 verbose 输出对比，不作 assertion
- [x] 6.6 输出 `tracing::info!` 到 `nocapture`：`miss = FsOpCounts{...}`、`hit = FsOpCounts{...}`、估算 SSH 真实场景节省 RTT 数
- [x] 6.7 本地跑 `cargo test -p cdt-api --test perf_metadata_cache_ssh_hit -- --ignored --nocapture` 验收

## 7. perf baseline 验证

- [x] 7.1 apply 前跑 `bash scripts/run-perf-bench.sh --runs 5` 记录 baseline 数据（`perf_cold_scan` 四维 + `perf_get_session_detail` 若 fixture 不在则跳过）
- [x] 7.2 apply 全部完成后再跑一次 `bash scripts/run-perf-bench.sh --runs 5`
- [x] 7.3 验收：wall+20% / user+50% / RSS+30% / user-real-ratio > 1.0 任一超即拒；数据填 PR Perf impact 段

## 8. src-tauri/Cargo.lock 同步

- [x] 8.1 跑 `cargo check --manifest-path src-tauri/Cargo.toml` 触发 lockfile 同步
- [x] 8.2 `git diff src-tauri/Cargo.lock`：若有变化，commit 进本 PR；无变化也无害
- [x] 8.3 `cargo build --manifest-path src-tauri/Cargo.toml --no-default-features --features prod-build` 干跑（如 host 上有 webkitgtk 可选）—— 可选验证，CI 会跑

## 9. 编译 + 测试 + spec validate

- [x] 9.1 `cargo fmt --all`
- [x] 9.2 `cargo clippy --workspace --all-targets -- -D warnings` 全过
- [x] 9.3 `cargo test --workspace` 全过
- [x] 9.4 `pnpm --dir ui run check` 全过（即便本 change 不动 ui，PR-A 影响后 worktree 可能未装依赖，跑前 `pnpm --dir ui install`）
- [x] 9.5 `openspec validate metadata-cache-context-prefix --strict` 过

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过（与 wait-ci 并行启动；如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

## 后续 follow-up（不在本 change，但提醒勿漏）

- [ ] FU-1 PR-D：把 `extract_session_metadata_with_ongoing` 内部 cache miss 扫描路径切 `FileSystemProvider::open_read`，与现有 SSH callsite（`list_sessions_skeleton` SSE inline emit / `get_session_detail` SSH 分支）一起接入 metadata cache wrapper —— 真正解 SSH 列表卡顿（本 change 是 cache key 拓扑就位，SSH 命中省 RTT 留 PR-D）
- [ ] FU-2 PR-E：把 `LocalDataApi.active_fs_and_context` 内部 fs 包装为 `InstrumentedFs` 提供 `FsOpCounts` 观测；本 change 不为此提前留字段

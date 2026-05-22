# Tasks — ssh-batch-readdir-with-metadata

## 1. fs trait SSH override + RemoteEntry → DirEntry 翻译层

- [x] 1.1 `crates/cdt-ssh/src/provider.rs::SshFileSystemProvider::read_dir` 把 `RemoteEntry → DirEntry` 映射改成：当 `e.mtime_missing = true` 时 `DirEntry.metadata = None`（不透传 `RusshSftpClient::read_dir` 填的 UNIX_EPOCH 占位）；非 missing 条仍透传 `e.metadata`。这是 codex 二审 #1 修订——避免 mtime_missing 条用 UNIX_EPOCH signature 走 batch lookup 必 mismatch 后再补 stat 的浪费路径（design D1）
- [x] 1.2 `crates/cdt-ssh/src/provider.rs::impl FileSystemProvider for SshFileSystemProvider` 加 `read_dir_with_metadata` override 直接调 `self.read_dir(path)`（复用 1.1 的翻译逻辑）；附 doc-comment 注明 SFTP READDIR reply 1 RTT 拿全 attrs 与 missing mtime 视同 mismatch 的上层语义
- [x] 1.3 `crates/cdt-ssh/src/provider.rs::tests` 加单测 `read_dir_with_metadata_uses_sftp_attrs_no_extra_stat`——`CountedFakeSftpClient`（在 FakeSftpClient 上加 `metadata_count: AtomicU32` 单文件局部）验证 SSH override 调用 1 次 read_dir + 0 次 metadata
- [x] 1.4 `crates/cdt-ssh/src/provider.rs::tests` 加单测 `read_dir_with_metadata_returns_missing_mtime_as_none`——fake client 返一条 `mtime_missing = true` 的 RemoteEntry，断言 DirEntry.metadata = None（验证 1.1 翻译层）；同时断言 `RemoteEntry.mtime_missing` 字段对 cdt-ssh 内部 caller（如 polling_watcher）仍可见（grep 既有 caller 不动）

## 2. `scan_metadata_for_page_dispatch` + `scan_metadata_for_page_batched`

- [x] 2.1 `crates/cdt-api/src/ipc/local.rs` 加 `scan_metadata_for_page_dispatch(...)` 函数（顶部加 `#[allow(clippy::too_many_arguments)]` 与既有 `scan_metadata_for_page` line 1896 一致——codex 二审 #3 显式：不抽 ScanRequest struct，保持与 PR-D 既有形态对齐），签名与既有 `scan_metadata_for_page` 完全一致；body 按 `context_id.backend_kind` 选 helper（Local → 原 `scan_metadata_for_page` / SSH → 新 `scan_metadata_for_page_batched`）
- [x] 2.2 `crates/cdt-api/src/ipc/local.rs` 加新 helper `scan_metadata_for_page_batched(...)`（同 `#[allow(clippy::too_many_arguments)]`），工作流按 design D2：early bail check → semaphore permit → `fs.read_dir_with_metadata(dir)` → build `HashMap<PathBuf, FsMetadata>`（filter `entry.metadata.is_some()` 跳过 mtime_missing 条，让它们走 mismatch sub-task 路径）→ drop permit → 逐条 page_jobs（命中走 `MetadataCache::lookup_with_known_signature` 直 broadcast；mismatch / 新增 / dir missing entry / metadata = None 走 JoinSet sub-task spawn 调 `extract_session_metadata_cached` 既有 wrapper）→ `set.join_next` 等齐 → cleanup `active_scans`
- [x] 2.3 `crates/cdt-api/src/ipc/local.rs` dir read 失败 fallback 路径：返 `Err` 时 drop permit → `tracing::warn!(target: "cdt_api::perf", project_id = %project_id, "batch read_dir_with_metadata failed, falling back to per-session scan")` → 调既有 `scan_metadata_for_page(...)`
- [x] 2.4 `crates/cdt-api/src/ipc/local.rs::list_sessions` 内 spawn 处把 `tokio::spawn(scan_metadata_for_page(...))` 改成 `tokio::spawn(scan_metadata_for_page_dispatch(...))`；ScanEntry 注册路径不变（既有 `context_id` 字段已支持 per-key abort）
- [x] 2.5 `crates/cdt-api/src/ipc/local.rs` 命中条 broadcast 时 `is_ongoing = entry.messages_ongoing` 加 inline 注释 `// SSH 跳 stale check：与 extract_session_metadata_cached SSH 分支 (session_metadata.rs:567+) 同语义；详 design D2 / change ssh-batch-readdir-with-metadata`

## 3. counted FakeRemoteSftp helper module

- [x] 3.1 `crates/cdt-api/tests/fake_remote_sftp.rs` 新文件：`CountedFakeRemoteSftp` struct 含 `files: HashMap<String, Vec<u8>>` / `dirs: HashMap<String, Vec<RemoteEntry>>` / `metadata_count` / `read_count` / `read_dir_count` / `read_lines_head_count` / `try_exists_count`（全部 `Arc<AtomicUsize>`）；构造器 `arc()` 返 `Arc<Self>`；helper `set_file` / `set_dir` 填充 fixture
- [x] 3.2 `crates/cdt-api/tests/fake_remote_sftp.rs::impl SftpClient` 每个方法 await 前 `.fetch_add(1, Ordering::SeqCst)` 对应 counter
- [x] 3.3 doc-comment 头部注明"本 helper 含 op counter，仅供 `perf_ssh_cache_hit.rs` 用；`ipc_contract.rs::FakeRemoteSftp` 与 `ssh_reconnect_lifecycle.rs::FakeRemoteSftp` 是独立 inline 副本，加计数留 followups.md 另一条 PR 收口（design D4）"

## 4. perf_ssh_cache_hit functional bench（进 CI）

- [x] 4.1 `crates/cdt-api/tests/perf_ssh_cache_hit.rs` 新文件：`#[path = "fake_remote_sftp.rs"] mod fake_remote_sftp;` 引共享 helper；fixture 含 5 sessions per project_dir
- [x] 4.2 测试 `ssh_list_sessions_first_call_populates_cache_via_batch`：首次 `list_sessions` → 用 `subscribe_session_metadata()` 拿 receiver，循环 `receiver.recv()` 直到收齐 5 条 SessionMetadataUpdate（每条 `tokio::time::timeout(Duration::from_secs(2), receiver.recv())`，timeout 即测试失败）→ 断言 `read_dir_count >= 1`（batch task 拿了 dir metadata） + `read_count >= 5`（per-session scanner 读全文）+ `metadata_count == 0`（batched 路径用 lookup_with_known_signature 跳 stat）。**禁止 sleep 硬编码**——codex 二审 #4
- [x] 4.3 测试 `ssh_list_sessions_second_call_hot_path_zero_fs_op`：snapshot counter → 二次 `list_sessions` 立刻 → 断言 counter **不变**（hot path lookup_trust_cached 0 fs op）；再 `subscribe_session_metadata()` 后 loop `recv().await` 收齐 5 条命中条 update（每条 `tokio::time::timeout(Duration::from_secs(2), ...)`）→ 断言 `read_dir_count` 增长 1（batch task 跑了）、`metadata_count` 不变（命中条跳 stat）。**禁止 sleep 硬编码**
- [x] 4.4 测试 `ssh_get_tool_output_second_call_one_stat_zero_read`：首次 `get_tool_output(sid, tu_a)` → 二次 `get_tool_output(sid, tu_b)` 同 session → 断言 `metadata_count` 增 1（cache wrapper 内部 stat 拿 signature byte-equal）、`read_count` 不变（不 read_to_string）、`read_dir_count` 不变
- [x] 4.5 测试 `ssh_disconnect_aborts_batch_task_no_orphan_broadcast`：spawn `list_sessions` 拿 receiver → 立刻 `ssh_disconnect` → 短超时 `tokio::time::timeout(Duration::from_millis(500), receiver.recv())` loop 收集所有到达的 update → 断言所有 update 的 sessionId **不属于** 当前 list 的 session（顶层 batch task abort + JoinSet drop 联级 sub-task abort 后不应有 broadcast）。**禁止裸 sleep**——用 timeout 边界判断有限延迟内无 orphan event

## 5. perf_scanner_open_read micro-bench（`#[ignore]`）

- [x] 5.1 `crates/cdt-api/tests/perf_scanner_open_read.rs` 新文件：`#[tokio::test] #[ignore]` 跑 5 runs min/median/stddev 对比 baseline（`tokio::fs::File::open + BufReader::new`）vs candidate（`LocalFileSystemProvider::open_read + BufReader::with_capacity(32 * 1024)`），两组 size：500KB jsonl + 5MB jsonl
- [x] 5.2 测试断言 candidate median ≤ baseline median × 1.3；超出 panic 打 baseline/candidate 数据点
- [x] 5.3 文件头 doc-comment 说明"`#[ignore]` 不进 CI；perf 调试本地 `cargo test -p cdt-api --release --test perf_scanner_open_read -- --ignored --nocapture`"

## 6. perf_ssh_scanner_chunked_read wall bench（`#[ignore]`）

- [x] 6.1 `crates/cdt-api/tests/perf_ssh_scanner_chunked_read.rs` 新文件：`#[tokio::test] #[ignore]`；用 `#[path = "fake_remote_sftp.rs"]` 引 helper 但额外注入 50ms RTT（`tokio::time::sleep(Duration::from_millis(50)).await` 每次 SftpClient method 调用前）+ read packet 切 32K
- [x] 6.2 fixture 准备 5MB jsonl 内容，调 `LocalDataApi::get_session_detail` 触发 cache miss → scanner via `parse_file_via_fs` → BufReader 32K + fake SFTP 32K READ
- [x] 6.3 断言 wall time < 9s（5MB / 32K ≈ 160 RTTs × 50ms = 8s + 测试 overhead buffer）

## 7. followups.md 更新

- [x] 7.1 `openspec/followups.md::ssh-remote-context::[coverage-gap] SSH 后台 batch read_dir_with_metadata + SSE 推差量（PR-D2）` body 加段："**已在 change `ssh-batch-readdir-with-metadata` 落地**：dispatch + batched helper + ssh override read_dir_with_metadata + spec MODIFIED"；标题改 `[coverage-gap → done]` 或保留并加 ✅
- [x] 7.2 `openspec/followups.md::ipc-data-api::[coverage-gap] SSH cache hit 路径计数器 + scanner dyn AsyncRead 性能基线（PR-D2）` body 加段："**已在 change `ssh-batch-readdir-with-metadata` 落地**：perf_ssh_cache_hit 进 CI（functional assertion）+ perf_scanner_open_read / perf_ssh_scanner_chunked_read `#[ignore]` 入仓"

## 8. 本地验证

- [x] 8.1 `cargo clippy --workspace --all-targets -- -D warnings` 全过
- [x] 8.2 `cargo fmt --all` 跑过
- [x] 8.3 `cargo test -p cdt-fs -p cdt-ssh -p cdt-api` 全过
- [x] 8.4 `cargo test -p cdt-api --test perf_ssh_cache_hit` 单独跑过（functional bench 进 CI）
- [x] 8.5 `cargo test -p cdt-api --release --test perf_scanner_open_read -- --ignored --nocapture` 单独跑过（dyn ≤ direct × 1.3）
- [x] 8.6 `cargo test -p cdt-api --release --test perf_ssh_scanner_chunked_read -- --ignored --nocapture` 单独跑过（5MB < 9s）
- [x] 8.7 `pnpm --dir ui run check` 全过（应无 ui 改动，但仍跑兜底）
- [x] 8.8 `openspec validate ssh-batch-readdir-with-metadata --strict` 通过
- [x] 8.9 `bash scripts/run-perf-bench.sh --runs 5` 对比 apply 前后 Local 路径 baseline 不退化（perf_cold_scan / perf_get_session_detail 四维齐看）

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

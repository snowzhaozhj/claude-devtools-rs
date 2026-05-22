## Why

PR-D（change `unify-fs-direct-calls`）落地了 G + D 两件套——SSH list 路径走 SkeletonThenStream + cache hit trust，hot path 0 fs op；但 E 段（per-project `read_dir_with_metadata` 后台 batch + SSE 推差量）显式留给 PR-D2。当前 `scan_metadata_for_page` 内部对 page_jobs 里每条 session 单独跑 `extract_session_metadata_cached`，每次先调 `fs.stat(path)` 拿 signature 比对 cache——SSH 上 `Arc<Mutex<SftpSession>>` 全锁串行，**50 sessions × 50ms = 2.5s 后台开销**。Hot path 用户已感知瞬时（cache hit trust），但 cache miss 路径与"外部进程改动 → 推差量"流的真实 wall 时间仍堆在串行 stat 上。

PR-A `FileSystemProvider::read_dir_with_metadata` trait method 已就位（local override 已 batch + 并发 stat）；PR-D 已在 `MetadataCache` / `ParsedMessageCache` 加 `lookup_with_known_signature` helper + ADR `#[allow(dead_code)]` 等 PR-D2 wire。SSH `read_dir` impl 已经从 SFTP READDIR reply 直接拿 entry attrs（见 `crates/cdt-ssh/src/provider.rs:421-450`），意味着 SSH 上"1 RTT 拿全 dir + 所有 entry 的 mtime/size"是协议天然支持的——本 PR 需要做的事：

1. **SSH override `read_dir_with_metadata`**：trait default 是 `read_dir + 逐项 stat`（N+1 RTT）。SSH `read_dir` 已带 metadata，override 直接复用 → 1 RTT。
2. **`scan_metadata_for_page` 加 SSH batch 分支**：SSH ctx 下 spawn 后台 task 先 `fs.read_dir_with_metadata(project_dir)` 一次拿全 dir entry metadata → build `HashMap<PathBuf, FsMetadata>` → 对每条 page_jobs path lookup metadata + 调 `MetadataCache::lookup_with_known_signature(ctx, path, &sig)` 命中跳 stat；mismatch / 新增 / dir read 失败 → 走原 cache miss 路径（cache wrapper 内部 stat + scanner）→ SSE 推差量。本 PR 保留 Local 路径走 per-session `extract_session_metadata_cached`（Local stat 廉价、cache invalidation 节奏与既有 metadata wrapper 一致，留 PR-D3 评估是否要改）。
3. **active_scans abort 复用既有路径**：新 batch task 走同一 `tokio::spawn` + `ScanEntry { generation, handle, context_id }` 注册路径——`ssh_disconnect` / `switch_context` / `shutdown_ssh_all` 已通过 `abort_scans_for_context` 按 ContextId 精确 abort（详 design D3-bis），不需要新增 abort 入口。
4. **补齐 PR-D §12 micro-bench 三件套**：`perf_scanner_open_read` / `perf_ssh_cache_hit`（含 fake-SSH op counter，断言"二次 list cache hit → batch 路径 read_dir_with_metadata 计数 ≥ 1，stat = 0"）/ `perf_ssh_scanner_chunked_read`（fake-SSH 50ms RTT + 32K packet，5MB jsonl < 9s threshold）。

**Non-Goals**：BackendPolicy struct（PR-E）；SFTP message-id pipeline（PR-F，真消除单文件大 jsonl scan 多 RTT）；Local 路径也走 batch（留 PR-D3 评估）；新 IPC 字段或前端契约改动。

## What Changes

- **fs-abstraction**: `SshFileSystemProvider` SHALL override `read_dir_with_metadata` 直接复用 `read_dir`（SFTP READDIR reply 自带 attrs），1 RTT；spec MODIFIED Requirement `FileSystemProvider trait 暴露 7 个核心方法` 加 Scenario "SSH override read_dir_with_metadata 复用 read_dir 不退化"。
- **ssh-remote-context**: MODIFIED 现有 Scenario "SSH list 路径 hot path cache hit trust"——把"per-session 串行 extract_session_metadata_cached，N→1 batch 优化留 PR-D2 follow-up"段改成"SSH ctx 后台校验 SHALL 通过 fs.read_dir_with_metadata(project_dir) 1 RTT 拿全 dir metadata，对 page_jobs 每条调 MetadataCache::lookup_with_known_signature 命中跳 stat"；MODIFIED "SSH list 路径冷启动走 SkeletonThenStream + page_jobs" 同步删除 PR-D2 follow-up 标注；新增 Scenario "SSH 后台 batch 校验 fs op 形态钉死"（fs.read_dir_with_metadata = M projects、fs.stat = 0 全命中、cache miss 条额外 1 stat + 1 open_read）。
- **ipc-data-api**: MODIFIED Requirement `Emit session metadata updates` 加段：`scan_metadata_for_page` SHALL 按 `fs ContextId.backend_kind` 分两条路径——`Local` 走既有 per-session `extract_session_metadata_cached`；`Ssh` 走新 helper `scan_metadata_for_page_batched`（per project_dir 一次 `fs.read_dir_with_metadata` + `MetadataCache::lookup_with_known_signature` 批量校验）；两条路径共享 `active_scans` 注册表、`Semaphore(METADATA_SCAN_CONCURRENCY=8)` 限流、`context_generation` race-free 校验。新增 Scenario "SSH ctx 后台校验走 batch read_dir_with_metadata 而非 per-session stat"。
- 新增 3 个 perf bench（`perf_ssh_cache_hit.rs` functional 断言进默认 CI；其余两个 `#[ignore]` 不进 CI 默认）+ 在 `crates/cdt-api/tests/` 共享一份 counted `FakeRemoteSftp` helper（`Arc<AtomicUsize>` metadata/read/read_dir 计数）。

## Impact

- Affected specs: `fs-abstraction` / `ssh-remote-context` / `ipc-data-api`
- Affected code:
  - `crates/cdt-ssh/src/provider.rs`（加 `read_dir_with_metadata` override + 单测）
  - `crates/cdt-api/src/ipc/local.rs`（`scan_metadata_for_page_dispatch` + 新 helper `scan_metadata_for_page_batched`；caller 切 dispatch）
  - `crates/cdt-api/tests/fake_remote_sftp.rs`（新；CountedFakeRemoteSftp 共享模块）
  - `crates/cdt-api/tests/perf_ssh_cache_hit.rs`（新；进 CI）
  - `crates/cdt-api/tests/perf_scanner_open_read.rs`（新；`#[ignore]`）
  - `crates/cdt-api/tests/perf_ssh_scanner_chunked_read.rs`（新；`#[ignore]`）
  - `openspec/followups.md`（标记 PR-D2 两条 coverage-gap 落地）
- BREAKING: 否（fs trait override 是性能补全，行为契约不变；IPC 字段不变；前端无联动改动）

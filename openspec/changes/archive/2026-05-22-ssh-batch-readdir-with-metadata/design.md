## Context

PR-D（change `unify-fs-direct-calls`）落地 G + D 两件套——SSH list hot path 0 fs op + SkeletonThenStream 与 Local 同入口。E 段（per-project `fs.read_dir_with_metadata` 后台 batch + SSE 推差量）显式 follow-up（详 `openspec/followups.md::ssh-remote-context::SSH 后台 batch read_dir_with_metadata + SSE 推差量（PR-D2）`）。

**底层数据当前状态**（grep 自 main 5d0207a）：

```
trait FileSystemProvider::read_dir_with_metadata（crates/cdt-fs/src/provider.rs:28）
  - default impl: read_dir + 逐项 stat（N+1 RTT）
  - Local override（crates/cdt-fs/src/local.rs:152）: 内部 read_dir_entries(true)
  - SSH: 未 override —— 当前走 default = N+1 RTT；但 SSH read_dir 实现
    （crates/cdt-ssh/src/provider.rs:421-450 RusshSftpClient::read_dir）已经
    从 SFTP READDIR reply 直接拿 entry attrs，意味着 SSH override 直接复用
    read_dir 就是 1 RTT 拿全

scan_metadata_for_page（crates/cdt-api/src/ipc/local.rs:1897）
  - 入 page_jobs 的每条 (session_id, jsonl_path) spawn 并发 task
  - 每个 task 调 extract_session_metadata_cached（内部一定先 fs.stat 拿 signature）
  - signature 命中 → 返 cache 内容；mismatch → uncached scanner
  - SSH 上每个 task 都要 1 RTT stat → N sessions × 50ms 串行 wall

MetadataCache::lookup_with_known_signature（crates/cdt-api/src/ipc/session_metadata.rs:447 PR-D 加，#[allow(dead_code)]）
  - 调用方先用 fs.read_dir_with_metadata 拿全 dir metadata；用每条 path 对应
    metadata 调本 helper 直接命中（跳过内部 stat）
  - 本 PR 把这条 helper wire 到 SSH batch 路径
```

**SFTP READDIR reply 实测**：`russh_sftp::client::SftpSession::read_dir` 返 `ReadDir` iter 每条 `DirEntry` 含 `metadata()`（size + mtime）—— PR-A 实现 `RusshSftpClient::read_dir`（line 421-450）已经把 metadata 填进 `RemoteEntry`，SSH provider 的 `read_dir` 已经返带 metadata 的 `DirEntry`。即 SFTP 协议层本身就是 1 RTT 拿全 dir + 全 entry attrs，本 PR 只是让 trait dispatch 复用此原生能力。

性能基线（`tests/perf-baseline.json` + `.claude/rules/perf.md`）：
- `perf_cold_scan` wall ≤ 500ms / user/real ≤ 0.6 / RSS ≤ 50000kb
- `perf_get_session_detail` wall ≤ 500ms / user/real ≤ 0.7 / RSS ≤ 140000kb

本 PR 是 SSH-only 改动，Local 基线不动；SSH 路径无 perf-baseline.json 入口（CI runner 无 SSH corpus），用本 PR 新增 micro-bench `perf_ssh_scanner_chunked_read` 验证 5MB jsonl 9s 上限作 SSH 端 sanity gate。

## Goals / Non-Goals

**Goals:**
- `SshFileSystemProvider::read_dir_with_metadata` override 直接调 `read_dir`（已带 metadata），避免 trait default N+1 RTT 退化
- `scan_metadata_for_page` 按 `ContextId.backend_kind` 分流：`Local` 走既有路径不动；`Ssh` 走新 helper `scan_metadata_for_page_batched`，先 `fs.read_dir_with_metadata(project_dir)` 1 RTT 拿全 dir metadata，build `HashMap<PathBuf, FsMetadata>`，对每条 page_jobs path lookup metadata + 调 `MetadataCache::lookup_with_known_signature(ctx, path, &sig)` 命中跳 stat；mismatch / 新增 / dir read 失败 → 走原 cache miss 路径
- 命中条 SHALL broadcast 既有 cache 值的 `SessionMetadataUpdate`（前端按 sessionId 增量 patch），与既有 per-session 路径产生的 update 形态完全一致
- 共享 `active_scans` 注册表 + `Semaphore(METADATA_SCAN_CONCURRENCY=8)` + `context_generation` race-free 校验
- 加 3 个 perf bench：
  - `perf_scanner_open_read.rs`（PR-D §12 D1 micro-bench，`#[ignore]`）—— Local 端 candidate ≤ baseline × 1.3 median
  - `perf_ssh_cache_hit.rs`（**不** `#[ignore]`，进 CI）—— 走 `LocalDataApi::list_sessions` / `get_session_detail` / `get_tool_output` 三个 user-facing handler，counted `FakeRemoteSftp` 断言 batch fs op 形态
  - `perf_ssh_scanner_chunked_read.rs`（`#[ignore]`）—— fake-SSH 注入 50ms RTT / read packet 32K，5MB jsonl scan wall < 9s threshold
- followups.md 把"PR-D2"两条 coverage-gap 标记为已落地

**Non-Goals:**
- **不**为 Local 路径加 batch 分支（Local stat 廉价 + cache invalidation 节奏稳定，留 PR-D3 评估）
- **不**改 `BackendPolicy` struct / wire 到 `LocalDataApi` 字段（PR-E 范围）
- **不**改 SFTP message-id pipeline 让单 file 多 SFTP READ 并发（PR-F 方案 C；本 PR 单 file scan 仍 N×RTT）
- **不**改 IPC 字段 / 前端契约（命中 / mismatch / 新增 / 删除 都通过既有 `session_metadata_update` event）
- **不**改 `ParsedMessageCache::lookup_trust_cached` 在 `get_tool_output` / `get_image_asset` 的 wire（与 batch readdir 解耦，留 PR-E 或独立 PR）
- **不**扩展 `fs.read_dir_with_metadata` 支持 cursor / partial pagination（trait spec 已显式拒，详 fs-abstraction "fs trait 不承担分页与排序语义" Requirement）

## Decisions

### D1: `SshFileSystemProvider::read_dir_with_metadata` 直接复用 `read_dir`

**问题**：trait default 是 `read_dir + 逐项 stat`，SSH 上 N+1 RTT；但 SSH 的 `read_dir` 已经从 SFTP READDIR reply 把 entry attrs 填进 `DirEntry.metadata`。default 走完会再 stat 一遍，浪费 N RTT。

**修法**：

```rust
// crates/cdt-ssh/src/provider.rs
#[async_trait]
impl FileSystemProvider for SshFileSystemProvider {
    // ... existing impls ...

    /// SSH `read_dir` 已经从 SFTP READDIR reply 拿全 entry attrs（详
    /// `RusshSftpClient::read_dir` 实现：每条 RemoteEntry 含 metadata.size + mtime
    /// 或 mtime_missing 标志）。trait default 会再逐项 stat 一遍浪费 N RTT；
    /// 本 override 直接复用 read_dir 的转换路径（DirEntry 把 mtime_missing 翻译为
    /// metadata = None，详 SshFileSystemProvider::read_dir 内 RemoteEntry → DirEntry
    /// 映射）。
    ///
    /// 缺 mtime 的 entry —— SFTP 协议允许 server 不返 modify time 字段；此时
    /// DirEntry.metadata = None，caller SHALL 把 metadata = None 视同 cache
    /// mismatch（走 cache wrapper miss 路径让内部 stat 补齐）。
    async fn read_dir_with_metadata(&self, path: &Path) -> Result<Vec<DirEntry>, FsError> {
        self.read_dir(path).await
    }
}
```

**RemoteEntry → DirEntry 映射层 mtime_missing 驱动 metadata=None**（codex 二审 #1）：当前 SSH provider 在 `RemoteEntry → DirEntry` 转换处（`SshFileSystemProvider::read_dir` 内 `.map(|e| DirEntry { ... })`）只透传 `e.metadata`——但 `RusshSftpClient::read_dir`（line 432-447）在 `mtime_missing = true` 时仍填 `Some(FsMetadata { mtime: UNIX_EPOCH, ... })`（保留 size 字段，仅 mtime 是占位）；这条 metadata 用 `FileSignature::from_fs_metadata` 计算后 mtime 永远 UNIX_EPOCH，与 cache 中真 mtime 永远 mismatch → 视同 cache miss 走 wrapper → 内部 stat 拿真 mtime → 浪费 1 次 stat。

本 PR SHALL 让 `SshFileSystemProvider::read_dir` 的 `RemoteEntry → DirEntry` 映射在 `mtime_missing = true` 时把 `DirEntry.metadata = None`（**不**透传 占位 UNIX_EPOCH metadata）；这样上层 batch 路径见到 `metadata = None` → `lookup_with_known_signature` 跳过该条 → mismatch sub-task 走 cache wrapper miss 路径，与"non-missing"路径下的 mismatch 语义完全一致；省去"占位 mtime 走 batch lookup 必 fail 再补 stat" 的额外语义。

**对 RemoteEntry 字段不变**：`RemoteEntry.mtime_missing` 字段保留供 `cdt-ssh::polling_watcher` 等 cdt-ssh 内部 caller 用（独立语义，与 fs trait 抽象解耦）；仅 `SshFileSystemProvider::read_dir` 的 `RemoteEntry → DirEntry` 转换层做翻译。

这条语义与 trait default impl 不完全一致——default 会 unconditional stat，本 override 在 missing 场景下不补 stat 而让上层 batch wrapper 处理；上层 batch 校验路径恰好接受 "metadata 缺失视同 mismatch → 走 cache wrapper miss" 与 cache 语义自洽。

**为何不让 SSH `read_dir` 内部对 missing 条 fallback stat**：会让 `read_dir` 在某些 server 行为下退化成 N+1 RTT，违反"列表 1 RTT"的契约；本 PR override 把"missing 视同 mismatch"语义钉死在上层 caller，更可控。

**替代方案**：
- (a) 不 override，沿用 default → 否决（N+1 RTT 性能不可接受）
- (b) override 内对 missing 条 fallback stat → 否决（退化为 N+1 RTT 在 missing 场景）
- (c) 让 `RusshSftpClient::read_dir` 内部 fallback stat → 否决（污染单点 `read_dir` 契约，本 PR 改动面扩大）

### D2: `scan_metadata_for_page` 按 `ContextId.backend_kind` 分流，SSH 走 batch helper

**问题**：现有 `scan_metadata_for_page(project_id, dir, page_jobs, tx, active_scans, ..., fs, context_id, ...)` 对每条 `(session_id, jsonl_path)` spawn 并发 task → 每个 task 调 `extract_session_metadata_cached` 必跑一次 `fs.stat(path)`。SSH ctx 上 stat 走 `Arc<Mutex<SftpSession>>` 全锁 = N 次串行 50ms RTT。

**修法**：在 `LocalDataApi` 内引入新 helper `scan_metadata_for_page_batched`，签名与既有 `scan_metadata_for_page` 同形（同一组参数），caller 端按 `context_id.backend_kind` 选用：

```rust
// crates/cdt-api/src/ipc/local.rs（伪代码）

async fn scan_metadata_for_page_dispatch(
    project_id: String,
    dir: std::path::PathBuf,
    page_jobs: Vec<(String, std::path::PathBuf)>,
    tx: broadcast::Sender<SessionMetadataUpdate>,
    active_scans: Arc<std::sync::Mutex<HashMap<String, ScanEntry>>>,
    cleanup_key: String,
    my_generation: u64,
    metadata_cache: Arc<std::sync::Mutex<MetadataCache>>,
    semaphore: Arc<Semaphore>,
    root_generation: Arc<AtomicU64>,
    expected_root_generation: u64,
    worktree_meta_cache: Arc<std::sync::RwLock<HashMap<String, WorktreeMeta>>>,
    fs: Arc<dyn FileSystemProvider>,
    context_id: cdt_fs::ContextId,
    context_generation: Arc<AtomicU64>,
    expected_context_generation: u64,
) {
    if context_id.backend_kind == cdt_fs::FsKind::Ssh {
        scan_metadata_for_page_batched(/* same args */).await;
    } else {
        scan_metadata_for_page(/* unchanged */).await;
    }
}

async fn scan_metadata_for_page_batched(
    /* same args as scan_metadata_for_page */
) {
    // 1. early bail: fs/ctx/root generation 校验同 scan_metadata_for_page
    if context_generation.load(Ordering::SeqCst) != expected_context_generation { return; }
    if root_generation.load(Ordering::SeqCst) != expected_root_generation { return; }

    // 2. 一次 fs.read_dir_with_metadata 拿全 dir entry attrs（acquire 同一
    //    全局 semaphore permit 不超 8 并发）
    let _permit = match semaphore.clone().acquire_owned().await { Ok(p) => p, Err(_) => return };
    let entries = match fs.read_dir_with_metadata(&dir).await {
        Ok(e) => e,
        Err(_) => {
            // dir read 失败 → fallback 到既有 per-session 路径（功能正确性兜底）
            drop(_permit);
            tracing::warn!(target: "cdt_api::perf", project_id = %project_id, "batch read_dir_with_metadata failed, falling back to per-session scan");
            scan_metadata_for_page(/* same args */).await;
            return;
        }
    };
    let by_name: HashMap<PathBuf, FsMetadata> = entries.into_iter()
        .filter_map(|e| {
            let meta = e.metadata?;
            Some((dir.join(e.name), meta))
        })
        .collect();
    drop(_permit);

    // 3. 逐条 page_jobs 校验 + 命中直 broadcast / mismatch 走 cache miss spawn
    let mut set = JoinSet::new();
    for (session_id, jsonl_path) in page_jobs {
        // 命中条：用 lookup_with_known_signature
        if let Some(meta) = by_name.get(&jsonl_path) {
            let sig = FileSignature::from_fs_metadata(meta);
            let cached = metadata_cache
                .lock()
                .expect("metadata cache mutex poisoned")
                .lookup_with_known_signature(&context_id, &jsonl_path, &sig);
            if let Some(entry) = cached {
                if context_generation.load(Ordering::SeqCst) != expected_context_generation { continue; }
                let group_id = worktree_meta_cache.read().ok()
                    .and_then(|c| c.get(&project_id).map(|m| m.group_id.clone()));
                let _ = tx.send(SessionMetadataUpdate {
                    project_id: project_id.clone(),
                    session_id,
                    title: entry.title,
                    message_count: entry.message_count,
                    is_ongoing: entry.messages_ongoing, // SSH 跳 stale check（详 D2-stale）
                    git_branch: entry.git_branch,
                    group_id,
                });
                continue;
            }
        }

        // mismatch / 新增 / dir missing entry → spawn 单独 task 走原 cache wrapper miss 路径
        let sem = semaphore.clone();
        let tx = tx.clone();
        let project_id = project_id.clone();
        let cache = metadata_cache.clone();
        let fs_clone = fs.clone();
        let ctx_clone = context_id.clone();
        let context_generation = context_generation.clone();
        let worktree_meta_cache = worktree_meta_cache.clone();
        set.spawn(async move {
            let Ok(_p) = sem.acquire_owned().await else { return };
            if context_generation.load(Ordering::SeqCst) != expected_context_generation { return; }
            let meta = extract_session_metadata_cached(&cache, &*fs_clone, &ctx_clone, &jsonl_path).await;
            if context_generation.load(Ordering::SeqCst) != expected_context_generation { return; }
            let group_id = worktree_meta_cache.read().ok()
                .and_then(|c| c.get(&project_id).map(|m| m.group_id.clone()));
            let _ = tx.send(SessionMetadataUpdate {
                project_id, session_id, title: meta.title,
                message_count: meta.message_count, is_ongoing: meta.is_ongoing,
                git_branch: meta.git_branch, group_id,
            });
        });
    }
    while set.join_next().await.is_some() {}

    // 4. cleanup（与 scan_metadata_for_page 同形）
    if let Ok(mut scans) = active_scans.lock() {
        if let Some(entry) = scans.get(&cleanup_key) {
            if entry.generation == my_generation { scans.remove(&cleanup_key); }
        }
    }
}
```

**fs op 形态对比**（codex 二审 #2 修订——既有 per-session 路径的真实 op 数是 `N stat + M open_read`，不是恒定 `2N`）：

| 场景 | PR-D 既有 per-session 路径 | 本 PR batch 路径 | 收益 |
|---|---|---|---|
| all-hit（N 条全 cache hit byte-equal）| `N stat + 0 open_read` = N ops | `1 read_dir_with_metadata + 0 stat + 0 open_read` = 1 op | 省 N-1 RTT（典型 50 sessions：50 → 1 RTT） |
| partial-hit（N 条中 H 条 hit、M 条 mismatch / 新增，H + M = N）| `N stat + M open_read` = N + M ops | `1 read_dir_with_metadata + M stat + M open_read` = 1 + 2M ops（mismatch 走 cache wrapper miss） | 省 N - 1 - M RTT，N ≫ M 时显著（H = 40, M = 10 → 50 ops → 21 ops） |
| all-miss（N 条全 mismatch / 新增）| `N stat + N open_read` = 2N ops | `1 read_dir_with_metadata + N stat + N open_read` = 1 + 2N ops（**多 1 op**） | -1（batch 反而多一次 read_dir） |

**接受条件**：典型用户场景 cache hit rate ≥ 80%（已访问过的 SSH host），batch 路径 strict superior。冷启动 all-miss 场景多 1 RTT（~50ms）相对"先用骨架渲染 + 后台异步刷"的 UX 已显式分离（hot path cache trust 0 op），不构成回归。

**mtime_missing entry 单条形态**：`DirEntry.metadata = None`（mapping 层翻译）→ `lookup_with_known_signature` 拿不到 entry → 走 mismatch sub-task 调 `extract_session_metadata_cached` 内部 stat + 可能 open_read（与一般 mismatch 路径一致），不构成额外 op 路径。

**为何 fallback to scan_metadata_for_page 而非返错**：dir read 失败可能是瞬时（PR-A `with_retry` 3 次后仍失败），但单条 session jsonl 仍可能读得到（独立 SFTP READ）；fallback 让功能正确性优先，性能退化为既有 PR-D 路径。

**为何不让命中条也 spawn task**（直 broadcast 而非 set.spawn）：命中条只需调一次 cache lookup（µs 级 sync 操作）+ broadcast 一次（µs 级 sync）；spawn task 反而引入 spawn overhead（µs 级也无收益）。mismatch 条需要 spawn 是因为 `extract_session_metadata_cached` 是 async I/O。

**为何命中条手算 `is_ongoing = entry.messages_ongoing` 而非走 `is_session_stale` 合成**：与 `extract_session_metadata_cached` 内部 SSH `backend_skips_stale` 分支保持一致（详 `crates/cdt-api/src/ipc/session_metadata.rs:567+`）—— SSH 远端 mtime 与本机 `SystemTime::now()` 跨 clock domain 不可比对，5min 阈值在远端时钟回拨场景下产生 false positive/negative，统一走"messages_ongoing 直返"语义。

**为何 dispatch 函数与 scan_metadata_for_page 共存而非 inline 改 scan_metadata_for_page**：scan_metadata_for_page 当前签名稳定、被 ipc_contract 集成测试覆盖；新 batch helper 独立函数让 unit test 易写、language server jump-to-def 清晰。dispatch 函数只做 backend_kind 分叉，无额外逻辑。

**替代方案**：
- (a) 让所有 backend 都走 batch → 否决（Local stat 廉价；改变 cache invalidation 节奏；scope 扩散且 Local 已无 perf 问题）
- (b) 在 `scan_metadata_for_page` 内部按 backend_kind 分叉 → 否决（函数本来已 too_many_arguments + 复杂；inline 后 cyclomatic complexity 爆，hard to test）
- (c) 让 batch 复用 PR-D `MetadataCache::lookup_with_known_signature` 之外**也**复用 `ParsedMessageCache::lookup_with_known_signature` 来 cache messages → 否决（messages cache 是 get_tool_output / get_image_asset 路径，与 list_sessions metadata 路径无关，scope 扩散）

### D3: 后台 batch task 与既有 per-key abort / context_generation 路径的兼容性

**问题**：新 batch helper 与既有 `scan_metadata_for_page` 共享 `active_scans` map 入口 + `context_generation` race 校验，能否复用既有 abort 路径？

**分析**：
- caller 端 `list_sessions` 内 spawn 时 insert `ScanEntry { generation, handle, context_id }` 到 `active_scans`——`context_id` 字段在 PR-D `unify-fs-direct-calls` 已加（per-key abort 按 ContextId 精确）。本 PR 走 `dispatch` 函数 spawn，由 `dispatch` 内部按 backend_kind 选 helper；spawn 时机不变，注册到 active_scans 的 ScanEntry 形态不变 → `abort_scans_for_context` / `abort_local_scans` / `shutdown_ssh_all` 三个 abort 入口都自然覆盖新 batch task。
- batch helper 内部 set.spawn 的 mismatch sub-task 是 batch helper 自己的子任务，由顶层 `JoinSet::join_next` 等待——顶层 batch task 被 abort 时 JoinSet drop 触发 sub-task abort（tokio JoinSet 语义）；不需要把 sub-task 也注册到 active_scans。
- `context_generation` 每次 fs op 前后双 check，与 scan_metadata_for_page 既有路径同语义；ssh_disconnect 触发的 bump 仍能让 in-flight batch task 在下次 check 时 silent return。

**结论**：abort 路径完全复用，无新加入口；本 PR 在 ssh-remote-context spec 加 Scenario "SSH 后台 batch 校验 task 在 ssh_disconnect 时 abort"（既有 Scenario 复用，措辞 MODIFIED 明确"含 batched 路径"）。

**替代方案**：
- (a) 给 batch sub-task 独立 active_scans key → 否决（顶层 batch task 已 abort 自动 cascade sub-task；额外 key 增加注册表噪音）
- (b) batch task 不注册 active_scans 走独立 cancel token → 否决（破坏既有 race-free cleanup 模式，需另写 abort 路径）

### D4: counted `FakeRemoteSftp` 抽 helper 模块共享

**问题**：`perf_ssh_cache_hit.rs` 需要 `FakeRemoteSftp` 暴露 `metadata_count` / `read_count` / `read_dir_count` 计数；既有 `ipc_contract.rs::FakeRemoteSftp` 与 `ssh_reconnect_lifecycle.rs::FakeRemoteSftp` 各有副本均无计数。

**修法**：在 `crates/cdt-api/tests/fake_remote_sftp.rs` 加共享 helper module。Rust integration test 每个 file 是独立 crate，跨文件共享 helper 用 `#[path]` 引入（Rust 官方惯例之一）：

```rust
// crates/cdt-api/tests/fake_remote_sftp.rs  ← new shared helper
pub struct CountedFakeRemoteSftp {
    files: std::collections::HashMap<String, Vec<u8>>,
    dirs: std::collections::HashMap<String, Vec<RemoteEntry>>,
    pub metadata_count: Arc<AtomicUsize>,
    pub read_count: Arc<AtomicUsize>,
    pub read_dir_count: Arc<AtomicUsize>,
    pub read_lines_head_count: Arc<AtomicUsize>,
    pub try_exists_count: Arc<AtomicUsize>,
}

// crates/cdt-api/tests/perf_ssh_cache_hit.rs
#[path = "fake_remote_sftp.rs"]
mod fake_remote_sftp;
use fake_remote_sftp::CountedFakeRemoteSftp;
```

**为何不动既有 ipc_contract / ssh_reconnect 的 inline FakeRemoteSftp**：本 PR 改这两个文件的 inline 副本会触动 N 处既有测试断言 + ipc_contract 是发布尾段 N.2 CI 必跑 → 改大且 scope 扩散。followups.md 既有独立 coverage-gap 项跟踪 ipc_contract.rs 加计数器是单独 PR——本 PR 不做。

**替代方案**：
- (a) 改 ipc_contract.rs FakeRemoteSftp 加计数 → 否决（scope 扩散 + 触发 ipc_contract.rs 大量断言重看）
- (b) 在 cdt-ssh 内 production code 加 InstrumentedSftpClient → 否决（生产 dep 引入测试用计数器是过度设计；testing-only helper 留 cdt-api/tests/ 即可）

### D5: perf bench 三件套的 `#[ignore]` + 验收阈值

- `perf_scanner_open_read.rs`：5 runs min/median/stddev 对比 `tokio::fs::File::open + BufReader` vs `LocalFileSystemProvider::open_read + BufReader::with_capacity(32 * 1024)`，文件大小 500KB + 5MB jsonl 两组。验收：candidate median ≤ baseline median × 1.3。**仅** `#[ignore]`（不进默认 CI；perf 调试时本地 `--ignored` 跑）。
- `perf_ssh_cache_hit.rs`：走 `LocalDataApi::list_sessions` / `get_session_detail` / `get_tool_output` 三个 user-facing handler，counted `CountedFakeRemoteSftp` 断言：
  - `list_sessions` 首次（cache 冷）→ 触发 batch task → `read_dir_count >= 1` / `read_count >= N`（per-session scanner 读全文）
  - `list_sessions` 二次（cache 暖）hot path → 立刻看 counter 仍为首次后值（hot path lookup_trust_cached → 0 新 fs op）
  - `list_sessions` 二次后台 batch 完成 → `read_dir_count` 增长 ≥ 1，`metadata_count` 不增长（batch 用 lookup_with_known_signature 跳 stat）
  - `get_tool_output` 同 session 二次 → `metadata_count` 增 1（cache wrapper 内部 stat 拿 signature byte-equal），`read_count` / `read_dir_count` 不变（不 read_to_string、不 read_dir）
  - **不**加 `#[ignore]`——这是 functional assertion 不是 perf wall time，**进默认 CI**。
- `perf_ssh_scanner_chunked_read.rs`：fake-SSH 实现注入 `tokio::time::sleep(50ms)` 每次 `read()` 调用 + 切 32K packet，5MB jsonl scan wall < 9s threshold。`#[ignore]` 不进默认 CI。

**为何 cache_hit assertion test 不 `#[ignore]`**：counter 断言是 functional contract（"batch 真在用 read_dir_with_metadata 而非 stat"），CI 必须拦回归；wall time bench 才需要 `#[ignore]` 避免污染 CI noise。

**等待后台 batch 完成的机制**：`perf_ssh_cache_hit` 内通过 `subscribe_session_metadata()` 拿 broadcast Receiver，调 `list_sessions` 后 `recv_timeout(2s)` 收齐预期 SessionMetadataUpdate 数；收齐即视为 batch task 完成（broadcast 是 batch task 的最后步骤），再 snapshot counter 断言。

**替代方案**：
- (a) 全部 `#[ignore]` → 否决（counter assertion 是 functional 不是 perf）
- (b) 全部进 CI → 否决（perf bench wall 时间不稳定 + CI runner 无 SSH corpus）

## Risks / Trade-offs

| 风险 | 缓解 |
|---|---|
| SSH `read_dir_with_metadata` override 后 SFTP server 不返 mtime 的 entry（`mtime_missing = true`）→ batch lookup miss → fallback 到 cache miss 路径 stat 一次拿 mtime → 等于退化为 N+1 RTT | mtime missing 是 SFTP 协议允许的罕见行为；本 PR override 不补 stat，把 missing 视同 mismatch 走 cache wrapper miss 路径；批量场景 typical 路径仍 1 RTT（绝大多数 server 返 mtime）；perf_ssh_cache_hit test 覆盖 mtime present 的 typical 路径 |
| `scan_metadata_for_page_batched` 与 `scan_metadata_for_page` 维护两条相似路径，二者 drift | dispatch 函数集中按 backend_kind 选；两条函数共享 `extract_session_metadata_cached` 与 `MetadataCache` helper；spec 加 Scenario 钉契约；codex 二审重点查二者 fs op 形态一致 |
| batch 路径下命中条手算 `is_ongoing = entry.messages_ongoing` 与 `extract_session_metadata_cached` SSH 分支同语义但散落两处，未来 SSH stale 策略改时易漏 | 加 inline 注释引用 `extract_session_metadata_cached` SSH 分支 + design D2；future SSH stale 改时 grep "messages_ongoing" 能定位双处 |
| dir read 失败 fallback 到 per-session scan 仍 N×RTT，但功能正确性优先 | fallback 路径加 `tracing::warn!` 让运维侧可见；用户感知层面 hot path 仍 cache trust 0 RTT |
| `CountedFakeRemoteSftp` 与既有 `ipc_contract.rs::FakeRemoteSftp` 副本 drift | 文档 `tests/fake_remote_sftp.rs` 头部 doc-comment 注明"本副本仅供 perf_ssh_cache_hit 用 + 含计数；ipc_contract.rs 副本不带计数留 followups.md 另一条 PR 收口" |
| Local perf baseline 退化 | 本 PR 不改 Local 路径；apply 前后跑 `bash scripts/run-perf-bench.sh --runs 5` 兜底 |
| codex 二审报新问题 | propose 阶段 codex 二审 design.md（IPC 字段 + 性能关键 + 后台并发三项命中）；apply push 后再调 codex 多轮 |
| trait default 与 SSH override 行为差异（missing mtime 场景） | spec fs-abstraction Scenario 显式钉死 "SHALL 不退化为 N+1 RTT 即使部分 entry mtime missing"；上层 caller 接受 "metadata 缺失视同 mismatch" 语义 |
| 顶层 batch task abort 时 JoinSet 内 sub-task 是否真 abort | tokio doc：`JoinSet::drop` SHALL abort 所有未完成 sub-task；perf_ssh_cache_hit 加一个 `list_sessions` 后立刻 `ssh_disconnect` 的 race 测试断言 batch sub-task 不再推 update |

## Migration Plan

本 change 是性能补全 + SSH-only 改动，对前端 IPC 无 BREAKING（响应字段不变、SSE event 形态不变）；对既有集成测试无 BREAKING（ipc_contract / ssh_reconnect_lifecycle 不动 inline FakeRemoteSftp）。

**部署顺序**（apply 阶段建议）：

1. `crates/cdt-ssh/src/provider.rs`：加 `read_dir_with_metadata` override + 单测断言 SFTP `read_dir` 1 RTT 路径 entries 含 metadata + missing 条 metadata = None（D1）
2. `crates/cdt-api/src/ipc/local.rs`：抽 `scan_metadata_for_page_dispatch` 函数 + 新 helper `scan_metadata_for_page_batched`；caller 端 `list_sessions` 内 spawn 处把 `tokio::spawn(scan_metadata_for_page(...))` 改 `tokio::spawn(scan_metadata_for_page_dispatch(...))`
3. `crates/cdt-api/tests/fake_remote_sftp.rs`：抽 counted helper module（CountedFakeRemoteSftp + `Arc<AtomicUsize>` 计数）
4. `crates/cdt-api/tests/perf_ssh_cache_hit.rs`：用 `#[path]` 引 fake helper；3 个 user-facing handler counter assertion test；**不**加 `#[ignore]`，进默认 CI
5. `crates/cdt-api/tests/perf_ssh_scanner_chunked_read.rs`：`#[ignore]` SSH 5MB jsonl 9s threshold
6. `crates/cdt-api/tests/perf_scanner_open_read.rs`：`#[ignore]` Local 端 dyn vs direct 1.3× 阈值
7. spec delta 三处：`fs-abstraction` / `ssh-remote-context` / `ipc-data-api`
8. followups.md 把 PR-D2 两条标记为 ssh-batch-readdir-with-metadata 落地
9. apply 前后 `bash scripts/run-perf-bench.sh --runs 5` 验 Local baseline 不退化
10. push + N.1-N.4 标准发布尾段（codex 二审多轮 + wait-ci + archive 原子 commit）

**回滚**：本 change 改动隔离在 cdt-ssh provider + cdt-api local.rs scanner dispatch + 三个新 test 文件 + spec delta；revert PR 即可。无数据迁移、无前端联动。

## Open Questions

1. **Local 路径是否要在 PR-D3 同样切 batch read_dir_with_metadata？** —— Local stat 廉价（µs 级 vs SSH ms 级 RTT），batch 收益小；但 cache invalidation 节奏与"per-session stat 拿真实 signature"的语义一致——批量后是 "per-dir 一次 read_dir_with_metadata 拿一组 mtime/size" 与 "per-file stat" 在外部进程在 read_dir 与 stat 之间改文件的窄窗口下可能产生差异（极罕见，但需评估）。**留 PR-D3 量化 + 评估**。

2. **CountedFakeRemoteSftp 是否应该最终上移到 ipc_contract.rs 那份合并？** —— followups.md 已有独立 coverage-gap 项跟踪 ipc_contract.rs 加计数器；本 PR 不收口避免 scope 扩散。**留独立 PR**。

3. **batch helper 的 `JoinSet` mismatch sub-task spawn 是否需要 abort handle 注册到 active_scans？** —— 当前实现：顶层 batch task 注册 active_scans，子 JoinSet 跟随父 task drop 自动 abort（tokio 语义）。**已 closed**：不需要额外注册，简化 cleanup。

4. **`fs.read_dir_with_metadata` 在 SSH dir 巨大（10K+ entry）场景内存/带宽影响？** —— 典型用户 ~/.claude/projects/ 单 project_dir 通常 ≤ 100 sessions（少于 1MB SFTP READDIR reply）；10K+ 是 abuse case。**留 follow-up** 若实测有 issue 再加 pagination spec change。

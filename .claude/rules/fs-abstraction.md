# fs 抽象六条硬契约（H1-H6）

`FileSystemProvider` trait 是 local / ssh / http-server 三套 mode 共用业务代码的**唯一抽象边界**。本文是**硬约束**——任何涉及 fs 调用 / cache 改动 / SSH / HTTP server mode 的 PR 都 SHALL 读一遍并按规则评估。

历史背景：trait 2025 年随 `port-project-discovery` 引入但**只在 `ProjectScanner` 内落地**，cache 层 + `cdt-api/src/ipc/local.rs` 30+ 处 IO 全绕过 trait 硬编码 `tokio::fs::*`，导致调用方 18+ 处 `if fs.kind() == Ssh { ... } else { ... }` 二选一分叉。本文档六条契约是 `unify-fs-abstraction` change 钉死的反复利复利墙。

物理位置：所有 fs 抽象类型（`FileSystemProvider` / `LocalFileSystemProvider` / `FsError` / `FsMetadata` / `FsKind` / `FsIdentity` / `DirEntry` / `EntryKind` / `ContextId` / `BackendPolicy` / `InitialLoadPolicy` / `PrefetchPolicy`）真相源住 `crates/cdt-fs/`，`cdt-discover` 仅做 `pub use cdt_fs::*` 兼容 re-export。

## 六条契约速查

| 契约 | 一句话约束 | Enforce 机制 |
|---|---|---|
| **H1** | 业务路径禁直调 `tokio::fs::*` | `xtask check-fs-direct-calls`（warn-only → PR-D 后 fail-on-match）+ `build_time_invariants` 集成测试 |
| **H2** | hot path 禁 N 次串行 `fs.stat / read` | `InstrumentedFs` + `FsOpCounter` instrumentation + 集成测试 fs op 上限断言 + PR review |
| **H3** | 业务**算法**层禁 `fs.kind() == Ssh`；策略层允许但只能选 policy 不复制算法 | PR review checklist 按 design.md D6 分类表逐行复核 |
| **H4** | HTTP backend 默认 `FullEager + max_round_trips=1`；Tauri 默认 `SkeletonThenStream` | `BackendPolicy::for_local() / for_ssh() / for_http()` 单测断言 |
| **H5** | fs trait 不承担分页 / 排序语义 | 集成测试 `no_pagination_in_trait` grep 方法签名禁含 `Cursor / Offset / SortBy / Order` |
| **H6** | `FsError` 必须可操作 —— `is_retryable / should_invalidate_cache` 元方法 | 每个 variant 元方法返回值单测覆盖 |

## H1：业务路径禁直调 `tokio::fs::*`

业务 crate（`cdt-api` / `cdt-config` 等）**禁止**直调 `tokio::fs::metadata / read / read_to_string / read_dir / File::open` 等任何 fs syscall 入口；SHALL 通过注入的 `Arc<dyn FileSystemProvider>` 走 trait 方法。仅以下路径例外（即 H1 Allowlist）。

### Allowlist（single source of truth）

下表是 H1 Allowlist 的**唯一真相源**——`xtask check-fs-direct-calls` 与 `crates/cdt-api/tests/build_time_invariants.rs` SHALL 在运行时 parse 本 markdown table 作为 allowlist 输入；任何 allowlist 增删 SHALL 改本表，**不**改测试或 xtask 源码。

| crate/path | reason |
|---|---|
| `crates/cdt-fs/**` | fs 抽象层 crate 本身（含 `LocalFileSystemProvider` 实现 + instrumentation 单测 + open_read overhead bench） |
| `crates/cdt-cli/src/main.rs` | binary entrypoint，初始化日志 / 配置加载读 file 是 boot phase |
| `crates/cdt-watch/src/**` | `notify` 库本身基于 inotify / FSEvents，非 fs read/write 抽象的范畴 |
| `**/tests/**` | 测试 setup 直读 fixture / 写 `TempDir`（覆盖 workspace 内任意 `tests/` 目录） |
| `crates/cdt-ssh/src/provider.rs` | `SshFileSystemProvider` 实现层，与 `LocalFileSystemProvider` 同等地位 |
| `crates/xtask/**` | dev tooling 自身 |

### 违反示例

```rust
// crates/cdt-api/src/ipc/local.rs
async fn read_session_content(&self, path: &Path) -> Result<String> {
    let bytes = tokio::fs::read(path).await?;   // 违反 H1：业务路径直调 tokio::fs
    Ok(String::from_utf8(bytes)?)
}
```

### 修法

```rust
// crates/cdt-api/src/ipc/local.rs
async fn read_session_content(&self, path: &Path) -> Result<String, FsError> {
    self.fs.read_to_string(path).await   // 走注入的 trait handle
}
```

### Enforce

- **`xtask check-fs-direct-calls`**：grep `crates/cdt-api/src/**/*.rs` + `crates/cdt-config/src/**/*.rs` 等业务路径，匹配 `tokio::fs::(metadata|read|read_to_string|read_dir|File::open)` 等模式；non-allowlist 命中本 change 期间为 warning（`--warn-only`），PR-D 完成后切 fail-on-match
- **`crates/cdt-api/tests/build_time_invariants.rs`**：集成测试同步 grep + parse 本规则文件的 Allowlist table；任一非 allowlist 路径命中即测试 fail（CI 拒）
- 入口：本地 `cargo xtask check-fs-direct-calls` / `cargo test -p cdt-api --test build_time_invariants`

## H2：hot path 禁 N 次串行 `fs.stat / read`

hot path（IPC 命令链 list / 翻页 / 详情 / SSE 推送）**禁止**写 `for path in paths { fs.stat(path).await? }` 类 N 次串行 stat / read 调用；SHALL 用 `read_dir_with_metadata`（一次 syscall 拿 dir + 子项 metadata）或 `stat_many` batched API（trait default 实现走 `join_all`，未来 SSH override 走 SFTP message-id 真 pipeline）。

理由：SSH 单次 stat 50-100ms RTT，N=50 串行 = 2.5-5s 卡顿；Local 单次 stat ~10μs 但 N=10000 累计也是 100ms 浪费。batched API 在 trait 层先就位，让调用方一致写 `stat_many` 而非散落写循环，等 PR-F 解开 SSH session 锁后自然受益。

### 违反示例

```rust
// 在 list_sessions_skeleton 中
let mut metas = Vec::with_capacity(paths.len());
for path in &paths {
    metas.push(fs.stat(path).await?);   // 违反 H2：N 次串行 RTT
}
```

### 修法

```rust
// 走 batched API；SSH default 当前仍走 join_all 假 batch，PR-F 后变真 pipeline
let metas: Vec<Result<FsMetadata, FsError>> = fs.stat_many(&paths.iter().collect::<Vec<_>>()).await;
```

或当 dir 内所有子项都要 metadata 时：

```rust
let entries = fs.read_dir_with_metadata(&dir).await?;  // 一次拿全
```

### instrumentation 接入样例

`cdt-fs` 提供 `InstrumentedFs<P>` wrapper + `with_fs_counter` async wrapper，在每个 IPC command 边界统计 fs op 次数（基于 `tokio::task_local!`，并发 IPC command 互不污染）：

```rust
use cdt_fs::{InstrumentedFs, with_fs_counter};

// 调用方注入时包一层
let fs = Arc::new(InstrumentedFs::new(LocalFileSystemProvider::new()));

// IPC command 边界包 with_fs_counter
let (result, counts) = with_fs_counter(async {
    api.list_sessions(project_id).await
}).await;

// counts.stat / counts.read_dir / counts.read_to_string ... 各操作计数
// 集成测试可断言 assert!(counts.stat <= 50);
// FsOpCounter Drop 时自动 emit tracing::info!(target = "cdt_fs::ops", ...)
```

### Enforce

- **`InstrumentedFs` + `FsOpCounter`**：本 change 提供基础设施（trait 边界自动 record，provider impl 不需配合）
- **集成测试 fs op 上限断言**：PR-B/C/D 接入业务时按 IPC command 在测试里加 `assert_fs_op_count(list_sessions) <= 50` 类断言（本 change 不写业务断言，只提供 counter）
- **PR review checklist**：reviewer 检查 hot path 是否有 `for path in paths { fs.stat / fs.read ... }` 反模式
- 入口：本 change 阶段只有 review checklist 立即生效；instrumentation 接入业务从 PR-B 起

## H3：业务算法禁 `fs.kind() == Ssh`，策略层允许但只能选 policy

业务**算法**代码（cache 实现、parser、grouper、sort 比较器、过滤谓词、cache key 构造）`fs.kind() == Ssh` / `is_remote` 默认拒——同一算法 SHALL 在 trait 之上写一份，不允许 `if Ssh / else` 写两遍。业务**策略**层（`LocalDataApi` 的 IPC command 入口）`fs.kind()` 允许但 SHALL ADR + inline 注释，且**只允许选 `BackendPolicy` 字段值**，不允许复制业务算法。

### 算法分叉 vs 策略分叉判据

| 类别 | 特征 | 处置 |
|---|---|---|
| **算法分叉**（拒） | 同一算法在 `if Ssh / else` 两路径里写两遍；改一边漏改另一边即引入行为不一致 | PR-D 强制消除（切 trait 方法 + ContextId 后自然统一） |
| **策略分叉**（允许） | 不同 backend 选不同 `BackendPolicy` 字段值（如 `initial_load_policy`），下游算法**只接受 policy 入参**不分叉 | 允许但 SHALL inline 注释 `// strategy fork: see design.md::Dx` + 在对应 ADR 锚点记录 |

### 违反示例（算法分叉，拒）

```rust
// LocalDataApi::list_sessions
let sessions = if fs.kind() == FsKind::Ssh {
    // SSH 路径：按 mtime 排
    raw.sort_by_key(|s| Reverse(s.mtime));
    raw
} else {
    // Local 路径：按 size 排
    raw.sort_by_key(|s| s.size);
    raw
};
// 违反 H3：sort 逻辑分两份，未来加排序字段 SSH/Local 不同步
```

### 允许示例（策略分叉，OK）

```rust
// LocalDataApi::list_sessions —— 只选 policy 不复制算法
// strategy fork: see design.md::D6 (PR-E 上移到 BackendPolicy::initial_load_policy)
let policy = match fs.kind() {
    FsKind::Ssh | FsKind::Http => BackendPolicy::for_ssh(),
    FsKind::Local => BackendPolicy::for_local(),
};

// 下游算法只接 policy 入参，单一实现
self.load_initial_page(project_id, &policy).await
```

### design.md D6 初步分类表

`unify-fs-abstraction` change `design.md::D6` 附了 23 处 `fs.kind() == Ssh` / `is_remote` 分叉的初步分类表，每行标 `algorithm`（H3 拒，PR-D 消除）/ `policy`（H3 允许，PR-E 上移到 `BackendPolicy`）。PR-D reviewer SHALL 用此表作起点逐行复核 + 在 PR-D 的 design.md 里固化最终分类（每行标 `algorithm` / `policy` / `temporary-workaround` + ADR 锚点）。

### Enforce

- **PR review checklist**：reviewer 按 design.md D6 分类表逐行确认；新增 `fs.kind()` 分叉 SHALL 明示属于 algorithm 还是 policy
- **未来可扩展 xtask**：加规则 `algorithm-level fs.kind() detection`（高级模式匹配，本 change 不实现）
- 入口：人工 review；design.md D6 表见 `openspec/changes/unify-fs-abstraction/design.md`

## H4：HTTP backend 默认 FullEager + max_round_trips=1

Tauri / HTTP transport 抽象延后，但本 change 钉死 `BackendPolicy::for_local() / for_ssh() / for_http()` 三个 const-like 构造器的默认值。HTTP backend `initial_load_policy: FullEager` + `max_round_trips_for_initial_page: 1` 是为了防止 HTTP server mode 走多 round trip 的 SkeletonThenStream 后端给前端的延迟放大。

### 三种 backend 默认值 table

| backend | `initial_load_policy` | `max_round_trips_for_initial_page` | `supports_incremental_updates` | `prefetch_policy` |
|---|---|---|---|---|
| `for_local()` | `SkeletonThenStream` | ≥ 2 | `true` | `None` |
| `for_ssh()` | `FullEager` | 1 | `false` | `None` |
| `for_http()` | `FullEager` | 1 | `false` | `None` |

`PrefetchPolicy` 与 `InitialLoadPolicy` 是**正交字段**——前者表达"翻页预取策略"，后者表达"首屏加载策略"。**不得**把 `PrefetchNext` 塞进 `InitialLoadPolicy` 当第三 variant；二者可自由组合（如未来 high-bandwidth desktop 可能 `SkeletonThenStream + PrefetchNext`）。

### 违反示例

```rust
// 给 BackendPolicy::for_http 加预取——本 change 阶段 SHALL 是 None
pub const fn for_http() -> Self {
    Self {
        initial_load_policy: InitialLoadPolicy::FullEager,
        prefetch_policy: PrefetchPolicy::PrefetchNext,  // 违反 H4：HTTP 当前不支持预取
        ..
    }
}
```

### 修法

```rust
pub const fn for_http() -> Self {
    Self {
        initial_load_policy: InitialLoadPolicy::FullEager,
        max_round_trips_for_initial_page: 1,
        supports_incremental_updates: false,
        prefetch_policy: PrefetchPolicy::None,
    }
}
```

### Transport 抽象延后的 anchor

Tauri 本地 transport vs HTTP transport 的真正抽象（替换 IPC 序列化层 / SSE pump 等）**延后**到 follow-up change，本 change 仅在 `BackendPolicy` 层面承认差异并暴露 `for_local / for_ssh / for_http` 三个入口。PR-E 才把 `BackendPolicy` wire 到 `LocalDataApi`；PR-E 之后再起的 change 才可能引入真正的 transport trait。

### Enforce

- **`BackendPolicy::for_local() / for_ssh() / for_http()` 单测**：本 change 实现，断言三种 backend 默认值与上表一致；`PrefetchPolicy` 与 `InitialLoadPolicy` 正交（显式构造 `BackendPolicy { initial_load_policy: SkeletonThenStream, prefetch_policy: PrefetchNext }` SHALL 编译通过）
- **PR-E wire 时单测**：断言 `LocalDataApi(http_mode).policy == BackendPolicy::for_http()`（PR-E 阶段加，本 change 不做）
- 入口：`cargo test -p cdt-fs backend_policy`

## H5：fs trait 不承担分页 / 排序语义

`FileSystemProvider` trait **不得**暴露任何按 mtime / size / 名字排序的方法，**不得**暴露 cursor / offset 等分页参数。`read_dir` 与 `read_dir_with_metadata` 返回 `Vec<DirEntry>` 顺序由底层文件系统决定；caller SHALL 在更高层（`ProjectScanner` / `SessionIndex` / `ProjectRepository` / IPC 层）自行排序与分页。

理由：trait 一旦带 `SortBy / Cursor` 参数，三套 provider 实现都要重复实现排序状态机，且未来加排序字段需改 trait API（破抽象）；高层抽象用一次 dir read + 内存排序更直接。

### 高层分页正确实现位置范例

PR #186（`simplify-repository-as-project`）引入的 `GroupCursor` k-way merge 是高层分页的正确位置范例 —— 实现在 `LocalDataApi::list_group_sessions`（业务策略层），**不在** `FileSystemProvider`。任何未来"按某种排序拿前 N 个 session"的需求 SHALL 走类似的高层抽象（`SessionIndex` / `ProjectRepository` 待引入），**不得**给 fs trait 加排序参数或分页 API。

### 违反示例

```rust
// 给 trait 加 cursor 参数——违反 H5
#[async_trait]
pub trait FileSystemProvider {
    async fn read_dir_paged(
        &self,
        path: &Path,
        cursor: Option<DirCursor>,
        limit: usize,
        sort_by: SortBy,
    ) -> Result<(Vec<DirEntry>, Option<DirCursor>), FsError>;
}
```

### 修法

trait 只暴露低层 dir read，caller 在调用方代码内排序：

```rust
let entries = fs.read_dir_with_metadata(&dir).await?;
let mut sorted: Vec<_> = entries.into_iter().collect();
sorted.sort_by_key(|e| Reverse(e.metadata.mtime));  // caller 自排
let page = sorted.into_iter().take(limit).collect::<Vec<_>>();
```

### Enforce

- **`crates/cdt-fs/tests/no_pagination_in_trait.rs`** 集成测试：grep `cdt_fs::FileSystemProvider` trait 的方法签名（通过源码字符串扫描），断言 SHALL NOT 含 `Cursor` / `Offset` / `Limit` / `SortBy` / `Order` 标识符；fail 时 CI 拒
- 入口：`cargo test -p cdt-fs --test no_pagination_in_trait`

## H6：`FsError` 必须可操作

`FsError` enum 每个 variant SHALL 提供两个 inherent 元方法 —— `fn is_retryable(&self) -> bool` 与 `fn should_invalidate_cache(&self) -> bool`，让 caller 不靠 string match 即可决定"该不该重试"与"该不该清 cache entry"。SSH `with_retry` 耗尽后 SHALL 返 `FsError::TransientExhausted` 而非类型擦除的 `Io::other("transient ...")`。

variants 至少含：`NotFound(PathBuf)` / `Io { path, source: io::Error }` / `Utf8 { path, source }` / `Unsupported(&'static str)` / `Disconnected { path, reason }` / `TransientExhausted { path, attempts, last_reason }`。

### 元方法使用样例

```rust
use cdt_fs::FsError;

match fs.stat(path).await {
    Ok(meta) => { /* ... */ }
    Err(err) => {
        // 清 cache：文件可能不存在或损坏
        if err.should_invalidate_cache() {
            self.metadata_cache.lock().await.remove(&(ctx_id.clone(), path.to_path_buf()));
        }

        // 决定是否重试
        if err.is_retryable() {
            // 瞬时错误（如 SSH 临时断开），重连后再来
            return Err(err);
        }

        // 永久错误（NotFound / TransientExhausted / Utf8），不要重试
        return Err(err);
    }
}
```

### 各 variant 元方法语义对照

| variant | `is_retryable()` | `should_invalidate_cache()` | 理由 |
|---|---|---|---|
| `NotFound(_)` | `false` | `true` | 文件不存在，cache 任何 entry 都应清 |
| `Io { .. }` | `false` | `false` | 通用 IO 错误，caller 决定；不主动清 cache 防误伤 |
| `Utf8 { .. }` | `false` | `true` | 文件损坏或非 UTF-8，cache 该 entry 应清 |
| `Unsupported(_)` | `false` | `false` | provider 不支持的操作，重试也无意义；与文件状态无关 |
| `Disconnected { .. }` | `true` | `false` | SSH 临时断开，重连后可能恢复；数据仍有效 |
| `TransientExhausted { .. }` | `false` | `false` | `with_retry` 已耗尽，再试无意义；远端可能恢复，cache 仍可能有效 |

### Enforce

- **单测覆盖每个 variant 的元方法返回值**：本 change 实现 `crates/cdt-fs/src/error.rs::tests::is_retryable_matrix` 与 `::should_invalidate_cache_matrix`，加 variant 后 SHALL 同步加测；fail 时 CI 拒
- 入口：`cargo test -p cdt-fs fs_error`

## 相关 change

- 主 spec：`openspec/specs/fs-abstraction/spec.md`（archive 后由 `unify-fs-abstraction` 同步生成）
- Change proposal：`openspec/changes/unify-fs-abstraction/proposal.md`（H1-H6 起源 + 18 处分叉历史背景）
- Change design：`openspec/changes/unify-fs-abstraction/design.md`（D1-D9 决策；D6 含 23 处分叉初步分类表；D9 钉死每条 H 的 enforce 方式）
- Follow-up roadmap：PR-B `MetadataCache` 切 trait / PR-C `ParsedMessageCache` / PR-D 清 30+ 直调 + 18 分叉 / PR-E `ProjectScanner` in-memory 复用 + `BackendPolicy` wire / PR-F SSH session 锁解开 + `stat_many` 真 SFTP pipeline

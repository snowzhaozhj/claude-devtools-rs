## MODIFIED Requirements

### Requirement: `FileSystemProvider` trait 暴露 7 个核心方法

> 标题中"7 个"是历史命名（最早 PR-A 引入时的方法数），保留作 archive sync 兼容；当前 trait 实际暴露 **12 个方法**（read 9 + write 3），后续 PR 加方法 SHALL 在本 Requirement body 内同步更新清单。

系统 SHALL 在 `cdt-fs::FileSystemProvider` trait 上暴露以下方法（编译时强制实现，default 实现可被 override）：

1. `fn kind(&self) -> FsKind`
2. `async fn exists(&self, path: &Path) -> bool`
3. `async fn read_dir(&self, path: &Path) -> Result<Vec<DirEntry>, FsError>`
4. `async fn read_dir_with_metadata(&self, path: &Path) -> Result<Vec<DirEntry>, FsError>`（default 实现：`read_dir` + 逐项 `stat`，N+1 RTT 兜底；后端 SHALL override 当 backend 协议原生支持 1 RTT 拿 dir + 全 entry attrs，避免性能退化）
5. `async fn read_to_string(&self, path: &Path) -> Result<String, FsError>`
6. `async fn stat(&self, path: &Path) -> Result<FsMetadata, FsError>`
7. `async fn read_lines_head(&self, path: &Path, max: usize) -> Result<Vec<String>, FsError>`
8. `async fn open_read(&self, path: &Path) -> Result<Box<dyn AsyncRead + Send + Unpin>, FsError>`（新增，替代 SSH `open_read_stream` 破抽象）
9. `async fn stat_many(&self, paths: &[&Path]) -> Vec<Result<FsMetadata, FsError>>`（新增 batched API，default 实现走 `futures::future::join_all`）
10. `async fn write_atomic(&self, path: &Path, content: &[u8]) -> Result<(), FsError>`（atomic 写——SHALL 通过 tmp file + rename 实现，写失败 best-effort 清理 tmp）
11. `async fn create_dir_all(&self, path: &Path) -> Result<(), FsError>`（递归创建目录，已存在 SHALL NOT 报错——等价 `tokio::fs::create_dir_all`）
12. `async fn remove_file(&self, path: &Path) -> Result<(), FsError>`（删文件，不存在 SHALL 返 `FsError::NotFound`，不递归删目录）

trait SHALL 保持 dyn-safe（`&dyn FileSystemProvider` 可用），不引入关联类型。

**`read_dir_with_metadata` override 契约**（change `ssh-batch-readdir-with-metadata` 引入）：

- **SSH override 复用 read_dir 语义**：`SshFileSystemProvider::read_dir_with_metadata` SHALL override default impl 并直接调 `self.read_dir(path)`，复用 SFTP READDIR reply 自带的 entry attrs（详 ssh-remote-context spec `Read sessions and files over SSH with same contract` + change `ssh-batch-readdir-with-metadata` design D1）
- **missing mtime 语义**：override 后部分 entry 若 metadata = None（SFTP server 未返 mtime），caller SHALL 把此条视同 cache mismatch（走 cache wrapper miss 路径补齐），实现 SHALL NOT 在 trait 实现层做 per-entry stat fallback（否则退化为 N+1 RTT）

**写方法 atomic 契约**（change `ssh-project-memory-remote-rw` 引入）：

- `write_atomic` SHALL 实现为：写到 `<path>.tmp.<atomic-seq-hex>.<pid-hex>` → rename to `<path>`；rename 失败 SHALL 调 `remove_file(<tmp>)` best-effort 清理（清理失败不向上传播）
- tmp suffix 来源 SHALL 是进程内 `static AtomicU64` 单调递增计数器（`fetch_add(1, Ordering::Relaxed)`）+ `std::process::id()`，**不**得依赖 `SystemTime::now()` 纳秒（Windows 100ns 时钟精度并发碰撞 race）
- `write_atomic` 在 reader 角度 SHALL 是原子覆盖——并发 reader 永远观察到旧内容或新内容整版，永不观察到截断 / 半写状态。Local 后端依赖 `tokio::fs::rename` 的 POSIX rename(2) 原子语义；SSH 后端 SHALL 优先走 `posix-rename@openssh.com` SFTP 扩展（`SftpSession::extensions()` 探测），不支持时降级为 `remove_file(<target>) + rename(<tmp>, <target>)` 两步（降级路径有极短窗口 reader 可见 target missing，acceptable）
- 同 path 多 caller 并发 `write_atomic` SHALL 是 last-write-wins 语义；`AtomicU64` 序号防 tmp 路径冲突
- `create_dir_all` SHALL 等价 `tokio::fs::create_dir_all`：递归创建中间目录，目标已存在 SHALL NOT 返错
- `remove_file` SHALL 仅删文件，路径是目录时 SHALL 返 `FsError::Io { path, source: <ENOTEMPTY 或等价> }`，不递归

**新写方法的 instrumentation 计数**（fs-abstraction 既有 `Provider instrumentation 入口可观测 fs op 次数` Requirement 配套）：

- `FsOpCounts` SHALL 新增 `write_atomic / create_dir_all / remove_file` 三个 `AtomicU64` 计数字段
- `InstrumentedFs<P>` SHALL 在三个新 trait 方法的入口调对应 `FsOpCounter::current().record_<op>()` hook，与既有 9 方法计数路径一致
- `tracing::info!(target = "cdt_fs::ops", ...)` Drop emit SHALL 包含三个新字段

#### Scenario: open_read 在 Local 上返回 tokio::fs::File 包装

- **WHEN** caller 在 `LocalFileSystemProvider` 上调 `open_read(path)` 且 path 存在可读
- **THEN** SHALL 返回 `Ok(Box::new(tokio::fs::File))` 或等价包装
- **AND** 返回的 reader 实现 `AsyncRead + Send + Unpin`，caller SHALL 能用 `BufReader::new(reader).lines()` 流式按行读取

#### Scenario: open_read 在 SSH 上走 SFTP 流式句柄

- **WHEN** caller 在 `SshFileSystemProvider` 上调 `open_read(path)`
- **THEN** SHALL 返回 SFTP 句柄包装（`russh_sftp::client::fs::File` 或 wrapper），实现 `AsyncRead + Send + Unpin`
- **AND** caller SHALL NOT 需要 downcast 到 `SshFileSystemProvider` 才能流式读

#### Scenario: stat_many 默认实现走 join_all

- **WHEN** caller 调 `fs.stat_many(&[p1, p2, p3])` 且 provider 未 override
- **THEN** 实现 SHALL 通过 `futures::future::join_all` 并发 `fs.stat(p)` 所有 path
- **AND** 返回 `Vec<Result<FsMetadata, FsError>>` 顺序与 input paths 严格对应

#### Scenario: stat_many 在 SSH 上当前是假 batch

- **WHEN** caller 在 `SshFileSystemProvider` 上调 `stat_many(&[p1, p2, ...])`
- **THEN** 实现 SHALL 使用 trait default `join_all`
- **AND** 由于底层 `Arc<Mutex<SftpSession>>` 全锁，实际执行仍是串行 N 次 RTT —— 此限制属已知，留 PR-F（SSH session pipeline）解决；trait 契约层面 caller SHALL 一律调 `stat_many` 而非循环 `stat`

#### Scenario: read_dir_with_metadata default impl 是 N+1 RTT 兜底

- **WHEN** 某后端未 override `read_dir_with_metadata`
- **THEN** SHALL 走 trait default：先 `read_dir(path)` 拿 entries 列表，再对每条 `entry.kind.is_file()` 调 `stat(path.join(entry.name))` 补 metadata
- **AND** 总 op 数 SHALL 为 1 + N（N = file entry 数）
- **AND** 此 default impl 仅作为 trait 兜底；性能敏感 backend SHALL override 以避免 N+1 退化

#### Scenario: SSH override read_dir_with_metadata 复用 read_dir 不退化

- **WHEN** 在 SSH context 下调 `fs.read_dir_with_metadata(<remote_dir>)`
- **THEN** `SshFileSystemProvider` SHALL override default impl，直接 delegate 到 `self.read_dir(path).await`
- **AND** 底层 SFTP `SSH_FXP_READDIR` reply 1 个 RTT 返完整 dir 内容 + 每个 entry 的 attrs（size/mtime）
- **AND** 总 SFTP RTT 数 SHALL = 1（**SHALL NOT** 退化为 N+1，即使部分 entry mtime missing）
- **AND** 缺 mtime 的 entry（SFTP server 未返 modify time）SHALL 在 `DirEntry.metadata = None` 状态返给 caller，由 caller 上层语义（如 cache batch 校验）决定 fallback 路径——**SHALL NOT** 在 trait 实现层补 stat

#### Scenario: write_atomic 走 tmp+rename 在 Local 上原子覆盖

- **WHEN** caller 在 `LocalFileSystemProvider` 上调 `fs.write_atomic(path, content)` 且目标 path 已有旧内容
- **THEN** 实现 SHALL 写到 `<path>.tmp.<rand>` → rename 到 `<path>`
- **AND** 并发 `fs.read_to_string(path)` SHALL 永远拿到旧 content 或新 content 整版，绝不拿到截断 / 半写中间态
- **AND** rename 失败 SHALL best-effort `remove_file(<tmp>)` 清理（清理失败不向上传播）

#### Scenario: write_atomic 走 tmp+rename 在 SSH 上原子覆盖

- **WHEN** caller 在 `SshFileSystemProvider` 上调 `fs.write_atomic(path, content)` 远端目标 path 已有旧内容
- **THEN** 实现 SHALL 通过 SFTP 写到 `<path>.tmp.<rand>` 后调 `SSH_FXP_RENAME` 原子覆盖
- **AND** 远端 SFTP server side OS 提供 POSIX rename(2) 原子性保证
- **AND** rename 失败重试 ≤ 3 次（与既有 SSH retry 策略对齐：`code=4 / EAGAIN / ECONNRESET / ETIMEDOUT / EPIPE`，指数退避 75ms × attempt）

#### Scenario: create_dir_all 不报已存在错

- **WHEN** caller 调 `fs.create_dir_all(<existing-dir>)`
- **THEN** SHALL 返 `Ok(())`，不返 `AlreadyExists` 错误

#### Scenario: remove_file 不存在返 NotFound

- **WHEN** caller 调 `fs.remove_file(<missing-path>)`
- **THEN** SHALL 返 `Err(FsError::NotFound(path))`

### Requirement: `BackendPolicy` enum 雏形定义

系统 SHALL 在 `cdt-fs` 内定义 `BackendPolicy` struct + `InitialLoadPolicy` enum + `PrefetchPolicy` enum + `StaleCheckStrategy` enum，作为 LocalDataApi 业务路径**选择后端相关行为**的真相源。`BackendPolicy` 字段 SHALL 是 primitive（`Copy + PartialEq + Eq + Clone + Debug` derive 安全）类型，**禁止**承担 `Arc<dyn Trait>` / 非 Copy 字段——业务侧的 trait object 与 Clone 类型策略（如 `GitIdentityResolver` / `SearchConfig`）SHALL 放在更高层（如 `cdt-api::ipc::backend_resolvers`），与 `BackendPolicy` 配套使用。

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackendPolicy {
    pub initial_load_policy: InitialLoadPolicy,
    pub max_round_trips_for_initial_page: u8,
    pub supports_incremental_updates: bool,
    pub prefetch_policy: PrefetchPolicy,
    /// 是否支持 memory 文件读写（Local true / SSH true / 未来若有不支持的 backend 设 false）。
    pub supports_memory: bool,
    /// 是否支持 subagent JSONL 扫描（Local true / SSH false）。
    pub supports_subagent_scan: bool,
    /// 5min stale 判定策略——`LocalClock5min` 用本机 mtime 比对，
    /// `SkipUntilClockSync` 跳过（远端 mtime 跨 clock domain 不可比对）。
    pub stale_check_strategy: StaleCheckStrategy,
}

pub enum InitialLoadPolicy {
    FullEager,
    SkeletonThenStream,
}

pub enum PrefetchPolicy {
    None,
    PrefetchNext,
}

pub enum StaleCheckStrategy {
    LocalClock5min,
    SkipUntilClockSync,
}
```

`PrefetchPolicy` 与 `InitialLoadPolicy` SHALL 是**正交字段**——前者表达"翻页预取策略"（不预取 vs 预取下一页），后者表达"首屏加载策略"（一次性 vs 增量补全）。**不得**把 `PrefetchNext` 塞进 `InitialLoadPolicy` 当第三 variant。

`StaleCheckStrategy` SHALL 至少含 `LocalClock5min` 与 `SkipUntilClockSync` 两个 variant；未来可扩展 `ClockSkewCompensated { offset_secs: i64 }` 等 variant。enum exhaustive match 保证调用方在加 variant 时编译期发现未处理路径。

本 capability SHALL 提供 `BackendPolicy::for_local()` / `BackendPolicy::for_ssh()` / `BackendPolicy::for_http()` 三个 const 构造器；每个 SHALL 完整覆盖 7 个字段值。

业务代码（典型如 `cdt-api::ipc::local::LocalDataApi` 的 IPC handler）SHALL 通过 `BackendPolicy` 字段读取选择 backend-specific 行为，**禁止**直接 `match fs.kind()` / `if fs.kind() == FsKind::Ssh / Local` 表达策略——`fs.kind()` 仅允许在策略**派生**点（如顶层 `active_fs_and_policy()` helper 内部、`BackendResolvers::from_fs(&fs)` 内部）使用，业务 callsite SHALL 读 policy 字段。

#### Scenario: for_local 返回 SkeletonThenStream + supports_memory true + LocalClock5min stale

- **WHEN** 调 `BackendPolicy::for_local()`
- **THEN** SHALL 返回 `initial_load_policy = SkeletonThenStream`，`max_round_trips_for_initial_page >= 2`，`supports_incremental_updates = true`，`prefetch_policy = None`
- **AND** SHALL 返回 `supports_memory = true`，`supports_subagent_scan = true`，`stale_check_strategy = StaleCheckStrategy::LocalClock5min`

#### Scenario: for_ssh 返回 FullEager + supports_memory true + SkipUntilClockSync stale

- **WHEN** 调 `BackendPolicy::for_ssh()`
- **THEN** SHALL 返回 `initial_load_policy = FullEager`，`max_round_trips_for_initial_page = 1`，`supports_incremental_updates = false`，`prefetch_policy = None`
- **AND** SHALL 返回 `supports_memory = true`（change `ssh-project-memory-remote-rw` 起：SSH 远端 memory CRUD 完整支持，不再 graceful skip），`supports_subagent_scan = false`，`stale_check_strategy = StaleCheckStrategy::SkipUntilClockSync`

#### Scenario: for_http 新增 PR-E 字段按 Local 数据源语义；initial-load 字段保持 HTTP 现状

- **WHEN** 调 `BackendPolicy::for_http()`
- **THEN** initial-load 相关字段 SHALL 保持现状（`initial_load_policy = FullEager`，`max_round_trips_for_initial_page = 1`，`supports_incremental_updates = false`，`prefetch_policy = None`）—— HTTP backend 当前 round trip 模型不变
- **AND** PR-E 新增的三字段 SHALL 按 Local 数据源语义填（`supports_memory = true`，`supports_subagent_scan = true`，`stale_check_strategy = StaleCheckStrategy::LocalClock5min`）—— HTTP server 当前把 LocalDataApi 作为数据源访问 Local `~/.claude/`，与 SSH 不共行为；未来若 HTTP server 接 SSH backend 再加分支

#### Scenario: PrefetchPolicy 与 InitialLoadPolicy 正交

- **WHEN** 显式构造 `BackendPolicy { initial_load_policy: SkeletonThenStream, prefetch_policy: PrefetchNext, .. }`
- **THEN** SHALL 编译通过且 `==` 自身（两字段独立可组合）
- **AND** SHALL NOT 出现 `InitialLoadPolicy` 含 `PrefetchNext` variant 的设计

#### Scenario: BackendPolicy 是 Copy + Eq 类型

- **WHEN** 编译 `cdt-fs`
- **THEN** `BackendPolicy` SHALL derive `Copy + Clone + PartialEq + Eq + Debug`
- **AND** 所有字段 SHALL 是 primitive 或 Copy 类型（bool / u8 / Copy enum），**禁止**含 `Arc<dyn Trait>` / `Box<dyn Trait>` / `Vec<T>` / `String` 等非 Copy 字段

#### Scenario: 业务代码通过 BackendPolicy 字段选择行为

- **WHEN** `cdt-api::ipc::local::LocalDataApi` 的 IPC handler 需要根据后端类型选择**新**行为
- **THEN** handler SHALL 读 `BackendPolicy` 字段（如 `policy.supports_memory` / `policy.stale_check_strategy`），**不得**新增 `if fs.kind() == FsKind::Ssh` / `let is_remote = fs.kind() == Ssh` / `matches!(fs.kind(), FsKind::Ssh)` 等任一等价直接派生
- **AND** 派生点 fs.kind() 比对仅允许出现在 `active_fs_and_policy()` 顶层 helper 内 + `cdt-api::ipc::backend_resolvers::BackendResolvers::from_fs()` 内 + cdt-fs / cdt-discover provider 实现内部
- **AND** `crates/cdt-api/tests/no_kind_compare_outside_resolvers.rs` 集成测试 SHALL 扫 `crates/cdt-api/src/ipc/local.rs` 用 substring 计数 + line-level 白名单两层守护：
  - `fs.kind() ==` 出现 ≤ 3 处（PR-D 残留派生 line ~812 / ~1601 + read_mentioned_file SSH gate line ~3133，全部已登记为已知例外）
  - `let is_remote =` 出现 ≤ 2 处（同 PR-D 残留派生 line ~812 / ~1601）
  - `match fs.kind()` 出现 ≤ 1 处（仅 `active_fs_and_policy` 派生 helper 内部）
  - `matches!(<expr>.kind(),` 出现 == 0 处（local.rs 内严禁此等价写法绕过）
- **AND** 已登记例外（PR-D 残留 + `read_mentioned_file`）SHALL 在 followup / 后续 PR 收口；其中 `read_mentioned_file` 预期 PR-G 加 `BackendPolicy::supports_mention_file_resolution: bool` 字段消除

#### Scenario: StaleCheckStrategy enum 至少包含 LocalClock5min 与 SkipUntilClockSync

- **WHEN** 编译 `cdt-fs`
- **THEN** `StaleCheckStrategy` SHALL 至少含 `LocalClock5min` 与 `SkipUntilClockSync` 两个 variant
- **AND** SHALL derive `Copy + Clone + PartialEq + Eq + Debug`
- **AND** 调用方对 `policy.stale_check_strategy` 的 `match` SHALL 通过 exhaustive 检查（未来加 variant 时编译期暴露未处理路径）

# fs-abstraction Specification

## Purpose
TBD - created by archiving change unify-fs-abstraction. Update Purpose after archive.
## Requirements
### Requirement: `cdt-fs` crate 是 fs 抽象的唯一物理位置

系统 SHALL 把所有文件系统抽象类型（`FileSystemProvider` trait、`LocalFileSystemProvider` 实现、`FsError` / `FsMetadata` / `FsKind` / `FsIdentity` / `DirEntry` / `EntryKind` / `ContextId` / `BackendPolicy` / `InitialLoadPolicy`）的**真相源**放在 `crates/cdt-fs/`。`cdt-discover` SHALL 通过 `pub use cdt_fs::*` re-export 兼容历史 import 路径，但**不得**在 `cdt-discover` 内重新定义这些类型。业务 crate（`cdt-api` / `cdt-config` / `cdt-ssh`）SHALL 直接依赖 `cdt-fs`，不得通过 `cdt-discover` 间接拿 fs 抽象。

#### Scenario: cdt-api 直接依赖 cdt-fs

- **WHEN** `cdt-api` 需要 `FileSystemProvider` trait
- **THEN** `crates/cdt-api/Cargo.toml` SHALL 直接 `cdt-fs = { workspace = true }`
- **AND** `use cdt_fs::FileSystemProvider` 是首选 import；`use cdt_discover::FileSystemProvider` 仅作为本 change 期间的兼容路径保留

#### Scenario: cdt-discover 提供兼容 re-export

- **WHEN** 老代码写 `use cdt_discover::{FileSystemProvider, FsKind, FsMetadata}`
- **THEN** 编译 SHALL 成功（通过 `cdt-discover/src/lib.rs` 内 `pub use cdt_fs::{FileSystemProvider, FsKind, FsMetadata}` re-export）
- **AND** 运行时行为与直接 import cdt-fs 完全一致

#### Scenario: cdt-fs 不依赖业务 crate

- **WHEN** 检查 `crates/cdt-fs/Cargo.toml` 的 `[dependencies]`
- **THEN** SHALL NOT 含 `cdt-discover` / `cdt-api` / `cdt-config` / `cdt-ssh` / `cdt-cli` / `cdt-watch` / `cdt-analyze` / `cdt-parse` / `cdt-core` 任何业务 crate
- **AND** 允许的 deps 仅限：`tokio` / `async-trait` / `thiserror` / `tracing`（运行时 / 错误 / 日志基础设施）

### Requirement: `FileSystemProvider` trait 暴露 7 个核心方法

系统 SHALL 在 `cdt-fs::FileSystemProvider` trait 上暴露以下方法（编译时强制实现，default 实现可被 override）：

1. `fn kind(&self) -> FsKind`
2. `async fn exists(&self, path: &Path) -> bool`
3. `async fn read_dir(&self, path: &Path) -> Result<Vec<DirEntry>, FsError>`
4. `async fn read_dir_with_metadata(&self, path: &Path) -> Result<Vec<DirEntry>, FsError>`（default 实现：`read_dir` + 逐项 `stat`）
5. `async fn read_to_string(&self, path: &Path) -> Result<String, FsError>`
6. `async fn stat(&self, path: &Path) -> Result<FsMetadata, FsError>`
7. `async fn read_lines_head(&self, path: &Path, max: usize) -> Result<Vec<String>, FsError>`
8. `async fn open_read(&self, path: &Path) -> Result<Box<dyn AsyncRead + Send + Unpin>, FsError>`（新增，替代 SSH `open_read_stream` 破抽象）
9. `async fn stat_many(&self, paths: &[&Path]) -> Vec<Result<FsMetadata, FsError>>`（新增 batched API，default 实现走 `futures::future::join_all`）

trait SHALL 保持 dyn-safe（`&dyn FileSystemProvider` 可用），不引入关联类型。

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

### Requirement: `FsMetadata.identity` 字段采 best-effort 策略

`FsMetadata` SHALL 携带 `identity: Option<FsIdentity>` 字段。`FsIdentity` 是 enum，至少含 `Unix { dev: u64, ino: u64 }` variant 与 `None` 等价的"未知" variant。各 provider 实现 SHALL 按以下策略填充：

- `LocalFileSystemProvider` 在 Unix（`cfg(unix)`）SHALL 填 `Some(FsIdentity::Unix { dev, ino })`，从 `std::os::unix::fs::MetadataExt` 取
- `LocalFileSystemProvider` 在 Windows（`cfg(not(unix))`）SHALL 填 `None`（stable Rust 拿不到 `file_index` / `volume_serial_number`）
- `SshFileSystemProvider` SHALL 永远填 `None`（SFTP 协议不暴露 inode 等价物）

cache 调用方使用 `FsMetadata.identity` 判断"两次 stat 是否同一文件实体"时 SHALL 当作 best-effort——`None` vs `None` 视为"identity 未知"匹配，**不**作为强等价判定（cache 仍以 mtime + size 为主签名，identity 是额外加强项）。文档 SHALL 承认 SSH / Windows 上 rename-replace 同 size 同 mtime 的边界 case 会让 cache 误命中。

#### Scenario: Local Unix 携带强 identity

- **WHEN** `LocalFileSystemProvider` 在 Linux / macOS 上调 `stat(path)` 成功
- **THEN** 返回的 `FsMetadata.identity` SHALL 是 `Some(FsIdentity::Unix { dev, ino })`
- **AND** `dev` / `ino` 值 SHALL 与 `std::fs::metadata(path)` + `MetadataExt::dev()` / `MetadataExt::ino()` 一致

#### Scenario: Windows 与 SSH identity 为 None

- **WHEN** `LocalFileSystemProvider` 在 Windows 上 stat 或 `SshFileSystemProvider` 任意场景 stat
- **THEN** 返回的 `FsMetadata.identity` SHALL 是 `None`

#### Scenario: cache 不强求 identity 匹配

- **WHEN** cache 比较两个 `FsMetadata` 是否等价（rename-replace 边界 case 检测）
- **AND** 任一方 `identity` 是 `None`
- **THEN** cache SHALL 不因 identity 不匹配就判 miss，应回退到 mtime + size 等价判定

### Requirement: `FsError` 提供错误语义元方法

`FsError` enum SHALL 至少含以下 variants：`NotFound(PathBuf)` / `Io { path, source: io::Error }` / `Utf8 { path, source }` / `Unsupported(&'static str)` / `Disconnected { path, reason }` / `TransientExhausted { path, attempts, last_reason }`。每个 variant SHALL 提供两个 inherent 元方法：

- `fn is_retryable(&self) -> bool` —— 返回 `true` 表示这个错误是瞬时的（caller 重试一次有意义），`false` 表示永久（不要重试）
- `fn should_invalidate_cache(&self) -> bool` —— 返回 `true` 表示 cache entry 对应这个 path 应该被清掉（文件可能不存在或损坏），`false` 表示 cache 保留（仅是临时网络抖动等）

#### Scenario: NotFound 不重试，清 cache

- **WHEN** `fs.stat(path)` 返回 `FsError::NotFound(path)`
- **THEN** `err.is_retryable()` SHALL 返回 `false`
- **AND** `err.should_invalidate_cache()` SHALL 返回 `true`（文件不存在，cache 任何 entry 都应该清）

#### Scenario: Disconnected 重试，不清 cache

- **WHEN** SSH 连接突然断开，操作返回 `FsError::Disconnected { ... }`
- **THEN** `err.is_retryable()` SHALL 返回 `true`（重连后可能恢复）
- **AND** `err.should_invalidate_cache()` SHALL 返回 `false`（数据仍可能有效，只是当前连不上）

#### Scenario: TransientExhausted 不重试，不清 cache

- **WHEN** SSH 操作 with_retry 耗尽 3 次仍失败，返回 `FsError::TransientExhausted { attempts: 3, ... }`
- **THEN** `err.is_retryable()` SHALL 返回 `false`（已经重试过了，再试也无意义）
- **AND** `err.should_invalidate_cache()` SHALL 返回 `false`（数据仍可能有效，远端可能恢复）

### Requirement: `ContextId` 三元组作为 cache key 前缀

系统 SHALL 在 `cdt-fs` 内定义 `ContextId` + `HostSignature` 类型：

```rust
pub struct ContextId {
    pub backend_kind: FsKind,
    pub host_signature: Option<HostSignature>,
    pub root_or_home: PathBuf,
}

pub struct HostSignature {
    pub config_digest: [u8; 32],   // SHA-256
    pub display_label: String,      // 仅展示，不参与 Hash/Eq
}
```

其中 `host_signature` 在 Local 时 SHALL 是 `None`，在 SSH 时 SHALL 是 `Some(HostSignature)`。`config_digest` SHALL 是 resolved ssh config 的 SHA-256 hash，hash 输入按以下字段排序拼接（来自 `ssh -G <alias>` 输出）：`hostname` / `port` / `user` / `identityfile`（多个时字典序排序）/ `proxyjump` / `proxycommand` / `hostkeyalias`。连接行为无关字段（如 `loglevel` / `compression` / `serveraliveinterval` / `connecttimeout` / `userknownhostsfile`）SHALL NOT 参与 hash。

`display_label` SHALL 是 `"{user}@{hostname}:{port}"` 格式可读字符串，**仅用于日志 / UI 展示**，不参与 `Hash / PartialEq / Eq`。

`root_or_home` 在 Local 时是 `claude_root` 配置路径（如 `~/.claude/projects/`），在 SSH 时是 `<remote_home>/.claude/projects/`。

`ContextId` 与 `HostSignature` SHALL 实现 `Hash + Eq + Clone + Debug`，让 cache 实现可作为 `HashMap` 的 key 或 key 前缀。

任何 fs 相关 cache（`MetadataCache` / `ParsedMessageCache` / `ProjectScanner` 结果缓存等）SHALL 把 `ContextId` 作为 key 的一部分，**禁止**只用 `PathBuf` 作 key 而忽略上下文。

#### Scenario: 不同 backend_kind 的同 path 不等价

- **WHEN** 比较 Local 上的 `~/.claude/projects/foo` 与 SSH 上的同字面路径的 `ContextId`
- **THEN** 两个 `ContextId` SHALL `!=`（即 Hash 与 Eq 都判不等）

#### Scenario: 同 user@host:port 但不同 ProxyJump 不等价

- **WHEN** 两个 SSH 配置 `user@host:port` 完全一致但 `ProxyJump` 不同（例如其中一个走跳板机，另一个直连）
- **THEN** `HostSignature.config_digest` SHALL 不同
- **AND** 两个 `ContextId` SHALL `!=`，cache 不串扰

#### Scenario: 同 user@host:port 同 ProxyJump 但不同 IdentityFile 不等价

- **WHEN** 两个 SSH 配置 `user@host:port` + `ProxyJump` 一致但 `IdentityFile` 不同
- **THEN** `HostSignature.config_digest` SHALL 不同
- **AND** 两个 `ContextId` SHALL `!=`

#### Scenario: 连接无关字段变化不影响 host_signature

- **WHEN** 同 ssh config 仅 `loglevel` / `compression` / `serveraliveinterval` 字段不同
- **THEN** `HostSignature.config_digest` SHALL 相同
- **AND** 两个 `ContextId` SHALL `==`，cache 跨配置微调复用

#### Scenario: display_label 不参与 Hash/Eq

- **WHEN** 两个 `HostSignature` 的 `config_digest` 字节相等但 `display_label` 不同
- **THEN** `==` SHALL 返回 `true`
- **AND** Hash 值 SHALL 相同

#### Scenario: 同 backend 同 host_signature 同 root 等价

- **WHEN** 同一次 SSH 会话内的两次 cache lookup 用同一 `ContextId`
- **THEN** Hash 与 Eq SHALL 判等，cache 命中

### Requirement: fs-related cache 必须采用"单实例 + ContextId key 前缀"拓扑

任何持有 `FsMetadata` / `FsSignature` / 解析后的 jsonl 消息 / `ProjectScanner::scan` 结果等 fs-derived 数据的 cache SHALL 采用以下拓扑：

1. **单实例**：`LocalDataApi` 持有该 cache 的**一个** `Arc<Mutex<...>>` / `Arc<RwLock<...>>` 实例，**不得**为每个 `ContextId` 创建独立 cache 实例
2. **key 含 ContextId 前缀**：cache 的 key 类型 SHALL 是 `(ContextId, ...)` 形式 tuple（或等价 struct），其中 `ContextId` 是第一成员
3. **LRU 容量按全局计算**：容量上限对所有 `ContextId` 总和适用，不按 context 拆分配额
4. **switch_context 时不必清 cache**：不同 `ContextId` 的 entry 自然不命中（依赖 Hash/Eq 隔离），TTL + signature 校验照常工作

本 change **不**改 `MetadataCache` / `ParsedMessageCache` 现状（PR-B/C 才动），但本 Requirement 是 PR-B/C 必须遵循的 SHALL 句——若 PR-B/C 选了"每 ContextId 一个实例"拓扑，违反本 Requirement，spec validate 应拒。

#### Scenario: cache 实例只有一个

- **WHEN** 检查 `LocalDataApi` 字段
- **THEN** `metadata_cache` SHALL 是单一 `Arc<Mutex<MetadataCache>>` 字段，**不得**是 `HashMap<ContextId, Arc<Mutex<MetadataCache>>>` 类型

#### Scenario: key 类型含 ContextId

- **WHEN** 检查 `MetadataCache` / `ParsedMessageCache` 内部 `HashMap` 类型
- **THEN** key 类型 SHALL 是 `(ContextId, PathBuf)` 或等价 newtype，**不得**仅 `PathBuf`

#### Scenario: 跨 context 复用同一 cache 实例

- **WHEN** 用户在 Local context 与 SSH context A 之间频繁切换
- **THEN** 同一 `MetadataCache` 实例 SHALL 同时持有两个 context 的 entry
- **AND** SSH context A 的 entry SHALL NOT 因切回 Local 被自动清除（依赖 LRU + TTL 自然淘汰）

### Requirement: H1-H6 六条硬契约 SHALL 通过 enforce 机制守护

本 capability SHALL 守护以下六条硬约束（H1-H6），作为 fs 抽象边界的代码组织契约。每条 SHALL 由对应 enforce 机制（自动化测试 / xtask / 单测 / PR review）守护，**不**单独依赖独立的散文档存在：

- **H1**: `cdt-api` / `cdt-config` / 业务路径**禁止**直调 `tokio::fs::*`；allowlist（豁免清单）SHALL 单源住在 `crates/cdt-fs/ALLOWLIST.md`，xtask 与 build-time 集成测试 SHALL 在运行时 parse 此文件作为唯一输入。**Enforce**: `cargo xtask check-fs-direct-calls`（本 change 实现，`--warn-only`；PR-D 完成后切 fail-on-match）
- **H2**: hot path（list / 翻页 / 详情）**禁止** N 次串行 `fs.stat / read`；SHALL 用 `read_dir_with_metadata` 或 `stat_many` batched API。**Enforce**: (a) `FsOpCounter` instrumentation 输出 tracing histogram（本 change 提供基础设施）；(b) 集成测试用 fake provider 断言 fs op 上限（PR-B/C/D 加测）；(c) PR review checklist
- **H3**: 业务**算法**代码 `fs.kind() == Ssh` 默认拒；业务**策略**层（`LocalDataApi`）允许但 SHALL ADR + inline 注释，且**只允许选 `BackendPolicy` 字段值，不允许复制业务算法**。**Enforce**: PR review checklist 按本 change design.md D6 分类表逐行复核；未来可扩展 xtask（本 change 不实现）
- **H4**: HTTP backend SHALL 默认 `initial_load_policy: FullEager` + `max_round_trips_for_initial_page: 1`；Tauri 本地 backend SHALL 默认 `SkeletonThenStream`；transport 层抽象延后处理。**Enforce**: `BackendPolicy::for_local() / for_ssh() / for_http()` 单测断言（本 change 实现）；PR-E wire 时单测断言 `LocalDataApi(http_mode).policy == BackendPolicy::for_http()`
- **H5**: `FileSystemProvider` trait **不**承担分页 / 排序语义——按 mtime 拿前 N 走更高层（`SessionIndex` / `ProjectRepository`），不污染 fs trait。**Enforce**: 集成测试 `cdt-fs/tests/no_pagination_in_trait.rs`（`syn` AST parse trait 方法签名，禁含 `Cursor / Offset / Limit / SortBy / Order` 类型）
- **H6**: `FsError` 必须可操作 —— `is_retryable / should_invalidate_cache` 元方法是 trait 契约的一部分。**Enforce**: 单测覆盖每个 variant 的元方法返回值（本 change 实现）

#### Scenario: H1 allowlist single source of truth 来自 `crates/cdt-fs/ALLOWLIST.md`

- **WHEN** `cargo xtask check-fs-direct-calls --warn-only` 在 CI 跑
- **THEN** xtask SHALL 从 `crates/cdt-fs/ALLOWLIST.md` 的 `## Allowlist` 段 markdown table 解析 allowlist
- **AND** `crates/cdt-api/tests/build_time_invariants.rs`（PR #186 留下的）若未来加入 `tokio::fs::*` 类似检查也 SHALL 从同一文件 parse，**禁止** xtask 源码或测试源码硬编码 allowlist 副本

#### Scenario: H5 fs trait 不暴露排序参数（自动化测试）

- **WHEN** CI 跑 `cargo test -p cdt-fs --test no_pagination_in_trait`
- **THEN** 必须有集成测试用 `syn::parse_str::<syn::File>` parse trait 方法签名，断言无方法参数 type 含 `Cursor` / `Offset` / `Limit` / `SortBy` / `Order`
- **AND** 该测试 fail 时 CI 拒，panic 消息含 method name + violating arg type + 指向本 spec H5 + design.md D2

### Requirement: `xtask check-fs-direct-calls` 自动化 H1

系统 SHALL 提供 `xtask check-fs-direct-calls` 命令（或等价 cargo / shell 脚本），扫描业务 crate 内 `tokio::fs::*` 直调反模式。脚本 SHALL：

1. 扫描路径：`crates/cdt-api/src/**/*.rs`、`crates/cdt-config/src/**/*.rs`、业务路径其他 crate
2. allowlist：从 `crates/cdt-fs/ALLOWLIST.md` 的 `## Allowlist` markdown table 解析；本 change 后 allowlist SHALL 含 provider 实现文件（`crates/cdt-fs/**`、`crates/cdt-ssh/src/provider.rs`）、`crates/cdt-cli/**`、`crates/cdt-watch/**`、`crates/xtask/**`、`**/tests/**`、以及 design.md D4 / D7 钉死的 Local-only 业务路径（`crates/cdt-config/**`、`crates/cdt-api/src/notifier.rs`、`crates/cdt-api/src/http/routes.rs`、`crates/cdt-api/src/ipc/image_disk_cache.rs`）
3. 匹配模式：`tokio::fs::metadata` / `tokio::fs::read` / `tokio::fs::File::open` / `tokio::fs::read_to_string` / `tokio::fs::read_dir` / `tokio::fs::write` / `tokio::fs::create_dir(_all)?` / `tokio::fs::remove(_file|_dir(_all)?)?` 等 13 个 forbidden patterns
4. 退出码：non-allowlist 命中时 SHALL 默认 exit 1 (`ExitCode::FAILURE`)，CI 拒；`--warn-only` flag 仅作本地诊断 opt-in（exit 0 + warning 输出），CI 不 SHALL 加此 flag

#### Scenario: xtask 命令存在且可调用

- **WHEN** 在仓库根跑 `cargo xtask check-fs-direct-calls` 或等价命令
- **THEN** 命令 SHALL 存在并产出 grep 结果到 stdout
- **AND** 退出码反映检查结果

#### Scenario: allowlist 路径不报警

- **WHEN** xtask 扫描时遇到 `crates/cdt-fs/src/local.rs` 内 `tokio::fs::metadata` 调用
- **THEN** SHALL NOT 报警（因为这是 LocalFileSystemProvider 内部实现，被 allowlist）

#### Scenario: 默认 fail-on-match（CI enforce）

- **WHEN** CI 跑 `cargo xtask check-fs-direct-calls`（**不**带 `--warn-only` flag）且业务路径出现一处非 allowlist 的 `tokio::fs::*` 直调
- **THEN** xtask 进程 SHALL 以 `ExitCode::FAILURE` 退出，CI step fail
- **AND** stdout SHALL 含 `error: <relpath> (H1 violation) -- '<pattern>' at <relpath>:<line_no>` 格式的错误行 + 末尾 `xtask: check-fs-direct-calls found N violation(s); allowlist source = crates/cdt-fs/ALLOWLIST.md`

#### Scenario: `--warn-only` 仅供本地诊断

- **WHEN** 开发者本地跑 `cargo xtask check-fs-direct-calls --warn-only`
- **THEN** xtask SHALL 以 `ExitCode::SUCCESS` 退出 + stdout 列出 `warning:` 前缀的违反清单 + 末尾打印 `xtask: --warn-only is on，exit 0`
- **AND** 这条路径仅作开发者迁移期诊断手段，CI workflow `.github/workflows/*.yml` SHALL NOT 在 `cargo xtask check-fs-direct-calls` invocation 上传 `--warn-only`

#### Scenario: ALLOWLIST.md 顶部固化豁免准则

- **WHEN** 阅读 `crates/cdt-fs/ALLOWLIST.md`
- **THEN** 文件 SHALL 在 `## Allowlist` table 之前的段落明示豁免准则：
  - 路径在 design.md 已分类为 Local-only 业务路径（typical: 用户配置 / 系统通知历史 / Local-only disk cache）
  - 或 SSH 路径有显式 graceful skip / 该路径永远不接 SSH context（HTTP routes / notifier）
  - 或测试 fixture / 测试 setup 写文件（覆盖 `**/tests/**`）
- **AND** 任何新加 ALLOWLIST 行的 PR SHALL 在 PR description 引用对应 design 决策的锚点（典型 D7 cdt-config 全 Local / D4 image disk cache 抽 module）

### Requirement: `BackendPolicy` enum 雏形定义

系统 SHALL 在 `cdt-fs` 内定义 `BackendPolicy` struct + `InitialLoadPolicy` enum + `PrefetchPolicy` enum，作为后续 PR-E 接入业务的契约锚点：

```rust
pub struct BackendPolicy {
    pub initial_load_policy: InitialLoadPolicy,
    pub max_round_trips_for_initial_page: u8,
    pub supports_incremental_updates: bool,
    pub prefetch_policy: PrefetchPolicy,
}

pub enum InitialLoadPolicy {
    FullEager,
    SkeletonThenStream,
}

pub enum PrefetchPolicy {
    None,
    PrefetchNext,
}
```

`PrefetchPolicy` 与 `InitialLoadPolicy` SHALL 是**正交字段**——前者表达"翻页预取策略"（不预取 vs 预取下一页），后者表达"首屏加载策略"（一次性 vs 增量补全）。**不得**把 `PrefetchNext` 塞进 `InitialLoadPolicy` 当第三 variant。

本 change SHALL 只定义类型 + 提供 `BackendPolicy::for_local()` / `BackendPolicy::for_ssh()` / `BackendPolicy::for_http()` 三个 const-like 构造器并加单测；**不得** wire 到 `LocalDataApi` 或任何业务路径。本 change 所有 backend 默认 `prefetch_policy: PrefetchPolicy::None`（PR-E 才可能改）。

#### Scenario: for_local 返回 SkeletonThenStream + None prefetch

- **WHEN** 调 `BackendPolicy::for_local()`
- **THEN** SHALL 返回 `initial_load_policy = SkeletonThenStream`，`max_round_trips_for_initial_page >= 2`，`supports_incremental_updates = true`，`prefetch_policy = None`

#### Scenario: for_ssh 与 for_http 返回 FullEager + None prefetch

- **WHEN** 调 `BackendPolicy::for_ssh()` 或 `BackendPolicy::for_http()`
- **THEN** SHALL 返回 `initial_load_policy = FullEager`，`max_round_trips_for_initial_page = 1`，`supports_incremental_updates = false`，`prefetch_policy = None`（HTTP 端可能未来加 SSE 但本 change 视为单 round trip eager；prefetch 是 PR-E 才能引入的能力）

#### Scenario: PrefetchPolicy 与 InitialLoadPolicy 正交

- **WHEN** 显式构造 `BackendPolicy { initial_load_policy: SkeletonThenStream, prefetch_policy: PrefetchNext, .. }`
- **THEN** SHALL 编译通过且 `==` 自身（两字段独立可组合）
- **AND** SHALL NOT 出现 `InitialLoadPolicy` 含 `PrefetchNext` variant 的设计

#### Scenario: 业务代码尚未消费 BackendPolicy

- **WHEN** grep workspace 寻找 `BackendPolicy` 字段读取
- **THEN** 只能找到 `cdt-fs` 内部定义 + 单测；`LocalDataApi` 或其他业务路径 SHALL NOT 已经读取 `BackendPolicy`（PR-E 才接入）

### Requirement: fs trait 不承担分页与排序语义

`FileSystemProvider` trait **不得**暴露任何按 mtime / size / 名字排序的方法，**不得**暴露 cursor / offset 等分页参数。`read_dir` 与 `read_dir_with_metadata` 返回 `Vec<DirEntry>` 顺序由底层文件系统决定，caller SHALL 在更高层（如 `ProjectScanner` / `SessionIndex` / IPC 层）自行排序与分页。

PR #186 引入的 `GroupCursor` k-way merge 是高层分页的正确实现位置范例 —— 它在 `LocalDataApi::list_group_sessions` 而非 `FileSystemProvider`。任何未来"按某种排序拿前 N 个 session"的需求 SHALL 走类似的高层抽象（`SessionIndex` / `ProjectRepository` 等待引入），**不得**给 fs trait 加排序参数或分页 API。

#### Scenario: trait 暴露面不含排序参数

- **WHEN** 检查 `cdt_fs::FileSystemProvider` trait 的方法签名
- **THEN** SHALL NOT 含任何接受 `SortBy` / `Order` / `Cursor` / `Offset` / `Limit` 类参数的方法

#### Scenario: 调用方自行排序

- **WHEN** `ProjectScanner` 需要 sessions 按 mtime 降序排
- **THEN** 调用方 SHALL 调 `fs.read_dir_with_metadata`，自己在调用方代码内 `Vec::sort_by_key(|e| Reverse(e.metadata.mtime))`，**不得**让 trait 帮排

### Requirement: Provider instrumentation 入口可观测 fs op 次数

`cdt-fs` SHALL 提供 `InstrumentedFs<P>` wrapper（`P: FileSystemProvider`）+ `FsOpCounter` + `with_fs_counter` 入口，让业务调用方（PR-B/C/D 起）可在每个 IPC command 边界统计 fs 操作次数（`stat / read / read_dir / read_dir_with_metadata / read_to_string / read_lines_head / open_read / stat_many` 各计数）。本 change 只提供基础设施 + 单测验证 counter 准确，**不**接入业务路径（PR-B 起接入）。

**注入机制钉死**（codex 第三轮 Medium G）：counter 通过 **`InstrumentedFs` wrapper** 在 trait 调用边界自动计数，**不**要求每个 provider impl（`LocalFileSystemProvider` / `SshFileSystemProvider` / fake provider）显式调 `record_stat()` 等 hook。具体语义：

1. `InstrumentedFs<P>` 实现 `FileSystemProvider`，每个 trait 方法内部先 `FsOpCounter::current().record_<op>()`（通过 `task_local!` 拿当前 counter），再 delegate 到 `inner.<op>()`
2. 调用方注入 fs handle 时包一层：`let fs = Arc::new(InstrumentedFs::new(LocalFileSystemProvider::new()))`；或更轻量的 `let fs = local_handle().instrumented()` 扩展方法
3. 测试场景：fake provider 也包 `InstrumentedFs` wrapper 即可，**不**需要修改 fake 内部代码
4. 未包 wrapper 的 fs handle 调 trait 方法不计数（向后兼容）

`with_fs_counter<F, Fut>(f: F) -> (Fut::Output, FsOpCounts)` async wrapper 用 `task_local!` 设置当前 counter，`InstrumentedFs` 调 `FsOpCounter::current()` 时通过 task_local 拿到——避免全局 atomic 跨并发 IPC command 污染。

instrumentation 入口 SHALL 满足：

1. 基于 `tokio::task_local!` 实现，避免全局 atomic 让并发 IPC command 互相干扰
2. `with_fs_counter<F, Fut>(f: F) -> (Fut::Output, FsOpCounts)` async wrapper：调用方包住代码，结束拿计数
3. `InstrumentedFs<P>` wrapper 在 trait 调用边界自动 record，无需 provider impl 配合
4. 与 `tracing::info!(target = "cdt_fs::ops", ...)` 集成——`FsOpCounter` Drop 时自动 emit 一条 tracing event，含每种操作的次数

#### Scenario: InstrumentedFs wrapper 在 trait 边界自动计数

- **WHEN** 调用方 `let fs = InstrumentedFs::new(LocalFileSystemProvider::new())` 然后 `with_fs_counter(async { fs.stat(p1).await; fs.stat(p2).await; fs.read_dir(p3).await; })`
- **THEN** 返回的 `FsOpCounts` SHALL 含 `stat: 2` + `read_dir: 1`
- **AND** `LocalFileSystemProvider` 内部代码 SHALL NOT 含任何 counter 调用（计数发生在 wrapper 层）

#### Scenario: 未包 wrapper 不计数

- **WHEN** 调用方直接用 `LocalFileSystemProvider`（未包 InstrumentedFs）+ `with_fs_counter(async { fs.stat(p1).await })`
- **THEN** 返回的 `FsOpCounts` SHALL 全 0
- **AND** SHALL NOT panic（向后兼容）

#### Scenario: counter 不跨 task 污染

- **WHEN** 两个并发 tokio task 各自调 `with_fs_counter` + 各自的 `InstrumentedFs`，task A 调 5 次 stat，task B 调 3 次 stat
- **THEN** task A 拿到 `stat: 5`，task B 拿到 `stat: 3`，互不影响（依赖 `tokio::task_local!` 隔离）

#### Scenario: tracing emit on Drop

- **WHEN** `with_fs_counter` 闭包正常结束
- **THEN** SHALL emit 一条 `tracing::info!(target = "cdt_fs::ops", ...)` event 含全部计数字段

#### Scenario: 业务路径暂不消费（PR-B 起接入）

- **WHEN** 本 change 合并后 grep `with_fs_counter` 在 `crates/cdt-api/src/**/*.rs`
- **THEN** SHALL NOT 找到调用（业务路径未接入）；只在 `crates/cdt-fs/**/*.rs` 内部 + 测试找到

### Requirement: 本 change 零业务变化下性能基线不退化

本 change 是基建 PR-A，原则上**零业务代码变化**——但 trait 加 `Box<dyn AsyncRead>` 动态分发改了底层 LocalFileSystemProvider 内部路径（之前调用方拿到 inherent typed File，现在拿 Box dyn）。系统 SHALL 通过两套性能 gate 验证零退化：

1. **端到端 baseline 校验**：`cargo test --release -p cdt-api --test perf_cold_scan -- --ignored --nocapture` 与 `perf_get_session_detail` 在本 change apply 前后各跑 **5 次**取 min / median / stddev。回归判据：
   - median 退化 > 5% → 拒
   - stddev > 8ms（baseline 95ms 的 ~8%）→ 拒（说明引入了不稳定性）
   - min 退化 > 8% → 拒
2. **Local micro benchmark**（D4 量化要求）：新增 `crates/cdt-fs/benches/open_read_overhead.rs`，对比同 jsonl 文件（~500KB 与 ~5MB 两个 size）走 `tokio::fs::File::open + BufReader::lines` 直读路径 vs 走 `FileSystemProvider::open_read` dyn 路径，跑 10 次取 min / median / stddev。dyn 路径 SHALL 在 median 上 ≤ 直读路径 × 1.3（vtable overhead 上限），超过则拒

性能 gate SHALL 在本 change apply commit 上有 reproducible 数据（PR 描述贴 `/usr/bin/time -lp` 四维输出 + micro bench 结果），不只口头声称"零变化"。

#### Scenario: 端到端 baseline 不退化

- **WHEN** apply 本 change 后跑 `perf_cold_scan` 5 次
- **THEN** median SHALL ≤ 主线 baseline × 1.05
- **AND** stddev SHALL ≤ 8ms

#### Scenario: open_read dyn 路径 micro bench 不超 1.3x

- **WHEN** 跑 `cargo bench -p cdt-fs --bench open_read_overhead` 10 次
- **THEN** `fs.open_read` dyn 路径的 median 耗时 SHALL ≤ `tokio::fs::File::open` 直读路径 × 1.3
- **AND** 若超过 1.3x，本 change PR review 拒，需重新评估 D4 决策（关联类型 vs dyn dispatch trade-off）


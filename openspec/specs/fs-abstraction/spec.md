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


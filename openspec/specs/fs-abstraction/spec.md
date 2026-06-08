# fs-abstraction Specification

## Purpose
TBD - created by archiving change unify-fs-abstraction. Update Purpose after archive.
## Requirements
### Requirement: `cdt-fs` crate 是 fs 抽象的唯一物理位置

系统 SHALL 把所有文件系统抽象类型（fs provider trait、其 Local 实现、`FsError` / `FsMetadata` / `FsKind` / `FsIdentity` / `DirEntry` / `EntryKind` / `ContextId` / `BackendPolicy` / `InitialLoadPolicy`）的**真相源**集中于专用的 fs 抽象 crate。其它 crate（业务侧 / 历史 discover crate）SHALL 通过 re-export 兼容历史 import 路径，**不得**重新定义同名类型。业务 crate SHALL 直接依赖 fs 抽象 crate，不得通过历史 discover crate 间接拿 fs 抽象。

fs 抽象 crate SHALL **不**依赖任何业务 crate；允许的运行时依赖仅限通用基础设施（async runtime / async-trait / 错误派生 / 日志 facade）。

#### Scenario: 业务 crate 直接依赖 fs 抽象 crate

- **WHEN** 业务 crate 需要 fs provider trait
- **THEN** 其 manifest SHALL 直接依赖 fs 抽象 crate
- **AND** import 首选直接路径；通过历史 discover crate 的间接 import 仅作为兼容期保留

#### Scenario: 兼容性 re-export 保持等价

- **WHEN** 老代码通过历史 discover crate import fs 抽象类型
- **THEN** 编译 SHALL 成功，运行时行为与直接 import fs 抽象 crate 完全一致

#### Scenario: fs 抽象 crate 不依赖业务 crate

- **WHEN** 检查 fs 抽象 crate 的 manifest 依赖
- **THEN** SHALL NOT 含任何业务 crate（discover / api / config / ssh / cli / watch / analyze / parse / core）
- **AND** 允许的依赖仅限运行时 / 错误 / 日志基础设施

### Requirement: `FileSystemProvider` trait 暴露 7 个核心方法

fs provider trait SHALL 暴露以下行为类别（编译时强制实现，default 实现可被 override）：

- **identity**：返回 backend 类型（Local / Ssh / 等）的同步访问器
- **目录探测**：判路径存在；列举目录条目（仅类型）；列举目录条目并附 metadata
- **元数据**：取 path 的 `FsMetadata`（含 size / mtime / `identity: Option<FsIdentity>`）；批量 stat 多 path（default 实现走并发等价 join_all，后端可 override 拿真 batch）
- **读**：全量读为 UTF-8；仅读首 N 行；返回流式 `AsyncRead + Send + Unpin` 句柄
- **写**：原子覆盖写（tmp + rename，rename 失败 best-effort 清理 tmp）；递归创建目录（已存在不报错）；删文件（不存在返 `FsError::NotFound`，不递归删目录）

trait SHALL 保持 dyn-safe（`&dyn` 可用），不引入关联类型。

**`read_dir_with_metadata` override 契约**：default 实现走 1 + N 兜底（`read_dir` 后逐条 stat），性能敏感后端 SHALL override 使用底层协议原生 1-RTT batch。SSH override 可能因 server 未返 mtime 出现部分 entry metadata = None；caller SHALL 把此条视同 cache mismatch（走 cache wrapper miss 路径补齐），实现 SHALL NOT 在 trait 实现层做 per-entry stat fallback（否则退化为 N+1 RTT）。

**原子写契约**：原子覆盖在 reader 角度 SHALL 永远观察到旧内容或新内容整版，永不观察到截断 / 半写状态。Local 后端依赖 OS rename(2) 原子语义；SSH 后端 SHALL 优先走 SFTP `posix-rename` 扩展，不支持时降级（降级路径有极短窗口 reader 可见 target missing，acceptable）。tmp 路径 suffix SHALL 使用进程内单调计数器 + pid hex 防并发碰撞，**不**得依赖 wall-clock 纳秒（Windows 时钟精度并发碰撞）。同 path 多 caller 并发写 SHALL 是 last-write-wins 语义。

**新写方法的 instrumentation 计数**：fs op counter 类型 SHALL 暴露写 / 创建目录 / 删文件三个独立计数字段；wrapper 在三个新方法的入口同步 record，与读侧已有计数路径一致；tracing 输出 SHALL 包含三个新字段。

#### Scenario: open_read 在 Local 上返回流式句柄

- **WHEN** caller 在 Local provider 上调 open_read 且 path 存在可读
- **THEN** SHALL 返回 `Ok(Box<dyn AsyncRead + Send + Unpin>)` 等价句柄
- **AND** caller SHALL 能用 buffered line reader 流式按行读取

#### Scenario: open_read 在 SSH 上走 SFTP 流式句柄

- **WHEN** caller 在 SSH provider 上调 open_read
- **THEN** SHALL 返回 SFTP 流式句柄包装，实现 `AsyncRead + Send + Unpin`
- **AND** caller SHALL NOT 需要 downcast 到 SSH provider 才能流式读

#### Scenario: stat_many 默认实现并发执行

- **WHEN** caller 调 stat_many 且 provider 未 override
- **THEN** 实现 SHALL 并发对所有 path 走 stat
- **AND** 返回结果顺序与 input paths 严格对应

#### Scenario: read_dir_with_metadata 未优化路径走 N+1 RTT

- **WHEN** 某后端未 override read_dir_with_metadata
- **THEN** SHALL 走 trait default：先 read_dir 拿 entries，再对每条 file entry 补 stat
- **AND** 总 op 数 SHALL 为 1 + N（N = file entry 数）

#### Scenario: SSH 路径 read_dir_with_metadata 复用 read_dir 不退化

- **WHEN** 在 SSH context 下调 read_dir_with_metadata
- **THEN** SSH provider SHALL override default impl，复用单次 SFTP READDIR reply 自带的 entry attrs
- **AND** 总 SFTP RTT 数 SHALL 为 1（**SHALL NOT** 退化为 N+1）
- **AND** 部分 entry mtime missing 时 SHALL 在 metadata = None 状态返 caller，由上层 cache 校验决定 fallback——**不**在 trait 实现层补 stat

#### Scenario: 原子写在 Local 上原子覆盖

- **WHEN** caller 在 Local provider 上调原子写且目标 path 已有旧内容
- **THEN** 实现 SHALL 走 tmp + rename
- **AND** 并发 reader SHALL 永远拿到旧 content 或新 content 整版，绝不拿到截断 / 半写中间态
- **AND** rename 失败 SHALL best-effort 清理 tmp（清理失败不向上传播）

#### Scenario: 原子写在 SSH 上原子覆盖

- **WHEN** caller 在 SSH provider 上调原子写远端目标 path
- **THEN** 实现 SHALL 通过 SFTP 写到 tmp 后调原子 rename
- **AND** 远端 SFTP server side OS 提供原子性保证
- **AND** 暂态错误重试遵循既有 SSH retry 策略（指数退避，上限 3 次）

#### Scenario: create_dir_all 不报已存在错

- **WHEN** caller 调 create_dir_all 路径已存在
- **THEN** SHALL 返 `Ok(())`，不返 AlreadyExists 错误

#### Scenario: remove_file 不存在返 NotFound

- **WHEN** caller 调 remove_file 路径不存在
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

`FsError` enum SHALL 至少含以下 variants：`NotFound(PathBuf)` / `Io { path, source: io::Error }` / `Utf8 { path, source }` / `Unsupported(&'static str)` / `Disconnected { path, reason }` / `TransientExhausted { path, attempts, last_reason }`。每个 variant SHALL 提供以下 inherent 元方法：

- `fn is_retryable(&self) -> bool` —— 返回 `true` 表示这个错误是瞬时的（caller 重试一次有意义），`false` 表示永久（不要重试）
- `fn should_invalidate_cache(&self) -> bool` —— 返回 `true` 表示 cache entry 对应这个 path 应该被清掉（文件可能不存在或损坏），`false` 表示 cache 保留（仅是临时网络抖动等）
- `fn is_likely_channel_dead(&self) -> bool` —— 返回 `true` 表示底层 transport channel（典型 SSH SFTP）很可能已死 / 半死，caller（典型 `cdt-discover::ProjectScanner` SSH 分支）SHALL 据此 fail-fast 而非 silent continue 凑半成品列表。语义独立于 `is_retryable`：channel-dead 是更强的"该不该 abort 整轮 scan"信号，与"该不该再 retry 一次"不同（`Disconnected.is_retryable() == true` 但 `is_likely_channel_dead() == true` 同时成立——表达"重连后能 retry，但当前 scan 已不该继续"）

`is_likely_channel_dead` 命中规则：

- `Disconnected { .. }`：恒 true（已经是显式 channel 断开信号）
- `TransientExhausted { last_reason }`：`last_reason.to_ascii_lowercase()` 含 `session closed` / `eof` / `broken pipe` / `epipe` / `connection reset` / `econnreset` 任一关键字时 true（`with_retry` 3 次后仍是 transport-dead 即视同 channel 真死）；纯 `timeout` / `eagain` 等不含 transport-dead 关键字的 transient exhausted 返 false（保留"远端短暂不可达"的容错语义）
- `Io { source }`：`source.kind()` 是 `BrokenPipe` / `ConnectionReset` / `ConnectionAborted` 时 true
- `NotFound` / `Utf8` / `Unsupported`：恒 false（与 channel 状态无关）

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

#### Scenario: Disconnected 触发 channel-dead

- **WHEN** 操作返回 `FsError::Disconnected { path, reason }`
- **THEN** `err.is_likely_channel_dead()` SHALL 返回 `true`（无论 reason 内容）

#### Scenario: TransientExhausted 含 transport-dead 关键字触发 channel-dead

- **WHEN** 操作返回 `FsError::TransientExhausted { last_reason: "broken pipe", attempts: 3, .. }` 或 `FsError::TransientExhausted { last_reason: "session closed", .. }` 或类似含 `eof` / `epipe` / `connection reset` / `econnreset` 关键字的 reason
- **THEN** `err.is_likely_channel_dead()` SHALL 返回 `true`
- **AND** caller 若是 `ProjectScanner` SSH 分支 SHALL 据此 abort 整轮 scan（详 `project-discovery::Scan Claude projects directory` Requirement 中的 SSH 模式分流规则）

#### Scenario: TransientExhausted 仅含纯 timeout 不触发 channel-dead

- **WHEN** 操作返回 `FsError::TransientExhausted { last_reason: "timeout", .. }` 或 `last_reason: "etimedout"` 或 `last_reason: "eagain"` 等不含 transport-dead 关键字的 reason
- **THEN** `err.is_likely_channel_dead()` SHALL 返回 `false`（保留"远端短暂不可达"的容错语义，留给 polling watcher 的独立 timeout counter 在持续 18s 后自行触发 dead_signal，与 scanner 一次性 abort 解耦）

#### Scenario: Io BrokenPipe / ConnectionReset / ConnectionAborted 触发 channel-dead

- **WHEN** 操作返回 `FsError::Io { source }`，`source.kind()` 是 `BrokenPipe` / `ConnectionReset` / `ConnectionAborted` 任一
- **THEN** `err.is_likely_channel_dead()` SHALL 返回 `true`

#### Scenario: NotFound / Utf8 / Unsupported 不触发 channel-dead

- **WHEN** 操作返回 `FsError::NotFound(_)` 或 `FsError::Utf8 { .. }` 或 `FsError::Unsupported(_)`
- **THEN** `err.is_likely_channel_dead()` SHALL 返回 `false`（这些错误与底层 transport 状态无关）

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

#### Scenario: cache key 含上下文身份

- **WHEN** 检查 `MetadataCache` / `ParsedMessageCache` 内部 `HashMap` 类型
- **THEN** key 类型 SHALL 是 `(ContextId, PathBuf)` 或等价 newtype，**不得**仅 `PathBuf`

#### Scenario: 跨 context 复用同一 cache 实例

- **WHEN** 用户在 Local context 与 SSH context A 之间频繁切换
- **THEN** 同一 `MetadataCache` 实例 SHALL 同时持有两个 context 的 entry
- **AND** SSH context A 的 entry SHALL NOT 因切回 Local 被自动清除（依赖 LRU + TTL 自然淘汰）

### Requirement: H1-H6 六条硬契约 SHALL 通过 enforce 机制守护

本 capability SHALL 守护以下六条硬约束（H1-H6），作为 fs 抽象边界的代码组织契约。每条 SHALL 由对应 enforce 机制（自动化测试 / xtask / 单测 / PR review）守护，**不**单独依赖独立的散文档存在。具体守护脚本 / 测试名 / allowlist 内容随代码组织演进，行为契约由本 Requirement 兜底。

- **H1**：业务路径**禁止**直调底层 OS fs API（统一走 fs 抽象 trait）；豁免清单 SHALL 单源住在 fs 抽象 crate 内的人类可读 allowlist 文件，xtask 与 build-time 集成测试 SHALL 在运行时 parse 此文件作为唯一输入。**Enforce**：xtask 默认 fail-on-match（CI 拒绝违例），本地诊断模式可降级为 warn-only。
- **H2**：hot path（list / 翻页 / 详情）**禁止** N 次串行 stat / read；SHALL 走带 metadata 的 batched API。**Enforce**：fs op counter instrumentation tracing histogram + 集成测试用 fake provider 断言 fs op 上限 + PR review checklist。
- **H3**：业务**算法**代码 `fs.kind() == Ssh` 默认拒；业务**策略**层允许但 SHALL 配 ADR + inline 注释，且**只允许选 BackendPolicy 字段值，不允许复制业务算法**。**Enforce**：PR review checklist 按 D6 分类表逐行复核。
- **H4**：HTTP backend SHALL 默认 FullEager + max_round_trips_for_initial_page=1；Tauri 本地 backend SHALL 默认 SkeletonThenStream；transport 层抽象延后处理。**Enforce**：BackendPolicy 构造器单测断言 + wire 时单测断言 HTTP backend policy。
- **H5**：fs provider trait **不**承担分页 / 排序语义——按 mtime 拿前 N 走更高层（session index / project repository），不污染 fs trait。**Enforce**：集成测试 AST parse trait 方法签名，禁含 `Cursor / Offset / Limit / SortBy / Order` 类型。
- **H6**：`FsError` 必须可操作 —— `is_retryable / should_invalidate_cache / is_likely_channel_dead` 元方法是 trait 契约的一部分。**Enforce**：单测覆盖每个 variant 的元方法返回值。

#### Scenario: H1 allowlist 单源真相

- **WHEN** xtask / build-time 集成测试需要 H1 allowlist
- **THEN** 数据来源 SHALL 是 fs 抽象 crate 内的 ALLOWLIST 文档；**禁止** xtask 源码或测试源码硬编码 allowlist 副本

#### Scenario: H5 fs trait 不暴露排序参数

- **WHEN** CI 跑 trait 方法签名检查
- **THEN** SHALL 自动化检验 trait 方法参数 type 不含 `Cursor` / `Offset` / `Limit` / `SortBy` / `Order`
- **AND** 测试 fail 时 CI 拒，错误信息含 method name + violating arg type + 指向本 spec H5 + design.md

### Requirement: `xtask check-fs-direct-calls` 自动化 H1

系统 SHALL 提供 H1 自动化扫描入口（xtask 命令或等价脚本），扫描业务 crate 内底层 fs API 直调反模式。脚本行为契约：

1. 扫描路径：所有业务 crate 源码（discover / api / config / ssh / cli / watch / analyze / parse / core 内非 provider 实现路径）
2. allowlist：从 fs 抽象 crate 内的 ALLOWLIST 文档解析（豁免准则：design 已分类的 Local-only 业务路径 / SSH 路径有显式 graceful skip / 测试 fixture 写文件）
3. 匹配模式：13 个底层 fs API 直调形态（含 metadata / read / open / read_to_string / read_dir / write / create_dir / create_dir_all / remove_file / remove_dir / remove_dir_all 等）
4. 退出码：non-allowlist 命中时 SHALL 默认非零退出（CI 拒），仅本地诊断 opt-in 时降级为 warn 级输出 + 零退出

#### Scenario: 扫描命令存在且可调用

- **WHEN** 在仓库根触发 H1 扫描入口
- **THEN** 命令 SHALL 存在并产出扫描结果到 stdout
- **AND** 退出码反映检查结果

#### Scenario: allowlist 路径不报警

- **WHEN** 扫描时遇到 fs 抽象 crate 内部 provider 实现的底层 fs API 调用
- **THEN** SHALL NOT 报警（被 allowlist）

#### Scenario: 默认禁直接调用并由 CI 拦截

- **WHEN** CI 跑 H1 扫描（不带本地诊断 flag），业务路径出现一处非 allowlist 的底层 fs API 直调
- **THEN** 进程 SHALL 以非零状态退出，CI step fail
- **AND** 输出 SHALL 含违例文件 / 行号 / 模式 / 总计违规数 + 指向 ALLOWLIST 文档的指针

#### Scenario: 本地诊断绕过

- **WHEN** 开发者本地用诊断 flag 跑扫描
- **THEN** SHALL 以零状态退出 + warn 级前缀列出违规清单
- **AND** CI workflow SHALL NOT 启用此诊断 flag

#### Scenario: ALLOWLIST 文档顶部固化豁免准则

- **WHEN** 阅读 ALLOWLIST 文档
- **THEN** 文档 SHALL 在 allowlist 表之前明示豁免准则（Local-only 业务路径、SSH graceful skip、测试 fixture 等）
- **AND** 任何新加 ALLOWLIST 行的 PR SHALL 在 PR description 引用对应 design 决策的锚点

### Requirement: `BackendPolicy` enum 雏形定义

fs 抽象 crate SHALL 定义 backend 行为策略 struct + 三个独立 enum 字段，作为业务路径**选择后端相关行为**的真相源：

- **InitialLoadPolicy**：表达"首屏加载策略"（一次性 vs 增量补全），至少含 `FullEager` 与 `SkeletonThenStream` 两个 variant
- **PrefetchPolicy**：表达"翻页预取策略"（不预取 vs 预取下一页），至少含 `None` 与 `PrefetchNext` 两个 variant
- **StaleCheckStrategy**：表达"5min stale 判定策略"，至少含 `LocalClock5min`（用本机 mtime 比对）与 `SkipUntilClockSync`（远端 mtime 跨 clock domain 不可比对）

`BackendPolicy` struct SHALL 是 Copy + Eq + Clone + Debug；字段 SHALL 是 primitive 或 Copy 类型（bool / u8 / Copy enum），**禁止**含 `Arc<dyn Trait>` / `Box<dyn Trait>` / `Vec<T>` / `String` 等非 Copy 字段——业务侧的 trait object 与 Clone 类型策略 SHALL 放在更高层（业务 IPC 层 backend resolvers）与 `BackendPolicy` 配套。

`PrefetchPolicy` 与 `InitialLoadPolicy` SHALL 是**正交字段**——**不得**把 `PrefetchNext` 塞进 `InitialLoadPolicy` 当 variant。

本 capability SHALL 提供三个 const 构造器：Local / SSH / HTTP 各一个，每个完整覆盖所有字段值。业务 callsite SHALL 通过 `BackendPolicy` 字段读取 backend-specific 行为，**禁止**在业务算法层直接 `match fs.kind()` 表达策略——`fs.kind()` 仅允许在策略**派生**点（顶层 helper 或 backend resolver 内部）使用。

#### Scenario: Local policy 含 SkeletonThenStream + 本机 stale

- **WHEN** 取 Local backend 的 policy 构造器结果
- **THEN** SHALL 返回 `initial_load_policy = SkeletonThenStream`、`max_round_trips_for_initial_page >= 2`、`supports_incremental_updates = true`、`prefetch_policy = None`
- **AND** SHALL 返回 `supports_memory = true`、`supports_subagent_scan = true`、`stale_check_strategy = LocalClock5min`

#### Scenario: SSH policy 含 FullEager + 跳过 stale

- **WHEN** 取 SSH backend 的 policy 构造器结果
- **THEN** SHALL 返回 `initial_load_policy = FullEager`、`max_round_trips_for_initial_page = 1`、`supports_incremental_updates = false`、`prefetch_policy = None`
- **AND** SHALL 返回 `supports_memory = true`、`supports_subagent_scan = false`、`stale_check_strategy = SkipUntilClockSync`

#### Scenario: HTTP policy 按 Local 数据源语义填

- **WHEN** 取 HTTP backend 的 policy 构造器结果
- **THEN** initial-load 相关字段 SHALL 保持现状（`FullEager` / `max_round_trips_for_initial_page = 1`）
- **AND** 其余字段按 Local 数据源语义填（`supports_memory = true` / `supports_subagent_scan = true` / `LocalClock5min`）

#### Scenario: PrefetchPolicy 与 InitialLoadPolicy 正交

- **WHEN** 显式构造 `BackendPolicy` 含 `SkeletonThenStream` + `PrefetchNext`
- **THEN** SHALL 编译通过且 `==` 自身（两字段独立可组合）
- **AND** `InitialLoadPolicy` 内 SHALL NOT 出现 `PrefetchNext` variant

#### Scenario: BackendPolicy 可按值复制并相等比较

- **WHEN** 编译 fs 抽象 crate
- **THEN** `BackendPolicy` SHALL derive Copy + Clone + PartialEq + Eq + Debug
- **AND** 所有字段 SHALL 是 primitive 或 Copy 类型，**禁止**含 `Arc<dyn Trait>` / `Box<dyn Trait>` / 非 Copy 容器

#### Scenario: 业务代码通过 BackendPolicy 字段选择行为

- **WHEN** 业务 IPC handler 需要根据后端类型选择行为
- **THEN** handler SHALL 读 `BackendPolicy` 字段，**不得**新增 `if fs.kind() == Ssh` / `let is_remote = ...` / `matches!(fs.kind(), ...)` 等等价直接派生
- **AND** `fs.kind()` 比对仅允许出现在策略**派生**点（顶层 helper / backend resolver 内部 / fs 抽象 crate 自身实现）

#### Scenario: StaleCheckStrategy 至少含本机时钟与跨时钟域两种策略

- **WHEN** 编译 fs 抽象 crate
- **THEN** `StaleCheckStrategy` SHALL 至少含 `LocalClock5min` 与 `SkipUntilClockSync` 两个 variant
- **AND** SHALL derive Copy + Clone + PartialEq + Eq + Debug
- **AND** 调用方对该字段 match SHALL 通过 exhaustive 检查

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

fs 抽象 crate SHALL 提供 `InstrumentedFs<P>` wrapper（`P` 是 fs provider）+ counter 类型 + counter 入口函数，让业务调用方可在每个 IPC command 边界统计 fs 操作次数（stat / read / read_dir / read_dir_with_metadata / read_to_string / read_lines_head / open_read / stat_many / write_atomic / create_dir_all / remove_file 各计数）。

**注入机制契约**：counter 通过 `InstrumentedFs` wrapper 在 trait 调用边界自动计数，**不**要求每个 provider 实现内嵌 record hook。具体语义：

1. wrapper 实现 fs provider trait，每个 trait 方法内部先 record 当前 counter，再 delegate 到 inner provider
2. 调用方注入 fs handle 时包一层 wrapper；测试 fake provider 同样包 wrapper 即可，不需要修改 fake 内部代码
3. 未包 wrapper 的 fs handle 调 trait 方法不计数（向后兼容）

counter 入口 SHALL 满足：

1. 基于 task-local 实现，避免全局 atomic 让并发 IPC command 互相干扰
2. async wrapper 函数让调用方包住代码块，结束后拿计数
3. wrapper 在 trait 调用边界自动 record，无需 provider 实现配合
4. 与日志 facade 集成——counter Drop 时自动 emit 一条结构化 event，含每种操作的次数

#### Scenario: fs op 在 trait 边界自动计数

- **WHEN** 调用方包 wrapper 后用 counter 入口跑一段含若干 fs op 的代码
- **THEN** 返回的 counter snapshot SHALL 含每种 op 的实际计数
- **AND** provider 实现 SHALL NOT 含任何 counter 调用（计数发生在 wrapper 层）

#### Scenario: 调用方未启用 instrumentation 时不计数

- **WHEN** 调用方直接用 provider（未包 wrapper）+ 调 counter 入口
- **THEN** counter snapshot SHALL 全 0
- **AND** SHALL NOT panic（向后兼容）

#### Scenario: counter 不跨 task 污染

- **WHEN** 两个并发 task 各自调 counter 入口 + 各自的 wrapper
- **THEN** 两 task 的计数 SHALL 互不影响（依赖 task-local 隔离）

#### Scenario: fs op counter 入口结束时输出诊断

- **WHEN** counter 入口闭包正常结束
- **THEN** SHALL emit 一条结构化日志 event 含全部计数字段

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

#### Scenario: open_read 动态分发路径开销不超单态化的 1.3x

- **WHEN** 跑 `cargo bench -p cdt-fs --bench open_read_overhead` 10 次
- **THEN** `fs.open_read` dyn 路径的 median 耗时 SHALL ≤ `tokio::fs::File::open` 直读路径 × 1.3
- **AND** 若超过 1.3x，本 change PR review 拒，需重新评估 D4 决策（关联类型 vs dyn dispatch trade-off）

### Requirement: `FsMetadata` 携带文件创建时间（birthtime）

`FsMetadata` SHALL 额外携带 `created: Option<SystemTime>` 字段。各 provider 实现 SHALL 从底层 `std::fs::Metadata::created()` 获取；返回 `Err` 时（典型：Linux ext2/ext3、部分网络文件系统）SHALL 填 `None`。

caller 需要 epoch 毫秒时 SHALL 调用 `created_ms()` 方法，该方法 SHALL 返回 `min(created, mtime)` 的 epoch 毫秒值（归一化：防止 cp/rsync 等场景下 birthtime > mtime 产生反向区间）。`created = None` 时 fallback 到 `mtime` 值——确保所有平台都能拿到一个有意义的时间戳。

#### Scenario: macOS/Windows 返回真实 birthtime

- **GIVEN** 运行在 macOS 或 Windows 系统上
- **WHEN** stat 一个正常文件
- **THEN** `FsMetadata.created` SHALL 是 `Some(t)`，其中 `t` <= `mtime`

#### Scenario: 不支持 birthtime 的文件系统 fallback

- **GIVEN** 运行在不支持 birthtime 的 Linux 文件系统上
- **WHEN** stat 一个正常文件
- **THEN** `FsMetadata.created` SHALL 是 `None`
- **AND** `created_ms()` SHALL 返回与 `mtime_ms()` 相同的值

#### Scenario: created > mtime 时归一化

- **GIVEN** 文件被 cp/rsync 复制导致 birthtime > mtime
- **WHEN** 调用 `created_ms()`
- **THEN** SHALL 返回 `min(created, mtime)` 的 epoch 毫秒值（不大于 `mtime_ms()`）


## Why

`FileSystemProvider` trait（`crates/cdt-discover/src/fs_provider.rs`）是为了让 local / ssh / http-server 三套 mode 复用同一份业务代码而设计的，但**只在 `ProjectScanner` 内落了地**。Cache 层（`MetadataCache` / `ParsedMessageCache` / `is_file_stale`）和 `cdt-api/src/ipc/local.rs` 30+ 处 IO 全绕过 trait 硬编码 `tokio::fs::*`，调用方反复写 `if fs.kind() == Ssh { ssh_path } else { tokio_fs_path }` 二选一分叉（统计 18 处，最近 PR #186 把分叉数从 9 翻倍到 18，**架构债复利在跑**）。

结果是用户报的 SSH/HTTP 卡顿（5-10s 列表渲染 / 翻页 / 切项目）—— 本地 NVMe + 进程内 IPC 把这些浪费掩盖了，SSH 把延迟放大 100-1000 倍后显形。**本 change 把抽象推到底**：所有 fs 调用走 trait，cache 用 `FsMetadata` 而非 `std::fs::Metadata` 构造签名，三套 mode 共用同一份业务代码——差异只下沉到 provider 实现层。

本 change 是基建（PR-A），**零业务代码变化**；解决 SSH/HTTP 卡顿是 follow-up change（PR-B 切 MetadataCache / PR-C 切 ParsedMessageCache / PR-D 清 30+ 处直调 + 18 处分叉 / PR-E ProjectScanner 结果 in-memory 复用 / PR-F SSH session 锁解开）。

## What Changes

### 新建 crate
- **新建 `cdt-fs` crate**，搬迁 `FileSystemProvider` + `LocalFileSystemProvider` + `FsError` + `FsMetadata` + `FsKind` + `DirEntry` + `EntryKind` 从 `cdt-discover` 到 `cdt-fs`；`cdt-discover` 用 `pub use cdt_fs::*` 一次性兼容老 import，消除 `cdt-api` / `cdt-config` 为拿 fs trait 而 import `cdt-discover` 的虚假依赖

### trait 补 4 缺口
- 加 `async fn open_read(&Path) -> Result<Box<dyn AsyncRead + Send + Unpin>, FsError>`（流式读全文，消除 `SshFileSystemProvider::open_read_stream` 不在 trait 的破抽象）
- 加 `async fn stat_many(&[&Path]) -> Vec<Result<FsMetadata, FsError>>` default 实现走 `join_all`，SSH override 暂用 default（真 SFTP pipeline 留 PR-F）
- `FsMetadata` 加 `identity: Option<FsIdentity>`，Local Unix 填 `Some((dev, ino))` / Windows + SSH 填 `None`，**Best-effort**——文档承认 rename-replace 同 size 同 mtime 边界 case 漏检
- `FsError` 加 `Disconnected` / `TransientExhausted` variant + `is_retryable() -> bool` / `should_invalidate_cache() -> bool` 两个元方法

### 兼容路径
- `FileSignature::from_fs_metadata(&FsMetadata) -> Self` 新构造路径
- 保留 `FileSignature::from_metadata(&std::fs::Metadata)` + `#[deprecated]` 让编译告警引导迁移

### 基础设施
- **`ContextId` 类型**：三元组 `(backend_kind, host_signature, remote_home_or_local_root)`，cache key scope 规范化防跨 host 串扰
- **`xtask check-fs-direct-calls`**：复用 PR #186 `build_time_invariants` 模式禁业务路径直调 `tokio::fs::*` / 业务算法层禁 `fs.kind() == Ssh`，加 CI gate；allowlist single source of truth 住 `crates/cdt-fs/ALLOWLIST.md`
- **Provider instrumentation**：每个 IPC command 统计 fs op 次数 + tracing histogram（H2 可执行性基础设施）
- **H1-H6 六条契约**：落地在 `openspec/specs/fs-abstraction/spec.md` 内对应 Requirement，**不**新增 `.claude/rules/*.md` 散文件（避免每会话自动加载浪费 token，遵守 30 行红线）
- **`BackendPolicy` enum 雏形**：`initial_load_policy: FullEager | SkeletonThenStream` + `max_round_trips_for_initial_page: u8`，定义 + 单测，**不 wire 到业务**（PR-E 才接入）

### 六条硬契约（H1-H6 写进新 capability spec + 各自 enforce 机制）
- **H1**: `cdt-api` / `cdt-config` / 业务路径**禁止**直调 `tokio::fs::*`（allowlist: provider 实现 + `cdt-cli` main + `cdt-watch` notify + 测试）
- **H2**: hot path（list / 翻页 / 详情）**禁止** N 次串行 `fs.stat / read`；SHALL 用 `read_dir_with_metadata` / `stat_many` batched API
- **H3**: 业务**算法**代码 `fs.kind() == Ssh` 默认拒；业务**策略**层（`LocalDataApi`）允许但 SHALL ADR + inline 注释 `// strategy fork: see design.md::Dx`，**收窄到只选 policy 不复制算法**
- **H4**: Tauri / HTTP transport 抽象延后，但本 change 承认 HTTP backend `initial_load_policy: FullEager` + `max_round_trips_for_initial_page: 1`
- **H5**: fs trait **不**承担分页 / 排序语义，按 mtime 拿前 N 走更高层（PR #186 `GroupCursor` 是范例）
- **H6**: `FsError` 必须可操作 —— `is_retryable / should_invalidate_cache` 元方法

### BREAKING
**无运行时 BREAKING**。`cdt_discover::FileSystemProvider` 等老 import 路径通过 `pub use cdt_fs::*` 保留，`#[deprecated]` 告警引导迁移。`FsError` / `FsMetadata` 加字段或 variant 对所有现有 `matches!` 和守卫 match 兼容（已 grep 确认全仓无 exhaustive match）。

## Capabilities

### New Capabilities
- `fs-abstraction`: 承载 `FileSystemProvider` trait 完整契约（4 缺口补齐后的暴露面）+ H1-H6 六条硬约束 + `ContextId` cache key 规范 + `xtask check-fs-direct-calls` CI gate + `BackendPolicy` enum 雏形 + provider instrumentation 接口

### Modified Capabilities
- `project-discovery`: trait 物理位置从 `cdt-discover` 搬到新 `cdt-fs` crate；`FsMetadata` 加 `identity` 字段；新增 `stat_many` batched API；显式声明"fs trait 不承担分页 / 排序"
- `ssh-remote-context`: `SshFileSystemProvider::open_read_stream` inherent 方法升级到 trait `open_read`，消除类型耦合；`FsError::Disconnected` / `TransientExhausted` 替代当前 `with_retry` 耗尽后的 `Io { source: io::Error::other("transient ...") }` 类型擦除

## Impact

### 代码
- **新增 crate** `crates/cdt-fs/`：trait 定义 + LocalFileSystemProvider + FsError + FsMetadata + FsKind + DirEntry + EntryKind + FsIdentity + ContextId + BackendPolicy
- **新增 crate** `xtask/`：`check-fs-direct-calls` 命令（如 workspace 已有 xtask 则复用）
- **修改** `crates/cdt-discover/src/fs_provider.rs` → 改为 `pub use cdt_fs::*` 兼容 re-export + `cdt-discover` 自身依赖切到 `cdt-fs`
- **修改** `crates/cdt-ssh/src/provider.rs` → `open_read_stream` inherent 方法实现 `FileSystemProvider::open_read` trait 方法；retry 耗尽后返 `FsError::TransientExhausted` 而非 `Io::other`
- **修改** `crates/cdt-api/src/cache_signature.rs` → 加 `FileSignature::from_fs_metadata`，老 `from_metadata` 加 `#[deprecated]`
- **修改** workspace `Cargo.toml` / 各 crate `Cargo.toml` → workspace.deps + cdt-discover / cdt-api / cdt-config / cdt-ssh / cdt-cli 加 `cdt-fs` dep；冗余 cdt-discover dep 标记 follow-up 清理（PR-D 时一起做）

### 规则与文档
- 新增 `crates/cdt-fs/ALLOWLIST.md`：crate-local H1 allowlist 数据（xtask + build_time_invariants 共用 parse 入口，single source of truth）
- 新增 `openspec/specs/fs-abstraction/spec.md`（新 capability 主 spec，由 archive 自动 sync；H1-H6 SHALL 句承载在此）
- **不**新增 `.claude/rules/fs-abstraction.md` —— 行为契约住 spec.md（按需 Read），跨域操作纪律不必新增散文件（避免每会话自动加载浪费 token）

### CI
- 新增 `xtask check-fs-direct-calls` 作为 PR check（grep `tokio::fs::*` 在业务 crate 内 + `fs.kind() == Ssh` 在业务算法路径，allowlist 控制）
- workspace 测试不破：所有 cache 单测 + provider 单测仍跑（本 change 不改实现，只加构造路径）

### 性能
- 本 change 零业务变化，**不影响**任何性能 baseline
- 但加的基础设施（instrumentation / BackendPolicy / ContextId）为 PR-B/C/D/E 提供约束 + 可观测性

### 依赖
- 新依赖：无（cdt-fs 用既有 `async-trait` + `tokio` + `thiserror`）
- `cdt-core` 守"sync only + no runtime deps" 红线不破（fs trait **不**进 cdt-core，开新 crate 而非上移）

### Out of scope（确认 follow-up）
- 不改 `MetadataCache` 实现（PR-B）
- 不改 `ParsedMessageCache` 实现（PR-C）
- 不清理 30+ 处 `tokio::fs` 直调 + 18 处 `is_remote` 分叉（PR-D）
- 不引入 `ProjectScanner` 结果 in-memory 复用（PR-E）
- 不解决 SSH `Arc<Mutex<SftpSession>>` 全锁串行（PR-F）
- 不实现 Tauri vs HTTP transport 抽象（更远期）

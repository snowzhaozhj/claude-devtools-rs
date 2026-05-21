## 1. cdt-fs crate 骨架

- [x] 1.1 新建 `crates/cdt-fs/` 目录与 `Cargo.toml`（deps: `tokio` + `async-trait` + `thiserror` + `tracing` + `futures`，**禁止**依赖任何业务 crate）
- [x] 1.2 创建 `crates/cdt-fs/src/lib.rs` + 子模块文件骨架（`provider.rs` / `local.rs` / `error.rs` / `metadata.rs` / `kind.rs` / `dir_entry.rs` / `context_id.rs` / `backend_policy.rs`）
- [x] 1.3 把 workspace 根 `Cargo.toml` 的 `[workspace.dependencies]` 加 `cdt-fs = { path = "crates/cdt-fs", version = "0.0.0" }`，并把 `cdt-fs` 加到 `[workspace] members` 列表
- [x] 1.4 设 `crates/cdt-fs/Cargo.toml` 内 `publish = false`，对齐仓库其他 crate

## 2. 搬迁 + 扩展类型定义到 cdt-fs

- [x] 2.1 把 `FsKind` enum 从 `cdt-discover/src/fs_provider.rs` 搬到 `cdt-fs/src/kind.rs`
- [x] 2.2 把 `EntryKind` + `DirEntry` 搬到 `cdt-fs/src/dir_entry.rs`
- [x] 2.3 把 `FsMetadata` 搬到 `cdt-fs/src/metadata.rs`，**新增** `identity: Option<FsIdentity>` 字段；同时定义 `FsIdentity` enum（`Unix { dev: u64, ino: u64 }` cfg(unix) variant + 跨平台共享的退化 variant）
- [x] 2.4 把 `FsError` 从 `cdt-discover/src/error.rs` 搬到 `cdt-fs/src/error.rs`，**新增** `Disconnected { path, reason }` / `TransientExhausted { path, attempts, last_reason }` 两个 variant；保留原有 `NotFound` / `Io` / `Utf8` / `Unsupported`
- [x] 2.5 给 `FsError` 加 inherent 方法 `is_retryable(&self) -> bool` + `should_invalidate_cache(&self) -> bool`，逐 variant 实现 + 单测覆盖（NotFound 不重试清 cache / Disconnected 重试不清 / TransientExhausted 不重试不清 / Io 视 source kind / Utf8 不重试清 / Unsupported 不重试不清）
- [x] 2.6 把 `FileSystemProvider` trait 从 `cdt-discover/src/fs_provider.rs` 搬到 `cdt-fs/src/provider.rs`；**新增** `async fn open_read(&Path) -> Result<Box<dyn AsyncRead + Send + Unpin>, FsError>` 抽象方法
- [x] 2.7 给 trait 新增 `async fn stat_many(&self, paths: &[&Path]) -> Vec<Result<FsMetadata, FsError>>` default 实现（走 `futures::future::join_all` 包装 `stat`）
- [x] 2.8 把 `LocalFileSystemProvider` 从 `cdt-discover/src/fs_provider.rs` 搬到 `cdt-fs/src/local.rs`；实现新的 `open_read`（用 `tokio::fs::File::open` + `Box::new`）；不 override `stat_many`（default 即真并发）；`stat` 实现填 `FsMetadata.identity = Some(FsIdentity::Unix { dev, ino })` on `cfg(unix)`，cfg(not(unix)) 填 `None`
- [x] 2.9 在 `cdt-fs/src/lib.rs` `pub use` 所有公开类型

## 3. ContextId + HostSignature 类型（**先于 task 4 完成**）

> 调整说明（codex 第二轮 Medium #10 + 第三轮 Blocking A）：`ContextId` / `HostSignature` / `BackendPolicy` 必须先于 cdt-discover re-export 定义完成；input 类型 `SshConfigDigestInput` 在 cdt-fs 自定义最小形状，**不**复制 cdt-ssh `ResolvedHost`（避免反向依赖）；cdt-ssh `ResolvedHost` 扩字段 task 放到 task 7（cdt-ssh 子工作内）做。

- [x] 3.1 在 `cdt-fs/src/context_id.rs` 定义 `HostSignature { config_digest: [u8; 32], display_label: String }` —— 实现 `Hash + Eq + Clone + Debug + PartialEq`，**只**用 `config_digest` 字段参与 Hash/Eq，`display_label` 不参与（codex Medium 验收点：display_label 改变但 digest 相等仍判等）
- [x] 3.2 在 `cdt-fs/src/context_id.rs` 定义 `ContextId { backend_kind: FsKind, host_signature: Option<HostSignature>, root_or_home: PathBuf }` + `Hash + Eq + Clone + Debug + PartialEq` derives
- [x] 3.3 提供构造器 `ContextId::local(claude_root: PathBuf) -> Self`（`host_signature: None`）+ `ContextId::ssh(host_sig: HostSignature, remote_home: PathBuf) -> Self`
- [x] 3.4 定义 `pub struct SshConfigDigestInput { hostname: String, port: u16, user: String, identity_files: Vec<PathBuf>, proxyjump: Option<String>, proxycommand: Option<String>, hostkeyalias: Option<String> }` —— 作为 `HostSignature::from_ssh_config_fields` 入参形状。**不**复制 cdt-ssh `ResolvedHost` 类型（避免 cdt-fs 反向依赖 cdt-ssh）；cdt-ssh 通过 `impl From<&ResolvedHost> for SshConfigDigestInput` 转换（在 task 7 内做）
- [x] 3.5 实现 `HostSignature::from_ssh_config_fields(input: &SshConfigDigestInput) -> Self` —— 内部先对 `identity_files` 字典序排序（防 input 顺序影响 hash），SHA-256 字段按 `\0` 分隔拼接：`hostname` `\0` `port` `\0` `user` `\0` `identity_files_sorted_joined` `\0` `proxyjump.unwrap_or("")` `\0` `proxycommand.unwrap_or("")` `\0` `hostkeyalias.unwrap_or("")`；`display_label = format!("{user}@{hostname}:{port}")`
- [x] 3.6 单测覆盖 spec scenarios：(a) 不同 backend_kind 同 path → `!=`；(b) 同 user@host:port 不同 ProxyJump → digest 不同；(c) 同 user@host:port + ProxyJump 不同 IdentityFile → digest 不同；(d) 仅 connecttimeout / loglevel / compression 不同 → digest 相同（**注**：这些字段不在 `SshConfigDigestInput`，自然不参与）；(e) display_label 不参与 Hash/Eq；(f) `from_ssh_config_fields` 内部排序 identity_files，input 顺序无关；(g) Hash + Eq + Clone 一致性；(h) degraded 模式 `proxyjump = proxycommand = hostkeyalias = None` 仍能成功产 digest
- [x] 3.7 `pub use` 到 cdt-fs lib 含 `HostSignature` / `ContextId` / `SshConfigDigestInput`

## 4. BackendPolicy enum 雏形（**先于 task 5 完成**）

- [x] 4.1 在 `cdt-fs/src/backend_policy.rs` 定义 `BackendPolicy { initial_load_policy, max_round_trips_for_initial_page, supports_incremental_updates, prefetch_policy }` struct + `InitialLoadPolicy { FullEager, SkeletonThenStream }` enum + `PrefetchPolicy { None, PrefetchNext }` enum（D8 codex Low #8 要求正交字段预留）
- [x] 4.2 实现 `Debug + Clone + PartialEq + Eq` derives
- [x] 4.3 提供 const-like 构造器：`BackendPolicy::for_local() / for_ssh() / for_http()`。本 change 所有构造器返回 `prefetch_policy: PrefetchPolicy::None`（PR-E 才可能改）
- [x] 4.4 单测覆盖 spec scenarios：for_local → SkeletonThenStream + None prefetch / for_ssh + for_http → FullEager + None prefetch；构造器返回值 deterministic；`PrefetchPolicy` 与 `InitialLoadPolicy` 正交（验收：`BackendPolicy { initial_load_policy: SkeletonThenStream, prefetch_policy: PrefetchNext, .. }` 也能构造）
- [x] 4.5 grep 验证 `BackendPolicy` 在 cdt-api / cdt-config 业务路径**零消费**（仅 cdt-fs 内部 + 测试）
- [x] 4.6 `pub use` 到 cdt-fs lib

## 5. cdt-discover 转为 re-export

- [x] 5.1 删除 `cdt-discover/src/fs_provider.rs` 内已搬走的 trait + Local impl + 类型定义（保留文件作为占位 + `pub use cdt_fs::*` 行）
- [x] 5.2 删除 `cdt-discover/src/error.rs` 内已搬走的 `FsError` 定义（保留 `DiscoverError` enum，其 `Fs(FsError)` variant 改为 `Fs(cdt_fs::FsError)`）
- [x] 5.3 `cdt-discover/src/lib.rs` 加 `pub use cdt_fs::{FileSystemProvider, LocalFileSystemProvider, FsKind, FsMetadata, FsIdentity, DirEntry, EntryKind, FsError, FsHandle, local_handle, ContextId, HostSignature, SshConfigDigestInput, BackendPolicy, InitialLoadPolicy, PrefetchPolicy, InstrumentedFs, FsOpCounter, with_fs_counter};` 兼容老 import 路径

> 前置约束：本 task 引用的所有类型在 task 2（搬迁 + 扩展）/ task 3（ContextId + HostSignature）/ task 4（BackendPolicy）/ task 9（InstrumentedFs / FsOpCounter / with_fs_counter）内必须已定义并 `pub use` 到 `cdt-fs/src/lib.rs`。本 task 5 必须在以上任务之后执行（task 顺序：2 → 3 → 4 → 5 → 6 → 7 → 8 → 8b → 9 → ...）。`FsHandle` / `local_handle` 在 task 2.8 搬迁 LocalFileSystemProvider 时一起搬，task 2.9 `pub use` 已包含；`InstrumentedFs` 等 instrumentation 类型由 task 9 定义后再补 `pub use`——本 task 5 可分两次提交：5.3a 先 re-export 已就位类型（task 2-4 产物）；5.3b 在 task 9 完成后补 instrumentation 类型 re-export。
- [x] 5.4 `cdt-discover/Cargo.toml` 加 `cdt-fs = { workspace = true }` dep
- [x] 5.5 `cargo check -p cdt-discover` 验证 re-export 链通畅

## 5b. 业务 crate Cargo.toml 加 cdt-fs dep（codex H2 补齐）

> 补 codex 第二轮 Blocking #2：spec 要求 cdt-api / cdt-config / cdt-cli / cdt-watch 都 SHALL 直接依赖 cdt-fs，但第一稿 tasks 只列了 cdt-ssh。

- [x] 5b.1 `crates/cdt-api/Cargo.toml` 加 `cdt-fs = { workspace = true }` dep（保留 cdt-discover dep 作为过渡——内部仍有调用，但 fs 抽象 import 切到 cdt-fs；本 change 不强制全部 use 路径迁移，PR-D 时另开 cleanup PR）
- [x] 5b.2 `crates/cdt-config/Cargo.toml` 加 `cdt-fs = { workspace = true }`（即便本 change 不接入业务，未来 PR-B/C/D 切 trait 时已就位）
- [x] 5b.3 `crates/cdt-cli/Cargo.toml` 加 `cdt-fs = { workspace = true }`
- [x] 5b.4 `crates/cdt-watch/Cargo.toml` 加 `cdt-fs = { workspace = true }`
- [x] 5b.5 `cargo check --workspace` 全过，验证依赖图无 cycle / 无遗漏
- [x] 5b.6 grep 验证业务 crate（cdt-api / cdt-config / cdt-cli / cdt-watch）的 `Cargo.toml` 中 `cdt-fs` 已出现；deprecated 提示 + use 路径迁移留 PR-D 一并处理

## 6. FileSignature 兼容路径

- [x] 6.1 在 `crates/cdt-api/src/cache_signature.rs` 新增 `FileSignature::from_fs_metadata(meta: &cdt_fs::FsMetadata) -> Self`——从 `FsMetadata.size` + `FsMetadata.mtime` + `FsMetadata.identity` 构造
- [x] 6.2 把现有 `FileSignature::from_metadata(&std::fs::Metadata)` 加 `#[deprecated(note = "请改用 FileSignature::from_fs_metadata，本路径将随 PR-D 移除")]`
- [x] 6.3 调整 `FileIdentity` 与 `cdt_fs::FsIdentity` 的桥接——`FileSignature::from_fs_metadata` 若 `FsMetadata.identity == Some(FsIdentity::Unix { dev, ino })` 则 `FileSignature.identity` 填 `FileIdentity::Unix { dev, ino }`，否则填 `None`
- [x] 6.4 单测：相同 `FsMetadata` 与对应 `std::fs::Metadata` 构造出的 `FileSignature` `==`（best-effort identity 维度容差）

## 7. SshFileSystemProvider 适配 + `ResolvedHost` 扩字段

> codex 第三轮 Blocking A + High E：`ResolvedHost` 必须扩字段以提供 `proxyjump / proxycommand / hostkeyalias` 给 `HostSignature::from_ssh_config_fields`，否则 host_signature 永久退化模式。

- [x] 7.1 `cdt-ssh/Cargo.toml` 加 `cdt-fs = { workspace = true }` dep（保留 cdt-discover dep 作为过渡，但本次起 import 改 cdt-fs）
- [x] 7.2 `crates/cdt-ssh/src/provider.rs` import 路径切换：`use cdt_discover::{...}` → `use cdt_fs::{...}`（保留 cdt-discover 兼容路径仍可用作为安全网）
- [x] 7.3 实现 trait `open_read`：包装 `russh_sftp::client::fs::File`（即现有 `open_read_stream` inherent 方法的内容）成 `Box<dyn AsyncRead + Send + Unpin>`；旧 inherent `open_read_stream` 保留为 `#[deprecated]` 兼容路径
- [x] 7.4 `stat_many` **不** override（使用 trait default `join_all`）；在 trait impl 旁加注释明确"因 `Arc<Mutex<SftpSession>>` 全锁串行，当前是假 batch，留 PR-F 解决"
- [x] 7.5 `SshFileSystemProvider::stat` 返回的 `FsMetadata` 填 `identity: None`（SSH 永远 best-effort）
- [x] 7.6 调整 retry 耗尽路径：当前 `map_client_error` 把 `SftpClientError::Transient` 投影到 `FsError::Io { source: io::Error::other("transient sftp error: ...") }`——改为返 `FsError::TransientExhausted { path, attempts: 3, last_reason }`
- [x] 7.7 新增 session disconnect 检测：当操作时发现 session 已 disconnect → 返 `FsError::Disconnected { path, reason }`（具体检测点 design.md D6 / 实施时按 russh-sftp API 现状决定）
- [x] 7.8 修单测：`read_to_string_gives_up_after_max_transient` 改为断言 `FsError::TransientExhausted` 而非 `FsError::Io`；同步更新 `open_read_stream_unsupported_in_fake_path` 测试（如改名为 `open_read_*`）
- [x] 7.9 **扩 `ResolvedHost` 字段**：在 `cdt-ssh/src/host_resolver.rs::ResolvedHost` 加 `proxyjump: Option<String>` / `proxycommand: Option<String>` / `hostkeyalias: Option<String>` 三个字段
- [x] 7.10 **更新 `ssh -G` 解析逻辑**：`host_resolver.rs` 内 `ssh -G` 输出解析时提取 `proxyjump` / `proxycommand` / `hostkeyalias` 行填入 `ResolvedHost`；退化路径（`config_parser` 兜底）SHALL 设这三个字段为 `None`
- [x] 7.11 **实现 From conversion**：在 `cdt-ssh/src/host_resolver.rs` 或新文件加 `impl From<&ResolvedHost> for cdt_fs::SshConfigDigestInput`，把 `ResolvedHost` 字段映射到 `SshConfigDigestInput`（`identity_files: resolved.identity_files.clone()`，三个新字段直接 clone）
- [x] 7.12 单测覆盖：`ResolvedHost` 三个新字段默认 `None`；degraded 模式下 `From` 转换不 panic；同 `user@host:port` 不同 `ProxyJump` 时通过 conversion + `HostSignature::from_ssh_config_fields` 产生不同 digest
- [x] 7.13 `cargo test -p cdt-ssh` 全过

## 8. xtask check-fs-direct-calls

> codex 第二轮 Medium #7：xtask 与 `build_time_invariants` 集成测试是不同机制并存，allowlist 必须单源（住在 `.claude/rules/fs-abstraction.md`）。

- [x] 8.1 调研：workspace 是否已有 `xtask` crate？若有，加 subcommand；若无，新建 `crates/xtask/` minimal binary crate（不进 publish 列表 + 不进 workspace `default-members`）
- [x] 8.2 实现 `check-fs-direct-calls` 命令：grep 业务 crate 内 `tokio::fs::(metadata|read|read_to_string|read_dir|File::open|write|create_dir_all|remove)` 模式
- [x] 8.3 allowlist **单源住在 `.claude/rules/fs-abstraction.md`** 的 "Allowlist" 段（markdown table 格式：`| crate/path | reason |`），xtask 启动时 parse 规则文件，**不**在 xtask 源码硬编码 allowlist
- [x] 8.4 `crates/cdt-api/tests/build_time_invariants.rs`（PR #186 留下的）若 future 需要也接 `tokio::fs::*` 类检查，**也**从同一 `.claude/rules/fs-abstraction.md` 读 allowlist，保证两套机制单源；本 change 不动 build_time_invariants（仅文档化它读规则文件作为 future contract）
- [x] 8.5 支持 `--warn-only` flag：命中时 warning + exit 0（本 change 期间默认开启）；不带 flag 时 exit 1
- [x] 8.6 加 CI 配置：`.github/workflows/check.yml` 或等价位置加 step `cargo xtask check-fs-direct-calls --warn-only`
- [x] 8.7 自测：手动跑 `cargo xtask check-fs-direct-calls`，确认输出 ~34 处现有违反作为 warning，CI 不 fail
- [x] 8.8 tasks 末尾标记 follow-up："PR-D 完成后另开 PR 把 `--warn-only` 切换为 fail-on-match"

## 8b. H5 fs trait 不暴露分页参数（自动化测试）

> codex 第二轮 High #9 + 第三轮 Medium F：必须用 syn parse trait 方法签名，**不**用字符串 grep（避免注释 / 文档中出现 `Cursor` `Offset` 字面量被误伤）。

- [x] 8b.1 在 `crates/cdt-fs/tests/no_pagination_in_trait.rs` 新建集成测试，用 **`syn` crate** parse `crates/cdt-fs/src/provider.rs` 源码（`include_str!` 后 `syn::parse_str::<syn::File>`），遍历 `trait FileSystemProvider` 的所有方法 signature，检查每个参数 `syn::FnArg` 的 type ident 是否含 `Cursor` / `Offset` / `Limit` / `SortBy` / `Order` —— 命中即测试 fail
- [x] 8b.2 测试 fail 时 panic 消息明确指向 H5 + design D2 决策依据，含具体方法名 + 违规参数名
- [x] 8b.3 测试 SHALL NOT 误伤注释 / doc-comment 中出现的 `Cursor` 等字面量（syn AST 解析天然只看代码不看注释）；加 negative test：在 trait doc-comment 加 "// example: don't add Cursor parameter" 验证测试仍 pass
- [x] 8b.4 `syn` dep 加到 `crates/cdt-fs/Cargo.toml` 的 `[dev-dependencies]`，**不**进生产 deps
- [x] 8b.5 `cargo test -p cdt-fs --test no_pagination_in_trait` 单跑通过

## 9. Provider instrumentation 入口 + InstrumentedFs wrapper

> codex 第三轮 Medium G：注入机制钉死为 `InstrumentedFs<P>` wrapper 在 trait 调用边界自动计数，**不**要求 provider impl 显式 record。

- [x] 9.1 在 `cdt-fs/src/provider.rs` 旁新增 `cdt-fs/src/instrumentation.rs` 模块
- [x] 9.2 定义 `FsOpCounts { stat: u32, read_to_string: u32, read_dir: u32, read_dir_with_metadata: u32, read_lines_head: u32, open_read: u32, stat_many: u32 }` 计数 struct + `FsOpCounter` 持有 `Arc<Mutex<FsOpCounts>>` 引用
- [x] 9.3 用 `tokio::task_local!` 定义 `pub(crate) static CURRENT_FS_COUNTER: FsOpCounter`；提供 `FsOpCounter::current() -> Option<FsOpCounter>`（任务上下文外返 None）
- [x] 9.4 实现 `InstrumentedFs<P> { inner: P }`，`P: FileSystemProvider`；`InstrumentedFs<P>` 实现 `FileSystemProvider`，每个 trait 方法内部先 `if let Some(c) = FsOpCounter::current() { c.record_<op>() }`，再 `self.inner.<op>(..).await`
- [x] 9.5 `local_handle().instrumented() -> Arc<InstrumentedFs<LocalFileSystemProvider>>` 扩展方法（trait extension 或 free function）
- [x] 9.6 实现 `with_fs_counter<F, Fut>(f: F) -> (Fut::Output, FsOpCounts)` async wrapper：内部 `CURRENT_FS_COUNTER.scope(counter, f).await`，结束后从 counter 取 FsOpCounts return
- [x] 9.7 接入 tracing：`FsOpCounter` Drop 时 emit `tracing::info!(target = "cdt_fs::ops", stat_count, read_count, ...)`
- [x] 9.8 单测覆盖 spec scenarios：(a) `InstrumentedFs(LocalFileSystemProvider)` 包裹后调 N 次 stat counter 准确；(b) 未包裹的 LocalFileSystemProvider 在 `with_fs_counter` 内调 stat 不计数（向后兼容）；(c) 两个并发 task 各自 `with_fs_counter` 不污染；(d) tracing emit on Drop（用 `tracing-test` crate 或 `tracing_subscriber::test` 捕获 event）
- [x] 9.9 grep 验证 `with_fs_counter` / `InstrumentedFs` 在 cdt-api 业务路径**零消费**（仅 cdt-fs 内部 + 测试）——PR-B/C/D 才接入

## 10. 规则文件 + CLAUDE.md 更新

> codex 第二轮 Medium #7：allowlist 单源住在规则文件；codex 第二轮 High #9：每条 H 必须标 enforce 方式。

- [x] 10.1 新建 `.claude/rules/fs-abstraction.md`，落 H1-H6 六条契约：每条标题 + 约束描述 + 至少一个违反示例 + 修法 + **Enforce 机制段**（H1 → xtask；H2 → instrumentation + 集成测试 + review；H3 → review + D6 分类表；H4 → BackendPolicy 单测；H5 → no_pagination_in_trait 测试；H6 → FsError 元方法单测）
- [x] 10.2 H1 段落含"Allowlist" markdown table，列每个允许的路径 + reason；xtask + build_time_invariants 都 SHALL parse 此 table（**single source of truth**）
- [x] 10.3 H2 段落含具体反模式（`for path in paths { fs.stat(path).await? }`）与修法（`fs.stat_many(&paths).await`）；附 instrumentation 接入样例 `with_fs_counter(async { api.list_sessions(...).await }).await`
- [x] 10.4 H3 段落含"算法分叉 vs 策略分叉"区分判据 + 具体反例（"if Ssh { sort by mtime } else { sort by size }" = 算法分叉拒；"if Ssh { initial_load_policy: FullEager }" = 策略分叉允许）+ 链接到本 change design.md D6 的 23 处分叉初步分类表
- [x] 10.5 H4 段落含 `BackendPolicy::for_local() / for_ssh() / for_http()` 三种默认值 table + transport 抽象延后的 anchor
- [x] 10.6 H5 段落引用 PR #186 `GroupCursor` 作为"高层分页正确实现位置"范例 + 链接 `no_pagination_in_trait.rs` 自动化测试
- [x] 10.7 H6 段落含 `FsError.is_retryable()` / `should_invalidate_cache()` 使用样例（cache 写入时检查 `should_invalidate_cache`；重试 backoff 用 `is_retryable` 守卫）
- [x] 10.8 修改 `CLAUDE.md` "按域去哪查" 段或等价位置加链接到 `.claude/rules/fs-abstraction.md`
- [x] 10.9 修改 `CLAUDE.md` 的 "跨域规则散文件" 表格加一行 `.claude/rules/fs-abstraction.md` + 何时读列：「任何 fs 调用 / cache 改动 / SSH / HTTP server mode 相关改动 SHALL 读」

## 11. 编译 + 测试 + 性能验证

- [x] 11.1 `cargo build --workspace` 全过
- [x] 11.2 `cargo clippy --workspace --all-targets -- -D warnings` 全过
- [x] 11.3 `cargo fmt --all`
- [x] 11.4 `cargo test --workspace` 全过（含 cdt-fs 新单测 + cdt-ssh 适配后的回归测试）
- [x] 11.5 `cargo test -p cdt-fs` 单独跑确认 cdt-fs 测试覆盖率达标
- [x] 11.6 `openspec validate unify-fs-abstraction --strict` 过
- [ ] 11.7 性能回归校验（**codex 第二轮 Medium #11 收严**）：本 change 零业务变化，但仍跑 `cargo test --release -p cdt-api --test perf_cold_scan -- --ignored --nocapture` 与 `cargo test --release -p cdt-api --test perf_get_session_detail -- --ignored --nocapture`，apply **前后各跑 5 次** 取 min / median / stddev。回归判据：median 退化 > 5% 拒 / min 退化 > 8% 拒 / stddev > 8ms 拒（不稳定性引入）。PR 描述贴 4 维数据
- [x] 11.8 `pnpm --dir ui run check` 全过（如改动 ui 文件则跑）
- [x] 11.9 build-time grep 拦回归测试通过（确认 `cdt-fs` 内允许 `tokio::fs::*`，业务路径仍按现状——本 change 不清理，PR-D 清理）
- [x] 11.10 **Local micro benchmark**（codex 第二轮 Medium #4 / D4 量化要求）：新建 `crates/cdt-fs/benches/open_read_overhead.rs`，对比同 jsonl 文件（fixture：~500KB 小会话 + ~5MB 大会话）走 `tokio::fs::File::open + BufReader::lines` 直读路径 vs 走 `FileSystemProvider::open_read` dyn 路径，跑 10 次取 min / median / stddev。dyn 路径 SHALL 在 median 上 ≤ 直读路径 × 1.3，否则拒
- [x] 11.11 在 `crates/cdt-fs/tests/no_pagination_in_trait.rs` 跑通（H5 自动化 enforce）
- [ ] 11.12 `cargo bench -p cdt-fs --bench open_read_overhead` 跑通且数据落 PR 描述

## N. 发布

- [ ] N.1 push 分支 + 开 PR（标题 `feat(fs): unify FileSystemProvider abstraction in cdt-fs crate`，PR 描述含 perf impact 表格 + design.md 链接）
- [ ] N.2 wait-ci 全绿（用 `/wait-ci <pr>` skill；CI 必须含新 `cargo xtask check-fs-direct-calls --warn-only` step）
- [ ] N.3 codex 二审通过（按 `.claude/rules/codex-usage.md` 第 3 节 design 阶段已审一次，本 change push 后 PR commit 阶段再审一次）；如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

## 后续 follow-up（不在本 change，但提醒勿漏）

- [ ] FU-1 PR-B：`MetadataCache` 切 `&dyn FileSystemProvider`，cache key 加 `ContextId` 前缀（解 SSH 列表卡顿核心）
- [ ] FU-2 PR-C：`ParsedMessageCache` 同样改造
- [ ] FU-3 PR-D：清 18 处 `is_remote` 分叉 + 30+ 处 `tokio::fs` 直调；xtask 切 fail-on-match
- [ ] FU-4 PR-E：`ProjectScanner` 结果在 `LocalDataApi` 内 in-memory 复用 + `BackendPolicy` wire 到业务
- [ ] FU-5 PR-F：SSH `Arc<Mutex<SftpSession>>` 锁解开，`stat_many` 真 SFTP pipeline

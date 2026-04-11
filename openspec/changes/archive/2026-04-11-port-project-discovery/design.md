## Context

TS 侧 `src/main/services/discovery/` 由 ~10 个模块组成，总代码量约 2 k 行。核心职责可以拆成三层：

1. **I/O 原语层** —— `FileSystemProvider` 接口 + `LocalFileSystemProvider` 实现（目前 TS 放在 `services/infrastructure/`，不在 discovery 里），这一层决定"本地 vs SSH"的可替换性。
2. **纯数据转换层** —— `pathDecoder.ts` / `SubprojectRegistry.ts` / `WorktreeGrouper.ts` 的"把字符串/结构转成另一个结构"部分，本质都是函数，不触 I/O。
3. **组合层** —— `ProjectScanner.ts` / `ProjectPathResolver.ts` 把前两层粘合起来并 cache，是 IPC / HTTP 入口最终调用的 facade。

当前 Rust workspace 已经完成 `cdt-core` / `cdt-parse` / `cdt-analyze`，且 `cdt-discover` crate 已经作为空壳存在。本次 port 要在 `cdt-discover` 里把上述三层按 Rust 习惯组织起来，同时**刻意收窄范围**：只覆盖 `project-discovery` spec 里的 5 条 Requirement，`SessionSearcher` / `SessionContentFilter` / `SearchTextCache` / `SubagentLocator` 这四个和 search / subagent 耦合的模块全部留给 `port-session-search` 与 `port-team-coordination-metadata` 再做。

约束：
- `cdt-core` 必须保持同步、零 runtime 依赖（`.claude/rules/rust.md`）。所以 `FileSystemProvider` trait 只能放在 `cdt-discover`。
- 所有 async fn 必须跑在 tokio 上；本次 port 是 workspace 里第 2 个引入 tokio 的 crate（第 1 个是 `cdt-parse`）。
- `#![forbid(unsafe_code)]` workspace-wide，clippy pedantic。

## Goals / Non-Goals

**Goals:**
- 让 `cdt_discover::ProjectScanner::scan()` 返回与 TS `scan()` 行为一致的 `Vec<Project>`，spec 的 5 条 Requirement 全部通过 scenario-level 单测覆盖。
- 定义稳定的 `FileSystemProvider` trait，使后续 `cdt-ssh` 只需要提供一个 `SshFileSystemProvider` 实现即可接入同一套 scanner（不改 scanner 代码）。
- 冻结 `subproject_registry` 的 composite ID 格式 (`{baseDir}::{hash8(cwd)}`) 并写进 spec delta。
- 单测全部走 `tempfile` + `LocalFileSystemProvider`，不引入 mock/stub 框架。
- 保持 `cdt-analyze` / `cdt-parse` 零改动。

**Non-Goals:**
- 不做 session 搜索（`SessionSearcher` / 全文扫描 / cache）——留给 `port-session-search`。
- 不做 subagent 定位（`SubagentLocator`）——留给 `port-team-coordination-metadata`。
- 不做 worktree source 检测（TS 里 `gitIdentityResolver.detectWorktreeSource`，用于 badge 显示）——这是 UI 装饰，跟 spec 的 Requirement 无关。本次只实现"同 repo 的 worktree 放一组"的核心分组，`isMainWorktree` 用"`.git` 是 dir 而不是 file"判断。
- 不做 SSH provider 实现。
- 不做 "pinned session 持久化"，`SubprojectRegistry` / 注册表仅在内存里存活，pinned 状态的持久化由 `port-configuration-management` 接手。本次的 spec 里我们只保证"pinned 状态若被外部注入，会在列表里反映出来"这一被动契约。
- 不做 IPC / HTTP 暴露。

## Decisions

### 决策 1：`FileSystemProvider` trait 的位置与形态

放在 `cdt-discover::fs_provider`（而不是 `cdt-core`），因为 `cdt-core` 禁止引入 runtime 依赖，而该 trait 的方法天然是 async。trait 形如：

```rust
#[async_trait::async_trait]
pub trait FileSystemProvider: Send + Sync + 'static {
    fn kind(&self) -> FsKind;                       // Local | Ssh
    async fn exists(&self, path: &Path) -> bool;
    async fn read_dir(&self, path: &Path) -> Result<Vec<DirEntry>, FsError>;
    async fn read_to_string(&self, path: &Path) -> Result<String, FsError>;
    async fn stat(&self, path: &Path) -> Result<FsMetadata, FsError>;
    async fn read_lines_head(&self, path: &Path, max: usize) -> Result<Vec<String>, FsError>;
}
```

- `DirEntry { name: String, kind: EntryKind }`，`EntryKind { File, Dir, Symlink, Other }` —— 不暴露 `std::fs::DirEntry`。
- `FsMetadata { size: u64, mtime: SystemTime }` —— 只保留 scanner 真正需要的字段。
- `FsError` 用 `thiserror::Error`，本 crate 的错误枚举。
- `read_lines_head` 专门给 `ProjectPathResolver` / `SessionContentFilter` 用：SSH 模式下不需要把整个 JSONL 拉下来，只读前 N 行就能拿到 `cwd`。TS 里没抽这个方法，是个隐性 bug（SSH 性能差）——本次 port 顺便修正。

**替代方案**：把 trait 放在 `cdt-core`，然后用 `feature = "async"` gate 掉 tokio 依赖。**拒绝**，因为 `cdt-core` 的"无 runtime"约束是写进 `.claude/rules/rust.md` 的硬红线，引入 feature gate 会让其他 crate 的 use-path 变复杂。

**替代方案 B**：trait 方法返回 `Pin<Box<dyn Future>>` 而不是用 `async_trait`。**拒绝**，pedantic-clippy 下 `async_trait` 的 boxed future 开销可以接受，而且 Rust 稳定版 2024 对 trait async fn 的 `dyn` 支持仍然不完整。

### 决策 2：`cdt-core` 里放哪些类型

只把"多 crate 共享"的类型上移到 `cdt-core::project`：`Project`、`Session`、`Worktree`、`RepositoryGroup`、`RepositoryIdentity`。这些后面 `cdt-api` 的 facade 也会用。

`Project` 的字段严格跟 TS `Project` 对齐：

```rust
pub struct Project {
    pub id: String,               // 编码目录名或 composite ID
    pub name: String,             // 展示名
    pub path: PathBuf,            // 解码后的 cwd
    pub sessions: Vec<String>,    // session id 列表（仅 id，不含 metadata）
    pub most_recent_session: Option<i64>, // epoch ms
    pub created_at: Option<i64>,
}
```

`cdt-discover` 内部用的中间类型（如 `ResolvedPathSource { Cache, Hint, CwdField, Decoded }` 这种枚举）不上移，只在本 crate 用。

### 决策 3：`path_decoder` 的边缘行为

TS `pathDecoder.ts` 的 `decodePath` 是"leading `-` → `/`"的 best-effort 替换；`extractBaseDir` 把 composite `foo::abcd1234` 还原成 `foo`；`extractProjectName` 从 path 取最后一段。这三个都是纯函数，直接翻译成 Rust 模块函数（不做 builder 模式）。

关键行为确认：
- Windows / WSL 路径：spec 要求 `/mnt/c/...` 原样返回。`decode_path` 对 `/mnt/` 前缀不做特殊处理——因为 TS 原版也不处理，平台语义交给调用方。
- 对于歧义 (`-Users-alice-my-app`)：`decode_path` 返回 best-effort（`/Users/alice/my/app`），真正的消歧由 `ProjectPathResolver` 读 JSONL 里的 `cwd` 完成。这跟 spec scenario"Path containing legitimate hyphens"一致。

### 决策 4：`SubprojectRegistry` 的线程模型

TS 版是 module-level singleton + 同步 map。Rust 换成 `ProjectScanner` 内部持有的 `Arc<Mutex<SubprojectRegistry>>` —— 不做 global，不做 `OnceCell`，因为：

1. 测试友好：每个测试 case 自己 new 一个 scanner 就有独立 registry，不会串状态。
2. 未来多 scanner 并存（本地 + SSH 同时挂载）时不会抢同一个 global。
3. `ProjectPathResolver` 与 `WorktreeGrouper` 通过构造注入同一个 `Arc<Mutex<_>>`，或者更简单：`ProjectScanner` 自己收束"清 registry → 扫描 → 分组"的整个流程，`Resolver` 和 `Grouper` 是 scanner 内部的私有字段。

**决策**：registry 设为 scanner 的 private field，不把它暴露出 `pub`；对外通过 scanner 方法间接操作。好处是避免外部代码直接改 registry 造成状态漂移。

### 决策 5：`WorktreeGrouper` 的 git 调用策略

TS 侧 `gitIdentityResolver` 通过 `child_process.exec('git ...')` 触发真实 git 二进制。Rust 同样用 `tokio::process::Command`：

- `git -C <path> rev-parse --git-common-dir` —— 拿 repo 身份（同一个 common dir 属于同一 repo）
- `git -C <path> rev-parse --abbrev-ref HEAD` —— 拿当前 branch
- `git -C <path> rev-parse --git-dir` —— 比较是否 `.git` 本身（主 worktree vs 附加 worktree）

所有 git 调用走 `FileSystemProvider::run_git(path, args) -> Result<String, FsError>`？**不**，因为 git 子进程在 SSH 模式下需要远端执行而不是本地——这是个抽象漏洞。**本次决策**：`WorktreeGrouper` 接受一个独立的 `GitIdentityResolver` trait 对象，`LocalGitIdentityResolver` 实现用本地 `Command`。SSH 版本留给 `port-ssh-remote-context`。`GitIdentityResolver` trait 只有 3 个方法（`resolve_identity` / `get_branch` / `is_main_worktree`），签名最小化。

**替代方案**：不做 trait，直接把 git 路径硬编码进 grouper。**拒绝**，SSH port 时要重写整个 grouper，违反 Open/Closed。

### 决策 6：错误处理

`cdt-discover` 定义 `DiscoverError`：

```rust
#[derive(Debug, thiserror::Error)]
pub enum DiscoverError {
    #[error("filesystem error: {0}")]
    Fs(#[from] FsError),
    #[error("git command failed: {0}")]
    Git(String),
    #[error("parse error: {0}")]
    Parse(#[from] cdt_parse::ParseError),
}
```

`scan()` 签名是 `Result<Vec<Project>, DiscoverError>`，但按 spec"根目录不存在时返回空列表并 warn"——这一路径内部消化，**不**上抛错误。只有"真实 I/O 错误"才返回 `Err`。

### 决策 7：测试策略

- **单元测试**（每个模块文件底部的 `#[cfg(test)] mod tests`）：
  - `path_decoder`：覆盖 spec 的 3 个 scenario（标准、歧义、WSL）。
  - `subproject_registry`：composite ID 生成、去重、session filter 获取。
- **集成测试**（`crates/cdt-discover/tests/project_scanner.rs`）：
  - 用 `tempfile::tempdir()` 构造假的 `~/.claude/projects/` 布局。
  - 覆盖 spec 的 5 条 Requirement × 多个 scenario。
  - `WorktreeGrouper` 的 git 测试：`tempdir` + 真跑 `git init` + `git worktree add`（要求 CI 环境有 git），两个 worktree 验证归到同一个 group。
- 不引入 `insta` 快照：本 port 的输出是结构化数据，用 `assert_eq!` 更直观。

## Risks / Trade-offs

- **[Risk]** `WorktreeGrouper` 的集成测试要真跑 `git`，在某些沙箱 CI 上 git 可能缺失 → **缓解**：给 grouper 测试加 `#[ignore]` + `#[cfg(feature = "git-integration-test")]`，默认只跑 mock 分支（用一个 `FakeGitIdentityResolver` 返回预置结果）。本地开发环境有 git 就跑全套。
- **[Risk]** `async_trait` 在 `FileSystemProvider` 里会产生 boxed future，每次调用有分配开销 → **缓解**：discovery 是低频操作（一次 scan 对几百个目录），boxed future 的开销相比磁盘 I/O 可以忽略。待 Rust stable `dyn async fn` 成熟后可以平滑替换。
- **[Risk]** composite ID 的 hash 长度 8 字节可能在大型仓库下产生碰撞 → **缓解**：TS 侧就是 8 字节 sha256 前缀，我们严格对齐；且碰撞只影响 UI 展示歧义，不影响数据正确性。spec delta 里会把"8 字符十六进制 sha256 前缀"写死。
- **[Trade-off]** 把 `SubprojectRegistry` 从 global 改成 per-scanner → **代价**：跨模块访问要走 scanner handle；**收益**：测试独立、多 scanner 安全。选收益。
- **[Trade-off]** 本 port 不做 SSH provider，`FileSystemProvider` trait 只有一个实现，形态可能在后续 port 中改动 → **缓解**：trait 方法签名本次只定最小可用集，后续 port 再扩容不破 API。

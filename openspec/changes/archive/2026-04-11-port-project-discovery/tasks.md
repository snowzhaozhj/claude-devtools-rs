## 1. 共享类型上移到 cdt-core

- [x] 1.1 在 `crates/cdt-core/src/project.rs` 新增 `Project`、`Session`、`Worktree`、`RepositoryGroup`、`RepositoryIdentity`、`SessionMetadata`，字段与 TS `src/main/types` 对齐（`id: String` / `path: PathBuf` / `sessions: Vec<String>` / `most_recent_session: Option<i64>` / `created_at: Option<i64>`）。
- [x] 1.2 在 `crates/cdt-core/src/lib.rs` 里 `pub mod project;` 并 `pub use project::*;`。
- [x] 1.3 给每个新类型加 `#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]`，保持 `cdt-core` 零 runtime 依赖。
- [x] 1.4 `cargo build -p cdt-core` + `cargo clippy -p cdt-core -- -D warnings` 通过。

## 2. cdt-discover 基建：依赖与错误类型

- [x] 2.1 在 workspace root `Cargo.toml` 的 `[workspace.dependencies]` 里补上（如缺）：`async-trait`、`sha2`、`tokio`（确认含 `fs`、`io-util`、`process`、`macros`、`rt-multi-thread`）。
- [x] 2.2 修改 `crates/cdt-discover/Cargo.toml`：声明 `cdt-core`、`cdt-parse`（workspace）依赖；加 `tokio`、`async-trait`、`sha2`、`tracing`、`thiserror`、`serde`、`serde_json`。
- [x] 2.3 在 `crates/cdt-discover/src/error.rs` 定义 `FsError` 与 `DiscoverError`（`thiserror::Error`），按 design.md 决策 6 的形状。
- [x] 2.4 `crates/cdt-discover/src/lib.rs` 里 `pub mod` 占位：`fs_provider` / `path_decoder` / `subproject_registry` / `project_scanner` / `worktree_grouper` / `project_path_resolver` / `error`。
- [x] 2.5 `cargo build -p cdt-discover` 通过（即便是空壳）。

## 3. FileSystemProvider trait + LocalFileSystemProvider

- [x] 3.1 在 `crates/cdt-discover/src/fs_provider.rs` 定义 `FsKind { Local, Ssh }`、`EntryKind { File, Dir, Symlink, Other }`、`DirEntry { name: String, kind: EntryKind }`、`FsMetadata { size: u64, mtime: SystemTime }`。
- [x] 3.2 定义 `#[async_trait] pub trait FileSystemProvider: Send + Sync + 'static`，方法 `kind`、`exists`、`read_dir`、`read_to_string`、`stat`、`read_lines_head`，签名对齐 design.md 决策 1。
- [x] 3.3 实现 `pub struct LocalFileSystemProvider;`，`impl FileSystemProvider` 全部方法：`exists` 用 `tokio::fs::metadata(...).is_ok()`；`read_dir` 返回 `Vec<DirEntry>`（过滤不可读条目，warn）；`read_lines_head` 用 `tokio::io::BufReader::lines()` 取前 N 行。
- [x] 3.4 `pub use fs_provider::*;` 在 `lib.rs` 里导出；`LocalFileSystemProvider` 暴露给下游 crate。
- [x] 3.5 单元测试（底部 `mod tests`）：`tempdir` 构造两层目录 + 一个文件，验证 `read_dir` / `exists` / `stat` / `read_lines_head` 的返回。
- [x] 3.6 `cargo clippy -p cdt-discover -- -D warnings` 通过。

## 4. 纯函数：path_decoder

- [x] 4.1 在 `path_decoder.rs` 实现 `decode_path(encoded: &str) -> PathBuf`（每个 leading `-` 换成 `/`，其余 `-` 原样保留；内部的 `_` / `.` 等字符不动）。
- [x] 4.2 实现 `extract_base_dir(project_id: &str) -> &str`：如果包含 `::`，返回 `::` 之前的部分；否则原样返回。
- [x] 4.3 实现 `extract_project_name(path: &Path) -> String`：取最后一段文件名。
- [x] 4.4 实现 `is_valid_encoded_path(name: &str) -> bool`：TS 里的规则是"必须以 `-` 开头"，按此对齐。
- [x] 4.5 实现 `get_projects_base_path() -> PathBuf`（`$HOME/.claude/projects/`）与 `get_todos_base_path() -> PathBuf`，读 `std::env::var("HOME")`。
- [x] 4.6 单元测试覆盖 spec scenario：标准编码、歧义编码（best-effort）、WSL 样式 `/mnt/c/...`。
- [x] 4.7 clippy 通过。

## 5. 纯函数：subproject_registry

- [x] 5.1 在 `subproject_registry.rs` 定义 `SubprojectEntry { base_dir: String, cwd: PathBuf, session_ids: BTreeSet<String> }` 与 `pub struct SubprojectRegistry { entries: HashMap<String, SubprojectEntry> }`。
- [x] 5.2 实现 `register(base_dir: &str, cwd: &Path, session_ids: &[String]) -> String`：用 `sha2::Sha256` 算 cwd 的 digest，取前 8 位 hex，拼 composite ID。
- [x] 5.3 实现 `get_base_dir`、`is_composite`、`get_session_filter`、`get_cwd`、`get_entry`、`clear`，接口与 TS 对齐。
- [x] 5.4 单测：同 cwd 两次 register 得到相同 ID；不同 cwd 得到不同 ID；`get_session_filter` 对 plain ID 返回 `None`。
- [x] 5.5 单测：composite ID 形状 `{baseDir}::[0-9a-f]{8}`。
- [x] 5.6 clippy 通过。

## 6. ProjectScanner：核心扫描循环

- [x] 6.1 `project_scanner.rs` 定义 `pub struct ProjectScanner<F: FileSystemProvider + ?Sized> { fs: Arc<F>, projects_dir: PathBuf, registry: SubprojectRegistry, path_resolver: ProjectPathResolver }`（或等价组合）。
- [x] 6.2 实现 `pub async fn scan(&mut self) -> Result<Vec<Project>, DiscoverError>`：调用 `fs.exists`；不存在则 `tracing::warn!` 并返回空 vec（**不报错**，按 spec scenario"Root directory missing"）。
- [x] 6.3 枚举 `fs.read_dir(projects_dir)`，用 `path_decoder::is_valid_encoded_path` 过滤目录。
- [x] 6.4 对每个目录调用内部 `scan_project(name)`：读取其下所有 `*.jsonl` entry（通过 `fs.read_dir`），按 mtime 降序排序，从每个 session 文件 head-read 前 20 行提 cwd（用 `cdt_parse::parse_entry_at` 逐行解析），按 cwd 分桶。
- [x] 6.5 若该目录只有一个 cwd 桶：构造一个 `Project { id: encoded_name, path: cwd_or_decoded, sessions: [...], ... }`。若有多个 cwd：对每个桶调 `registry.register` 得 composite ID，构造多个 `Project`。
- [x] 6.6 `scan` 最终把所有 project 按 `most_recent_session` 降序排序返回。
- [x] 6.7 单元测试占位：空目录（0 project）、目录不存在（0 project + warn）、单文件单 session（1 project）。

## 7. ProjectScanner：session 列表与 pinned

- [x] 7.1 实现 `pub async fn list_sessions(&self, project_id: &str) -> Result<Vec<Session>, DiscoverError>`：解出 base_dir，`fs.read_dir` + `fs.stat` 拿 mtime/size，按 mtime 降序返回 `Session { id, last_modified, size }`，只保留 `*.jsonl`。
- [x] 7.2 若 `project_id` 是 composite：只返回 `registry.get_session_filter(project_id)` 命中的 session id。
- [x] 7.3 接受一个外部注入的 `pinned_sessions: &BTreeSet<String>` 参数（或通过构造函数传入），在返回的 `Session` 里标记 `is_pinned`；本 port 不做持久化，只做"若被传入则反映出来"。
- [x] 7.4 集成测试：在 tempdir 里造 5 个 `.jsonl` + 1 个 `.txt`，验证只返回 5 条且按 mtime 降序。
- [x] 7.5 集成测试：传入 pinned set 时对应 session 的 `is_pinned = true`。

## 8. ProjectPathResolver

- [x] 8.1 `project_path_resolver.rs` 定义 `pub struct ProjectPathResolver { fs: Arc<dyn FileSystemProvider>, projects_dir: PathBuf, cache: Mutex<HashMap<String, PathBuf>> }`。
- [x] 8.2 `pub async fn resolve_project_path(&self, project_id: &str, hint: Option<&Path>, session_paths: Option<&[PathBuf]>) -> PathBuf`：优先 cache → composite registry cwd → 绝对路径 hint → 逐个 session 文件 head-read 抽 cwd → `decode_path(base_dir)` fallback。
- [x] 8.3 若 `fs.kind() == Ssh`，`session_paths` 最多只检查 1 个文件（避免远端全量扫描）；Local 模式下遍历所有。
- [x] 8.4 `pub fn invalidate(&self, project_id: &str)` 清单条 cache；`pub fn clear(&self)` 清空。
- [x] 8.5 单测：cache 命中路径；composite registry short-circuit；cwd 字段优先于 decode。
- [x] 8.6 clippy 通过。

## 9. WorktreeGrouper + GitIdentityResolver

- [x] 9.1 `worktree_grouper.rs` 定义 `#[async_trait] pub trait GitIdentityResolver: Send + Sync` 三方法 `resolve_identity(&self, path: &Path) -> Option<RepositoryIdentity>` / `get_branch(&self, path: &Path) -> Option<String>` / `is_main_worktree(&self, path: &Path) -> bool`。
- [x] 9.2 实现 `LocalGitIdentityResolver`：内部用 `tokio::process::Command::new("git").current_dir(path).args(["rev-parse", "--git-common-dir"])` 等命令，失败一律返回 `None`（非 git 目录）。
- [x] 9.3 实现 `pub struct WorktreeGrouper<G: GitIdentityResolver> { git: G }` 与 `pub async fn group_by_repository(&self, projects: Vec<Project>) -> Vec<RepositoryGroup>`：按 `identity.id` 分组；`identity == None` 的项目各自成组（单 worktree 的 group）；每组内主 worktree 排前、其余按 `most_recent_session` 降序；全组按最近活动降序。
- [x] 9.4 实现 `FakeGitIdentityResolver`（`#[cfg(test)]` only），接受预置 `HashMap<PathBuf, (Option<RepositoryIdentity>, Option<String>, bool)>`，用于集成测试。
- [x] 9.5 单测：两个 worktree 共享同一 `identity.id` → 合并成一组；没 identity 的项目 → 单成员组。
- [x] 9.6 可选集成测试（`#[cfg(feature = "git-integration-test")]` gate，CI 默认 off）：tempdir 里 `git init` + `git worktree add`，跑 `LocalGitIdentityResolver` 走端到端。
- [x] 9.7 clippy 通过。

## 10. 端到端集成测试

- [x] 10.1 新建 `crates/cdt-discover/tests/project_scanner.rs`，用 `tempfile::tempdir` 造一个假的 `~/.claude/projects/` 布局：
  - 目录 `-Users-alice-code-foo/` 含 2 个 `.jsonl`（cwd 都是 `/Users/alice/code/foo`）→ 1 个 Project
  - 目录 `-Users-alice-code-bar/` 含 2 个 `.jsonl`（cwd 分别为 `/Users/alice/code/bar` 和 `/Users/alice/code/bar-v2`）→ 2 个 Project（composite ID）
  - 目录 `-Users-alice-empty/` 不含任何文件 → 0 个 Project
- [x] 10.2 断言 `scan()` 返回 3 个 Project，排序正确。
- [x] 10.3 断言 composite ID 形如 `-Users-alice-code-bar::[0-9a-f]{8}`；两次 scan 拿到一样的 ID（稳定性）。
- [x] 10.4 断言根目录不存在时 `scan()` 返回空 vec 且无 panic、无 Err。
- [x] 10.5 断言 `list_sessions` 只返回 `.jsonl`，按 mtime 降序。
- [x] 10.6 `WorktreeGrouper` 用 `FakeGitIdentityResolver` 跑：
  - 两个 Project 预置相同 `RepositoryIdentity` → 1 个 RepositoryGroup 含 2 个 Worktree
  - 1 个 Project 无 identity → 1 个单成员 RepositoryGroup
  - 总计 2 个 group

## 11. spec fidelity 与 followups 联动

- [x] 11.1 运行 `spec-fidelity-reviewer` subagent（或手工审计）：确认 `openspec/specs/project-discovery/spec.md` 的每条 Requirement × Scenario 都能在 `cdt-discover` 单测或集成测试里找到对应 case，特别是 spec 原本 5 条 Requirement 的 9 个 scenario。
- [x] 11.2 更新 `openspec/followups.md` 的 `## project-discovery` 段落：
  - spec-gap "路径解码歧义消解" 的 Rust 实现状态标记为 ✅，链接到 `project_path_resolver.rs` 与对应测试。
- [x] 11.3 更新根 `CLAUDE.md` 的 Capability → crate map：把 `project-discovery` 从 `not started` 改为 `done ✓`，并在"剩余 port order"里删掉第 1 项。

## 12. CI 与合规

- [x] 12.1 `cargo fmt --all`。
- [x] 12.2 `cargo clippy --workspace --all-targets -- -D warnings` 全量绿。
- [x] 12.3 `cargo test --workspace` 全量绿。
- [x] 12.4 `openspec validate port-project-discovery --strict` 通过，准备 `/opsx:apply`。

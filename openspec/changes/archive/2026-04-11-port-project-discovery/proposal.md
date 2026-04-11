## Why

`project-discovery` 是数据层第二组（scan / search / watch / ssh）共同依赖的根能力：它负责枚举 `~/.claude/projects/` 下的所有工程、把编码目录名解码回真实 cwd、按 git worktree 分组、并维护 subproject / pinned-session 注册表。没有它，后续 `session-search`、`file-watching`、`ssh-remote-context`、`project-path-resolver` 都无法落地，因此它是第 4 个 port 目标。

同时本次 port 要引入关键抽象 —— `FileSystemProvider` trait —— 把"枚举目录 / 读文件 / stat"等 I/O 原语抽出接口，为后续 `ssh-remote-context` 直接替换成 SSH 后端铺路。TS 侧 `LocalFileSystemProvider` / `FileSystemProvider` 的结构直接按 idiomatic Rust 翻译。

## What Changes

- 新增 `cdt-discover` crate 的第一批模块：`fs_provider` / `path_decoder` / `subproject_registry` / `project_scanner` / `worktree_grouper` / `project_path_resolver`。
- 在 `cdt-core` 里补充共享类型：`Project`、`Session`、`Worktree`、`RepositoryGroup`、`RepositoryIdentity`、`SessionMetadata`。这些类型会被 `cdt-discover`、`cdt-watch`、`cdt-api` 等下游 crate 复用。
- 引入 `FileSystemProvider` trait（sync + async 混合接口，使用 `async_trait`）作为所有 discovery/search/watch I/O 的抽象层。首个实现 `LocalFileSystemProvider` 基于 `tokio::fs`。
- 实现 baseline spec 的全部 5 条 Requirement：扫描 projects 根目录、解码编码路径、列 session、按 git worktree 分组、追踪 subproject 与 pinned sessions。
- **ADDED Requirement**（spec delta）：显式冻结"通过 `FileSystemProvider` 抽象读写文件系统"这一契约，便于后续 `ssh-remote-context` port 时只替换 provider 实现，不改 scanner 逻辑。
- **ADDED Requirement**（spec delta）：冻结 "composite project ID for subprojects" 的形式（`{baseDir}::{hash8}`），这是 TS 里事实上的 API 但 baseline spec 没写进去；Rust 实现需要它来按 cwd 细分同一个编码目录。
- **不包含**：session 搜索（下一轮 `port-session-search`）、文件变更监听（`port-file-watching`）、SSH provider 实现（`port-ssh-remote-context`）、git identity 的完整解析链（只做到够用的 `git rev-parse --git-common-dir` 级别，复杂的 worktree source 检测留给 team-coordination port 时再补）。
- **fs seam**：`cdt-core` 仍保持 sync，不引入 `tokio`；`FileSystemProvider` trait 定义在 `cdt-discover` 里（而非 `cdt-core`），因为它需要 async 语义。

## Capabilities

### New Capabilities
<!-- 无：project-discovery 已存在于 openspec/specs/ -->

### Modified Capabilities
- `project-discovery`: 以 Rust 实现替代 TS baseline，并在 spec 中补齐两条此前未冻结的契约（FileSystemProvider 抽象、composite subproject ID 形态）。不改动已有 Requirement 的语义。

## Impact

- **新代码**：
  - `crates/cdt-core/src/project.rs`（共享数据类型）
  - `crates/cdt-discover/src/lib.rs` + `fs_provider.rs` + `path_decoder.rs` + `subproject_registry.rs` + `project_scanner.rs` + `worktree_grouper.rs` + `project_path_resolver.rs`
  - 对应的单测与 fixtures（`crates/cdt-discover/tests/`）
- **依赖新增**：`cdt-discover` 加 `tokio`（`fs`、`io-util`、`process` features）、`async-trait`、`sha2`（composite ID hashing）、`tracing`、`thiserror`、`serde`。版本统一走 workspace root。`cdt-core` 只新增 `serde`（已有）相关的 `project` 模块。
- **下游影响**：
  - `cdt-watch`、`cdt-api`、`cdt-ssh` 后续 port 会 `use cdt_discover::FileSystemProvider` 作为 seam；本次 port 需要把这个 trait 的 shape 稳定下来。
  - `cdt-parse` 本次保持不动：discovery 只用 `cdt_parse::parse_entry_at` 来从一个 JSONL 中抽 `cwd`，不依赖流式 `parse_file`。
- **无破坏**：目前 workspace 里没有 discovery 相关代码，纯新增。

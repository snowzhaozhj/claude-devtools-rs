# project-discovery Spec Delta

## MODIFIED Requirements

### Requirement: Abstract filesystem access through a provider trait

系统 SHALL 把所有 project / session 的文件 I/O 都走单一的 `FileSystemProvider` trait，使其它后端（例如 SSH 远端）可在不改 `ProjectScanner` / 路径解析器 / worktree grouper 的前提下接入。

trait 的**真相源** SHALL 住在独立的 `cdt-fs` crate 内（`crates/cdt-fs/`），不再属于 `cdt-discover`。`cdt-discover` SHALL 通过 `pub use cdt_fs::*` re-export 兼容历史 import 路径，但**不得**重新定义同名类型。

trait SHALL 至少暴露这些操作：

1. `kind()` 返回 `FsKind`（Local / Ssh）
2. `exists(path)` 判路径是否存在
3. `read_dir(path)` 列举目录条目（含 file/dir 类型）
4. `read_dir_with_metadata(path)` 列举目录条目并附 metadata（默认实现可走 `read_dir + 逐项 stat`，但 SSH 可 override 用单次 SFTP readdir 拿全量元数据避免 N 次 stat）
5. `stat(path)` 取 `FsMetadata`，含 `size` / `mtime` / `identity: Option<FsIdentity>`
6. `read_to_string(path)` 把文件全量读为 UTF-8
7. `read_lines_head(path, max)` 仅读文件前 N 行
8. `open_read(path)` 返回 `Box<dyn AsyncRead + Send + Unpin>` 流式句柄（**新增**，替代 SSH provider 内部 `open_read_stream` 破抽象）
9. `stat_many(paths)` 批量 stat（**新增** batched API，default 实现走 `join_all`）

`FsMetadata` SHALL 包含 `identity: Option<FsIdentity>` 字段——Local Unix 填 `Some(FsIdentity::Unix { dev, ino })`，Local Windows 与所有 SSH 场景填 `None`（best-effort）。

`FileSystemProvider` trait **不得**承担分页 / 排序语义。任何按 mtime / size 排序拿前 N 个的需求 SHALL 走更高层抽象（`ProjectScanner` 自身排序、`SessionIndex` 等未来引入的高层 API），不污染 fs trait。

#### Scenario: Local filesystem provider satisfies the scanner

- **WHEN** `ProjectScanner::scan` 配 `LocalFileSystemProvider` 调用
- **THEN** scanner SHALL 仅通过 trait 方法列举 project 与抽取 per-session 元数据，SHALL NOT 直接调任何平台特定文件系统 API

#### Scenario: Path resolver avoids full-file reads in remote mode

- **WHEN** 当前 provider 上报 `kind() == FsKind::Ssh` 且 resolver 需要从 session 文件抽 `cwd`
- **THEN** resolver SHALL 调 `read_lines_head(path, N)` 取足以覆盖首条 user / summary 记录的有限 N 行，SHALL NOT 下载整个文件

#### Scenario: fs 抽象 trait 是替换 backend 的唯一接口

- **WHEN** 后续某个 port 引入新后端（例如 SSH / WSL / fake test provider）
- **THEN** 引入仅 SHALL 要求实现 `cdt_fs::FileSystemProvider`，SHALL NOT 要求改 `ProjectScanner` / `ProjectPathResolver` / `WorktreeGrouper`

#### Scenario: discover capability 暴露兼容 alias 给老调用方

- **WHEN** 老代码写 `use cdt_discover::FileSystemProvider`
- **THEN** 编译 SHALL 成功，行为与 `use cdt_fs::FileSystemProvider` 等价

#### Scenario: fs trait 暴露面不含排序

- **WHEN** 检查 `FileSystemProvider` 方法签名
- **THEN** SHALL NOT 含任何接受 `SortBy` / `Order` / `Cursor` / `Offset` 类参数的方法
- **AND** 调用方按 mtime 排序拿前 N 时 SHALL 自己在调用方代码内排序，不让 trait 帮排

### Requirement: Compare paths case-insensitively on Windows

系统 SHALL 在所有路径比较点（HashMap/BTreeMap key、HashSet 元素、`starts_with` / `eq` 判定、hash 输入）使用统一的跨平台规范化 helper，使**Windows 平台**上仅大小写不同的两条路径被视为相等，**非 Windows 平台**保持字节精确比较。

规范化 helper SHALL 由 `cdt-discover::path_compare` 模块统一提供，是整个 workspace 中跨平台路径比较的唯一来源；任何其它 crate 需要做路径比较 / hash 时 SHALL 引用该模块的公开函数，**不得**自行实现 lowercase / equality 逻辑。规范化策略 SHALL 使用 ASCII lowercase（与 TS 原版 `pathValidation.ts::normalizeForCompare` 行为对齐），不做 Unicode 大小写折叠。

`ProjectPathResolver` 的内部 cache key（encoded `project_id`）以及 `ProjectScanner::scan_project_dir` 的 `Project.distinct_cwds` 去重 key 都 SHALL 在插入与查询前经过此规范化。`distinct_cwds` 展示值 SHALL 保留首次出现的原始 cwd 字面量（不归一），以便消费方（UI / agent-configs）拿到与文件系统真实大小写一致的路径。

#### Scenario: Windows 上同一路径不同大小写归一

- **WHEN** 在 Windows 平台运行，两条 session 的 `cwd` 字段分别为 `C:\Users\Alice\app` 与 `c:\users\alice\app`
- **THEN** `ProjectPathResolver` SHALL 把两条 session 视为同一 project
- **AND** `ProjectScanner::scan_project_dir` 产出的 `Project.distinct_cwds` SHALL 只含一条 cwd（去重命中），其值为首次出现的原始字面量

#### Scenario: 非 Windows 平台保持精确比较

- **WHEN** 在 Linux 或 macOS 平台运行，两条 session 的 `cwd` 字段分别为 `/Users/alice/App` 与 `/users/alice/app`
- **THEN** `ProjectPathResolver` SHALL 把两条 session 视为不同 project

#### Scenario: 跨大小写命中同一项目路径解析缓存

- **WHEN** 在 Windows 平台运行，调用方先用 encoded `project_id = "-C:-Users-Alice-app"` 触发解析并写 cache，再用 `"-C:-users-alice-app"`（同一目录、不同大小写）查询
- **THEN** `ProjectPathResolver::resolve` SHALL 命中第一次的 cache 条目，返回相同 `PathBuf`，不重新走文件系统扫描

### Requirement: Expose session cwd for downstream display

系统 SHALL 在 `Session`（`cdt-core::Session`，IPC 序列化形态）中暴露 `cwd: Option<String>` 字段，值取自该 session jsonl 内首条带 `cwd` 字段消息的 `cwd` 值；该字段为空（jsonl 不含 cwd）时 SHALL 为 `None`。序列化 SHALL 使用 camelCase（`cwd`），并在为 `None` 时通过 `#[serde(skip_serializing_if = "Option::is_none")]` 省略输出。

`cdt-core::Session` SHALL NOT 增加 `cwd_relative_to_repo_root` 字段——该派生字段属于 worktree 维度展示信息，由 `Worktree.cwd_relative_to_repo_root` 持有（见 `Group projects by git worktree` Requirement）；IPC 层 `SessionSummary` 在序列化时通过 group→worktree join 填入（见 ipc-data-api spec `SessionSummary 增加 worktree 元信息字段`），避免 scanner 阶段重走 repo 解析。

`ProjectScanner::scan_project_dir` SHALL 在产生 `Session` 时把 `extract_session_cwd` 的结果直接写入 `Session.cwd`；该 cwd 提取沿用现有 head-read（仅读 jsonl 前 `SESSION_HEAD_LINES` 行）+ `FILE_READ_CONCURRENCY` 信号量限流路径，**不**得为获取 cwd 而触发全文件读取（除非 head 不含 cwd 字段时按现有 `extract_session_cwd` SSH fallback 路径回滚）。

#### Scenario: 单 cwd session 暴露 cwd 字段

- **WHEN** 一个 jsonl session 首条消息 `cwd = "/Users/foo/myrepo"`
- **THEN** 系统 SHALL 在 `Session.cwd` 中返回 `Some("/Users/foo/myrepo")`
- **AND** IPC 序列化结果 SHALL 包含 `"cwd": "/Users/foo/myrepo"`

#### Scenario: 缺 cwd session 暴露 None

- **WHEN** 一个 jsonl session 所有消息均不含 `cwd` 字段
- **THEN** 系统 SHALL 在 `Session.cwd` 中返回 `None`
- **AND** IPC 序列化结果 SHALL 省略 `cwd` 键（不出现 `"cwd": null`）

#### Scenario: 同一 encoded 目录多 cwd 的 session 各自暴露真实 cwd

- **WHEN** 一个 encoded 目录 `D` 下含两条 session，cwd 分别为 `/a/b` 与 `/a/c`
- **THEN** 系统 SHALL 输出**一条** `Project`（`id = D`，不再拆分），其 `sessions` 列表两条目分别带 `cwd = Some("/a/b")` 与 `cwd = Some("/a/c")`

#### Scenario: 提取 cwd 不触发全文件读

- **WHEN** 一个 session jsonl 文件大小 100 MB，cwd 在前 20 行内
- **THEN** 系统 SHALL 仅通过 head-read（`FileSystemProvider::read_lines_head`）拿到 cwd
- **AND** SHALL NOT 触发对该文件的 `read_to_string`

#### Scenario: Session payload 不含 cwd_relative_to_repo_root 字段

- **WHEN** grep `cdt-core/src/project.rs::Session` 的字段定义
- **THEN** SHALL 不出现 `cwd_relative_to_repo_root` 字段（该字段仅在 `cdt-core::Worktree` 与 IPC 层 `SessionSummary` 上存在）

# project-discovery Specification

## Purpose

扫描 `~/.claude/projects/` 目录、解码 Claude Code 的 encoded 项目目录名（POSIX / Windows / WSL 多格式）、识别同 git 仓库下的多 worktree、按 `cwd` 把单目录拆分为子项目。本 capability 通过 `FileSystemProvider` trait 抽象 I/O，使 `ssh-remote-context` 可无侵入地接入。
## Requirements
### Requirement: Scan Claude projects directory

系统 SHALL 扫描当前 Claude root 下的 `projects` 根目录，把每个一级子目录视为一个 project。当前 Claude root SHALL 来自 `general.claudeRootPath`；当该字段为 `null` 时，默认 Unix root 为 `~/.claude`、Windows root 为 `%USERPROFILE%\.claude\`，projects 根目录分别为 `~/.claude/projects/` 与 `%USERPROFILE%\.claude\projects\`。

系统 SHALL 按 `HOME` → `USERPROFILE` → `HOMEDRIVE` + `HOMEPATH` → 平台默认（`dirs::home_dir()`）的优先级解析用户 home 目录。这与 TS 原版 `pathDecoder.ts::getHomeDir` 的 fallback 链一致：让 WSL / Git Bash / Cygwin 用户可经 `HOME` 覆写，同时仍能在 Windows 原生 shell 里定位到 `%USERPROFILE%\.claude\`。

#### Scenario: Empty root directory

- **WHEN** projects 根目录存在但无任何子目录
- **THEN** 系统 SHALL 返回空 project 列表，不抛错

#### Scenario: Root directory missing

- **WHEN** projects 根目录不存在
- **THEN** 系统 SHALL 返回空 project 列表并记录 warning，不抛错

#### Scenario: Multiple project directories present

- **WHEN** projects 根目录含 N 个子目录
- **THEN** 系统 SHALL 返回 N 条 project 条目，每条暴露 decode 后的文件系统路径、显示名、session 数

#### Scenario: Home directory resolution on Windows native

- **WHEN** 在 Windows 上运行，`HOME` 未设而 `USERPROFILE` 设为 `C:\Users\alice`
- **THEN** 系统 SHALL 解析 projects 根目录为 `C:\Users\alice\.claude\projects\`

#### Scenario: Home directory resolution via HOMEDRIVE/HOMEPATH fallback

- **WHEN** 在 Windows 上运行，`HOME` 与 `USERPROFILE` 均未设，但 `HOMEDRIVE=C:` 与 `HOMEPATH=\Users\alice` 已设
- **THEN** 系统 SHALL 解析 home 目录为 `C:\Users\alice`、projects 根目录为 `C:\Users\alice\.claude\projects\`

#### Scenario: HOME env variable takes priority over USERPROFILE

- **WHEN** `HOME=/home/user` 与 `USERPROFILE=C:\Users\alice` 同时设置（典型 WSL / Git Bash on Windows 场景）
- **THEN** 系统 SHALL 优先用 `HOME`，解析 projects 根目录为 `/home/user/.claude/projects/`

#### Scenario: Custom Claude root projects directory

- **WHEN** 当前 Claude root 配置为 `/data/claude-alt`
- **THEN** scanner SHALL 扫描 `/data/claude-alt/projects/`
- **AND** scanner SHALL NOT 扫描默认 `~/.claude/projects/`

#### Scenario: Clearing custom Claude root restores default projects directory

- **WHEN** 当前 Claude root 从 `/data/claude-alt` 清空为 `null`
- **THEN** scanner SHALL 重新使用默认 home 下 `.claude/projects/`

### Requirement: Decode encoded project paths

系统 SHALL 把 Claude Code encoded 目录名转回原始文件系统路径。decoder SHALL 按以下顺序识别三种格式：

1. **Legacy Windows format** `^([A-Za-z])--(.+)$`（例如 `C--Users-alice-app`）SHALL 解码为 `<drive_upper>:/<rest_with_slashes>`（例如 `C:/Users/alice/app`）。
2. **New Windows format**（去 legacy 后 `-C:-Users-alice-app`）：剥离首个 `-`、把剩余 `-` 替换为 `/`，若结果命中 `^[A-Za-z]:/` 则**原样返回**（例如 `C:/Users/alice/app`），不再添加 POSIX 前导 `/`。
3. **POSIX format**（`-Users-alice-app`）：decoder SHALL 剥离首个 `-`、把剩余 `-` 替换为 `/`，并补一个前导 `/` 形成绝对路径（例如 `/Users/alice/app`）。

当目标平台为 Windows 时，decoder SHALL 额外做 WSL 挂载点翻译：任何 decode 后命中 `^/mnt/([A-Za-z])(/.*)?$` 的路径 SHALL 被改写为 `<drive_upper>:<rest>`（例如 `/mnt/c/code` → `C:/code`）。

非 Windows 平台 SHALL 把 WSL 挂载路径原样返回，不改写（与已有 scenario "WSL-style path" 一致）。

#### Scenario: Standard encoded name

- **WHEN** project 目录名为 `-Users-alice-code-app`
- **THEN** decode 结果 SHALL 为 `/Users/alice/code/app`

#### Scenario: Path containing legitimate hyphens

- **WHEN** project 目录名为 `-Users-alice-my-app`（在 `/Users/alice/my-app` 与 `/Users/alice/my/app` 间存在歧义）
- **THEN** decoder SHALL 返回 best-effort 替换（每个前导 `-` 都换成 `/`），权威 cwd SHALL 在该 project 目录下的 session 记录 `cwd` 字段可用时由其恢复

#### Scenario: WSL-style path on non-Windows platforms

- **WHEN** decode 后的路径指向 WSL 挂载（例如 `/mnt/c/...`）且当前平台非 Windows
- **THEN** 系统 SHALL 原样返回该路径，不做平台改写

#### Scenario: New Windows format decodes to drive-letter path

- **WHEN** project 目录名为 `-C:-Users-alice-app`
- **THEN** decode 结果 SHALL 为 `C:/Users/alice/app`（不带 POSIX 前导 `/`）

#### Scenario: Legacy Windows format decodes to drive-letter path

- **WHEN** project 目录名为 `C--Users-alice-app`（无前导 `-`，冒号编码为 `--`）
- **THEN** decode 结果 SHALL 为 `C:/Users/alice/app`；驱动器字母 SHALL 强制大写（即使源名为小写）

#### Scenario: WSL mount translation on Windows

- **WHEN** 在 Windows 上运行，decode 结果为 `/mnt/c/code`
- **THEN** 系统 SHALL 改写为 `C:/code`

#### Scenario: is_valid_encoded_path accepts legacy Windows format

- **WHEN** 测试 `is_valid_encoded_path("C--Users-alice-app")`
- **THEN** 结果 SHALL 为 `true`；任意命中 `^[A-Za-z]--[A-Za-z0-9_.\s-]+$` 的输入同样如此

### Requirement: List sessions per project

系统 SHALL 列出指定 project 目录下所有 `*.jsonl` session 文件，每条返回 session id（去扩展名的 basename）、最后修改时间、文件大小。

#### Scenario: Project with multiple sessions
- **WHEN** project 目录含 5 个 `.jsonl` 文件
- **THEN** session 列表 SHALL 含 5 条，按最后修改时间倒序

#### Scenario: Project with non-jsonl files
- **WHEN** project 目录含 `.jsonl` 与其它类型文件混合
- **THEN** session 列表 SHALL 仅含 `.jsonl` 文件

### Requirement: Group projects by git worktree

系统 SHALL 把同一 git 仓库的多个 worktree 对应的 project 目录归为一个逻辑仓库条目，同时把每个 worktree 保留为该条目的独立成员；MUST 区分"主 working tree 根"与"主 working tree 子目录"两种 walk-up 都能到达同一 `.git` 的情况，避免子目录 cwd 被误标为独立的 main worktree。

仓库分组通过 `LocalGitIdentityResolver` 的**纯 fs 路径**（`crates/cdt-discover/src/worktree_grouper.rs::LocalGitIdentityResolver`，0 个 git 子进程）：向上 walk 找到 `.git` 条目，目录 → main worktree `(common_dir = git_dir = <repo>/.git)`；文件（gitlink）→ 解析 `gitdir:` 行后看 `<gitdir>/commondir` 文件区分 linked worktree（用 commondir）vs submodule（common = gitdir）。`identity = canonical(common_dir)` 字符串、`name = canonical.parent().file_name()`、`git_branch` 解析 `<git_dir>/HEAD`。**整个解析路径 MUST 不 spawn 任何 git 子进程**（替换 git 子进程为 syscall 是历史性能改造的成果，详 `worktree_grouper.rs::78-117`，27 project 累计 ~50ms 量级）。

聚合结果 `RepositoryGroup` MUST 含 `id`（稳定的 repo id，通常是 git-common-dir 的绝对路径）/ `identity`（`Option<RepositoryIdentity>`，无 git 时为 `None`）/ `name`（展示名）/ `worktrees`（`Vec<Worktree>`）/ `most_recent_session`（`Option<i64>`，所有 worktree 的 max）/ `total_sessions`（所有 worktree 的 sessions 总和）字段。

每个 `Worktree` MUST 含 `id`（对齐底层 `Project.id`）/ `path` / `name` / `git_branch`（`Option<String>`）/ `is_main_worktree`（`bool`，语义：common-dir 是主 `.git` 而非 linked worktree gitdir，用于排序与 main worktree 子目录分组）/ `is_repo_root`（`bool`，语义：`path` 自身就是主 working tree 的根目录，**仅当** `start == <repo>` 且 `<repo>/.git` 是目录时为 `true`；子目录 cwd 即便 walk-up 到主 `.git` 也 SHALL 为 `false`）/ `cwd_relative_to_repo_root`（`Option<String>`，repo 根本身为 `None`，子目录为相对路径如 `crates`、`.claude/worktrees/feat-x`，无法计算 repo 根时为 `None`；计算 SHALL 是纯字符串 `path.strip_prefix(repo_root)`，**0 额外 syscall**）/ `sessions`（`Vec<String>`）/ `created_at`（`Option<i64>`）/ `most_recent_session`（`Option<i64>`）字段。

Worktree 排序 SHALL 按 `is_repo_root` 优先（repo 根排前）、再按 `is_main_worktree` 优先（main common-dir 排前）、再按 `most_recent_session` 倒序（活跃 worktree 排前）。Group 排序 SHALL 按 `most_recent_session` 倒序。

#### Scenario: Two worktrees of one repo
- **WHEN** 两个 project 路径分别落在同一仓库的两个 worktree（共享同一 `git common dir`）
- **THEN** 系统 SHALL 输出一个仓库分组，含两个 worktree 成员

#### Scenario: Standalone project not in a worktree
- **WHEN** 一个 project 路径无 git 元数据
- **THEN** 系统 SHALL 把它输出为只含自己的单成员分组，`identity` 字段 SHALL 为 `None`

#### Scenario: Main worktree 排在附加 worktree 之前
- **WHEN** 一个 group 内含主 worktree 与附加 worktree，附加 worktree 的 `most_recent_session` 更新
- **THEN** group.worktrees[0].is_main_worktree SHALL 为 true，附加 worktree 排在后面（main 优先级压过时间）

#### Scenario: Group 排序按最近活动倒序
- **WHEN** 两个独立 repo group A、B，A 的最近 session 比 B 早
- **THEN** `group_by_repository` 返回数组 SHALL 含 B 在前、A 在后

#### Scenario: 主仓子目录 cwd 不被误标为 repo root
- **WHEN** 主 repo `/repo` 含 `.git` 目录；另存在 project 路径 `/repo/crates`（用户在主仓子目录 cwd 跑 claude 产生的独立 encoded 目录）
- **THEN** grouper SHALL 把 `/repo` 与 `/repo/crates` 归到同一 group
- **AND** `/repo` 对应的 Worktree `is_repo_root` SHALL 为 `true`，`is_main_worktree` SHALL 为 `true`，`cwd_relative_to_repo_root` SHALL 为 `None`
- **AND** `/repo/crates` 对应的 Worktree `is_repo_root` SHALL 为 `false`，`cwd_relative_to_repo_root` SHALL 为 `Some("crates")`
- **AND** 排序后 `/repo` SHALL 排在 `/repo/crates` 之前

#### Scenario: linked worktree cwd 含 cwd_relative_to_repo_root
- **WHEN** 主 repo `/repo` 在 `/repo/.claude/worktrees/feat-x` 创建 linked worktree（已 prune 或仍在），有对应 encoded project
- **THEN** 对应 Worktree `is_repo_root` SHALL 为 `false`，`is_main_worktree` SHALL 为 `false`
- **AND** `cwd_relative_to_repo_root` SHALL 为 `Some(".claude/worktrees/feat-x")`

### Requirement: Resolve subprojects and pinned sessions

系统 SHALL 把 subproject 关联与用户 pin 的 session 视作配置状态，与扫描得到的 project 一并暴露。

#### Scenario: Pinned session exists
- **WHEN** 一条 session 经配置被 pin
- **THEN** 系统 SHALL 在 session 列表中标记其为 pinned，无视其修改时间

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

#### Scenario: Trait is the sole seam for alternative backends

- **WHEN** 后续某个 port 引入新后端（例如 SSH / WSL / fake test provider）
- **THEN** 引入仅 SHALL 要求实现 `cdt_fs::FileSystemProvider`，SHALL NOT 要求改 `ProjectScanner` / `ProjectPathResolver` / `WorktreeGrouper`

#### Scenario: cdt-discover 继续兼容老 import

- **WHEN** 老代码写 `use cdt_discover::FileSystemProvider`
- **THEN** 编译 SHALL 成功，行为与 `use cdt_fs::FileSystemProvider` 等价

#### Scenario: fs trait 暴露面不含排序

- **WHEN** 检查 `FileSystemProvider` 方法签名
- **THEN** SHALL NOT 含任何接受 `SortBy` / `Order` / `Cursor` / `Offset` 类参数的方法
- **AND** 调用方按 mtime 排序拿前 N 时 SHALL 自己在调用方代码内排序，不让 trait 帮排

### Requirement: Encode absolute paths into directory names

系统 SHALL 在 `cdt-discover::path_decoder` 中暴露唯一的规范函数 `encode_path(absolute_path: &str) -> String`，把任意绝对路径转为 `~/.claude/projects/` 下的目录名。编码规则 SHALL：

1. 把**所有** `/` **与** `\` 一次替换为 `-`（一遍处理两种分隔符，以兼容 Windows 路径混用情况）。
2. 保留 drive-letter 冒号（例如 `C:`）原样在中间——不转义、不重复——使 Windows 路径与"Decode encoded project paths"中描述的新格式 decoder 形成完整 round-trip。
3. 确保结果以单个 `-` 起首；若原始输入以 `/` 或 `\` 起首（替换后已为 `-...`），则不再前缀；否则 SHALL 前缀一个 `-`。

该函数 SHALL 是整个 workspace 中路径编码的唯一实现。任何其它需要编码路径的 crate（例如 `cdt-config::claude_md` 算 auto-memory 路径）SHALL `use cdt_discover::path_decoder::encode_path`，**不得**自行复制一份私有版本。这样能让 encode / decode 处于同一模块、同一测试套件下，避免出现像 Windows auto-memory 查找失败那样的分叉。

#### Scenario: POSIX absolute path encoding

- **WHEN** 调用 `encode_path("/Users/alice/code/app")`
- **THEN** 结果 SHALL 为 `-Users-alice-code-app`

#### Scenario: Windows absolute path with backslashes

- **WHEN** 调用 `encode_path("C:\\Users\\alice\\app")`
- **THEN** 结果 SHALL 为 `-C:-Users-alice-app`

#### Scenario: Windows absolute path with forward slashes

- **WHEN** 调用 `encode_path("C:/Users/alice/app")`
- **THEN** 结果 SHALL 同样为 `-C:-Users-alice-app`（与反斜杠形式一致）

#### Scenario: Mixed separators encoding

- **WHEN** 调用 `encode_path("C:\\a/b\\c")`
- **THEN** 结果 SHALL 为 `-C:-a-b-c`

#### Scenario: Round-trip with decode_path for Windows paths

- **WHEN** Windows 路径 `C:/Users/alice/app` 先 encode 再 decode
- **THEN** `decode_path(encode_path("C:/Users/alice/app"))` SHALL 等于 `C:/Users/alice/app`

#### Scenario: Round-trip with decode_path for POSIX paths

- **WHEN** POSIX 路径 `/Users/alice/app` 先 encode 再 decode
- **THEN** `decode_path(encode_path("/Users/alice/app"))` SHALL 等于 `/Users/alice/app`

#### Scenario: Empty input produces empty string

- **WHEN** 调用 `encode_path("")`
- **THEN** 结果 SHALL 为 `""`

### Requirement: Resolve historical Claude worktree directories

系统 SHALL 在扫描历史 / 已删除 Claude Code worktree 会话目录时，从 encoded 目录结构和父 repo session `cwd` 恢复可归组的逻辑 worktree 路径。

当 encoded project 目录名形如 `<repo-encoded>-.claude-worktrees-<worktree-name>`（即 `encode_path("<repo>/.claude/worktrees/<worktree-name>")` 的 canonical 形态；实现可兼容历史 `--claude-worktrees-` 形态），且该目录内 session JSONL 没有可用 `cwd` 时，scanner SHALL 优先读取同级 `<repo-encoded>/` 目录下 session 的 `cwd` 作为父 repo 路径，并把该历史 worktree 的 `Project.path` 设为 `<parent-cwd>/.claude/worktrees/<worktree-name>`。如果父 repo 目录不存在或无可用 `cwd`，scanner MAY fallback 到对 `<repo-encoded>` 的 best-effort decode。

`WorktreeGrouper` 在历史 worktree path 本身无法解析 git identity 时，SHALL 识别 `<parent>/.claude/worktrees/<worktree-name>` 形态并使用 `<parent>` 解析 repo identity，使该历史 worktree 归入父 repo `RepositoryGroup`。无法从历史 worktree path 解析 branch 时，`git_branch` SHALL 保持 `None`，MUST NOT 使用父 repo 当前 branch 伪造。

#### Scenario: 无 cwd 的历史 worktree 从父 repo cwd 恢复路径
- **WHEN** `~/.claude/projects/` 下存在 `<repo-encoded>/`，其 session JSONL 含 `cwd = "/repo-with-hyphen"`
- **AND** 同级存在 `<repo-encoded>-.claude-worktrees-old-feature/`，其 session JSONL 不含 `cwd`
- **THEN** scanner SHALL 输出该历史 worktree `Project.path = "/repo-with-hyphen/.claude/worktrees/old-feature"`
- **AND** SHALL NOT 通过 best-effort decode 把 `repo-with-hyphen` 拆成多级目录

#### Scenario: 已删除历史 worktree 归入父 repo group
- **WHEN** `WorktreeGrouper` 处理一个 path 为 `/repo/.claude/worktrees/old-feature` 的 project
- **AND** 该历史 worktree path 本身无法通过 git 解析 identity
- **AND** `/repo` 能解析出 repo identity
- **THEN** 系统 SHALL 把该 project 归入 `/repo` 对应的 `RepositoryGroup`
- **AND** 该 worktree 的 `is_main_worktree` SHALL 为 false
- **AND** 该 worktree 的 `git_branch` SHALL 为 `None`

### Requirement: Project session enumeration minimizes per-file overhead

Project session enumeration SHALL preserve sorted, paginated results while avoiding unnecessary repeated per-file filesystem metadata work during a single list operation. The implementation MUST keep `total`, `nextCursor`, and descending recency order consistent with the files present in the project directory at scan time.

#### Scenario: Listing many sessions preserves recency order

- **WHEN** a project directory contains many `.jsonl` session files with different modification times
- **THEN** session enumeration returns sessions in descending recency order
- **AND** the order is identical whether the caller requests all sessions at once or consumes them through cursor pagination

#### Scenario: Pagination reports complete directory total

- **WHEN** a caller requests a limited page of sessions from a project directory
- **THEN** the response reports the total number of session files in that directory
- **AND** `nextCursor` points to the next page only when more sessions remain

### Requirement: Runtime project list refresh

项目列表消费者 SHALL 能在收到项目刷新信号后重新扫描当前 Claude root 下的 `projects` 目录并暴露新增项目。新增项目的显示名、路径、session 数与启动时全量扫描结果 MUST 使用同一 `project-discovery` 规则计算。

#### Scenario: Newly added project appears after rescan

- **WHEN** 应用启动后当前 Claude root 的 `projects` 目录下新增一个包含 `.jsonl` 会话的 project 目录
- **AND** 项目列表消费者触发重新扫描
- **THEN** 返回的 project 列表 SHALL 包含该新增 project
- **AND** 该 project 的 displayName、path、sessionCount SHALL 与冷启动扫描结果一致

#### Scenario: Rescan preserves existing project metadata

- **WHEN** 项目列表刷新前已有 N 个 project
- **AND** 新增一个 project 后触发重新扫描
- **THEN** 刷新后的列表 SHALL 包含原 N 个 project 与新增 project
- **AND** 原有 project 的 id SHALL 保持稳定

#### Scenario: Project list refresh after Claude root update

- **WHEN** 用户把当前 Claude root 从默认值更新为 `/data/claude-alt`
- **AND** 项目列表消费者触发重新扫描
- **THEN** 返回的 project 列表 SHALL 来自 `/data/claude-alt/projects/`
- **AND** 默认 `~/.claude/projects/` 中仅存在的 project SHALL NOT 出现在结果中

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

#### Scenario: 跨大小写命中同一 ProjectPathResolver 缓存

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

#### Scenario: cdt-core::Session 不含 cwd_relative_to_repo_root 字段

- **WHEN** grep `cdt-core/src/project.rs::Session` 的字段定义
- **THEN** SHALL 不出现 `cwd_relative_to_repo_root` 字段（该字段仅在 `cdt-core::Worktree` 与 IPC 层 `SessionSummary` 上存在）


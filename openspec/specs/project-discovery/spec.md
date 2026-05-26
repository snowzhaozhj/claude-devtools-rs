# project-discovery Specification

## Purpose

扫描 `~/.claude/projects/` 目录、解码 Claude Code 的 encoded 项目目录名（POSIX / Windows / WSL 多格式）、识别同 git 仓库下的多 worktree、按 `cwd` 把单目录拆分为子项目。本 capability 通过 `FileSystemProvider` trait 抽象 I/O，使 `ssh-remote-context` 可无侵入地接入。
## Requirements
### Requirement: Scan Claude projects directory

系统 SHALL 扫描当前 Claude root 下的 `projects` 根目录，把每个一级子目录视为一个 project。当前 Claude root SHALL 来自 `general.claudeRootPath`；当该字段为 `null` 时，默认 Unix root 为 `~/.claude`、Windows root 为 `%USERPROFILE%\.claude\`，projects 根目录分别为 `~/.claude/projects/` 与 `%USERPROFILE%\.claude\projects\`。

系统 SHALL 按 `HOME` → `USERPROFILE` → `HOMEDRIVE + HOMEPATH` → 平台默认 home 目录的 fallback 链解析用户 home 目录，让 WSL / Git Bash / Cygwin 用户可经 `HOME` 覆写、同时仍能在 Windows 原生 shell 里定位到默认目录。

**SSH 模式下单 project 扫描错误处理**：当 backend 是 SSH 时，scanner 对每个 sub-project 扫描的错误 SHALL 按 `FsError::is_likely_channel_dead()` 元方法分流：

- channel-dead 类错误（连接断开 / transport 死 / broken pipe / connection reset）：scanner SHALL **立即** abort 整轮 scan 返 hard error，让上层触发自愈路径，**不**得 silent skip 凑半成品列表
- 其它类错误（普通单文件 IO / NotFound / 单 project 临时不可读 / 纯 timeout exhausted）：保留现有 warn 级日志 + 跳过该 project 行为

理由：SSH channel-dead 时 silent skip 让用户看到不完整列表 + UI 表现"还在加载"，自愈路径瘫痪；而普通单 project 失败（典型权限 / 单文件损坏）silent skip 让其它 project 仍可见是合理的。

#### Scenario: Empty / missing root directory

- **WHEN** projects 根目录不存在或存在但无任何子目录
- **THEN** 系统 SHALL 返回空 project 列表，不抛错；不存在场景 SHALL 记录 warning

#### Scenario: Multiple project directories present

- **WHEN** projects 根目录含 N 个子目录
- **THEN** 系统 SHALL 返回 N 条 project 条目，每条暴露 decode 后的文件系统路径、显示名、session 数

#### Scenario: Home directory resolution fallback chain

- **WHEN** 应用启动需要解析 home 目录
- **THEN** 系统 SHALL 按 `HOME` → `USERPROFILE` → `HOMEDRIVE + HOMEPATH` → 平台默认的优先级查找
- **AND** 同时设置 `HOME` 与 `USERPROFILE`（典型 WSL / Git Bash on Windows）SHALL 优先用 `HOME`
- **AND** 仅设置 `HOMEDRIVE + HOMEPATH`（Windows 经典 fallback）SHALL 拼接两者得到 home 路径

#### Scenario: Custom Claude root projects directory

- **WHEN** 当前 Claude root 配置为非默认绝对路径
- **THEN** scanner SHALL 扫描配置路径下的 `projects/`
- **AND** SHALL NOT 扫描默认 home 下的 `.claude/projects/`

#### Scenario: Clearing custom Claude root restores default projects directory

- **WHEN** 当前 Claude root 从自定义路径清空为 `null`
- **THEN** scanner SHALL 重新使用默认 home 下 `.claude/projects/`

#### Scenario: SSH channel-dead error aborts full scan instead of silent skip

- **WHEN** active backend 是 SSH，scanner 单 project 扫描返 channel-dead 类错误（典型连接断开）
- **AND** 仍有未扫描的 sub-project 在迭代队列中
- **THEN** scanner SHALL **立即** return Err 跳出整轮 scan
- **AND** SHALL NOT 继续扫描后续 sub-project
- **AND** SHALL error 级日志记录决策
- **AND** 上层 SHALL 把该错误传播到 IPC caller（避免半成品列表误导用户）

#### Scenario: SSH transport-dead exhausted aborts scan

- **WHEN** active backend 是 SSH，scanner 单 project 扫描返 transport-dead 关键字（典型 session closed / eof / broken pipe / connection reset）的暂态耗尽错误
- **THEN** scanner SHALL 识别为 channel-dead → 立即 abort 整轮 scan

#### Scenario: SSH per-project NotFound 仍 silent skip 不 abort

- **WHEN** active backend 是 SSH，scanner 单 project 扫描返 NotFound（典型扫描期间被远端进程删除）
- **THEN** scanner SHALL warn + continue 后续 sub-project
- **AND** 最终返 Ok 含其它扫描成功的 project（缺失被删 project）

#### Scenario: SSH per-project pure timeout exhausted 仍 silent skip 不 abort

- **WHEN** active backend 是 SSH，scanner 单 project 扫描返不含 transport-dead 关键字的暂态耗尽（纯 timeout / eagain）
- **THEN** scanner SHALL 识别为非 channel-dead → warn + continue 保持容错（避免误把远端读盘慢当 channel 死）

### Requirement: Decode encoded project paths

系统 SHALL 把 Claude Code encoded 目录名转回原始文件系统路径。decoder SHALL 按以下顺序识别三种格式：

1. **Legacy Windows format**（驱动器字母 + 双连字符开头，如 `C--Users-alice-app`）SHALL 解码为带冒号的驱动器路径形式（如 `C:/Users/alice/app`）
2. **New Windows format**（去 legacy 后形如 `-C:-Users-alice-app`）SHALL 剥离首个 `-`、把剩余 `-` 替换为 `/`；若结果命中驱动器字母 + 冒号 + 斜杠开头则**原样返回**（不再加 POSIX 前导 `/`）
3. **POSIX format**（如 `-Users-alice-app`）SHALL 剥离首个 `-`、把剩余 `-` 替换为 `/`，并补一个前导 `/` 形成绝对路径

当目标平台为 Windows 时，decoder SHALL 额外做 WSL 挂载点翻译：任何 decode 后命中 `/mnt/<drive_letter>/...` 的路径 SHALL 被改写为驱动器字母 + 冒号 + 路径形式。非 Windows 平台 SHALL 把 WSL 挂载路径原样返回，不改写。

#### Scenario: Standard POSIX encoded name

- **WHEN** project 目录名为标准 POSIX encoded（多段连字符，无驱动器字母）
- **THEN** decode 结果 SHALL 为绝对路径形式（前导 `/` + 段间 `/`）

#### Scenario: Path containing legitimate hyphens

- **WHEN** project 目录名含原本就含连字符的路径段（在多种拆分间存在歧义）
- **THEN** decoder SHALL 返回 best-effort 替换；权威 cwd SHALL 在该 project 目录下的 session 记录 cwd 字段可用时由其恢复

#### Scenario: WSL-style path on non-Windows platforms

- **WHEN** decode 后的路径指向 WSL 挂载（典型 `/mnt/c/...`）且当前平台非 Windows
- **THEN** 系统 SHALL 原样返回该路径，不做平台改写

#### Scenario: Windows new format decodes to drive-letter path

- **WHEN** project 目录名是新 Windows format（首段为 `-<drive>:`）
- **THEN** decode 结果 SHALL 为驱动器字母 + 冒号 + 斜杠 + 余下路径（不带 POSIX 前导 `/`）

#### Scenario: Windows legacy format decodes to drive-letter path

- **WHEN** project 目录名是 legacy Windows format（首段为 `<drive>--`）
- **THEN** decode 结果 SHALL 为驱动器字母 + 冒号 + 斜杠 + 余下路径；驱动器字母 SHALL 强制大写（即使源名为小写）

#### Scenario: WSL mount translation on Windows

- **WHEN** 在 Windows 上运行，decode 结果命中 `/mnt/<drive>/...`
- **THEN** 系统 SHALL 改写为驱动器字母 + 冒号 + 路径形式

#### Scenario: encoded-path 检测接受 legacy Windows format

- **WHEN** 测试 encoded-path 检测器对 legacy Windows format 输入
- **THEN** 结果 SHALL 为 true；命中"驱动器字母 + `--` + 路径段"模式的输入同样如此

### Requirement: List sessions per project

系统 SHALL 列出指定 project 目录下所有 `*.jsonl` session 文件，每条返回 session id（去扩展名的 basename）、最后修改时间、文件大小。

#### Scenario: Project with multiple sessions
- **WHEN** project 目录含 5 个 `.jsonl` 文件
- **THEN** session 列表 SHALL 含 5 条，按最后修改时间倒序

#### Scenario: Project with non-jsonl files
- **WHEN** project 目录含 `.jsonl` 与其它类型文件混合
- **THEN** session 列表 SHALL 仅含 `.jsonl` 文件

### Requirement: Group projects by git worktree

系统 SHALL 把同一 git 仓库的多个 worktree 对应的 project 目录归为一个逻辑仓库条目，同时把每个 worktree 保留为该条目的独立成员；MUST 区分"主 working tree 根"与"主 working tree 子目录"两种 walk-up 都能到达同一 git 元数据的情况，避免子目录 cwd 被误标为独立的 main worktree。

仓库分组 SHALL 通过纯 fs 路径解析（**0 个 git 子进程**）：向上 walk 找到 `.git` 条目，目录 → main worktree（`common_dir = git_dir`）；文件（gitlink）→ 解析 `gitdir:` 行后看 `commondir` 文件区分 linked worktree（用 commondir）vs submodule（common = gitdir）。`identity` 取 canonical common_dir 字符串、name 取其父目录文件名、git_branch 解析 `HEAD` 文件。整个解析路径 MUST 不 spawn 任何 git 子进程（性能改造的成果）。

聚合结果（仓库分组）MUST 含 `id`（稳定的 repo id，通常是 git common-dir 的绝对路径）/ `identity`（无 git 时为 `None`）/ `name`（展示名）/ `worktrees` / `most_recent_session` / `total_sessions` 字段。

每个 worktree MUST 含 `id` / `path` / `name` / `git_branch` / `is_main_worktree`（语义：common-dir 是主 git 元数据而非 linked worktree gitdir）/ `is_repo_root`（语义：path 自身就是主 working tree 的根目录，**仅当** start path 等于 repo 根且 repo 根存在 `.git` 目录时为 `true`；子目录 cwd 即便 walk-up 到主 git 元数据也 SHALL 为 `false`）/ `cwd_relative_to_repo_root`（repo 根本身为 `None`，子目录为相对路径，无法计算 repo 根时为 `None`；计算 SHALL 是纯字符串前缀剥离，**0 额外 syscall**）/ `sessions` / `created_at` / `most_recent_session` 字段。

worktree 排序 SHALL 按 `is_repo_root` 优先（repo 根排前）、再按 `is_main_worktree` 优先（主 common-dir 排前）、再按 `most_recent_session` 倒序（活跃 worktree 排前）。group 排序 SHALL 按 `most_recent_session` 倒序。

#### Scenario: Two worktrees of one repo

- **WHEN** 两个 project 路径分别落在同一仓库的两个 worktree（共享同一 git common dir）
- **THEN** 系统 SHALL 输出一个仓库分组，含两个 worktree 成员

#### Scenario: Standalone project not in a worktree

- **WHEN** 一个 project 路径无 git 元数据
- **THEN** 系统 SHALL 把它输出为只含自己的单成员分组，identity 字段 SHALL 为 `None`

#### Scenario: Main worktree 排在附加 worktree 之前

- **WHEN** 一个 group 内含主 worktree 与附加 worktree，附加 worktree 的 `most_recent_session` 更新
- **THEN** group 第一项 SHALL 为主 worktree（`is_main_worktree=true`），附加 worktree 排在后面（main 优先级压过时间）

#### Scenario: Group 排序按最近活动倒序

- **WHEN** 两个独立 repo group 的最近 session 时间不同
- **THEN** 返回数组 SHALL 含活动更晚者在前

#### Scenario: 主仓子目录 cwd 不被误标为 repo root

- **WHEN** 主 repo 含 `.git` 目录；另存在 project 路径是其子目录（用户在主仓子目录 cwd 跑 claude 产生的独立 encoded 目录）
- **THEN** grouper SHALL 把两者归到同一 group
- **AND** repo 根对应 worktree 的 `is_repo_root = true`、`is_main_worktree = true`、`cwd_relative_to_repo_root = None`
- **AND** 子目录对应 worktree 的 `is_repo_root = false`、`cwd_relative_to_repo_root = Some(<相对路径>)`
- **AND** 排序后 repo 根 SHALL 排在子目录之前

#### Scenario: linked worktree cwd 含 cwd_relative_to_repo_root

- **WHEN** 主 repo 在某子目录创建 linked worktree（已 prune 或仍在），有对应 encoded project
- **THEN** 对应 worktree 的 `is_repo_root = false`、`is_main_worktree = false`
- **AND** `cwd_relative_to_repo_root` SHALL 为相对路径形式（非 None）

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

### Requirement: Encode absolute paths into directory names

系统 SHALL 暴露唯一的规范函数把任意绝对路径转为 `~/.claude/projects/` 下的目录名。编码规则 SHALL：

1. 把**所有** `/` **与** `\` 一次替换为 `-`（一遍处理两种分隔符，以兼容 Windows 路径混用情况）
2. 保留驱动器字母冒号原样在中间（不转义、不重复）——使 Windows 路径与 decode 形成完整 round-trip
3. 确保结果以单个 `-` 起首：原始输入以分隔符起首时不再前缀；否则 SHALL 前缀一个 `-`

该函数 SHALL 是整个 workspace 中路径编码的唯一实现。任何其它需要编码路径的 crate（典型 auto-memory 路径计算）SHALL import 该函数，**不得**自行复制一份私有版本。

#### Scenario: POSIX absolute path encoding

- **WHEN** 编码 POSIX 绝对路径
- **THEN** 结果 SHALL 是首段 `-` + 段间 `-` 的目录名形态

#### Scenario: Windows absolute path encoding

- **WHEN** 编码 Windows 绝对路径（含驱动器字母 + 反斜杠 / 正斜杠 / 混合分隔符）
- **THEN** 反斜杠 / 正斜杠 SHALL 被一次替换为 `-`
- **AND** 驱动器字母后冒号 SHALL 原样保留
- **AND** 不同分隔符形态的等价路径 SHALL 编码为相同结果

#### Scenario: Round-trip with decode

- **WHEN** 任意绝对路径先 encode 再 decode
- **THEN** SHALL 等于原路径

#### Scenario: Empty input produces empty string

- **WHEN** 编码空字符串
- **THEN** 结果 SHALL 为空字符串

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

### Requirement: `extract_session_cwd` 仅读首行的不变量

session JSONL `cwd` 抽取算法 SHALL 在 JSONL 首行（第 1 行）即命中 `cwd` 字段并返回；MUST NOT 走"读整文件兜底"分支当首行已含 cwd。

**为何此不变量重要**：依赖此前提的失效语义包括 project scan cache（已知 session 的 JSONL 追加 SHALL NOT 改变 `cwd` 抽取结果）。若未来 claude-code 引入"先建空 jsonl 再补 cwd"或"cwd 在中后段"的格式，本不变量会被破坏，需要先在此 capability 重新评估抽取语义并对应调整下游 cache 失效粒度。

**测试断言机制**：测试 SHALL 用 fs op counter 包裹 `cwd` 抽取调用并对其返回的 op 计数 snapshot 做断言；不能仅靠返回值（cwd）断言（cwd 正确不代表未走兜底，可能首行 + 兜底都命中得到同一 cwd）。测试构造 fs handle 时 MUST 包 instrumentation wrapper，否则 counter 不计数。

#### Scenario: 首行含 cwd 时 SHALL 不触发整文件 fallback

- **WHEN** 测试构造一个多行 session JSONL：第 1 行为含合法 `cwd` 的 user message JSON；其余行为不含 `cwd` 的 assistant message
- **AND** 测试构造 fs handle 包 instrumentation wrapper 并据此构造 scanner
- **AND** 测试用 fs op counter 入口包住 `cwd` 抽取调用
- **THEN** 抽取结果 cwd MUST 等于首行字面量
- **AND** counter snapshot 的 read_to_string 计数 MUST == 0（兜底分支未触发）

#### Scenario: 已有首行 cwd 时 JSONL 后续追加 SHALL NOT 改变抽取结果

- **WHEN** 测试构造仅含 1 行 user message + cwd 的 JSONL，scanner fs 同上 wrapper
- **AND** 调 cwd 抽取拿到 R1 + counts1
- **AND** 在该 JSONL 末尾追加若干不含 cwd 的 assistant message
- **AND** 再次调用 cwd 抽取拿到 R2 + counts2
- **THEN** R1 MUST == R2
- **AND** counts1 与 counts2 的 read_to_string 计数 MUST 都 == 0

### Requirement: Cached snapshot SHALL 反映已知 session 普通 append 推进的 most_recent_session

`Project.most_recent_session` 字段对外承诺反映该 project 下所有 jsonl session 的最新 mtime（毫秒 since UNIX epoch）。当上层经 cache 命中路径返回 `Project` / `RepositoryGroup` 时（典型：`list_projects` / `list_repository_groups` 返回的 `RepositoryGroup.most_recent_session`），系统 SHALL 在用户感知时长内（一次正常 file-change 事件投递时延加合成开销，详 `[[file-watching]]::事件投递时延、远端 polling 频率与停止时延`）让该字段反映自上次 cache 写入以来 watcher 观测到的最新 jsonl mtime。

不变量：

- 已知 session 普通 append（不改变 sessions 集合 / cwd / topology）SHALL NOT 触发 `ProjectScanner::scan()` 重扫——仅推进 `Project.most_recent_session` 显示值
- 已知 session 普通 append SHALL NOT 改变 `Project.sessions` / `Project.distinct_cwds` / `Project.path` / `Project.created_at` 等其它字段
- 用户在 dashboard 项目卡片上看到的"最近活动"时间 SHALL 与 sidebar 当前打开会话的 modified 时间在同一文件追加事件后保持视觉一致（差异 < 一次 debounce 窗口 + 一次合成开销）
- 按 `most_recent_session` 倒序的项目排序 SHALL 反映最新的 mtime——同一组数据下 dashboard 卡片排序与 sidebar 切项目时的 group 排序应一致

SSH context 下，上述用户感知时长以 `[[file-watching]]::事件投递时延、远端 polling 频率与停止时延` 定义的远端 polling 节拍为上界（默认 3 秒，catch-up 30 秒）；两次 poll 之间发生的 append 允许短暂显示上一轮 mtime——这是 SSH 远端无 OS 通知机制的物理上界，本 capability 接受为 limitation。

实现路径（不进 spec 的具体合成机制）由 `[[ipc-data-api]]::ProjectScanCache 维护 per-project mtime overlay 让 cache 命中路径返回新鲜 mtime` Requirement 单独承担——本 Requirement 仅定义用户视角的契约。

#### Scenario: 已知 session 持续追加后 dashboard 项目卡片的 mostRecentSession 跟随推进

- **WHEN** `list_repository_groups` 在 `t0` 时刻被首次调用、写入 cache，返回的 `RepositoryGroup.most_recent_session` 为 `t0_max`
- **AND** 同 project 下某已知 session jsonl 在 `t1 > t0` 时刻被追加，watcher 投递对应 file-change 事件
- **AND** 调用方在 `t2 > t1`（`t2 - t0 < cache TTL`）时再次调用 `list_repository_groups`
- **THEN** 返回的 `RepositoryGroup.most_recent_session` SHALL ≥ `t1`（反映追加事件的 mtime）
- **AND** SHALL NOT 仍为旧的 `t0_max`

#### Scenario: 已知 session 普通追加不改变 sessions 集合

- **WHEN** project `pa` 的 cached snapshot 含 `sessions = ["sa", "sb"]`，已知 session `sa` 被追加内容
- **THEN** 紧接着的 `list_projects` cache hit 路径返回的 `Project { id: "pa", sessions, ... }` SHALL 仍含且仅含 `["sa", "sb"]`
- **AND** `Project.most_recent_session` SHALL 反映 `sa` 追加后的 mtime
- **AND** `Project.distinct_cwds` 与 `Project.created_at` 字段 SHALL NOT 变化

#### Scenario: dashboard 卡片排序按最新活动倒序

- **WHEN** 两个 project `pa` / `pb` 在 cache 写入时刻分别有 `most_recent_session = t_a < t_b`
- **AND** `pa` 后续被持续追加内容，watcher 推进对应 mtime 至 `t_a' > t_b`
- **AND** 调用方此时调 `list_repository_groups`
- **THEN** 返回数组排序 SHALL 把 `pa` 对应 group 排在 `pb` 之前（反映 `pa` 当前最新 mtime 已超过 `pb`）

#### Scenario: 新 session 首次出现仍走结构性 invalidate 路径

- **WHEN** project `pa` 下首次出现新 session `sc.jsonl`（cache snapshot 不含 `sc`）
- **THEN** 对应 file-change 事件 SHALL 被判定为结构性（unknown_session 命中）
- **AND** `ProjectScanCache` SHALL 走 invalidate + 下次 scan 重新拿到含 `sc` 的 fresh snapshot——**不**通过 mtime overlay 路径"假装"看到 sessions 列表更新

#### Scenario: 删除 session 仍走结构性 invalidate 路径

- **WHEN** project `pa` 下已知 session `sa.jsonl` 被删除
- **THEN** 对应 file-change 事件 `deleted=true` SHALL 命中三档第一档
- **AND** `ProjectScanCache` SHALL invalidate + 重扫拿到不含 `sa` 的 fresh snapshot——**不**依赖 overlay

### Requirement: Resolve project id from session id alone

`DataApi` trait SHALL 暴露 `find_session_project(session_id: &str) -> Result<Option<String>, ApiError>`，让仅持有 `session_id` 的调用方反查所属 `project_id`。HTTP `GET /api/sessions/:id` 与 trait 内 `get_sessions_by_ids` MUST 走该方法配合 `get_session_detail(project_id, session_id)` 的复合路径，**不**得直接调 `get_session_detail("", session_id)`。

trait 默认实现 SHALL 遍历 `list_projects()` 取每个 `project_id`，依次调 `list_sessions_sync(project_id, { page_size: usize::MAX, cursor: None })`，命中第一个含 `session_id` 的项目立即返回 `Ok(Some(project_id))`；遍历完无命中返 `Ok(None)`。**主会话**（`<projects_dir>/<encoded>/<session_id>.jsonl`）必然能被默认实现命中；subagent jsonl 是否被命中 SHALL 视具体实现的覆盖能力而定（默认实现不强制覆盖）。

`LocalDataApi` SHALL 覆盖默认实现，直接 `read_dir(scanner.projects_dir())` 扫每个 project 子目录，按以下顺序匹配（命中即返回 `Ok(Some(<encoded_project_id>))`）：

1. **主会话快路径**：`<project_dir>/<session_id>.jsonl` 存在。
2. **legacy subagent**：`<project_dir>/agent-<session_id>.jsonl` 存在。
3. **新结构 subagent**：`<project_dir>/<parent>/subagents/agent-<session_id>.jsonl` 存在（任一 parent）。

实现 SHALL 复用既有 `find_subagent_jsonl` helper，与 `LocalDataApi::get_session_detail` 的查找口径完全一致——避免出现"`find_session_project` 命中但 `get_session_detail` 又取不到"的不一致状态。

#### Scenario: 默认实现命中主会话
- **WHEN** 调用方对一个 mock `DataApi` 调 `find_session_project("sid-A")`，`sid-A` 是项目 `proj-1` 下的主会话
- **AND** mock 实现走 trait 默认 `list_projects` + `list_sessions_sync` 路径
- **THEN** 返回 SHALL 为 `Ok(Some("proj-1"))`

#### Scenario: 默认实现找不到时返 None
- **WHEN** 调用方对 mock `DataApi` 调 `find_session_project("sid-ghost")`，所有 project 的 `list_sessions_sync` 都不含该 id
- **THEN** 返回 SHALL 为 `Ok(None)`

#### Scenario: LocalDataApi 直扫 FS 命中主会话
- **WHEN** tmpdir 下构造 `LocalDataApi`，写入 `<projects_dir>/<encoded-A>/sid-1.jsonl`
- **AND** 调用方调 `find_session_project("sid-1")`
- **THEN** 返回 SHALL 为 `Ok(Some("<encoded-A>"))`

#### Scenario: LocalDataApi 命中 subagent jsonl
- **WHEN** tmpdir 下构造 `LocalDataApi`，写入 `<projects_dir>/<encoded-B>/parent/subagents/agent-sid-2.jsonl`
- **AND** 调用方调 `find_session_project("sid-2")`
- **THEN** 返回 SHALL 为 `Ok(Some("<encoded-B>"))`

#### Scenario: LocalDataApi 多 project 命中第一个
- **WHEN** tmpdir 下两个 project 目录都不含目标 sid，第三个含 `sid-3.jsonl`
- **AND** 调用方调 `find_session_project("sid-3")`
- **THEN** 返回 SHALL 为 `Ok(Some("<encoded-的第三个>"))`，不报错且只命中一次

#### Scenario: LocalDataApi 找不到时返 None 不报错
- **WHEN** tmpdir 下所有 project 目录都不含目标 sid
- **AND** 调用方调 `find_session_project("sid-ghost")`
- **THEN** 返回 SHALL 为 `Ok(None)`（**不**得返回 `Err`、**不**得 panic）

#### Scenario: 与 get_session_detail 口径一致
- **WHEN** `find_session_project(sid)` 返回 `Ok(Some(pid))`
- **THEN** 紧接着调 `get_session_detail(pid, sid)` SHALL 成功返回 `SessionDetail`（不**得**返回 `not_found`）；反之，`Ok(None)` 时 `get_session_detail` 任意 `project_id` 调用 SHALL 都返回 `not_found`

### Requirement: Expose git branch on session summary and metadata updates

`SessionSummary` 与 `SessionMetadataUpdate` SHALL 在已有字段集（`sessionId` / `projectId` / `timestamp` / `title` / `messageCount` / `isOngoing`）之外**额外**携带 `git_branch: Option<String>` 字段（IPC 序列化时为 camelCase `gitBranch`）。骨架返回（`list_sessions` 同步阶段）SHALL 为 `None`，真实值由后端异步元数据扫描在 `LocalDataApi::list_sessions` 后台 JoinSet 任务内填充并通过 `session-metadata-update` 事件 push 到前端。

后端取值规则：解析 session JSONL 时 SHALL 遍历 `cdt_parse::ParsedMessage.message.git_branch`，记录**最后一条** `Some(...)` 作为最终值（与原版 `claude-devtools/src/renderer/utils/sessionExporter.ts` 取值方式一致——反映会话最后所在的 git 分支）。session 中所有行的 `git_branch` 都为 `None`（非 git 仓库）时 SHALL 保持 `None`。

`cdt-api/tests/ipc_contract.rs` SHALL 加断言验证 `SessionSummary` 与 `SessionMetadataUpdate` 序列化结果含 `gitBranch` camelCase 字段，与 `messageCount` 等同位。

#### Scenario: list_sessions skeleton has gitBranch null

- **WHEN** caller 调用 `list_sessions("p")`
- **THEN** 同步返回的每个 `SessionSummary` SHALL 含字段 `gitBranch`（值为 `null`，因尚未异步扫描）

#### Scenario: session-metadata-update payload contains gitBranch

- **WHEN** 后端后台扫描某个 session 完毕，最后一行 `git_branch` 为 `Some("feat/foo")`
- **AND** 该 session 通过 `session-metadata-update` 推送
- **THEN** event payload SHALL 含 `gitBranch: "feat/foo"`（camelCase）

#### Scenario: session without any git_branch line

- **WHEN** 后端扫描 session 所有行 `git_branch` 均为 `None`（非 git 项目）
- **AND** 该 session 通过 `session-metadata-update` 推送
- **THEN** event payload `gitBranch` SHALL 为 `null`

#### Scenario: backend takes last non-empty git_branch

- **WHEN** session 内消息行 `git_branch` 序列依次为 `Some("main")` / `None` / `Some("feat/x")` / `Some("feat/y")` / `None`
- **THEN** 该 session 元数据推送的 `gitBranch` SHALL 为 `"feat/y"`（最后一条非空）

#### Scenario: contract test asserts camelCase serialization

- **WHEN** `cargo test -p cdt-api --test ipc_contract` 执行
- **THEN** 断言 `SessionSummary { git_branch: Some("main"), ... }` 序列化为 JSON 后 SHALL 含字段名 `"gitBranch"`，且 `SessionMetadataUpdate` 同样

### Requirement: Expose repository group queries

系统 SHALL 暴露 `list_repository_groups()` IPC：把 `ProjectScanner::scan()` 结果通过 `WorktreeGrouper::group_by_repository` 聚合为 `Vec<RepositoryGroup>`，每个 group 含 `id` / `identity` / `name` / `worktrees[]` / `mostRecentSession` / `totalSessions` 字段。Worktree 排序 SHALL 按 `is_main_worktree` 优先、再按 `most_recent_session` 倒序（已在 `WorktreeGrouper` 内部实现）。Group 排序 SHALL 按 `mostRecentSession` 倒序。

序列化 SHALL 使用 camelCase（`isMainWorktree`、`gitBranch`、`mostRecentSession`、`totalSessions`、`createdAt`）。

#### Scenario: 列出多 worktree 仓库分组
- **WHEN** 同一 git 仓库下存在主 worktree 与一个用户开的附加 worktree，且两者都有 sessions
- **THEN** `list_repository_groups()` SHALL 返回一个 group，`worktrees` 数组含两项，`worktrees[0].isMainWorktree=true`、`worktrees[1].isMainWorktree=false`

#### Scenario: 独立项目作为单成员分组
- **WHEN** 一个 project 路径无 git 元数据（不属任何 worktree）
- **THEN** `list_repository_groups()` SHALL 返回一个 group，`worktrees` 数组含该项目一项，`identity` 为 `null`

#### Scenario: 序列化 camelCase
- **WHEN** `list_repository_groups()` 返回结果被序列化为 JSON
- **THEN** 字段名 SHALL 为 `isMainWorktree` / `gitBranch` / `mostRecentSession` / `totalSessions` / `createdAt`（不是 snake_case）

### Requirement: Expose worktree sessions query

系统 SHALL 实现 `get_worktree_sessions(group_id, pagination)` IPC：定位 `group_id` 对应 `RepositoryGroup`，把该 group 下所有 worktree 的 sessions 合并为单一列表，按 `timestamp` 倒序后再应用 `PaginatedRequest`（`pageSize` + `cursor`）。返回 `PaginatedResponse<SessionSummary>`，每个条目 SHALL 额外携带 `worktreeId` / `worktreeName` 字段以便 UI 标注归属。

`pageSize == 0` 时 SHALL 立即拒绝（`ApiError::validation`），`pageSize` 不再被静默 clamp 为 1，避免隐藏调用方错误参数。

未命中 `group_id` 时 SHALL 拒绝（`ApiError::not_found`）。

错误形态遵循既有项目约定：trait / HTTP 层产 `ApiError { code, message }` 结构化错误；Tauri command wrapper 沿用 `Result<_, String>` —— 把 `ApiError` 通过 `to_string()` 序列化为含错误前缀的人类可读字符串（与 `list_sessions` / `get_session_detail` 等既有 command 一致），结构化 `code` 字段仅在 HTTP `axum::IntoResponse` 路径暴露。

Tauri command 入参 SHALL 与既有 `list_sessions` 风格一致——顶层 `groupId: string` + `pageSize?: number` + `cursor?: string`，**不**嵌套 `pagination` 对象（保持 IPC 调用形态在所有 paginated command 间一致）。HTTP 路径走 `GET /api/worktrees/{groupId}/sessions?pageSize=...&cursor=...` query string。

#### Scenario: 合并多 worktree sessions 按时间排序
- **WHEN** caller 调用 `invoke("get_worktree_sessions", { groupId: "repo-1", pageSize: 10 })`，repo-1 含两个 worktree 各 5 个 session
- **THEN** 响应 `items` SHALL 含 10 项，按 `timestamp` 倒序排列
- **AND** 每项 SHALL 含 `worktreeId` / `worktreeName` 字段

#### Scenario: 分页继续
- **WHEN** caller 接上一页 `nextCursor` 再调 `invoke("get_worktree_sessions", { groupId, pageSize, cursor: nextCursor })`
- **THEN** 响应 SHALL 返回剩余 sessions，不重复返回上一页内容

#### Scenario: pageSize 为 0 时拒绝
- **WHEN** caller 调用 `invoke("get_worktree_sessions", { groupId: "g1", pageSize: 0 })`
- **THEN** trait 层 SHALL 立刻返 `ApiError::validation(...)`，message 含 `pageSize must be > 0`
- **AND** Tauri command wrapper 把 ApiError 字符串化后让 `invoke` Promise reject 含该 message；HTTP 层走 `IntoResponse` 返 400 + `{code: "validation_error", message}` 结构化 JSON
- **AND** SHALL NOT 静默 clamp 为 1 也 SHALL NOT 返回部分结果

#### Scenario: group_id 不存在
- **WHEN** caller 调用 `invoke("get_worktree_sessions", { groupId: "nonexistent-group", pageSize: 10 })`
- **THEN** trait 层 SHALL 返 `ApiError::not_found(...)`，message 含 group id 标识符
- **AND** Tauri command wrapper 把 ApiError 字符串化后让 `invoke` Promise reject；HTTP 层走 `IntoResponse` 返 404 + `{code: "not_found", message}` 结构化 JSON

### Requirement: Tauri commands for repository groups and worktree sessions

系统 SHALL 通过 Tauri `invoke_handler!` 注册 `list_repository_groups` 与 `get_worktree_sessions` 两个 IPC command，参数与返回类型 SHALL 与上述 IPC trait 方法一致。两个 command 名 SHALL 同步出现在 `crates/cdt-api/tests/ipc_contract.rs::EXPECTED_TAURI_COMMANDS` 与 `ui/src/lib/tauriMock.ts::KNOWN_TAURI_COMMANDS` 两处常量列表中。

#### Scenario: invoke list_repository_groups 返回 camelCase 数组
- **WHEN** 前端调用 `invoke("list_repository_groups")`
- **THEN** 响应 SHALL 为 JSON 数组，每项含 `id` / `identity` / `name` / `worktrees` / `mostRecentSession` / `totalSessions` 字段（camelCase）

#### Scenario: invoke get_worktree_sessions 返回 PaginatedResponse
- **WHEN** 前端调用 `invoke("get_worktree_sessions", { groupId: "g1", pageSize: 20, cursor: null })`（顶层 `pageSize` / `cursor` 与既有 `list_sessions` 一致，不嵌套 `pagination`）
- **THEN** 响应 SHALL 为 `{ items: SessionSummary[], nextCursor: string | null, total: number }` 形态

### Requirement: Expose group session listing via k-way merge pagination

系统 SHALL 实现 `list_group_sessions(group_id, page_size, cursor)` IPC：定位 `group_id` 对应 `RepositoryGroup`，对 group 内 N 个 worktree 各自的 sessions（已在 `WorktreeGrouper` / `ProjectScanner` 层按 `mtime` 倒序）做 **k-way merge 流式分页**，返回 `GroupSessionPage { sessions: Vec<SessionSummary>, next_cursor: Option<String> }`。

实现 MUST 满足：
- **Server 无状态**：cursor 自描述每个 worktree 当前指针位置（`BTreeMap<worktree_id, WorktreeOffset>`，`WorktreeOffset` 枚举为 `NotStarted` / `AfterMtime { mtime_ms, sid }` / `Exhausted`），序列化为 base64(JSON)，重启服务后仍可继续分页
- **全序定义**：全局排序方向为 `(mtime_ms desc, sid asc)`——`mtime_ms` 大的排前，同 `mtime_ms` 时 `sid` 字典序小的排前
- **k-way merge**：内部用 `BinaryHeap<HeapEntry { mtime_ms, sid, worktree_id, idx }>`，`Ord` 实现按全序"排前者优先 pop"（max-heap 视角：`mtime_ms` 大 / 同 mtime 时 `sid` 小为"大"），取 `page_size` 条；每次 pop 后把对应 worktree 的下一条 push 回堆
- **续页定位**：cursor `AfterMtime { mtime_ms: last_mtime, sid: last_sid }` 表示"已消费到 `(last_mtime, last_sid)` 这条"；续页时对每个 worktree 二分定位 SHALL 找到第一条**严格在 `(last_mtime, last_sid)` 之后**的 session，即满足 `(s.mtime_ms < last_mtime) || (s.mtime_ms == last_mtime && s.sid > last_sid)` 的最早条目；MUST NOT 重复返回 `(last_mtime, last_sid)` 自身，MUST NOT 漏掉同 mtime 但 sid 更大的条目
- **不全量收集**：MUST NOT 在产出当前页前把 group 所有 sessions 全部 collect 到 `Vec`（避免 RSS 击穿）；MUST NOT 对每个 worktree 调 `list_sessions_sync(page_size = usize::MAX)` 复用全量路径
- **共享并发限流**：内部并发跑 `ProjectScanner::scan_project_dir` SHALL 使用 `LocalDataApi` 持有的共享 `Arc<Semaphore>`（见 `ProjectScanner shared read semaphore injection`），不得为每个 worktree 新建独立 semaphore
- **页面 SSE detail 触发**：返回页骨架后，SHALL fire-and-forget 触发 `session-metadata-update` 后台拉取，**仅**对当前页 sessions（key on `(project_id /*worktree id*/, session_id)`，复用现有 detail 拉取 active_scans 键空间），借 `active_scans` per-key cancel 在切页 / 切 group / 切 worktree filter 时取消旧拉取
- **worktree filter 通过 cursor 表达**：前端切 worktree filter 为某 worktree `wt-X` 时 SHALL 构造初始 cursor，让所有非 X 的 worktree `WorktreeOffset = Exhausted`，k-way merge 自然只产出 X 的 sessions（server 不感知 filter，纯 cursor 语义复用）
- **(groups, fs, ctx, captured_generation) 同源快照**：`build_group_session_page` 实现 SHALL 通过单一内部 helper（`list_repository_groups_inner`）一次原子调用拿 `(groups, fs, projects_dir, ctx, captured_generation)` 五元组，MUST NOT 各自独立 `await` `list_repository_groups()` 与 `active_fs_and_context_strict()` 两次抽样。理由：两次独立 await 之间可被 `switch_context` / `ssh_connect` / `ssh_disconnect` / `reconfigure_claude_root` 跨过 → 拿到 (OLD ctx 的 groups, NEW ctx 的 fs/ctx) 拼接 → 用 OLD worktree_id 在 NEW fs 上 scan 返空页（用户可观察的 "切换后立刻看 group 是空的"）。inner 内部 scan + grouper 自身仍可被 ssh switch 跨过，但 caller 拿到的五元组保持 self-consistent（要么全 OLD 要么全 NEW），下游 scan 基于同一 fs/ctx，不会出现混合态。
- **后台 metadata scan task spawn 前二次校验**：`build_group_session_page` 在 page 骨架组装完成后、spawn `scan_metadata_for_page` 后台 task **之前** SHALL 短暂获取 `ssh_watcher_ops: Mutex<()>` 锁，并在锁内做 (current_ctx == captured_ctx) **AND** (current_generation == captured_generation) 双重校验：
  - **匹配** → 在锁内完成所有 `tokio::spawn(scan_metadata_for_page(...))` + active_scans.insert，然后释放锁；spawn 过的 task 自身仍按既有约束在 broadcast 前校验 `expected_context_generation` 不变
  - **任一 mismatch** → SHALL 返 `GroupSessionPage` 骨架但 SHALL NOT spawn 任何 metadata scan task；SHALL 在 `tracing` 写 `debug` 留痕
  - 理由：bump-first 顺序使得 inner 拿到的 captured_generation 可能等于 ssh_mgr.switch_context 完成后的 current_generation（同值都为 bumped 后值），此时单 generation 校验会让 task spawn 后 broadcast 校验 `current == expected` 误判为"context 没变"，向新 ctx UI 发旧 ctx update。spawn 前在锁内识别 ctx 变化结构性闭合该 sub-window；spawn 在锁内进行确保 spawn 期间没有 mutate 跑（switch / connect 等也持同锁）。

错误形态：
- `page_size == 0` SHALL 立刻返 `ApiError::validation`，message 含 `pageSize must be > 0`
- `group_id` 不存在 SHALL 返 `ApiError::not_found`
- cursor 反序列化失败 SHALL 视为首页请求（fallback 为 `cursor = null`），并在 tracing 写 `warn` 留痕

序列化 SHALL 使用 camelCase（`pageSize` / `nextCursor` / `worktreeId` / `worktreeName` / `cwdRelativeToRepoRoot`）。

#### Scenario: 首页请求返回 page_size 条按全局 mtime 倒序

- **WHEN** caller 调用 `invoke("list_group_sessions", { groupId: "g1", pageSize: 50, cursor: null })`，g1 含 2 个 worktree 各 30 个 session（mtime 交错）
- **THEN** 响应 `sessions` SHALL 含 50 条，按 `timestamp` 严格倒序
- **AND** 响应 `nextCursor` SHALL 非空，每个 worktree 的 offset 反映已消费到的最后一条

#### Scenario: 续页请求按 cursor 续位

- **WHEN** caller 接上一页 `nextCursor` 再调 `invoke("list_group_sessions", { groupId, pageSize: 50, cursor })`
- **THEN** 响应 SHALL 返回剩余 sessions，不重复返回上一页内容；保持全局 mtime 倒序

#### Scenario: 所有 worktree 流耗尽时 next_cursor 为 null

- **WHEN** caller 续到最后一页，所有 worktree offset SHALL 为 `Exhausted`
- **THEN** 响应 `nextCursor` SHALL 为 `null`

#### Scenario: 同 mtime session 按 sid 字典序稳定排序

- **WHEN** 两个 worktree 各含一条 `mtime_ms = 1000` 但 `sid` 不同的 session（`sidA` < `sidB`）
- **THEN** 全局排序 SHALL 把 `sidA` 排在 `sidB` 之前
- **AND** cursor 记录的 `AfterMtime { mtime_ms: 1000, sid: "sidA" }` SHALL 在续页时跳过 sidA 自身但保留 sidB

#### Scenario: 续页定位边界

- **WHEN** worktree W1 的 sessions 按全序为 `[(2000,"a"), (1000,"b"), (1000,"d"), (500,"c")]`，cursor `AfterMtime { mtime_ms: 1000, sid: "b" }`
- **THEN** 续页 SHALL 跳过 `(2000,"a")` 与 `(1000,"b")`，从 `(1000,"d")` 开始返回
- **AND** SHALL NOT 重复返回 `(1000,"b")`（cursor 自身已消费）
- **AND** SHALL NOT 漏掉 `(1000,"d")`（同 mtime 但 sid > "b"）

#### Scenario: worktree filter via cursor Exhausted

- **WHEN** caller 构造 cursor `{ "wt-X": NotStarted, "wt-other-1": Exhausted, "wt-other-2": Exhausted }` 调 `list_group_sessions`
- **THEN** 响应 sessions SHALL 仅含 `wt-X` 的 sessions（按 mtime 倒序）
- **AND** 续页 cursor 中 `wt-other-1` / `wt-other-2` SHALL 仍为 `Exhausted`

#### Scenario: 不全量收集

- **WHEN** group 含 10 个 worktree 各 100 个 session（共 1000 条），caller 请求 `pageSize: 20`
- **THEN** 实现内部 MUST NOT 把 1000 条 session 全部加载到内存再排序分页
- **AND** 单次请求 RSS 增量 SHALL 在 200 KB 量级（骨架字段 × 1000 条）

#### Scenario: pageSize 为 0 时拒绝

- **WHEN** caller 调用 `invoke("list_group_sessions", { groupId: "g1", pageSize: 0 })`
- **THEN** SHALL 立即返 `ApiError::validation`，message 含 `pageSize must be > 0`

#### Scenario: 损坏 cursor fallback 为首页

- **WHEN** caller 调用 `invoke("list_group_sessions", { groupId: "g1", pageSize: 50, cursor: "invalid-base64" })`
- **THEN** 实现 SHALL fallback 为首页请求（等价 `cursor = null`），返回首页内容
- **AND** SHALL 在 tracing 写 `warn` 留痕

#### Scenario: build_group_session_page 用单一 snapshot 不出现 (groups OLD, fs NEW) 拼接

- **WHEN** active context = `Ssh<host_a>` 且 g1 在 host_a 下有 worktrees `[wt-a-1, wt-a-2]`
- **AND** 调用方 task A 触发 `switch_context("local")` 期间
- **AND** 调用方 task B 并发调 `list_group_sessions("g1", 50, None)`
- **THEN** task B 实现内部 SHALL 仅调用一次 `list_repository_groups_inner` 拿五元组（含 captured_generation），**不得**独立再调 `active_fs_and_context_strict`
- **AND** 拿到的 (groups, fs, ctx) SHALL 来自同一原子抽样（要么全 host_a 要么全 Local）
- **AND** 后续 `scanner.list_sessions(wt_id, ...)` 用五元组里的 fs 扫五元组里 groups 内的 worktree_id —— 不会出现"用 host_a 的 wt-a-1 ID 在 Local fs 上 scan 返空"的混合态错乱

#### Scenario: build_group_session_page 在 ctx mismatch 时返页面骨架但跳 metadata scan spawn

- **WHEN** active context = `Ssh<host_a>` 且 g1 在 host_a 下有 worktrees + sessions
- **AND** 调用方 task B 调 `list_group_sessions("g1", 50, None)`，inner 拿到 (host_a 的 groups, fs, ctx, captured_generation = N+1)，page 骨架 sessions 已组装完
- **AND** 调用方 task A 在 task B 拿 `ssh_watcher_ops` 锁之前完成 `switch_context("local")`（ssh_mgr.active 切到 Local，generation 已 bump 到 N+1，post-mutate 不再 bump）
- **THEN** task B 在锁内识别 `current_ctx = Local` ≠ `captured_ctx = Ssh<host_a>` → mismatch
- **AND** task B SHALL 返回 `GroupSessionPage { sessions: page_sessions, next_cursor }` 给 caller（page 骨架内容是 host_a 的真实数据 self-consistent）
- **AND** task B SHALL NOT spawn 任何 `scan_metadata_for_page` task；session_metadata SSE channel SHALL NOT 收到本次调用产出的 update
- **AND** task A 完成切换后用户在 Local 主动调 `list_group_sessions` 时 SHALL 走全新一轮（拿 Local 的 groups + fs + spawn Local 的 scan task）

### Requirement: Tauri command for list_group_sessions

系统 SHALL 通过 Tauri `invoke_handler!` 注册 `list_group_sessions` IPC command，入参顶层 `groupId: string` + `pageSize?: number` + `cursor?: string`（**不**嵌套 `pagination` 对象，与既有 `list_sessions` 保持一致），返回 `GroupSessionPage`。command 名 SHALL 同步出现在 `crates/cdt-api/tests/ipc_contract.rs::EXPECTED_TAURI_COMMANDS` 与 `ui/src/lib/tauriMock.ts::KNOWN_TAURI_COMMANDS` 两处常量列表中。

HTTP 路径 SHALL 走 `GET /api/repository-groups/{groupId}/sessions?pageSize=...&cursor=...` query string。

#### Scenario: invoke list_group_sessions 返回 GroupSessionPage
- **WHEN** 前端调用 `invoke("list_group_sessions", { groupId: "g1", pageSize: 20, cursor: null })`
- **THEN** 响应 SHALL 为 `{ sessions: SessionSummary[], nextCursor: string | null }` 形态（camelCase）

#### Scenario: command 注册在 invoke_handler 与 mock 列表
- **WHEN** ipc_contract 测试遍历 `EXPECTED_TAURI_COMMANDS`
- **THEN** `list_group_sessions` SHALL 在列表内
- **AND** `ui/src/lib/tauriMock.ts::KNOWN_TAURI_COMMANDS` SHALL 含 `list_group_sessions`

### Requirement: SessionSummary 增加 worktree 元信息字段

系统 SHALL 在 `SessionSummary`（IPC 序列化形态）中增加 `worktreeId: String` / `worktreeName: String` / `groupId: String` / `cwdRelativeToRepoRoot: Option<String>` 四个字段：
- `worktreeId` = 该 session 所属 worktree 的 id（等同底层 `Project.id`，encoded project dir 名）
- `worktreeName` = 该 session 所属 worktree 的展示名
- `groupId` = 该 session 所属 `RepositoryGroup.id`（让前端按 group 维度过滤 SSE event / cache key）
- `cwdRelativeToRepoRoot` = 该 session 所属 `Worktree.cwd_relative_to_repo_root`（`None` 时通过 `#[serde(skip_serializing_if = "Option::is_none")]` 省略）

这四个字段 SHALL 同时出现在 `list_sessions` / `list_group_sessions` / `get_worktree_sessions` 三个 IPC 返回的 `SessionSummary` 中，保证 UI 在任一调用路径下都能拿到 worktree / group 归属信息。

**填值来源（scheme c join）**：IPC handler 在序列化 `SessionSummary` 时，从 `LocalDataApi` 持有的轻量 `worktree_id → (worktree_name, group_id, cwd_relative_to_repo_root)` 映射缓存（`worktree_meta_cache`，`HashMap<String, WorktreeMeta>` flat key）查表填入。`cdt-core::Session` SHALL NOT 持有这些字段，避免 scanner 阶段重走 repo 解析。

**映射缓存刷新约束**：

- 映射缓存 MUST 在 `list_repository_groups` 调用过程中按"captured-snapshot safe refresh"模式更新。`list_repository_groups` 实现 SHALL 通过内部 `list_repository_groups_inner()` 拿到 `(groups, fs, projects_dir, captured_ctx, captured_generation)` 同源快照——`captured_generation` SHALL 在 `active_fs_and_policy()` 完成之**后**立即 load `context_generation`，与 (fs, ctx) 同 snapshot；inner 内后续不修改 generation。
- `list_repository_groups` 在调 `refresh_worktree_meta_cache(&groups)` 之前 SHALL 短暂获取 `ssh_watcher_ops: Mutex<()>` 锁，并在锁内做**双重校验**：
  - 比较 `current_ctx`（锁内通过 `ssh_mgr.active_context_id().await` + `ssh_mgr.provider_and_context_id(...).await` 重建 ContextId；Local active 时 fall through 到 `ContextId::local(self.projects_dir.lock().await.clone())`）与 `captured_ctx` 全等
  - 比较 `current_generation = context_generation.load(SeqCst)` 与 `captured_generation` 全等
  - **两条同时匹配** → 在锁保护下 clear-and-rebuild `worktree_meta_cache`
  - **任一 mismatch** → SHALL skip refresh（safe degrade，旧 mapping 保留至下次 IPC 自然刷新）；SHALL 在 `tracing` 写 `debug` 留痕（`captured`/`current` ContextId + 两个 generation 值）；SHALL 仍把 `groups` 返回给 caller（caller 自身消费 groups 不依赖 cache 状态）
- 后续 IPC（含 `list_sessions` / `list_group_sessions` / `get_worktree_sessions`）SHALL 复用同一映射；缓存失效 SHALL 在 grouper 重跑（filesystem 变化触发 refresh）时整体替换。
- 设计动机：`switch_context` / `ssh_connect` / `ssh_disconnect` / `reconfigure_claude_root` / `shutdown_ssh_all` 五个 context 切换入口 **bump-first** 顺序（先 `context_generation.fetch_add(1, SeqCst)` 再 await `ssh_mgr.switch_context/connect/disconnect` 等 mutate）使得 `context_generation` 在 ssh_mgr / projects_dir 状态 mutate 之前就领先；任何并发 `list_repository_groups` 在 inner 内的 generation pre/post snapshot 都可能落在 ① bump 之后 ② mutate 完成之前 的 window 内 —— pre 与 post 同值（仍是 bumped 后值）误判 "context 未切"，把旧 ctx 的 groups 写入 flat-key cache 污染新 ctx 后续查询。**单 ctx-equality 校验**也无法识别"同 host 快速 disconnect+reconnect 期间 ContextId 等价但 generation bumped 两次"边角；**单 generation-equality**无法识别"reconfigure_claude_root 改 Local projects_dir 但 ssh_mgr.active 不变"边角。**(ctx + generation) 双重校验**结构性闭合两类边角：refresh 路径锁内与 5 处 mutate 入口互斥，锁内读到的状态是稳定真相值。

序列化 SHALL 使用 camelCase。

#### Scenario: 映射缓存随 list_repository_groups 刷新

- **WHEN** caller 调 `invoke("list_repository_groups")` 后再调 `invoke("list_group_sessions", { groupId })`
- **THEN** 后者返回的每条 `SessionSummary` SHALL 含 `worktreeId` / `worktreeName` / `groupId` / `cwdRelativeToRepoRoot`（非 None 时）字段
- **AND** 这些字段 SHALL 与 `list_repository_groups` 返回的 group 内对应 worktree 信息一致

#### Scenario: 缓存未填充时 SessionSummary 缺 worktree 字段

- **WHEN** caller 在首次 `list_repository_groups` 之前调用 `list_sessions(projectId, ...)`（理论上不发生，UI 启动顺序保证 list_repository_groups 在前）
- **THEN** 返回的 SessionSummary `worktreeId` SHALL 等于 `projectId`（fallback：worktree id 就是 project id），`groupId` SHALL 等于 `projectId`（fallback：单 worktree group），`cwdRelativeToRepoRoot` SHALL 为 None

#### Scenario: list_sessions 返回 SessionSummary 含 worktree 字段

- **WHEN** caller 调用 `invoke("list_sessions", { projectId, pageSize: 10 })`
- **THEN** 响应 `items[i]` SHALL 含 `worktreeId` / `worktreeName` / `groupId` 字段（对应该 session 所在 Project / Worktree / Group）

#### Scenario: repo 根 session 省略 cwdRelativeToRepoRoot

- **WHEN** session 所属 worktree `is_repo_root = true`
- **THEN** SessionSummary 序列化 SHALL 省略 `cwdRelativeToRepoRoot` 键
- **AND** SHALL 仍含 `worktreeId` / `worktreeName` / `groupId` 字段

#### Scenario: 子目录 session 含 cwdRelativeToRepoRoot

- **WHEN** session 所属 worktree `is_repo_root = false` 且 `cwd_relative_to_repo_root = Some("crates")`
- **THEN** SessionSummary 序列化 SHALL 含 `"cwdRelativeToRepoRoot": "crates"`

#### Scenario: switch_context 期间并发 list_repository_groups 不污染 worktree_meta_cache

- **WHEN** active context = `Ssh<host_a>` 且 `worktree_meta_cache` 已有 host_a 的 worktree mapping
- **AND** 调用方 task A 触发 `switch_context("local")`，进入 `ssh_mgr.switch_context(None).await` 期间（context_generation 已 bump 到 N+1 但 ssh_mgr 状态尚未切完）
- **AND** 调用方 task B 并发调 `list_repository_groups()`，task B 的 `list_repository_groups_inner()` 拿到 captured_ctx = `Ssh<host_a>` + captured_generation = `N+1`
- **THEN** task A 完成后 worktree_meta_cache 的内容 SHALL 仍是切换前 host_a 的 mapping（被 skip 不清空）
- **AND** task B 调 refresh 路径 SHALL 在 `ssh_watcher_ops` 锁内识别 `current_ctx = Local` ≠ `captured_ctx = Ssh<host_a>` → skip refresh
- **AND** SHALL NOT 出现 "host_a 的 mapping 在 Local active 时被 clear-and-rebuild 入 cache" 的错乱状态
- **AND** task B 仍 SHALL 返回它扫到的 host_a groups 给 caller（不报错；caller 消费这一次返回值不依赖 cache 状态）

#### Scenario: 同 host 快速 disconnect+reconnect 期间 generation bump 触发 skip refresh

- **WHEN** active context = `Ssh<host_a>`，`worktree_meta_cache` 已有 host_a mapping
- **AND** 调用方 task B 进入 `list_repository_groups_inner()`，拿到 captured_ctx = `Ssh<host_a>` + captured_generation = `N`
- **AND** 调用方 task A 在 task B inner 完成之后、wrapper 拿锁之前完成 `ssh_disconnect("host_a")`（generation N→N+1）+ `ssh_connect("host_a")` 同 host 重连（generation N+1→N+2），active 重回 `Ssh<host_a>`，新的 SshSessionResources 已就位
- **THEN** task B wrapper 拿锁后 `current_ctx == captured_ctx == Ssh<host_a>`（同 host ContextId 相等）但 `current_generation == N+2` ≠ `captured_generation == N` → mismatch → skip refresh
- **AND** SHALL NOT 把 task B inner 用旧 host_a session 拿到的 groups 写入 `worktree_meta_cache`（避免覆盖新 session 应有的最新 mapping）

#### Scenario: reconfigure_claude_root 改 Local projects_dir 期间 list_repository_groups 不污染 cache

- **WHEN** active context = `Local`，`projects_dir = /old/dir`，`worktree_meta_cache` 已有 /old/dir 的 mapping
- **AND** 调用方 task B 进入 `list_repository_groups_inner()`，拿到 captured_ctx = `Local { projects_dir: /old/dir }` + captured_generation = `N+1`（reconfigure 已 bump-first 到 N+1）
- **AND** 调用方 task A 在 task B inner 完成之后、wrapper 拿锁之前完成 `reconfigure_claude_root(Some("/new/root"))`，projects_dir 切换到 `/new/dir`
- **THEN** task B wrapper 拿锁后 `current_ctx = Local { projects_dir: /new/dir }` ≠ `captured_ctx = Local { projects_dir: /old/dir }` → ctx mismatch → skip refresh
- **AND** SHALL NOT 把 /old/dir 扫到的 groups 写入 cache 污染 /new/dir 后续查询


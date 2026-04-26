# project-discovery Specification

## Purpose

扫描 `~/.claude/projects/` 目录、解码 Claude Code 的 encoded 项目目录名（POSIX / Windows / WSL 多格式）、识别同 git 仓库下的多 worktree、按 `cwd` 把单目录拆分为子项目。本 capability 通过 `FileSystemProvider` trait 抽象 I/O，使 `ssh-remote-context` 可无侵入地接入。

## Requirements

### Requirement: Scan Claude projects directory

系统 SHALL 扫描配置中的 projects 根目录（默认 Unix `~/.claude/projects/`、Windows `%USERPROFILE%\.claude\projects\`），把每个一级子目录视为一个 project。

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

系统 SHALL 把同一 git 仓库的多个 worktree 对应的 project 目录归为一个逻辑仓库条目，同时把每个 worktree 保留为该条目的独立成员。

#### Scenario: Two worktrees of one repo
- **WHEN** 两个 project 路径分别落在同一仓库的两个 worktree（共享同一 `git common dir`）
- **THEN** 系统 SHALL 输出一个仓库分组，含两个 worktree 成员

#### Scenario: Standalone project not in a worktree
- **WHEN** 一个 project 路径无 git 元数据
- **THEN** 系统 SHALL 把它输出为只含自己的单成员分组

### Requirement: Resolve subprojects and pinned sessions

系统 SHALL 把 subproject 关联与用户 pin 的 session 视作配置状态，与扫描得到的 project 一并暴露。

#### Scenario: Pinned session exists
- **WHEN** 一条 session 经配置被 pin
- **THEN** 系统 SHALL 在 session 列表中标记其为 pinned，无视其修改时间

### Requirement: Abstract filesystem access through a provider trait

系统 SHALL 把所有 project / session 的文件 I/O 都走单一的 `FileSystemProvider` trait，使其它后端（例如 SSH 远端）可在不改 ProjectScanner / 路径解析器 / worktree grouper 的前提下接入。该 trait SHALL 至少暴露这些操作：(a) 路径是否存在、(b) 列举目录条目（含 file/dir 类型）、(c) 对路径 stat 取 `size` 与 `mtime`、(d) 把文件全量读为 UTF-8 字符串、(e) 仅读文件前 N 行（不加载其余内容）。

#### Scenario: Local filesystem provider satisfies the scanner

- **WHEN** `ProjectScanner::scan` 配 `LocalFileSystemProvider` 调用
- **THEN** scanner SHALL 仅通过 trait 方法列举 project 与抽取 per-session 元数据，SHALL NOT 直接调任何平台特定文件系统 API

#### Scenario: Path resolver avoids full-file reads in remote mode

- **WHEN** 当前 provider 上报 `kind() == FsKind::Ssh` 且 resolver 需要从 session 文件抽 `cwd`
- **THEN** resolver SHALL 调 `read_lines_head(path, N)` 取足以覆盖首条 user / summary 记录的有限 N 行，SHALL NOT 下载整个文件

#### Scenario: Trait is the sole seam for alternative backends

- **WHEN** 后续某个 port 引入新后端（例如 SSH）
- **THEN** 引入仅 SHALL 要求实现 `FileSystemProvider`，SHALL NOT 要求改 `ProjectScanner` / `ProjectPathResolver` / `WorktreeGrouper`

### Requirement: Represent split subprojects with a stable composite identifier

系统 SHALL 在同一 encoded project 目录下出现两条以上不同 `cwd` 值的 session 时，把该目录拆分为多个逻辑 "subproject"，每个 subproject 由形如 `{baseDir}::{hash8}` 的复合 ID 标识：`baseDir` 为原 encoded 目录名，`hash8` 为该 subproject canonical `cwd` 字符串 SHA-256 摘要前 8 个字符的小写十六进制表示。复合 ID SHALL 是确定性的——同一 `baseDir` + `cwd` 组合 SHALL 始终产生同一 ID。

#### Scenario: Single-cwd directory keeps its plain ID

- **WHEN** 一个 project 目录下所有 session 共享同一 `cwd`
- **THEN** 系统 SHALL 输出一条 `Project`，其 `id` 等于 encoded 目录名（无 `::` 后缀）

#### Scenario: Multi-cwd directory splits into composite IDs

- **WHEN** 一个 project 目录含两条 session，`cwd` 互不相同
- **THEN** 系统 SHALL 输出两条 `Project`，各自 `id` 为形如 `{encodedDir}::{8-char-hex}` 的复合 ID，`path` 字段分别为各自 `cwd`

#### Scenario: Composite ID is stable across scans

- **WHEN** 同一目录在 session 内容不变的前提下被扫描两次
- **THEN** 两次扫描 SHALL 对同一 subproject 产出相同的复合 ID

#### Scenario: Registry exposes session filter for a composite ID

- **WHEN** 调用方拿一个复合 ID 查 subproject registry
- **THEN** registry SHALL 返回属于该 subproject 的 session id 集合（使 session 列表可据此过滤），并对任意 plain（非复合）ID 返回 `None`

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

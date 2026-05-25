## MODIFIED Requirements

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

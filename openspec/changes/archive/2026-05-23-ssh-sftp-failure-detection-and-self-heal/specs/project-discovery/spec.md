## MODIFIED Requirements

### Requirement: Scan Claude projects directory

系统 SHALL 扫描当前 Claude root 下的 `projects` 根目录，把每个一级子目录视为一个 project。当前 Claude root SHALL 来自 `general.claudeRootPath`；当该字段为 `null` 时，默认 Unix root 为 `~/.claude`、Windows root 为 `%USERPROFILE%\.claude\`，projects 根目录分别为 `~/.claude/projects/` 与 `%USERPROFILE%\.claude\projects\`。

系统 SHALL 按 `HOME` → `USERPROFILE` → `HOMEDRIVE` + `HOMEPATH` → 平台默认（`dirs::home_dir()`）的优先级解析用户 home 目录。这与 TS 原版 `pathDecoder.ts::getHomeDir` 的 fallback 链一致：让 WSL / Git Bash / Cygwin 用户可经 `HOME` 覆写，同时仍能在 Windows 原生 shell 里定位到 `%USERPROFILE%\.claude\`。

**SSH 模式下单 project 扫描错误处理**：当 `fs.kind() == FsKind::Ssh` 时，scanner 对每个 sub-project 调 `scan_project_dir` 的错误 SHALL 按 `FsError::is_likely_channel_dead()` 元方法分流：

- `is_likely_channel_dead() == true`（含 `Disconnected` 任意 / `TransientExhausted { last_reason }` 含 transport-dead 关键字 / `Io { source.kind() }` 是 `BrokenPipe / ConnectionReset / ConnectionAborted`）：scanner SHALL **立即** `return Err(DiscoverError::Fs(err))` abort 整轮 scan，让上层 `list_repository_groups` 拿到 hard error 触发自愈路径，**不**得 silent skip 凑半成品列表
- `is_likely_channel_dead() == false`（普通单文件 IO / NotFound / 单 project 临时不可读）：保留现有 `tracing::warn!(dir, error, "skip unreadable project dir")` + 跳过该 project 行为

理由：SSH channel-dead 时 silent skip 让用户 sidebar 看到不完整列表 + UI 表现"还在加载"，自愈路径瘫痪；而普通单 project 失败（典型权限 / 单文件损坏）silent skip 让其它 project 仍可见是合理的。

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

#### Scenario: SSH channel-dead error aborts full scan instead of silent skip

- **WHEN** active context 是 `Ssh<host>`，scanner 调 `scan_project_dir(dir_name_a)` 返 `DiscoverError::Fs(FsError::Disconnected { ... })`
- **AND** 仍有未扫描的 sub-project `dir_name_b` / `dir_name_c` 在迭代队列中
- **THEN** scanner SHALL **立即** return `Err(DiscoverError::Fs(err))` 跳出整轮 scan
- **AND** SHALL NOT 继续尝试 `scan_project_dir(dir_name_b)` / `scan_project_dir(dir_name_c)`
- **AND** SHALL `tracing::error!(dir, error, "ssh channel appears dead; aborting full scan")` 记录决策
- **AND** 上层 `list_repository_groups` SHALL 把该错误传播到 IPC caller（与 issue #231 触发自愈路径预期一致，避免半成品列表误导用户）

#### Scenario: SSH TransientExhausted with transport-dead keyword aborts scan

- **WHEN** active context 是 `Ssh<host>`，scanner 调 `scan_project_dir(dir_name)` 返 `DiscoverError::Fs(FsError::TransientExhausted { last_reason: "session closed", attempts: 3, ... })`
- **THEN** scanner SHALL 识别 `last_reason` 含 transport-dead 关键字 → `is_likely_channel_dead() == true`
- **AND** SHALL 立即 abort 整轮 scan return `Err(...)`

#### Scenario: SSH per-project NotFound 仍 silent skip 不 abort

- **WHEN** active context 是 `Ssh<host>`，scanner 调 `scan_project_dir(dir_name_a)` 返 `DiscoverError::Fs(FsError::NotFound(_))`（典型场景：扫描期间该 project 被远端进程删除）
- **THEN** scanner SHALL `tracing::warn!` + continue，继续扫描后续 sub-project
- **AND** 最终 `scan` 返 `Ok(Vec<Project>)` 包含其它扫描成功的 project（缺失 dir_name_a）

#### Scenario: SSH per-project pure timeout TransientExhausted 仍 silent skip 不 abort

- **WHEN** active context 是 `Ssh<host>`，scanner 调 `scan_project_dir(dir_name_a)` 返 `DiscoverError::Fs(FsError::TransientExhausted { last_reason: "timeout", attempts: 3, ... })`
- **THEN** scanner SHALL 识别 `last_reason` 不含 transport-dead 关键字 → `is_likely_channel_dead() == false`
- **AND** SHALL `tracing::warn!` + continue 保持现有容错行为（避免误把远端 readdir 慢盘当 channel 死）

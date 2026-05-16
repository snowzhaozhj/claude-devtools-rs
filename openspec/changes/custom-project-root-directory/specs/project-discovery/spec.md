## MODIFIED Requirements

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

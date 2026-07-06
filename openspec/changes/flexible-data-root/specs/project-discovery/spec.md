## ADDED Requirements

### Requirement: Resolve tilde prefix in Claude root path

系统 SHALL 在把当前 Claude root 拼接为任何数据目录之前，对以 `~/`（Windows 上等价 `~\`）开头的 root 值展开为用户 home 目录下的绝对路径。此展开 SHALL 作用于该 root 派生的**全部**消费路径，不得只展开其一而让其余走字面 tilde 路径：

- 会话项目目录（`projects/`，含 watcher 监听）
- 待办目录（`todos/`，含 watcher 监听）
- 基于该 root 读取的 CLAUDE.md 文件与会话内存（auto-memory）文件路径

home 目录解析 SHALL 复用既有 `HOME` → `USERPROFILE` → `HOMEDRIVE + HOMEPATH` → 平台默认的 fallback 链（详本 spec `Scan Claude projects directory`）。仅 `~/` / `~\` 前缀（紧跟路径分隔符）SHALL 被展开；`~user/` 具名 home 形式 SHALL NOT 被展开（与 [[configuration-management]] 的校验口径一致）。绝对路径 root SHALL 原样使用、不做任何展开。

此展开 SHALL 是数据读取侧唯一的 home 展开点——配置持久化层保留 `~/` 原形（详 [[configuration-management]]），使同一 `~/` 配置在不同机器 / 用户 home 下均解析到各自正确的目录。claude-devtools 自身的 jobs 目录（[[background-jobs]]）不由 `claudeRootPath` 派生，SHALL NOT 随数据根切换。

#### Scenario: Tilde root expands all derived data paths

- **WHEN** 当前 Claude root 配置为 `~/.qoder`
- **THEN** scanner SHALL 扫描 `<home>/.qoder/projects/`（`<home>` 为 fallback 链解析出的用户 home）
- **AND** todos 数据目录 SHALL 解析为 `<home>/.qoder/todos/`，而非字面 `~/.qoder/todos/`
- **AND** 基于该 root 的 CLAUDE.md / auto-memory 读取 SHALL 使用 `<home>/.qoder/` 下的绝对路径，而非字面 `~/.qoder/`

#### Scenario: Windows backslash tilde expands

- **WHEN** 当前 Claude root 配置为 `~\.qoder`（Windows 反斜杠形式）
- **THEN** 系统 SHALL 等价于 `~/.qoder` 展开到 `<home>\.qoder\` 下的 projects 与 todos

#### Scenario: Absolute root used verbatim

- **WHEN** 当前 Claude root 配置为绝对路径 `/data/claude-alt`
- **THEN** scanner SHALL 扫描 `/data/claude-alt/projects/`，不做任何 `~` 处理

#### Scenario: Named-home tilde not expanded

- **WHEN** 当前 Claude root 值为 `~alice/data`（具名 home 形式）
- **THEN** 系统 SHALL NOT 展开为 `alice` 的 home 目录（该形态在配置校验层已被拒绝，不会成为持久化的 root 值）

## ADDED Requirements

### Requirement: Search uses current Claude root

系统 SHALL 在执行 project 搜索与全局搜索时使用当前 Claude root 下的 `projects` 目录定位 session 文件；当前 Claude root 来自 `general.claudeRootPath`，为空时使用默认 home 下 `.claude`。

#### Scenario: Project search uses custom Claude root
- **WHEN** 当前 Claude root 配置为 `/data/claude-alt`
- **AND** 指定 project 的 sessions 位于 `/data/claude-alt/projects/<project_id>/`
- **THEN** 搜索 SHALL 从该目录读取 sessions
- **AND** SHALL NOT 从默认 `~/.claude/projects/<project_id>/` 读取 sessions

#### Scenario: Global search uses custom Claude root
- **WHEN** 当前 Claude root 配置为 `/data/claude-alt`
- **AND** `/data/claude-alt/projects/` 与默认 `~/.claude/projects/` 各自包含不同 project
- **THEN** 全局搜索 SHALL 只扫描 `/data/claude-alt/projects/`
- **AND** 默认 root 中仅存在的命中 SHALL NOT 出现在结果中

#### Scenario: Global search follows root change without restart
- **WHEN** 全局搜索已在默认 Claude root 下执行过
- **AND** 用户把 Claude root 更新为 `/data/claude-alt`
- **THEN** 后续全局搜索 SHALL 使用 `/data/claude-alt/projects/`

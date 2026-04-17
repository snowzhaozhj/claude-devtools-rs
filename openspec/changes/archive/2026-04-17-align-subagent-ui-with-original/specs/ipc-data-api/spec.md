## MODIFIED Requirements

### Requirement: Expose auxiliary read operations

The system SHALL expose auxiliary data operations used by the renderer beyond the core session and project queries: read agent configs (subagent definitions), batch get sessions by ids, get session chat groups, get repository groups, get worktree sessions, read CLAUDE.md files (global/project/directory scopes), read a specific directory's CLAUDE.md, and read a single `@mention`-resolved file.

针对 Rust 侧实现，`read_agent_configs` SHALL 由 `LocalDataApi::read_agent_configs()` 提供并经 Tauri `read_agent_configs` command 暴露给前端；返回值 SHALL 为 `Vec<AgentConfig>` 序列化结果（详见 `agent-configs` capability）。

#### Scenario: Batch get sessions by ids
- **WHEN** a caller invokes the batch get-sessions-by-ids operation with an array of session ids
- **THEN** the response SHALL contain one session entry per requested id, with missing ids returned as not-found placeholders

#### Scenario: Read three-scope CLAUDE.md
- **WHEN** a caller invokes the read-claude-md-files operation for a given project
- **THEN** the response SHALL include entries for the global, project, and (when applicable) directory scopes

#### Scenario: Get worktree sessions
- **WHEN** a caller invokes the get-worktree-sessions operation for a repository group
- **THEN** the response SHALL list sessions belonging to every worktree in that group

#### Scenario: Read agent configs
- **WHEN** a caller invokes the read-agent-configs operation
- **THEN** the response SHALL contain the parsed subagent definitions from `~/.claude/agents/` and project-scoped agent directories

#### Scenario: Read agent configs via Tauri command
- **WHEN** 前端调用 `invoke("read_agent_configs")`
- **THEN** 响应 SHALL 为 JSON 数组，每个元素含 `name`、`color`、`description`、`scope`、`filePath` 字段（camelCase）

#### Scenario: Agent configs 在两个作用域目录都不存在时
- **WHEN** 全局 `~/.claude/agents/` 与所有项目的 `.claude/agents/` 目录都缺失
- **THEN** 命令 SHALL 返回空数组并且不返回错误

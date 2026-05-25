## MODIFIED Requirements

### Requirement: Scan agent config files from global and project scopes

系统 SHALL 扫描 `~/.claude/agents/`（全局作用域）以及每个已发现 project 下**所有 session cwd 的去重集合**对应的 `<cwd>/.claude/agents/` 目录（项目作用域）下的 `*.md` 文件，并把两个作用域结果聚合后返回。任一作用域目录缺失时 SHALL degrade gracefully：返回另一作用域已有的条目，不抛错。

合并后同一 `(scope, name)` 出现重复时 SHALL 沿用 "按 `(scope_global_first, name)` 排序去重" 策略，去重保留 IPC 入口收到的 pairs 中先出现的条目；调用入口 SHALL 把多 cwd 的 pairs **按 session mtime 倒序**展开，使最新 cwd 的 agent 优先占据 dedup slot。

`read_agent_configs` IPC 入口 SHALL 对每个 `Project` 收集其 sessions 列表里所有非空 `cwd` 值的去重集合、按 session mtime 倒序构造 `(project_id, cwd)` pairs；当 session 无 `cwd` 时 fallback 到 `Project.path`。

#### Scenario: 全局 + 项目同时存在

- **WHEN** `~/.claude/agents/code-reviewer.md` 与 `/some/project/.claude/agents/deep-explorer.md` 同时存在
- **THEN** `read_agent_configs` SHALL 返回两个条目，分别带 `AgentConfigScope::Global` 与 `AgentConfigScope::Project(project_id)`

#### Scenario: 仅全局存在

- **WHEN** `~/.claude/agents/` 有文件但项目路径下无 `.claude/agents/` 目录
- **THEN** SHALL 仅返回全局条目，不报错

#### Scenario: 仅项目级存在

- **WHEN** `~/.claude/agents/` 不存在而项目 `.claude/agents/` 有文件
- **THEN** SHALL 仅返回项目级条目，不报错

#### Scenario: 所有作用域目录缺失

- **WHEN** 两个作用域目录均不存在
- **THEN** SHALL 返回空数组，不报错

#### Scenario: 同 project 多 cwd 下的 agents 全部被扫到

- **WHEN** 同一 encoded project 目录下含两条 session，cwd 分别为 `/repo/main` 与 `/repo/.claude/worktrees/feat-x`
- **AND** `/repo/main/.claude/agents/main-helper.md` 与 `/repo/.claude/worktrees/feat-x/.claude/agents/feat-helper.md` 都存在
- **THEN** `read_agent_configs` 返回值 SHALL 同时含 `main-helper` 与 `feat-helper` 两条 project-scoped 条目
- **AND** SHALL NOT 因 `Project.path` 仅指向其中一个 cwd 而漏扫另一个

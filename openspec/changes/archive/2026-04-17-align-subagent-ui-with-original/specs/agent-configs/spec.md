## ADDED Requirements

### Requirement: Scan agent config files from global and project scopes

The system SHALL scan `*.md` files under `~/.claude/agents/`（全局作用域）以及每个已发现 project 的 cwd 下 `.claude/agents/`（项目作用域），并将结果聚合返回。扫描 SHALL 在两个作用域任一目录缺失时 degrade gracefully，返回存在路径下的条目，不抛错。

#### Scenario: 全局 + 项目同时存在
- **WHEN** `~/.claude/agents/code-reviewer.md` 与 `/some/project/.claude/agents/deep-explorer.md` 同时存在
- **THEN** `read_agent_configs` SHALL 返回两个条目，各自带 `AgentConfigScope::Global` 与 `AgentConfigScope::Project(project_id)`

#### Scenario: 仅全局存在
- **WHEN** `~/.claude/agents/` 有文件但项目路径无 `.claude/agents/` 目录
- **THEN** SHALL 仅返回全局条目，不报错

#### Scenario: 仅项目级存在
- **WHEN** `~/.claude/agents/` 不存在而项目 `.claude/agents/` 有文件
- **THEN** SHALL 仅返回项目级条目，不报错

#### Scenario: 所有作用域目录缺失
- **WHEN** 两个作用域目录均不存在
- **THEN** SHALL 返回空数组，不报错

### Requirement: Parse frontmatter for name / color / description

每个 agent md 文件 SHALL 按 `---\n<frontmatter>\n---\n<body>` 结构解析。frontmatter SHALL 按 `key: value` 单行提取 `name`、`color`、`description` 三个键，缺失的键对应字段 SHALL 为 `None`。不支持多行或嵌套 YAML；遇到不可解析行 SHALL 跳过而非终止整个文件。

#### Scenario: 完整 frontmatter
- **WHEN** 文件内容以 `---\nname: code-reviewer\ncolor: purple\ndescription: Reviews code for bugs\n---\n` 开头
- **THEN** 解析结果 SHALL 为 `AgentConfig { name: "code-reviewer", color: Some("purple"), description: Some("Reviews code for bugs"), ... }`

#### Scenario: 部分 frontmatter
- **WHEN** 文件只有 `name: deep-explorer` 而无 color/description
- **THEN** 其 `color` 与 `description` 字段 SHALL 为 `None`，其余字段照常填入

#### Scenario: 无 frontmatter
- **WHEN** 文件不以 `---` 开头或 frontmatter 块不闭合
- **THEN** SHALL 以**文件名（去扩展名）**作为 `name`，其它字段为 `None`

#### Scenario: 带引号值
- **WHEN** frontmatter 含 `color: "#ff0000"` 或 `color: 'red'`
- **THEN** 解析结果 SHALL 去除外层双引号或单引号，保留字符串字面量内容

#### Scenario: 非法行跳过
- **WHEN** frontmatter 中存在不符合 `key: value` 格式的行（例如纯注释、缩进列表）
- **THEN** 该行 SHALL 被跳过，不影响后续行解析

### Requirement: Expose agent configs through data API

The system SHALL expose a `read_agent_configs()` 方法在 `LocalDataApi` 上，返回 `Vec<AgentConfig>`，并由 Tauri `read_agent_configs` command 透传给前端。

#### Scenario: 通过 Tauri command 读取
- **WHEN** 前端调用 `invoke("read_agent_configs")`
- **THEN** 返回值 SHALL 是 JSON 数组，每个元素含 `name` / `color` / `description` / `scope` / `filePath` 字段（camelCase 序列化）

#### Scenario: 返回值稳定排序
- **WHEN** 多个 agent 配置被扫描
- **THEN** 返回数组 SHALL 按 `(scope, name)` 稳定排序：global 优先于 project，同作用域内按 `name` 字典序

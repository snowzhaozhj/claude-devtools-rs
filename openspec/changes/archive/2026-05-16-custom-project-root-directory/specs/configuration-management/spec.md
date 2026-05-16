## MODIFIED Requirements

### Requirement: Persist application configuration

系统 SHALL 把应用配置（triggers、UI 偏好、pinned sessions、HTTP 端口、SSH hosts、feature toggles、Claude 数据根目录）持久化到用户级配置文件 `~/.claude/claude-devtools-config.json`，并在启动时加载。

`general.claudeRootPath` SHALL 表示 Claude 数据根目录；当该字段为 `null` 时，系统 MUST 使用默认 home 下 `.claude`。该字段 SHALL 只控制 Claude 数据读取根目录，MUST NOT 改变 `claude-devtools-config.json` 自身的存储位置。

#### Scenario: First launch with no config file
- **WHEN** 启动时配置文件不存在
- **THEN** 系统 SHALL 物化默认配置、持久化、继续运行
- **AND** `general.claudeRootPath` SHALL 为 `null`

#### Scenario: Corrupted config file
- **WHEN** 配置文件存在但无法解析
- **THEN** 系统 SHALL 把损坏文件重命名为 `<path>.bak.<unix_timestamp_ms>`，记录带备份路径的 warn 日志，加载默认配置，持久化新配置，继续运行

#### Scenario: Partial config with missing fields
- **WHEN** 配置文件解析成功但缺少部分字段
- **THEN** 系统 SHALL 与默认配置合并以补齐缺失字段，保留已有值

#### Scenario: Custom Claude root persists
- **WHEN** 调用方把 `general.claudeRootPath` 更新为绝对路径 `/data/claude-alt`
- **THEN** 该值 SHALL 被持久化
- **AND** 下次读取配置时 SHALL 返回同一绝对路径

#### Scenario: Clearing Claude root restores default
- **WHEN** 调用方把已配置的 `general.claudeRootPath` 更新为 `null`
- **THEN** 该值 SHALL 被持久化为 `null`
- **AND** 后续 Claude 数据读取 SHALL 回退到默认 home 下 `.claude`

### Requirement: Read CLAUDE.md files

系统 SHALL 从八种作用域读取 CLAUDE.md 文件，每个文件返回路径、是否存在标记、字符数与估算 token 数（`char_count / 4`）。全局用户作用域、用户 rules 与 auto-memory 作用域 SHALL 使用当前 Claude root；当前 Claude root 来自 `general.claudeRootPath`，为空时使用默认 home 下 `.claude`。

#### Scenario: All eight scopes enumerated
- **WHEN** 调用方请求指定 project root 的 CLAUDE.md 文件
- **THEN** 系统 SHALL 按以下顺序检查八个作用域：
  1. `enterprise` —— 平台特定路径（macOS：`/Library/Application Support/ClaudeCode/CLAUDE.md`）
  2. `user` —— `<claude_base>/CLAUDE.md`
  3. `project` —— `<project_root>/CLAUDE.md`
  4. `project-alt` —— `<project_root>/.claude/CLAUDE.md`
  5. `project-rules` —— `<project_root>/.claude/rules/**/*.md`（递归收集，合并统计）
  6. `project-local` —— `<project_root>/CLAUDE.local.md`
  7. `user-rules` —— `<claude_base>/rules/**/*.md`（递归收集，合并统计）
  8. `auto-memory` —— `<claude_base>/projects/<encoded_project_root>/memory/MEMORY.md`（仅前 200 行）

#### Scenario: Only global CLAUDE.md exists
- **WHEN** 用户有全局 CLAUDE.md 但项目没有
- **THEN** 结果 SHALL 含一个 `user` 作用域条目标记为存在，其它作用域全部标记为不存在

#### Scenario: All three original scopes present
- **WHEN** global、project、cwd 三处 CLAUDE.md 同时存在
- **THEN** 结果 SHALL 包含 `user`、`project`、`project-alt`（若存在）三个条目，全部标记为存在

#### Scenario: File not readable
- **WHEN** CLAUDE.md 存在但无法读取（例如 permission denied）
- **THEN** 系统 SHALL 该作用域返回 `exists: false` 并 zero counts，记录错误日志

#### Scenario: Custom Claude root scopes
- **WHEN** 当前 Claude root 为 `/data/claude-alt`
- **AND** `/data/claude-alt/CLAUDE.md` 与 `/data/claude-alt/rules/rule.md` 存在
- **THEN** `user` 与 `user-rules` 作用域 SHALL 从 `/data/claude-alt` 读取
- **AND** 系统 SHALL NOT 从默认 `~/.claude` 读取这些作用域

### Requirement: Validate configuration fields before persistence

系统 SHALL 对传入的配置更新做校验（HTTP 端口范围、regex 模式、文件路径等），非法值 SHALL 被拒绝并附错误说明，不写入坏状态。

#### Scenario: Invalid port number
- **WHEN** 调用方把 HTTP 端口设为 1024–65535 之外的值
- **THEN** 更新 SHALL 被拒绝并返回 validation error，已存储值 SHALL 保持不变

#### Scenario: Invalid regex pattern
- **WHEN** 调用方提交长度超过 100 字符的 regex 或含危险结构（嵌套量词等）
- **THEN** 该 regex SHALL 被拒绝并返回错误说明

#### Scenario: Invalid `claude_root_path`
- **WHEN** 调用方把 `claude_root_path` 设为非绝对路径
- **THEN** 更新 SHALL 被拒绝并返回 validation error
- **AND** 已存储值 SHALL 保持不变

#### Scenario: Empty `claude_root_path` clears override
- **WHEN** 调用方把 `claude_root_path` 设为 `null` 或仅空白字符串
- **THEN** 系统 SHALL 将该值规范化为 `None`

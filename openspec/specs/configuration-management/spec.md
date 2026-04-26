# configuration-management Specification

## Purpose

管理应用配置文件 `~/.claude/claude-devtools-config.json` 的持久化、加载、字段更新与校验，扫描八种作用域下的 CLAUDE.md 文件并估算 token，对会话内 `@path` mention 引用做安全解析（防越权 / 防符号链接逃逸 / 拒绝敏感文件）。本 capability 由前端设置面板和 context-tracking 共同消费。
## Requirements
### Requirement: Persist application configuration

系统 SHALL 把应用配置（triggers、UI 偏好、pinned sessions、HTTP 端口、SSH hosts、feature toggles）持久化到用户级配置文件 `~/.claude/claude-devtools-config.json`，并在启动时加载。

#### Scenario: First launch with no config file
- **WHEN** 启动时配置文件不存在
- **THEN** 系统 SHALL 物化默认配置、持久化、继续运行

#### Scenario: Corrupted config file
- **WHEN** 配置文件存在但无法解析
- **THEN** 系统 SHALL 把损坏文件重命名为 `<path>.bak.<unix_timestamp_ms>`，记录带备份路径的 warn 日志，加载默认配置，持久化新配置，继续运行

#### Scenario: Partial config with missing fields
- **WHEN** 配置文件解析成功但缺少部分字段
- **THEN** 系统 SHALL 与默认配置合并以补齐缺失字段，保留已有值

### Requirement: Expose config read and update operations

系统 SHALL 暴露读取当前配置、更新单个字段、增删 trigger、pin/unpin session、用外部编辑器打开配置文件这些操作。

#### Scenario: Update a single config field
- **WHEN** 调用方把 HTTP 端口更新为新值
- **THEN** 新值 SHALL 被持久化，下次读取时返回该值

#### Scenario: Add a new trigger
- **WHEN** 调用方通过 add-trigger 操作新增一个 trigger
- **THEN** 该 trigger SHALL 被持久化（携带生成的 id），后续读取可见

### Requirement: Read CLAUDE.md files

系统 SHALL 从八种作用域读取 CLAUDE.md 文件，每个文件返回路径、是否存在标记、字符数与估算 token 数（`char_count / 4`）。

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

### Requirement: Resolve and read mentioned files safely

系统 SHALL 把 `@path` mention 解析为相对于当前 session cwd 的路径并读取文件内容，拒绝逃逸到允许根之外的路径。

#### Scenario: Valid in-project mention
- **WHEN** mention `@src/foo.ts` 解析后位于 session 的 project root 内
- **THEN** 文件 SHALL 被读取并返回，附带绝对路径、字符数、估算 token 数

#### Scenario: Path traversal attempt
- **WHEN** mention 解析到允许根之外（例如 `@../../etc/passwd`）
- **THEN** 读取 SHALL 被拒绝并返回 validation error

#### Scenario: Sensitive file blocked
- **WHEN** mention 解析后命中敏感文件模式（`.ssh/`、`.env`、`.aws/`、私钥等）
- **THEN** 读取 SHALL 被拒绝，即使路径在允许目录内

#### Scenario: Symlink escape
- **WHEN** mention 解析路径在 project root 内但符号链接目标在外部
- **THEN** 系统 SHALL canonicalize 路径，若真实路径在允许根外则拒绝

#### Scenario: Token limit exceeded
- **WHEN** 被引用文件估算 token 数超过调用方指定的最大值
- **THEN** 读取 SHALL 返回 `null` / `None`

### Requirement: Validate configuration fields before persistence

系统 SHALL 对传入的配置更新做校验（HTTP 端口范围、regex 模式、文件路径等），非法值 SHALL 被拒绝并附错误说明，不写入坏状态。

#### Scenario: Invalid port number
- **WHEN** 调用方把 HTTP 端口设为 1024–65535 之外的值
- **THEN** 更新 SHALL 被拒绝并返回 validation error，已存储值 SHALL 保持不变

#### Scenario: Invalid regex pattern
- **WHEN** 调用方提交长度超过 100 字符的 regex 或含危险结构（嵌套量词等）
- **THEN** 该 regex SHALL 被拒绝并返回错误说明

#### Scenario: Invalid `claude_root_path`
- **WHEN** 调用方把 `claude_root_path` 设为非绝对或空路径
- **THEN** 该值 SHALL 被规范化为 `None`

### Requirement: Update notifications SHALL accept full triggers replacement

`ConfigManager::update_notifications` SHALL 处理传入 JSON payload 的 `triggers` 字段：反序列化为 `Vec<NotificationTrigger>`、对每条调用 `validate_trigger` 拒绝非法条目、整体替换 `self.config.notifications.triggers`、并调用 `TriggerManager::set_triggers(list)` 同步内存状态，最后 `save()` 持久化。未识别的键 SHALL 仍被忽略但 MUST 通过 `tracing::warn!(key = %k, "unknown notifications update key ignored")` 记录，避免再次静默丢字段。

#### Scenario: triggers 字段被整体替换并落盘

- **WHEN** 调用方向 `update_config` IPC 发送 `section="notifications", data={ "triggers": [<新数组>] }`
- **THEN** `ConfigManager` SHALL 把 `config.notifications.triggers` 替换为该数组、同步 `TriggerManager::triggers`、写入磁盘
- **AND** 下一次调用 `get_enabled_triggers()` SHALL 返回新数组中 `enabled=true` 的子集

#### Scenario: 非法 trigger 拒绝整组写入

- **WHEN** 新 triggers 数组中任意一条经 `validate_trigger` 返回 `valid=false`
- **THEN** `update_notifications` SHALL 返回 `ConfigError::validation` 携带该 trigger id 与失败原因
- **AND** `self.config.notifications.triggers` 与 `TriggerManager::triggers` SHALL 保持修改前状态（不部分写入）
- **AND** 磁盘文件 SHALL NOT 被更新

#### Scenario: 未知通知键发出 warn 但不报错

- **WHEN** payload 中含除 `enabled` / `soundEnabled` / `includeSubagentErrors` / `snoozeMinutes` / `triggers` 之外的其它键（例如 `fooBar`）
- **THEN** 该键 SHALL 被忽略，操作仍返回成功
- **AND** 日志 SHALL 以 `warn` 级别包含被忽略的键名

### Requirement: 持久化「启动时自动检查更新」开关

`ConfigData` SHALL 包含字段 `auto_update_check_enabled: bool`，序列化为 JSON 字段名 `autoUpdateCheckEnabled`，缺省值通过 `#[serde(default = "<fn>")]` 物化为 `true`。该字段 SHALL 控制应用启动后 5 秒后台自动检查更新行为，但 MUST NOT 影响手动「检查更新」按钮的可用性。

#### Scenario: 默认值为启用

- **WHEN** 首次启动或老配置文件中无该字段
- **THEN** `ConfigData::auto_update_check_enabled` SHALL 反序列化为 `true`
- **AND** 后端启动 5 秒后台检查 SHALL 正常执行

#### Scenario: 关闭开关并持久化

- **WHEN** 调用方通过 `update_config` IPC 把 `autoUpdateCheckEnabled` 设为 `false`
- **THEN** `ConfigManager` SHALL 把 `config.auto_update_check_enabled = false` 持久化到磁盘
- **AND** 下次启动 SHALL 跳过后台自动检查

#### Scenario: 开启开关并持久化

- **WHEN** 调用方把 `autoUpdateCheckEnabled` 设为 `true`
- **THEN** `ConfigManager` SHALL 持久化为 `true`
- **AND** 下次启动 SHALL 恢复后台自动检查

#### Scenario: 与既有字段合并

- **WHEN** 老配置文件含其它字段但无 `autoUpdateCheckEnabled`
- **THEN** 加载逻辑 SHALL 与默认配置合并，该字段取默认值 `true`，其它已有字段 SHALL NOT 被覆盖

### Requirement: 持久化跳过的更新版本号

`ConfigData` SHALL 包含字段 `skipped_update_version: Option<String>`，序列化为 JSON 字段名 `skippedUpdateVersion`，遵循既有 schema 演进约定（`#[serde(default, skip_serializing_if = "Option::is_none")]`），用于记录用户主动「跳过此版本」的目标版本号。

#### Scenario: 默认值为空

- **WHEN** 首次启动或老配置文件中无该字段
- **THEN** `ConfigData::skipped_update_version` SHALL 反序列化为 `None`
- **AND** 持久化时 `skippedUpdateVersion` 字段 SHALL NOT 出现在 JSON 中（`skip_serializing_if`）

#### Scenario: 写入跳过版本

- **WHEN** 调用方通过 `update_config` IPC 传 `{ section: "skippedUpdateVersion", data: "0.3.0" }` 或在 patch 中包含 `skippedUpdateVersion: "0.3.0"`
- **THEN** `ConfigManager` SHALL 把 `config.skipped_update_version = Some("0.3.0".into())` 持久化到磁盘
- **AND** 下次读取 SHALL 返回该值

#### Scenario: 清空跳过版本

- **WHEN** 调用方传 `skippedUpdateVersion: null`
- **THEN** `ConfigManager` SHALL 把 `config.skipped_update_version = None` 持久化
- **AND** 下次读取 SHALL 返回 `None`

#### Scenario: 与既有字段合并

- **WHEN** 老配置文件含 triggers / pinned 等字段但无 `skippedUpdateVersion`
- **THEN** 加载逻辑 SHALL 与默认配置合并，保留已有字段，`skippedUpdateVersion` 取默认 `None`，**不**覆盖其他字段


# configuration-management Specification

## Purpose

管理应用配置文件 `~/.claude/claude-devtools-config.json` 的持久化、加载、字段更新与校验，扫描八种作用域下的 CLAUDE.md 文件并估算 token，对会话内 `@path` mention 引用做安全解析（防越权 / 防符号链接逃逸 / 拒绝敏感文件）。本 capability 由前端设置面板和 context-tracking 共同消费。
## Requirements
### Requirement: Persist application configuration

系统 SHALL 把应用配置（triggers、UI 偏好、pinned sessions、HTTP 端口、SSH hosts、feature toggles、Claude 数据根目录）持久化到用户级配置文件 `~/.claude/claude-devtools-config.json`，并在启动时加载。

`general.claudeRootPath` SHALL 表示 Claude 数据根目录；当该字段为 `null` 时，系统 MUST 使用默认 home 下 `.claude`。该字段 SHALL 只控制 Claude 数据读取根目录，MUST NOT 改变 `claude-devtools-config.json` 自身的存储位置。

`ssh` 段 SHALL 包含：

- `ssh.profiles[]`：用户保存的命名连接配置数组，每条含 `{ name, host, port, username, authMethod, passwordRequired }` 六个字段；`passwordRequired: bool` 标记该 profile 是否 password 模式（用于 UI 重新填表时决定是否弹密码输入框）。**字段集 MUST NOT 含 password 明文**——密码值绝不持久化到磁盘。
- `ssh.last_connection`：最近一次成功连接的配置 `{ host, port, username, authMethod }`；同样 MUST NOT 含 password 字段。可为 `null`（从未成功连接过 SSH）。
- `ssh.auto_reconnect: bool`：v1 仅持久化字段，自动重连本身留 v2 实现；默认 `false`。

#### Scenario: First launch with no config file

- **WHEN** 启动时配置文件不存在
- **THEN** 系统 SHALL 物化默认配置、持久化、继续运行
- **AND** `general.claudeRootPath` SHALL 为 `null`
- **AND** `ssh.profiles` SHALL 为空数组，`ssh.last_connection` SHALL 为 `null`，`ssh.auto_reconnect` SHALL 为 `false`

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

#### Scenario: Save SSH last connection without password

- **WHEN** 调用方调 `ssh_save_last_connection` 携带 `{ host, port, username, authMethod, password: "secret" }`
- **THEN** 持久化文件中 `ssh.last_connection` SHALL 为 `{ host, port, username, authMethod }` 四字段
- **AND** SHALL NOT 包含 `password` 键，即使输入有 password 字段
- **AND** 文件 grep `secret` SHALL 无任何匹配

#### Scenario: Save SSH profile without password

- **WHEN** 调用方通过 `update_config("ssh", { profiles: [{ name: "prod", host, port, username, authMethod: "password", passwordRequired: true, password: "secret" }] })` 新增 profile
- **THEN** 持久化结果 `ssh.profiles[0]` SHALL 含 `name / host / port / username / authMethod / passwordRequired` 六字段
- **AND** SHALL NOT 含 `password` 键

#### Scenario: Load existing config restores SSH profiles

- **WHEN** 配置文件已有 `ssh.profiles: [{ name: "p1", host: "h1", port: 22, username: "u1", authMethod: "sshConfig", passwordRequired: false }]`
- **THEN** 启动后 ConfigStore SHALL 暴露该 profile 给 UI 渲染 saved profiles 列表

### Requirement: Expose config read and update operations

系统 SHALL 暴露读取当前配置、更新单个字段、增删 trigger、pin/unpin session、用外部编辑器打开配置文件这些操作。

#### Scenario: Update a single config field
- **WHEN** 调用方把 HTTP 端口更新为新值
- **THEN** 新值 SHALL 被持久化，下次读取时返回该值

#### Scenario: Add a new trigger
- **WHEN** 调用方通过 add-trigger 操作新增一个 trigger
- **THEN** 该 trigger SHALL 被持久化（携带生成的 id），后续读取可见

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

HTTP 端口校验 SHALL 同时应用于：(a) 通过 `update_config` IPC 直接更新 `httpServer.port` 字段；(b) 通过 `http_server_start(port)` IPC 间接持久化 `httpServer.port` 字段（详 [[server-mode]]）。两条路径 SHALL 共用同一 `cdt_config::validate_http_port` 实现，保证端口语义一致——任何能存入 `httpServer.port` 的值都已通过 1024–65535 范围校验。

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

#### Scenario: http_server_start 入参端口超范围被拒绝

- **WHEN** 调用 `http_server_start(port=80)` 或 `http_server_start(port=70000)`
- **THEN** server SHALL **不**被启动
- **AND** `httpServer.port` SHALL 保持原值
- **AND** IPC SHALL 返回 validation error 文案

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

### Requirement: Display config exposes user-customizable font family

系统 SHALL 在 `display` 配置段暴露 `fontSans` 与 `fontMono` 两个可选字段，允许用户覆盖 UI 默认 sans / mono 字体。`null` 或缺失字段表示使用应用内置默认字体栈，非空字符串表示用户提供的 CSS `font-family` 值。空白字符串 SHALL 在持久化前归一化为 `null`。

#### Scenario: Font fields default to null on first launch
- **WHEN** 首次启动且配置文件不存在
- **THEN** 物化的默认配置 SHALL 包含 `display.fontSans = null` 与 `display.fontMono = null`

#### Scenario: Old config file without font fields is forward-compatible
- **WHEN** 已有配置文件解析成功但 `display` 段缺少 `fontSans` / `fontMono` 字段
- **THEN** 系统 SHALL 把缺失字段视为 `null`，已有配置值保留不变，无需迁移

#### Scenario: User sets a custom sans font
- **WHEN** 调用方通过 `update_field` 把 `display.fontSans` 设为 `"\"JetBrains Mono\", monospace"`
- **THEN** 该值 SHALL 被持久化，下次读取时返回相同字符串

#### Scenario: Whitespace-only value normalizes to null
- **WHEN** 调用方把 `display.fontSans` 设为 `"   "`（仅空白字符）
- **THEN** 系统 SHALL 把该字段持久化为 `null`，不保留空白字符串

#### Scenario: Restore default by setting null
- **WHEN** 调用方把已设过的 `display.fontMono` 重新设为 `null`
- **THEN** 该字段 SHALL 被持久化为 `null`，前端 SHALL 回落到应用内置默认字体栈

#### Scenario: Excessively long value rejected
- **WHEN** 调用方把 `display.fontSans` 设为长度超过 500 字符的字符串
- **THEN** 更新 SHALL 被拒绝并返回 validation error，已存储值 SHALL 保持不变

#### Scenario: Atomic display patch rejects entire batch on any invalid field
- **WHEN** 调用方一次 `update_display` 同时设置 `fontSans = "<合法值>"` 与 `fontMono = "<超过 500 字符>"`
- **THEN** 整次更新 SHALL 被拒绝并返回 validation error，`display.fontSans` 与 `display.fontMono` 两者已存储值 SHALL 保持不变（不允许半写状态）

#### Scenario: Reset to defaults clears font overrides
- **WHEN** 用户已设过自定义 `fontSans` / `fontMono`，随后触发 `reset_to_defaults`
- **THEN** 重置后 `display.fontSans` 与 `display.fontMono` SHALL 都为 `null`，前端 SHALL 回落到应用内置默认字体栈

### Requirement: IPC contract exposes font fields in camelCase

`getConfig` 与 `updateConfig` 的 IPC 响应 SHALL 在 `display` 段以 camelCase 字段名 `fontSans` 与 `fontMono` 暴露字体配置，类型为 `string | null`。

#### Scenario: getConfig response shape
- **WHEN** 前端调用 `getConfig`
- **THEN** 响应 `display` 段 SHALL 同时包含 `fontSans` 与 `fontMono` 两个键，值为字符串或 `null`

#### Scenario: updateConfig accepts null to clear
- **WHEN** 前端调用 `updateConfig({ display: { fontSans: null } })`
- **THEN** 调用 SHALL 成功，下一次 `getConfig` 返回的 `display.fontSans` 为 `null`

#### Scenario: updateConfig accepts non-empty string
- **WHEN** 前端调用 `updateConfig({ display: { fontMono: "\"Fira Code\", monospace" })`
- **THEN** 调用 SHALL 成功，下一次 `getConfig` 返回该字符串

### Requirement: Display config exposes time format preference

系统 SHALL 在 `display` 配置段暴露 `timeFormat` 字段，类型为枚举 `"24h" | "12h"`，控制 UI 渲染绝对时间戳时是否使用 12 小时制（带 AM/PM 前缀）。**默认值 SHALL 为 `"24h"`**。该字段缺失（旧配置文件兼容）SHALL 反序列化为默认值 `"24h"`。任何非 `"24h"` / `"12h"` 的字符串 SHALL 被 `update_display` 拒绝并返回 validation error，已存储值保持不变。

#### Scenario: 默认配置物化包含 timeFormat 字段

- **WHEN** 系统首次启动且 `~/.claude/devtools-config.json` 不存在
- **THEN** 物化的默认配置 SHALL 包含 `display.timeFormat = "24h"`

#### Scenario: 旧配置文件缺字段时落默认值

- **WHEN** 已有配置文件解析成功但 `display` 段缺少 `timeFormat` 字段
- **THEN** `getConfig` 返回的 `display.timeFormat` SHALL 为 `"24h"`，且后续 `updateConfig` 写入新值后字段 SHALL 持久化到磁盘

#### Scenario: 合法值切换到 12 小时制

- **WHEN** 调用方通过 `update_display` 把 `display.timeFormat` 设为 `"12h"`
- **THEN** 调用 SHALL 成功，下一次 `getConfig` 返回的 `display.timeFormat` 为 `"12h"`

#### Scenario: 合法值切换回 24 小时制

- **WHEN** 调用方把已设为 `"12h"` 的 `display.timeFormat` 重新设为 `"24h"`
- **THEN** 调用 SHALL 成功，下一次 `getConfig` 返回的 `display.timeFormat` 为 `"24h"`

#### Scenario: 非法字符串被拒绝且已存储值不变

- **WHEN** 调用方把 `display.timeFormat` 设为 `"bogus"` 或空字符串
- **THEN** 整次 `update_display` 调用 SHALL 被拒绝并返回 validation error；错误信息 SHALL 包含字段名 `timeFormat`；磁盘上已存储的 `display.timeFormat` 值 SHALL 保持不变

#### Scenario: 重置回默认时 timeFormat 回到 24h

- **WHEN** 调用方调用 reset-to-defaults 入口（如有）或物化全新默认配置
- **THEN** `display.timeFormat` SHALL 为 `"24h"`

### Requirement: IPC contract exposes timeFormat in camelCase

`getConfig` 与 `updateConfig` 的 IPC 响应 SHALL 在 `display` 段以 camelCase 字段名 `timeFormat` 暴露时间格式偏好，值为字符串 `"24h"` 或 `"12h"`。

#### Scenario: getConfig 响应包含 timeFormat 字段

- **WHEN** 前端调用 `getConfig`
- **THEN** 响应 `display` 段 SHALL 包含 `timeFormat` 键，值为 `"24h"` 或 `"12h"` 之一

#### Scenario: updateConfig 接受 camelCase timeFormat patch

- **WHEN** 前端调用 `updateConfig({ display: { timeFormat: "12h" } })`
- **THEN** 调用 SHALL 成功，下一次 `getConfig` 返回的 `display.timeFormat` 为 `"12h"`

### Requirement: HTTP server enabled / port SHALL be persisted in lockstep with lifecycle

`HttpServerConfig.enabled: bool` 与 `HttpServerConfig.port: u16` 字段 SHALL 持久化到 `~/.claude/claude-devtools-config.json`，键名分别为 `httpServer.enabled` 与 `httpServer.port`，缺省值通过 `#[serde(default = "<fn>")]` 物化为 `enabled=false` / `port=3456`。该字段 SHALL 与 `server-mode` capability 的 server lifecycle 协同：

- **`http_server_start(port)` IPC 成功**时 SHALL 把 `enabled=true` + `port=<入参>` 持久化（即使入参 port 与已存值相同）
- **`http_server_stop()` IPC**（成功或幂等）SHALL 把 `enabled=false` 持久化（`port` 字段保留，让用户下次开启时复用上次端口）
- **Tauri app 启动时**读取的 `enabled=true` SHALL 触发自动恢复（详 [[server-mode]]）
- **`http_server_start` 启动失败**（端口冲突 / 校验失败）SHALL **不**写持久化，避免把"想开但开不起来"的状态写盘

`port` 字段独立持久化让用户在 toggle 关闭后再开启时仍能记住上次配的端口；`enabled` 字段是用户意图（"我想要 server mode 开"），与运行时实际状态可能短暂不一致（启动时端口冲突的情况）。

#### Scenario: 启动 server 同时持久化 enabled=true 与 port

- **WHEN** 用户调 `http_server_start(port=3500)` 成功
- **THEN** `claude-devtools-config.json` SHALL 含 `httpServer.enabled = true` 与 `httpServer.port = 3500`
- **AND** 重启 Tauri app SHALL 自动启动 server 在 `127.0.0.1:3500`

#### Scenario: 关闭 server 仅写 enabled=false

- **WHEN** 用户调 `http_server_stop()`
- **THEN** `httpServer.enabled` SHALL 写为 `false`
- **AND** `httpServer.port` SHALL 保留上次成功值（不重置为默认 3456）

#### Scenario: 启动失败不写持久化

- **WHEN** 用户调 `http_server_start(port=3500)`，但 3500 已被占用
- **THEN** IPC SHALL 返回 `Err`
- **AND** `claude-devtools-config.json` 中 `httpServer.enabled` SHALL **不**被改为 `true`（保持 `false` 或上次成功值）

#### Scenario: 老配置文件无 httpServer 字段时使用默认

- **WHEN** 升级到含本 change 的版本，老配置文件无 `httpServer` 字段
- **THEN** 反序列化 SHALL 物化默认 `{ enabled: false, port: 3456 }`
- **AND** 行为 SHALL 与升级前一致（不自动启动 server）

### Requirement: Migrate composite project IDs in pinned sessions on load

`ConfigManager::load` SHALL 在反序列化配置后、暴露给消费方之前，扫描 `SessionsConfig.pinned_sessions: HashMap<String, Vec<PinnedSession>>` 中所有 key（project_id），把含 `"::"` 分隔的 composite id（形如 `{baseDir}::{hash8}`）fold 为 base_dir（即 `"::"` 之前的部分）。fold 时若多个 composite key 共享同一 base_dir，SHALL 把它们的 `Vec<PinnedSession>` 合并并按 `(session_id, pinned_at)` 去重，**保留 `pinned_at` 最早**的条目（即用户最早 pin 的时间戳）。

迁移触发（即检测到至少一个 composite key）SHALL 在写回配置文件前把当前文件备份到 `<config-path>.pre-merge-composite.bak`（覆盖已存在的同名备份），再 atomic-write 新内容。备份命名与现有"损坏配置自动备份到 `.bak.<timestamp_ms>`"机制独立，便于人工识别本次迁移的回滚点。

迁移 SHALL 是幂等的——纯粹基于 input 重写，不依赖任何"已迁移"标志位。写盘失败时 SHALL 通过 `tracing::warn!` 记录，**不**阻塞启动；下次启动 `ConfigManager::load` 命中同样的 composite key 时再次尝试 fold + 写盘。

`HiddenSession` 等其它 `HashMap<String, _>` key 为 project_id 的配置字段 SHALL 同样应用本迁移规则。`NotificationTrigger.repository_ids` 存的是 `RepositoryGroup.id`（git-common-dir 绝对路径，详见 `project-discovery` spec `Group projects by git repository identity` Requirement），与 composite project id 形态完全不同，SHALL NOT 被本迁移触及。

#### Scenario: pinned_sessions 含 composite key 被 fold 为 base_dir

- **WHEN** 配置文件 `pinned_sessions` 含 `"-Users-foo-repo::abcd1234": [{ sessionId: "s1", pinnedAt: 1000 }]` 与 `"-Users-foo-repo::ef567890": [{ sessionId: "s2", pinnedAt: 2000 }]`
- **AND** `ConfigManager::load` 被调用
- **THEN** load 完成后内存中的 `pinned_sessions` SHALL 含 `"-Users-foo-repo": [{ sessionId: "s1", pinnedAt: 1000 }, { sessionId: "s2", pinnedAt: 2000 }]`（顺序不要求，按 `session_id` 字典序或 mtime 倒序均可）
- **AND** SHALL NOT 残留 `"-Users-foo-repo::abcd1234"` 或 `"-Users-foo-repo::ef567890"` key

#### Scenario: 同 session_id 重复条目去重保留 pinned_at 最早

- **WHEN** 配置文件含 `"D::h1": [{ sessionId: "s", pinnedAt: 200 }]` 与 `"D::h2": [{ sessionId: "s", pinnedAt: 100 }]`
- **AND** `ConfigManager::load` 被调用
- **THEN** load 完成后 `pinned_sessions["D"]` SHALL 含**且仅含**一条 `{ sessionId: "s", pinnedAt: 100 }`

#### Scenario: 触发迁移时备份原文件

- **WHEN** 配置文件 `<path>` 含至少一条 composite key 且 fold 后内容与原内容不同
- **AND** `ConfigManager::load` 被调用
- **THEN** 系统 SHALL 在写回前把原文件内容写入 `<path>.pre-merge-composite.bak`
- **AND** 备份写盘 SHALL 在主文件 atomic-write 之前完成

#### Scenario: 未含 composite key 不写盘

- **WHEN** 配置文件 `pinned_sessions` 所有 key 均不含 `"::"`
- **AND** `ConfigManager::load` 被调用
- **THEN** 系统 SHALL NOT 写回主配置文件、SHALL NOT 创建 `.pre-merge-composite.bak`

#### Scenario: 写盘失败不阻塞启动

- **WHEN** fold 检测到 composite key 需要写回
- **AND** atomic-write 失败（磁盘满 / 权限拒绝）
- **THEN** 系统 SHALL 通过 `tracing::warn!` 记录失败原因
- **AND** 内存中的 fold 后状态仍 SHALL 暴露给消费方（避免运行时仍持有 composite key）
- **AND** `ConfigManager::load` SHALL 正常返回（不返回 Err）

#### Scenario: 迁移是幂等的

- **WHEN** 已 fold 的配置文件（不含 composite key）再次被 `ConfigManager::load` 加载
- **THEN** 系统 SHALL NOT 触发任何写盘
- **AND** 内存中的 `pinned_sessions` SHALL 与配置文件内容字节一致

#### Scenario: NotificationTrigger repository_ids 不受迁移影响

- **WHEN** 配置文件含 `NotificationTrigger { repository_ids: Some(vec!["/Users/foo/repo/.git"]), ... }`
- **AND** `pinned_sessions` 同时含 composite key
- **AND** `ConfigManager::load` 被调用
- **THEN** load 完成后该 trigger 的 `repository_ids` SHALL 保持 `["/Users/foo/repo/.git"]` 字节不变（无论是否含 `"::"`）

### Requirement: Persist keyboard shortcut overrides

`Config` 结构 SHALL 新增字段 `keyboard_shortcuts: HashMap<String, String>`，序列化时 SHALL 使用 serde `rename_all = "camelCase"`（IPC 字段名 `keyboardShortcuts`）。该字段 SHALL 仅持有用户自定义覆盖（diff），key 为 `keyboard-shortcuts` capability 的 `ShortcutSpec.id`，value 为 normalized binding 字符串（如 `"mod+shift+b"`）。

字段 SHALL 标注 `#[serde(default)]`：

- `default` SHALL 让旧版本 config 反序列化时缺失该字段不报错（兼容性）
- 字段 SHALL NOT 加 `skip_serializing_if`——empty HashMap SHALL 序列化为 `{}` 出现在 `get_config` IPC 响应与 `claude-devtools-config.json` 写入两处。理由：单一 serde 序列化路径不能同时实现"IPC 含 empty / 文件不含 empty"；选择"两处都含 empty `{}`"让前端 / 文件 reader 都不需要 undefined fallback，少 5 字节文件体积是可接受成本。

`get_config` IPC 响应 SHALL 包含 `keyboardShortcuts` 字段；`set_config` IPC 接受 `section="keyboardShortcuts"` + `data` 为 `Record<string, string>` 的整体覆盖更新（同 `notifications` 整体替换 `triggers` 数组的模式）。前端 `ui/src/lib/api.ts` 的 `AppConfig` 类型 SHALL 同步增加 `keyboardShortcuts: Record<string, string>` 字段。

#### Scenario: 字段反序列化兼容旧 config
- **WHEN** 读取一个旧版本写入的 `claude-devtools-config.json`（无 `keyboardShortcuts` 字段）
- **THEN** `Config::keyboard_shortcuts` SHALL 反序列化为 empty HashMap，不报错
- **AND** 启动 SHALL 走 builtin defaults

#### Scenario: 空 HashMap 序列化为 `{}`
- **WHEN** 用户从未改动任何快捷键，`AppConfig::keyboard_shortcuts` 为 empty HashMap
- **AND** 触发 `save()` 写入 config 文件 / 或 `get_config` IPC 响应
- **THEN** 写出的 JSON SHALL 含 `"keyboardShortcuts": {}`（不省略键）

#### Scenario: 持久化用户覆盖
- **WHEN** 用户通过 Settings 改动 `sidebar.toggle` 为 `mod+shift+b`
- **AND** 点击 Save 触发 `set_config`
- **THEN** `Config::keyboard_shortcuts` SHALL 更新为 `{"sidebar.toggle": "mod+shift+b"}`
- **AND** 写入后的 JSON SHALL 包含 `"keyboardShortcuts": {"sidebar.toggle": "mod+shift+b"}`

#### Scenario: get_config IPC 响应字段 camelCase
- **WHEN** 前端调用 `invoke("get_config")`
- **THEN** 响应 JSON SHALL 含 `keyboardShortcuts` 键（camelCase，非 `keyboard_shortcuts`）
- **AND** 前端 `AppConfig` TypeScript 类型 SHALL 含 `keyboardShortcuts: Record<string, string>`

#### Scenario: set_config 接收 camelCase 字段
- **WHEN** 前端调用 `invoke("set_config", { keyboardShortcuts: {"sidebar.toggle": "mod+shift+b"} })`
- **THEN** Rust 端 `Config::keyboard_shortcuts` SHALL 反序列化为 `{"sidebar.toggle": "mod+shift+b"}`
- **AND** `save()` SHALL 持久化新值


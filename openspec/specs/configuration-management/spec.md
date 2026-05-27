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

- **WHEN** mention `@docs/note.md` 解析后位于 session 的 project root 内
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

系统 SHALL 对传入的配置更新做校验（HTTP 端口范围、regex 模式、文件路径、枚举值合法性、URL 模板、scheme 白名单等），非法值 SHALL 被拒绝并附错误说明，不写入坏状态。校验失败 SHALL 整体拒绝该次 update（**不允许半写状态**），settings 文件保持原值。

HTTP 端口校验 SHALL 同时应用于：(a) `update_config` 直接更新 `httpServer.port` 字段；(b) `http_server_start(port)` 间接持久化 `httpServer.port` 字段（详 [[server-mode]]）。两条路径共用同一端口语义——任何能存入 `httpServer.port` 的值都已通过 1024–65535 范围校验。

GeneralConfig 三字段校验：

- `externalEditor` / `terminalApp`：严格枚举校验，invalid 值返回 `ApiError::ValidationError`
- `searchEngine`：`Custom` variant 的 `urlTemplate` SHALL 含 `{query}` 占位符；URL scheme SHALL ∈ `{http, https}`（拒绝 `javascript:` / `file:` / `data:` / `chrome:` 等危险 scheme）
- `terminalApp` 跨平台不匹配（macOS 写 Windows / Linux 终端值或反之）SHALL **不**触发 ValidationError——保留写入并在运行时调用对应平台默认终端，附 warn 级日志（详 D-Impl-1）

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

#### Scenario: 未知字段拒绝

- **WHEN** 前端调 `update_general` 含未注册键
- **THEN** 后端 SHALL 返回 `ApiError::ValidationError`
- **AND** settings 文件 SHALL **不**被修改

#### Scenario: 枚举非法值拒绝

- **WHEN** 前端调 `update_general({ externalEditor: <非白名单值> })` 或 `update_general({ terminalApp: <非白名单值> })`
- **THEN** 后端 SHALL 返回 `ApiError::ValidationError`
- **AND** settings 文件 SHALL **不**被修改

#### Scenario: SearchEngine.custom 缺 {query} 占位符或危险 scheme 拒绝

- **WHEN** 前端调 `update_general({ searchEngine: { type: "custom", urlTemplate: <缺 {query}> } })`，或 `urlTemplate` scheme ∈ `{javascript:, file:, data:, chrome:}` 等非 http/https
- **THEN** 后端 SHALL 返回 `ApiError::ValidationError`
- **AND** settings 文件 SHALL **不**被修改

#### Scenario: terminalApp 跨平台值不报错

- **WHEN** macOS 上前端调 `update_general({ terminalApp: <Linux 平台值> })`
- **THEN** 后端 SHALL 接受并持久化（统一 enum 跨平台合法）
- **AND** 后续运行时打开终端时 SHALL warn 级日志记录 mismatch + fallback 到 macOS 默认终端

### Requirement: Update notifications SHALL accept full triggers replacement

当调用方更新 notifications 段时，系统 SHALL 把 payload 的 `triggers` 字段解析为通知触发器数组、对每条做 trigger 校验拒绝非法条目、整体替换内存中的 triggers 列表、同步给运行时 trigger 调度器、最后持久化到磁盘。未识别的键 SHALL 仍被忽略但 SHALL 在日志中以 warn 级别附带键名记录，避免再次静默丢字段。

#### Scenario: triggers 字段被整体替换并落盘

- **WHEN** 调用方向 `update_config` IPC 发送 `section="notifications", data={ "triggers": [<新数组>] }`
- **THEN** 系统 SHALL 把 `notifications.triggers` 替换为该数组、同步给运行时 trigger 调度器、写入磁盘
- **AND** 下一次查询启用 triggers SHALL 返回新数组中 `enabled=true` 的子集

#### Scenario: 非法 trigger 拒绝整组写入

- **WHEN** 新 triggers 数组中任意一条 trigger 校验失败
- **THEN** 更新 SHALL 返回 validation error 携带该 trigger id 与失败原因
- **AND** 内存中 `notifications.triggers` 与运行时 trigger 调度器状态 SHALL 保持修改前状态（不部分写入）
- **AND** 磁盘文件 SHALL NOT 被更新

#### Scenario: 未知通知键发出 warn 但不报错

- **WHEN** payload 中含除 `enabled` / `soundEnabled` / `includeSubagentErrors` / `snoozeMinutes` / `triggers` 之外的其它键（例如 `fooBar`）
- **THEN** 该键 SHALL 被忽略，操作仍返回成功
- **AND** 系统 SHALL 在日志中以 warn 级别附带被忽略的键名记录该事件

### Requirement: 持久化「启动时自动检查更新」开关

应用配置 SHALL 包含 `autoUpdateCheckEnabled` 字段，类型为 bool，缺省值为 `true`，在配置文件缺失该字段时 SHALL 反序列化为默认值 `true`。该字段 SHALL 控制应用启动后台自动检查更新行为（详 [[app-auto-update]]），但 MUST NOT 影响手动「检查更新」按钮的可用性。

#### Scenario: 默认值为启用

- **WHEN** 首次启动或老配置文件中无该字段
- **THEN** 配置 `autoUpdateCheckEnabled` SHALL 反序列化为 `true`
- **AND** 后端启动后台检查 SHALL 正常执行

#### Scenario: 关闭开关并持久化

- **WHEN** 调用方通过 `update_config` IPC 把 `autoUpdateCheckEnabled` 设为 `false`
- **THEN** 系统 SHALL 把该字段持久化到磁盘
- **AND** 下次启动 SHALL 跳过后台自动检查

#### Scenario: 开启开关并持久化

- **WHEN** 调用方把 `autoUpdateCheckEnabled` 设为 `true`
- **THEN** 系统 SHALL 持久化为 `true`
- **AND** 下次启动 SHALL 恢复后台自动检查

#### Scenario: 与既有字段合并

- **WHEN** 老配置文件含其它字段但无 `autoUpdateCheckEnabled`
- **THEN** 加载逻辑 SHALL 与默认配置合并，该字段取默认值 `true`，其它已有字段 SHALL NOT 被覆盖

### Requirement: 持久化跳过的更新版本号

应用配置 SHALL 包含 `skippedUpdateVersion` 字段，类型为可空字符串（`null` 或版本号字符串）。缺省值为 `null`；配置文件缺该字段时 SHALL 反序列化为 `null`；持久化时若值为 `null` SHALL 在写盘 JSON 中省略该键以保持文件简洁。该字段用于记录用户主动「跳过此版本」的目标版本号。

#### Scenario: 默认值为空

- **WHEN** 首次启动或老配置文件中无该字段
- **THEN** 配置 `skippedUpdateVersion` SHALL 反序列化为 `null`
- **AND** 持久化时 `skippedUpdateVersion` 字段 SHALL NOT 出现在 JSON 中

#### Scenario: 写入跳过版本

- **WHEN** 调用方通过 `update_config` IPC 传 `{ section: "skippedUpdateVersion", data: "0.3.0" }` 或在 patch 中包含 `skippedUpdateVersion: "0.3.0"`
- **THEN** 系统 SHALL 把 `skippedUpdateVersion = "0.3.0"` 持久化到磁盘
- **AND** 下次读取 SHALL 返回该值

#### Scenario: 清空跳过版本

- **WHEN** 调用方传 `skippedUpdateVersion: null`
- **THEN** 系统 SHALL 把 `skippedUpdateVersion = null` 持久化
- **AND** 下次读取 SHALL 返回 `null`

#### Scenario: 与既有字段合并

- **WHEN** 老配置文件含 triggers / pinned 等字段但无 `skippedUpdateVersion`
- **THEN** 加载逻辑 SHALL 与默认配置合并，保留已有字段，`skippedUpdateVersion` 取默认 `null`，**不**覆盖其他字段

### Requirement: Display config exposes user-customizable font family

系统 SHALL 在 `display` 配置段暴露 `fontSans` 与 `fontMono` 两个可选字段（IPC camelCase），允许用户覆盖 UI 默认 sans / mono 字体。`null` 或缺失字段表示使用应用内置默认字体栈，非空字符串表示用户提供的 CSS `font-family` 值。空白字符串 SHALL 在持久化前归一化为 `null`。任何字段超过 500 字符 SHALL 触发 validation error；同次 update 含任一非法字段 SHALL 整体拒绝（不允许半写状态）。reset 默认时 SHALL 清空两字段为 `null`。

#### Scenario: Font fields default to null on first launch

- **WHEN** 首次启动且配置文件不存在
- **THEN** 物化的默认配置 SHALL 包含 `display.fontSans = null` 与 `display.fontMono = null`

#### Scenario: User sets a custom font

- **WHEN** 调用方更新 `display.fontSans` 或 `display.fontMono` 为合法非空字符串
- **THEN** 该值 SHALL 被持久化，下次读取时返回相同字符串

#### Scenario: Whitespace-only value normalizes to null

- **WHEN** 调用方把 `display.fontSans` 设为仅空白字符
- **THEN** 系统 SHALL 把该字段持久化为 `null`，不保留空白字符串

#### Scenario: Excessively long value rejected atomically

- **WHEN** 调用方一次更新同时设置 `fontSans = <合法值>` 与 `fontMono = <超过 500 字符>`
- **THEN** 整次更新 SHALL 被拒绝并返回 validation error，`display.fontSans` 与 `display.fontMono` 已存储值 SHALL 保持不变

#### Scenario: Reset to defaults clears font overrides

- **WHEN** 用户已设过自定义 `fontSans` / `fontMono`，随后触发 reset_to_defaults
- **THEN** 重置后 `display.fontSans` 与 `display.fontMono` SHALL 都为 `null`

### Requirement: IPC contract exposes font fields in camelCase

`getConfig` 与 `updateConfig` 的 IPC 响应 SHALL 在 `display` 段以 camelCase 字段名 `fontSans` 与 `fontMono` 暴露字体配置，类型为 `string | null`，accept `null` 清空 / 非空字符串覆盖。

#### Scenario: getConfig response shape

- **WHEN** 前端调用 `getConfig`
- **THEN** 响应 `display` 段 SHALL 同时包含 `fontSans` 与 `fontMono` 两个键，值为字符串或 `null`

#### Scenario: updateConfig accepts null or non-empty string

- **WHEN** 前端调用 `updateConfig` 含 `display.fontSans = null` 或非空字符串
- **THEN** 调用 SHALL 成功，下一次 `getConfig` 返回写入值

### Requirement: Display config exposes time format preference

系统 SHALL 在 `display` 配置段暴露 `timeFormat` 字段（IPC camelCase），合法值为枚举 `"24h" | "12h"`，控制 UI 渲染绝对时间戳时是否使用 12 小时制（带 AM/PM 前缀）。**默认值 SHALL 为 `"24h"`**。该字段缺失（旧配置文件兼容）SHALL 反序列化为默认值 `"24h"`。任何非 `"24h"` / `"12h"` 的值 SHALL 触发 validation error，已存储值保持不变。reset 默认时 SHALL 回到 `"24h"`。

#### Scenario: 默认值与旧 config 缺字段兼容

- **WHEN** 首次启动且 settings 文件不存在；或老配置文件解析成功但 `display` 段缺少 `timeFormat` 字段
- **THEN** `display.timeFormat` SHALL 反序列化为 `"24h"`
- **AND** 后续更新写入新值后字段 SHALL 持久化到磁盘

#### Scenario: 合法值切换

- **WHEN** 调用方把 `display.timeFormat` 设为 `"12h"` 或 `"24h"`
- **THEN** 调用 SHALL 成功，下一次 getConfig 返回写入值

#### Scenario: 非法值被拒绝且已存储值不变

- **WHEN** 调用方把 `display.timeFormat` 设为 `"24h" | "12h"` 之外的字符串
- **THEN** 整次 update SHALL 被拒绝并返回 validation error；磁盘上已存储的 `display.timeFormat` SHALL 保持不变

#### Scenario: 重置回默认时 timeFormat 回到 24h

- **WHEN** 调用方调用 reset-to-defaults 或物化全新默认配置
- **THEN** `display.timeFormat` SHALL 为 `"24h"`

### Requirement: IPC contract exposes timeFormat in camelCase

`getConfig` 与 `updateConfig` 的 IPC 响应 SHALL 在 `display` 段以 camelCase 字段名 `timeFormat` 暴露时间格式偏好，值为 `"24h"` 或 `"12h"` 之一。

#### Scenario: getConfig 响应包含 timeFormat 字段

- **WHEN** 前端调用 `getConfig`
- **THEN** 响应 `display` 段 SHALL 包含 `timeFormat` 键，值 ∈ `{"24h", "12h"}`

### Requirement: HTTP server enabled / port SHALL be persisted in lockstep with lifecycle

应用配置 SHALL 持久化 `httpServer.enabled`（bool）与 `httpServer.port`（端口整数）字段到 `~/.claude/claude-devtools-config.json`，缺省值为 `enabled=false` / `port=3456`，配置文件缺该 section 时 SHALL 反序列化为该默认值。该字段 SHALL 与 `server-mode` capability 的 server lifecycle 协同：

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

配置加载 SHALL 在反序列化配置后、暴露给消费方之前，扫描 `pinnedSessions` 字段（key 为 project_id），把含 `"::"` 分隔的 composite id（形如 `{baseDir}::{hash8}`）fold 为 base_dir（即 `"::"` 之前的部分）。fold 时若多个 composite key 共享同一 base_dir，SHALL 把它们的 pinned 数组合并并按 `(sessionId, pinnedAt)` 去重，**保留 `pinnedAt` 最早**的条目（即用户最早 pin 的时间戳）。

迁移触发（即检测到至少一个 composite key）SHALL 在写回配置文件前把当前文件备份到 `<config-path>.pre-merge-composite.bak`（覆盖已存在的同名备份），再原子写入新内容。备份命名与现有「损坏配置自动备份到 `.bak.<unix_timestamp_ms>`」机制独立，便于人工识别本次迁移的回滚点。

迁移 SHALL 是幂等的——纯粹基于 input 重写，不依赖任何「已迁移」标志位。写盘失败时 SHALL 在日志中以 warn 级别记录失败原因，**不**阻塞启动；下次启动加载时命中同样的 composite key 时再次尝试 fold + 写盘。

`hiddenSessions` 等其它以 project_id 为 key 的同形态配置字段 SHALL 同样应用本迁移规则。`NotificationTrigger.repositoryIds` 存的是 repository group 标识（git-common-dir 绝对路径，详见 [[project-discovery]] `Group projects by git worktree` Requirement），与 composite project id 形态完全不同，SHALL NOT 被本迁移触及。

#### Scenario: pinnedSessions 含 composite key 被 fold 为 base_dir

- **WHEN** 配置文件 `pinnedSessions` 含 `"-Users-foo-repo::abcd1234": [{ sessionId: "s1", pinnedAt: 1000 }]` 与 `"-Users-foo-repo::ef567890": [{ sessionId: "s2", pinnedAt: 2000 }]`
- **AND** 配置加载被触发
- **THEN** 加载完成后内存中的 `pinnedSessions` SHALL 含 `"-Users-foo-repo": [{ sessionId: "s1", pinnedAt: 1000 }, { sessionId: "s2", pinnedAt: 2000 }]`（顺序不要求，按 `sessionId` 字典序或 mtime 倒序均可）
- **AND** SHALL NOT 残留 `"-Users-foo-repo::abcd1234"` 或 `"-Users-foo-repo::ef567890"` key

#### Scenario: 同 sessionId 重复条目去重保留 pinnedAt 最早

- **WHEN** 配置文件含 `"D::h1": [{ sessionId: "s", pinnedAt: 200 }]` 与 `"D::h2": [{ sessionId: "s", pinnedAt: 100 }]`
- **AND** 配置加载被触发
- **THEN** 加载完成后 `pinnedSessions["D"]` SHALL 含**且仅含**一条 `{ sessionId: "s", pinnedAt: 100 }`

#### Scenario: 触发迁移时备份原文件

- **WHEN** 配置文件 `<path>` 含至少一条 composite key 且 fold 后内容与原内容不同
- **AND** 配置加载被触发
- **THEN** 系统 SHALL 在写回前把原文件内容写入 `<path>.pre-merge-composite.bak`
- **AND** 备份写盘 SHALL 在主文件原子写入之前完成

#### Scenario: 未含 composite key 不写盘

- **WHEN** 配置文件 `pinnedSessions` 所有 key 均不含 `"::"`
- **AND** 配置加载被触发
- **THEN** 系统 SHALL NOT 写回主配置文件、SHALL NOT 创建 `.pre-merge-composite.bak`

#### Scenario: 写盘失败不阻塞启动

- **WHEN** fold 检测到 composite key 需要写回
- **AND** 原子写入失败（磁盘满 / 权限拒绝）
- **THEN** 系统 SHALL 在日志中以 warn 级别记录失败原因
- **AND** 内存中的 fold 后状态仍 SHALL 暴露给消费方（避免运行时仍持有 composite key）
- **AND** 配置加载 SHALL 正常返回（不返回 Err）

#### Scenario: 迁移是幂等的

- **WHEN** 已 fold 的配置文件（不含 composite key）再次被加载
- **THEN** 系统 SHALL NOT 触发任何写盘
- **AND** 内存中的 `pinnedSessions` SHALL 与配置文件内容字节一致

#### Scenario: NotificationTrigger repositoryIds 不受迁移影响

- **WHEN** 配置文件含一条 trigger，其 `repositoryIds` 字段为 `["/Users/foo/repo/.git"]`
- **AND** `pinnedSessions` 同时含 composite key
- **AND** 配置加载被触发
- **THEN** 加载完成后该 trigger 的 `repositoryIds` SHALL 保持 `["/Users/foo/repo/.git"]` 字节不变（无论是否含 `"::"`）

### Requirement: Persist keyboard shortcut overrides

应用配置 SHALL 持久化用户自定义快捷键覆盖。字段 IPC 名 `keyboardShortcuts`（camelCase），值为 `Record<string, string>` 仅持有用户**覆盖项**（diff），未覆盖的 ID 走 builtin defaults。key 形如 `<category>.<action>`，value 为 normalized binding（含跨平台 `mod` 字面量，详 [[keyboard-shortcuts]]）。该字段缺失（旧版本 config 反序列化）SHALL 反序列化为 empty 视图、不报错；启动 SHALL 走 builtin defaults。

`getConfig` 响应 SHALL 含 `keyboardShortcuts` 字段；`set_config` SHALL 接受 `keyboardShortcuts` 整体覆盖更新（同 `notifications.triggers` 整体替换数组的模式）。empty 视图 SHALL 序列化为 `{}` 出现在 IPC 响应与配置文件两处（**不**省略键）——让前端 / 文件 reader 都不需要 undefined fallback。

#### Scenario: 字段反序列化兼容旧 config

- **WHEN** 读取一个旧版本写入的 config（无 `keyboardShortcuts` 字段）
- **THEN** 视图 SHALL 反序列化为 empty，不报错
- **AND** 启动 SHALL 走 builtin defaults

#### Scenario: 持久化用户覆盖

- **WHEN** 用户通过 Settings 改动某 ID 的 binding
- **AND** 触发 `set_config`
- **THEN** `keyboardShortcuts` 视图 SHALL 更新为含该 ID → 新 binding 一项
- **AND** 写入后的 JSON SHALL 包含该覆盖项

#### Scenario: 空视图序列化为 `{}`

- **WHEN** 用户从未改动任何快捷键
- **AND** 触发 save / 或 `getConfig` IPC 响应
- **THEN** 写出的 JSON SHALL 含 `"keyboardShortcuts": {}`（不省略键）

#### Scenario: IPC 字段 camelCase

- **WHEN** 前端调用 `getConfig` / `set_config`
- **THEN** 响应 / 入参 JSON SHALL 用 `keyboardShortcuts`（camelCase，非 `keyboard_shortcuts`）

### Requirement: 持久化外部编辑器偏好

应用配置 SHALL 持久化 `externalEditor` 字段，合法值集合 = `{system, vs_code, cursor, zed, sublime}`（IPC snake_case 字符串），默认值为 `system`。`update_general` SHALL 校验枚举合法性，invalid 值返回 `ApiError::ValidationError`。该字段控制 `open_in_editor` IPC 选用的外部编辑器 CLI；`system` 时走 OS 默认。

#### Scenario: 默认值与白名单合法值

- **WHEN** 应用首次启动 / 写入合法白名单值
- **THEN** 反序列化与持久化 SHALL 成功，IPC 返回写入值
- **AND** `open_in_editor` 调用走对应 CLI（VS Code: `code --goto`、Cursor / Zed / Sublime 同形态、`system` fallback OS 默认）

#### Scenario: 非白名单值拒绝

- **WHEN** 前端调 `update_general({ externalEditor: <非白名单值> })`
- **THEN** 后端返回 `ApiError::ValidationError`
- **AND** settings 文件 SHALL **不**被修改

### Requirement: 持久化浏览器搜索引擎偏好

应用配置 SHALL 持久化 `searchEngine` 字段（IPC internally-tagged enum），合法 type 集合 = `{google, bing, duck_duck_go, custom}`，默认值为 `google`。`Custom` variant 的 `urlTemplate` SHALL 含 `{query}` 占位符且 scheme ∈ `{http, https}`，否则 `update_general` 返回 `ApiError::ValidationError`。

#### Scenario: 默认值与白名单合法值

- **WHEN** 应用首次启动 / 写入合法白名单值
- **THEN** 反序列化与持久化 SHALL 成功
- **AND** "在浏览器搜索" action 拼接 URL 时按枚举对应模板（Google / Bing / DuckDuckGo / Custom）

#### Scenario: Custom 模板含 {query}

- **WHEN** 前端调 `update_general` 提供合法 Custom URL 模板
- **THEN** 后端 SHALL 持久化
- **AND** 拼接搜索 URL 时 SHALL 用 `urlTemplate.replace("{query}", encodeURIComponent(query))`

#### Scenario: Custom 模板缺 {query} 占位符或危险 scheme 拒绝

- **WHEN** 前端调 `update_general` 提供 `urlTemplate` 缺 `{query}` 或 scheme ∉ `{http, https}`
- **THEN** 后端返回 `ApiError::ValidationError`
- **AND** settings 文件 SHALL **不**被修改

### Requirement: 持久化首选终端 app

应用配置 SHALL 持久化 `terminalApp` 字段，合法值跨平台并集 `{terminal, i_term, warp, windows_terminal, cmd, power_shell, x_terminal_emulator, gnome_terminal, konsole, alacritty}`（IPC snake_case），默认值为 `terminal`。**统一 enum 跨平台合法**——配置文件可移植；运行时打开终端按 `cfg!(target_os)` 判断当前平台，与持久化值不匹配时 SHALL warn 级日志记录 + fallback 到平台默认终端，**不**返回错误（详 D-Impl-1）。

#### Scenario: 平台默认值

- **WHEN** macOS 应用首次启动且 settings 无 `terminalApp` 字段
- **THEN** 反序列化为 `terminal`，IPC 返回 `"terminal"`

#### Scenario: 跨平台不匹配 fallback

- **WHEN** macOS 上 settings 含 `terminalApp = <Windows / Linux 平台值>`（用户从其它平台同步配置过来）
- **THEN** 打开终端调用 SHALL warn 级日志记录 mismatch
- **AND** SHALL fallback 到 macOS 平台默认终端
- **AND** **不**返回 `ApiError`，菜单点击仍成功打开终端

### Requirement: list_available_terminals IPC

应用 SHALL 提供 `list_available_terminals` Tauri command 返回当前平台支持的 `terminalApp` 枚举值列表（snake_case 字符串数组），用于前端 Settings dropdown 过滤合法选项。返回值仅包含当前 OS 的合法终端集合，不含其它平台的 enum 值。

#### Scenario: 当前平台返回值

- **WHEN** 前端调 `list_available_terminals`
- **THEN** 后端 SHALL 按 `cfg!(target_os)` 返回当前平台合法集合：
  - macOS：`["terminal", "i_term", "warp"]`
  - Windows：`["windows_terminal", "cmd", "power_shell"]`
  - Linux：`["x_terminal_emulator", "gnome_terminal", "konsole", "alacritty"]`

### Requirement: Optimistic concurrency control for config updates

系统 SHALL 在配置读取响应中附加版本号，在配置更新请求中接受版本号并做乐观并发检查，以防止多客户端并发写入导致静默覆盖。

#### Scenario: get_config returns version field

- **WHEN** 调用方请求当前配置
- **THEN** 返回的 JSON 顶层 SHALL 包含 `_version` 字段，值为 `u64` 类型
- **AND** `_version` 的值 SHALL 等于 `ConfigManager` 当前内部 version

#### Scenario: update_config with matching version succeeds

- **WHEN** 调用方发送 update 请求，`configData` 中携带 `_version` 等于服务端当前 version
- **THEN** 更新 SHALL 成功
- **AND** 返回的配置 SHALL 包含递增后的新 `_version`

#### Scenario: update_config with stale version fails

- **WHEN** 调用方发送 update 请求，`configData` 中携带 `_version` 小于服务端当前 version
- **THEN** 系统 SHALL 返回包含 "Config version mismatch" 的错误信息
- **AND** 配置 SHALL 未被修改

#### Scenario: update_config without version is backward-compatible

- **WHEN** 调用方发送 update 请求，`configData` 中不含 `_version` 字段
- **THEN** 系统 SHALL 跳过版本检查，正常处理更新
- **AND** 这保证旧客户端 / CLI 工具的向后兼容

#### Scenario: Frontend shows conflict toast on version mismatch

- **WHEN** 前端发送 update 请求被拒（version mismatch）
- **THEN** 前端 SHALL 弹出 error 级别 toast 提示用户
- **AND** 前端 SHALL 自动重新获取最新配置以同步本地状态


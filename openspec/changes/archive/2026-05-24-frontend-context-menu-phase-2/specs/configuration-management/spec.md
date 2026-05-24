## ADDED Requirements

### Requirement: 持久化外部编辑器偏好

`GeneralConfig` SHALL 含 `externalEditor` 字段（serde rename: `external_editor`），类型为枚举 `ExternalEditor` 的扁平 snake_case 序列化，合法值为 `system | vs_code | cursor | zed | sublime`，默认值为 `system`。`update_general` IPC SHALL 接受该字段并通过 `serde_json::from_value::<ExternalEditor>` 校验枚举合法性后写入 settings 文件，invalid 值返回 `ApiError::ValidationError`。

#### Scenario: 默认值

- **WHEN** 应用首次启动且 settings 文件无 `externalEditor` 字段
- **THEN** `GeneralConfig` SHALL 反序列化为 `ExternalEditor::System`
- **AND** IPC 返回的 `externalEditor` 字段值 SHALL 为 `"system"`

#### Scenario: 更新到 vs_code

- **WHEN** 前端调 `update_general({ externalEditor: "vs_code" })`
- **THEN** 后端 SHALL 反序列化成 `ExternalEditor::VsCode` 并持久化到 settings 文件
- **AND** 后续 `get_general` IPC 返回 `{ externalEditor: "vs_code", ... }`
- **AND** `open_in_editor` IPC 后续调用 SHALL 走 `code --goto path:line:col`

#### Scenario: invalid 值拒绝

- **WHEN** 前端调 `update_general({ externalEditor: "vim" })`
- **THEN** 后端 SHALL 返回 `ApiError { code: ValidationError, message: "..." }`
- **AND** settings 文件 SHALL **不**被修改

### Requirement: 持久化浏览器搜索引擎偏好

`GeneralConfig` SHALL 含 `searchEngine` 字段（serde rename: `search_engine`），类型为 internally-tagged 枚举 `SearchEngine`：`{ "type": "google" }` | `{ "type": "bing" }` | `{ "type": "duck_duck_go" }` | `{ "type": "custom", "urlTemplate": "<URL>" }`。`Custom` variant 的 `urlTemplate` SHALL 包含 `{query}` 占位符，否则 `update_general` 返回 `ApiError::ValidationError`。默认值为 `Google`。

#### Scenario: 默认 Google

- **WHEN** 应用首次启动且 settings 无 `searchEngine` 字段
- **THEN** `GeneralConfig` SHALL 反序列化为 `SearchEngine::Google`
- **AND** IPC 返回的 `searchEngine` 字段值 SHALL 为 `{ "type": "google" }`

#### Scenario: 切到 DuckDuckGo

- **WHEN** 前端调 `update_general({ searchEngine: { type: "duck_duck_go" } })`
- **THEN** 后端 SHALL 反序列化成 `SearchEngine::DuckDuckGo` 并持久化
- **AND** 前端"在浏览器搜索"action 拼接 URL 时走 `https://duckduckgo.com/?q=<encoded-query>`

#### Scenario: 设置 Custom 模板含 {query}

- **WHEN** 前端调 `update_general({ searchEngine: { type: "custom", urlTemplate: "https://example.com/search?q={query}" } })`
- **THEN** 后端 SHALL 校验 `urlTemplate` 含 `{query}` 占位符并持久化
- **AND** 拼接搜索 URL 时 SHALL 用 `urlTemplate.replace("{query}", encodeURIComponent(query))`

#### Scenario: Custom 模板缺 {query} 拒绝

- **WHEN** 前端调 `update_general({ searchEngine: { type: "custom", urlTemplate: "https://example.com/search" } })`
- **THEN** 后端 SHALL 返回 `ApiError { code: ValidationError, message: "urlTemplate must contain {query} placeholder" }`
- **AND** settings 文件 SHALL **不**被修改

### Requirement: 持久化首选终端 app

`GeneralConfig` SHALL 含 `terminalApp` 字段（serde rename: `terminal_app`），类型为统一扁平枚举 `TerminalApp`：合法值跨平台并集 `terminal | i_term | warp | windows_terminal | cmd | power_shell | x_terminal_emulator | gnome_terminal | konsole | alacritty`，默认值为 `Terminal`。**统一 enum 而非 per-platform enum**——配置文件跨平台可移植，运行时 `open_in_terminal` 根据 `cfg!(target_os)` 判断与当前平台不匹配时 `tracing::warn!` + fallback 到平台默认终端，**不**报错。

#### Scenario: macOS 默认值

- **WHEN** macOS 应用首次启动且 settings 无 `terminalApp` 字段
- **THEN** `GeneralConfig` SHALL 反序列化为 `TerminalApp::Terminal`
- **AND** IPC 返回的 `terminalApp` 字段值 SHALL 为 `"terminal"`

#### Scenario: 切到 iTerm

- **WHEN** 前端调 `update_general({ terminalApp: "i_term" })`（注意 ITerm 序列化为 `i_term` 不是 `iterm`）
- **THEN** 后端 SHALL 反序列化成 `TerminalApp::ITerm` 并持久化
- **AND** 后续 `open_in_terminal` 调用 SHALL 走 `open -a iTerm <path>`

#### Scenario: 跨平台不匹配 fallback

- **WHEN** macOS 上 settings 含 `terminalApp: "windows_terminal"`（用户从 Windows 同步配置过来）
- **THEN** `open_in_terminal` 调用时 SHALL `tracing::warn!` 记录 mismatch
- **AND** SHALL fallback 到 macOS 平台默认 `TerminalApp::Terminal`
- **AND** **不**返回 `ApiError`，菜单点击仍成功打开终端

### Requirement: list_available_terminals IPC

应用 SHALL 提供 `list_available_terminals` Tauri command 返回当前平台支持的 `TerminalApp` 枚举值列表（snake_case 字符串数组），用于前端 Settings dropdown 过滤合法选项。返回值仅包含当前 OS 的合法终端集合，不含其它平台的 enum 值。

#### Scenario: macOS 返回值

- **WHEN** macOS 上前端调 `list_available_terminals`
- **THEN** 返回 `["terminal", "i_term", "warp"]`

#### Scenario: Windows 返回值

- **WHEN** Windows 上前端调 `list_available_terminals`
- **THEN** 返回 `["windows_terminal", "cmd", "power_shell"]`

#### Scenario: Linux 返回值

- **WHEN** Linux 上前端调 `list_available_terminals`
- **THEN** 返回 `["x_terminal_emulator", "gnome_terminal", "konsole", "alacritty"]`

## MODIFIED Requirements

### Requirement: Validate configuration fields before persistence

系统 SHALL 对传入的配置更新做校验（HTTP 端口范围、regex 模式、文件路径等），非法值 SHALL 被拒绝并附错误说明，不写入坏状态。

HTTP 端口校验 SHALL 同时应用于：(a) 通过 `update_config` IPC 直接更新 `httpServer.port` 字段；(b) 通过 `http_server_start(port)` IPC 间接持久化 `httpServer.port` 字段（详 [[server-mode]]）。两条路径 SHALL 共用同一 `cdt_config::validate_http_port` 实现，保证端口语义一致——任何能存入 `httpServer.port` 的值都已通过 1024–65535 范围校验。

`update_general` IPC handler 在前述基础上 SHALL 同时校验 Phase 2 加入的 GeneralConfig 三字段：(a) `externalEditor` / `terminalApp` 走 `serde_json::from_value::<EnumType>` 严格枚举校验，invalid 值返回 `ApiError::ValidationError`；(b) `searchEngine` 走 internally tagged enum 反序列化，且 `Custom` variant SHALL 额外校验 `urlTemplate` 含 `{query}` 占位符 + scheme ∈ `{http, https}`（拒绝 `javascript:` / `file:` / `data:` 等危险 scheme）；(c) `terminalApp` 跨平台不匹配 SHALL **不**触发 ValidationError——保留写入并在运行时 `open_in_terminal` 调用时 `tracing::warn!` + fallback 到当前平台默认终端。任何字段校验失败 SHALL 整体拒绝更新，settings 文件保持原值。

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

- **WHEN** 前端调 `update_general({ unknownField: "value" })`
- **THEN** 后端 SHALL 返回 `ApiError::ValidationError`
- **AND** settings 文件 SHALL **不**被修改

#### Scenario: 枚举非法值拒绝

- **WHEN** 前端调 `update_general({ externalEditor: "emacs" })` 或 `update_general({ terminalApp: "fish" })`
- **THEN** `serde_json::from_value::<ExternalEditor>` / `<TerminalApp>` SHALL 失败并返回 `ApiError::ValidationError`
- **AND** settings 文件 SHALL **不**被修改

#### Scenario: SearchEngine.custom 缺 {query} 占位符拒绝

- **WHEN** 前端调 `update_general({ searchEngine: { type: "custom", urlTemplate: "https://example.com/search" } })`
- **THEN** 后端 SHALL 校验 `urlTemplate` 含 `{query}` 失败并返回 `ApiError::ValidationError`
- **AND** settings 文件 SHALL **不**被修改

#### Scenario: SearchEngine.custom 危险 scheme 拒绝

- **WHEN** 前端调 `update_general({ searchEngine: { type: "custom", urlTemplate: "javascript:alert({query})" } })` 或 scheme 为 `file://` / `data:` / `chrome://`
- **THEN** 后端 SHALL 校验 URL scheme ∈ `{http, https}` 失败并返回 `ApiError::ValidationError("urlTemplate scheme must be http or https")`
- **AND** settings 文件 SHALL **不**被修改

#### Scenario: terminalApp 跨平台值不报错

- **WHEN** macOS 上前端调 `update_general({ terminalApp: "konsole" })`（Linux 平台值）
- **THEN** 后端 SHALL 接受并持久化（统一 enum 跨平台合法）
- **AND** 后续 `open_in_terminal` 调用时 SHALL `tracing::warn!` + fallback 到 macOS 默认终端

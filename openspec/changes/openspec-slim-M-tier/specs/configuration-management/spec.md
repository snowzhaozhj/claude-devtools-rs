## MODIFIED Requirements

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

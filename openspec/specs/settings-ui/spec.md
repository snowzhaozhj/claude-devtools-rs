# settings-ui Specification

## Purpose

定义 Settings 页面的行为契约：打开方式（TabBar 齿轮单例 tab）、section 导航（General / Notifications）、配置展示与修改、trigger 列表的启用 / 禁用与持久化。所有布尔开关 SHALL 走统一的 `SettingsToggle` 滑块组件以保证视觉一致与无障碍可达。
## Requirements
### Requirement: 打开 Settings 页面

用户 SHALL 能通过 TabBar 齿轮图标打开 Settings 页面。Settings tab SHALL 为单例——若已打开则切换焦点。

#### Scenario: 点击齿轮图标打开 Settings
- **WHEN** 用户点击 TabBar 的齿轮图标且无 Settings tab
- **THEN** 系统 SHALL 创建 type 为 "settings" 的 tab 并设为 active

#### Scenario: 重复点击齿轮图标
- **WHEN** 已有 Settings tab 时用户再次点击齿轮图标
- **THEN** 系统 SHALL 切换焦点到已有 Settings tab

### Requirement: Settings Section 导航

Settings 页面 SHALL 包含 section tab 导航。MVP 阶段 SHALL 支持 General 和 Notifications 两个 section。

#### Scenario: 默认显示 General section
- **WHEN** Settings 页面首次打开
- **THEN** SHALL 默认显示 General section

#### Scenario: 切换到 Notifications section
- **WHEN** 用户点击 Notifications section tab
- **THEN** SHALL 显示通知配置内容

### Requirement: General Section 展示

General section SHALL 展示当前配置值。MVP 阶段 SHALL 至少展示 theme 设置与 Claude 数据根目录设置。Claude 数据根目录设置 SHALL 显示当前 `general.claudeRootPath`；当值为 `null` 时，UI SHALL 明确展示正在使用默认 Claude root。

#### Scenario: 展示当前 theme
- **WHEN** General section 渲染
- **THEN** SHALL 显示当前 theme 值（dark/light/system）

#### Scenario: 展示默认 Claude root
- **WHEN** General section 渲染且 `general.claudeRootPath = null`
- **THEN** SHALL 显示“使用默认 Claude 目录”或等价提示
- **AND** SHALL 显示默认 root 说明，帮助用户理解项目来自默认 `.claude/projects`

#### Scenario: 展示自定义 Claude root
- **WHEN** General section 渲染且 `general.claudeRootPath = "/data/claude-alt"`
- **THEN** SHALL 在输入框或等价控件中显示 `/data/claude-alt`

### Requirement: Notifications Section 展示

Notifications section SHALL 展示通知全局开关和 trigger 列表。所有开关（`enabled` / `soundEnabled` / 每个 trigger 启用状态）MUST 使用 `SettingsToggle` 滑块组件，而非文字按钮，以便用户能一眼分辨开/关状态。trigger 启用态切换 SHALL 通过 `update_config("notifications", { triggers: [...] })` 路径持久化，并依赖 `configuration-management` 的 "Update notifications SHALL accept full triggers replacement" requirement 保证真正落盘与内存同步。

#### Scenario: 展示通知开关

- **WHEN** Notifications section 渲染
- **THEN** SHALL 显示 enabled 和 soundEnabled 的开关状态，使用 `SettingsToggle` 滑块组件

#### Scenario: Toggle 通知开关

- **WHEN** 用户切换 enabled 开关
- **THEN** 系统 SHALL 调用 update_config API 更新 notifications section，成功后更新 UI 状态

#### Scenario: 展示 trigger 列表

- **WHEN** Notifications section 渲染且配置中有 triggers
- **THEN** SHALL 显示 trigger 名称、颜色、启用状态列表；每个 trigger 的启用状态 SHALL 使用 `SettingsToggle` 滑块呈现

#### Scenario: Toggle 单个 trigger 启用态

- **WHEN** 用户切换某个 trigger 的 `SettingsToggle`
- **THEN** 系统 SHALL 乐观更新本地 `config.notifications.triggers[i].enabled`、调用 `update_config("notifications", { triggers: [...] })`
- **AND** 成功后 UI SHALL 保持新状态；失败时 SHALL 重新 `get_config` 回滚并显示错误

### Requirement: 配置加载与错误处理

Settings 页面打开时 SHALL 从后端加载配置。加载失败 SHALL 显示错误提示。用户修改配置时，UI SHALL 先乐观更新本地状态并调用后端；后端失败时 SHALL 重新 `get_config` 回滚并显示错误。

#### Scenario: 配置加载成功
- **WHEN** Settings 页面打开
- **THEN** SHALL 调用 get_config API，显示 loading 状态，成功后渲染配置内容

#### Scenario: 配置加载失败
- **WHEN** get_config API 调用失败
- **THEN** SHALL 显示错误提示，不崩溃

#### Scenario: 通过系统目录选择器保存自定义 Claude root
- **WHEN** 用户在 General section 中点击选择目录并从系统文件管理器选择 `/data/claude-alt`
- **THEN** UI SHALL 调用 `update_config("general", { claudeRootPath: "/data/claude-alt" })`
- **AND** 成功后 UI SHALL 保持该路径为当前值

#### Scenario: 手动输入保存自定义 Claude root
- **WHEN** 用户在 General section 中手动输入绝对路径 `/data/claude-alt` 并保存
- **THEN** UI SHALL 调用 `update_config("general", { claudeRootPath: "/data/claude-alt" })`
- **AND** 成功后 UI SHALL 保持该路径为当前值

#### Scenario: 清空 Claude root 恢复默认
- **WHEN** 用户清空 Claude root 输入并保存或点击恢复默认控件
- **THEN** UI SHALL 调用 `update_config("general", { claudeRootPath: null })`
- **AND** 成功后 UI SHALL 显示默认 Claude root 状态

#### Scenario: 相对路径保存失败并回滚
- **WHEN** 用户输入相对路径 `relative/path` 并保存
- **AND** 后端返回 validation error
- **THEN** UI SHALL 显示错误提示
- **AND** UI SHALL 重新加载配置并恢复到保存前的 `general.claudeRootPath`

### Requirement: 布尔开关视觉规范统一为滑块样式

Settings 页面中所有布尔开关（通用区 `autoExpandAiGroups`、通知区 `enabled` / `soundEnabled`、以及每个 trigger 的启用态）SHALL 使用统一的 `SettingsToggle` 组件——Linear 风格的滑块 Switch。该组件 MUST 表达以下可辨识状态：未启用（灰色 track + 左侧 thumb）、启用（紫色 track + 右侧 thumb）、禁用（整体半透明且不可点击）。组件 API MUST 提供 `enabled: boolean / onChange: (v: boolean) => void / disabled?: boolean` 三个属性，并在按钮元素上设置 `role="switch"` + `aria-checked` 以保证可访问性。

#### Scenario: 启用态显示紫色 track + 右侧 thumb

- **WHEN** `enabled=true`
- **THEN** 组件 SHALL 渲染紫色 track + thumb 位于右侧
- **AND** `aria-checked` SHALL 为 `true`

#### Scenario: 未启用态显示灰色 track + 左侧 thumb

- **WHEN** `enabled=false`
- **THEN** 组件 SHALL 渲染灰色 track + thumb 位于左侧
- **AND** `aria-checked` SHALL 为 `false`

#### Scenario: 点击触发 onChange

- **WHEN** 用户点击组件且 `disabled=false`
- **THEN** 组件 SHALL 调用 `onChange(!enabled)`

#### Scenario: disabled 态不响应点击

- **WHEN** `disabled=true`
- **THEN** 组件 SHALL 渲染 50% 透明度 + 光标 `not-allowed`
- **AND** 点击 SHALL NOT 触发 `onChange`

### Requirement: General section "Use WSL" 按钮

General section SHALL 在 `claudeRootPath` 输入控件下方提供 "Use WSL" 按钮。该按钮 SHALL 仅在 Windows 平台显示。点击按钮 SHALL 触发 `list_wsl_distros` IPC 调用，IPC 返回结构为 `WslDistroScanReport { candidates, distrosWithoutHome }`，UI SHALL 按以下分支处理：

- `candidates.length == 1`：SHALL 自动调用 `update_config("general", { claudeRootPath: candidates[0].claudeRootPath })`，并通过 toast 或 inline 文案提示已切换
- `candidates.length >= 2`：SHALL 显示 distro 选择 modal（apply 阶段新建的共享 `Modal.svelte` 组件），用户选择后 SHALL 调用 `update_config` 应用所选 candidate
- `candidates.length == 0 && distrosWithoutHome.length == 0`：SHALL 在按钮下方显示 inline 文案"未检测到 WSL distro"
- `candidates.length == 0 && distrosWithoutHome.length > 0`：SHALL 在按钮下方显示 inline 文案"检测到 WSL distro 但无法解析 home"，并附 `distrosWithoutHome` 的 distro 名列表（用于排查）
- IPC 调用失败：SHALL 显示 inline 错误提示

distro 选择 modal SHALL 在每个候选项展示 `distro` 名、`claudeRootPath` UNC 路径，并在 `claudeRootExists = false` 时附加视觉提示（例"该 distro 内尚无 Claude 数据"）但**不**禁用选择。

#### Scenario: 非 Windows 平台不显示按钮

- **WHEN** Settings 页面在 macOS 或 Linux 上渲染
- **THEN** General section SHALL NOT 渲染 "Use WSL" 按钮

#### Scenario: Windows 平台单 distro 自动应用

- **WHEN** Windows 用户点击 "Use WSL" 按钮
- **AND** `list_wsl_distros` 返回单个 candidate `{ distro: "Ubuntu", claudeRootPath: "\\\\wsl.localhost\\Ubuntu\\home\\alice\\.claude", ... }`
- **THEN** 系统 SHALL 调用 `update_config("general", { claudeRootPath: "\\\\wsl.localhost\\Ubuntu\\home\\alice\\.claude" })`
- **AND** SHALL 在 UI 显示成功提示
- **AND** SHALL NOT 弹出 modal

#### Scenario: Windows 平台多 distro 弹选择

- **WHEN** Windows 用户点击 "Use WSL" 按钮
- **AND** `list_wsl_distros` 返回 `["Ubuntu", "Debian-12"]` 两个 candidate
- **THEN** SHALL 弹出 distro 选择 modal
- **AND** modal SHALL 列出两个 candidate 的 distro 名与 `claudeRootPath`
- **AND** 用户选定 `Debian-12` 并确认后 SHALL 调用 `update_config("general", { claudeRootPath: "\\\\wsl.localhost\\Debian-12\\..." })`

#### Scenario: distro 内尚无 Claude 数据

- **WHEN** modal 渲染某 candidate 时 `claudeRootExists = false`
- **THEN** SHALL 在该候选行显示视觉提示文案（例"该 distro 内尚无 Claude 数据"）
- **AND** SHALL 仍允许用户选择该候选

#### Scenario: WSL 未检测到

- **WHEN** Windows 用户点击 "Use WSL" 按钮
- **AND** `list_wsl_distros` 返回 `{ candidates: [], distrosWithoutHome: [] }`
- **THEN** SHALL 在按钮下方显示 inline 文案"未检测到 WSL distro"
- **AND** SHALL NOT 弹出 modal
- **AND** SHALL NOT 调用 `update_config`

#### Scenario: 检测到 distro 但全部 home 解析失败

- **WHEN** Windows 用户点击 "Use WSL" 按钮
- **AND** `list_wsl_distros` 返回 `{ candidates: [], distrosWithoutHome: ["Ubuntu", "Debian-12"] }`
- **THEN** SHALL 在按钮下方显示 inline 文案"检测到 WSL distro 但无法解析 home"
- **AND** 文案 SHALL 包含 `Ubuntu` 与 `Debian-12` 的 distro 名供用户排查
- **AND** SHALL NOT 弹出 modal
- **AND** SHALL NOT 调用 `update_config`

#### Scenario: IPC 调用失败

- **WHEN** Windows 用户点击 "Use WSL" 按钮
- **AND** `list_wsl_distros` IPC 调用返回错误
- **THEN** SHALL 在按钮下方显示 inline 错误提示
- **AND** SHALL NOT 弹出 modal
- **AND** SHALL NOT 调用 `update_config`


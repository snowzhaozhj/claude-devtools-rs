## ADDED Requirements

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

## MODIFIED Requirements

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

## ADDED Requirements

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

General section SHALL 展示当前配置值。MVP 阶段 SHALL 至少展示 theme 设置。

#### Scenario: 展示当前 theme
- **WHEN** General section 渲染
- **THEN** SHALL 显示当前 theme 值（dark/light/system）

### Requirement: Notifications Section 展示

Notifications section SHALL 展示通知全局开关和 trigger 列表。

#### Scenario: 展示通知开关
- **WHEN** Notifications section 渲染
- **THEN** SHALL 显示 enabled 和 soundEnabled 的开关状态

#### Scenario: Toggle 通知开关
- **WHEN** 用户切换 enabled 开关
- **THEN** 系统 SHALL 调用 update_config API 更新 notifications section，成功后更新 UI 状态

#### Scenario: 展示 trigger 列表
- **WHEN** Notifications section 渲染且配置中有 triggers
- **THEN** SHALL 显示 trigger 名称、颜色、启用状态列表

### Requirement: 配置加载与错误处理

Settings 页面打开时 SHALL 从后端加载配置。加载失败 SHALL 显示错误提示。

#### Scenario: 配置加载成功
- **WHEN** Settings 页面打开
- **THEN** SHALL 调用 get_config API，显示 loading 状态，成功后渲染配置内容

#### Scenario: 配置加载失败
- **WHEN** get_config API 调用失败
- **THEN** SHALL 显示错误提示，不崩溃

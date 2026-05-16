## MODIFIED Requirements

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

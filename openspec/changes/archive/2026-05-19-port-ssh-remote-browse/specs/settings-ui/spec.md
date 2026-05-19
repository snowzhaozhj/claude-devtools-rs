## MODIFIED Requirements

### Requirement: Settings Section 导航

Settings 页面 SHALL 包含 section tab 导航。SHALL 支持 General、Notifications、Connection 三个 section。Connection section SHALL 仅在 Tauri 桌面 host 渲染（HTTP standalone 模式 hide）；判定方式为前端检测 `window.__TAURI_INTERNALS__` 是否存在或后端通过 `runtime_info()` IPC 返回 `kind: "tauri"`。

#### Scenario: 默认显示 General section

- **WHEN** Settings 页面首次打开
- **THEN** SHALL 默认显示 General section

#### Scenario: 切换到 Notifications section

- **WHEN** 用户点击 Notifications section tab
- **THEN** SHALL 显示通知配置内容

#### Scenario: 切换到 Connection section

- **WHEN** 用户点击 Connection section tab（仅 Tauri 桌面可见）
- **THEN** SHALL 显示 SSH 连接配置内容

#### Scenario: standalone 模式隐藏 Connection tab

- **WHEN** Settings 页面在 HTTP standalone 模式下渲染（无 Tauri runtime）
- **THEN** Section 导航 SHALL NOT 渲染 Connection tab
- **AND** 即使 URL 含 `#connection` hash，SHALL 静默回退到 General section

## ADDED Requirements

### Requirement: Connection Section 展示与 SSH 连接表单

Connection section SHALL 提供 SSH 连接管理界面。表单字段 SHALL 包含：

- `host`：combobox 控件，下拉联想列表来自 `ssh_get_config_hosts` IPC 调用结果（`~/.ssh/config` 中所有 Host alias）；用户也可手输非 alias 的 hostname
- `port`：默认 22；当用户在 host combobox 选中 alias 后 SHALL 自动调 `ssh_resolve_host` 并填充解析得到的 port
- `username`：同 port，alias 选中后自动填充
- `authMethod`：单选控件，选项 `sshConfig`（默认，使用鉴权候选链） / `password`
- `password`：单行输入框，仅在 `authMethod === "password"` 时可见；密码值 SHALL 仅 in-memory 持有，绝不通过 `ssh_save_last_connection` 持久化

控件区下方 SHALL 显示三个按钮：`Connect` / `Test connection` / `Save as profile`；当 active context 已是 SSH 时 SHALL 同时显示 `Disconnect` 按钮。已保存的 profiles（来自 `ssh.profiles[]`）SHALL 显示为按钮列表，点击一键填充表单。当前连接状态 SHALL 通过 `ConnectionStatusBadge` 组件展示（图标 + 状态文字）。Windows 平台 SHALL 在表单上方显示 inline 提示"v1 Windows 仅支持密码模式或 IdentityFile 直读，命名管道 ssh-agent 计划在 v2 加入"。

#### Scenario: 默认渲染状态

- **WHEN** Connection section 首次渲染且 `ssh.last_connection` 未持久化
- **THEN** 表单 SHALL 渲染空 host / port=22 / 空 username / authMethod="sshConfig"
- **AND** ConnectionStatusBadge SHALL 显示 `disconnected`

#### Scenario: host combobox 联想 ssh config

- **WHEN** 用户聚焦 host 输入框
- **THEN** 系统 SHALL 调 `ssh_get_config_hosts` IPC 拿 alias 列表
- **AND** 输入框下方 SHALL 显示联想下拉，按用户当前输入子串 fuzzy 过滤

#### Scenario: 选中 alias 自动填充

- **WHEN** 用户在 combobox 中选中 alias `myserver`
- **THEN** 前端 SHALL 调 `ssh_resolve_host("myserver")` 取解析结果
- **AND** 解析得到的 port / user / identityFile 字段 SHALL 自动填充表单（用户仍可覆盖）

#### Scenario: password 模式只读 password 字段不持久化

- **WHEN** 用户选 `authMethod = password`
- **AND** 输入密码 `secret` 后点击 Connect
- **THEN** 前端 SHALL 调 `ssh_connect` 携带 password 字段
- **AND** 即使用户后点击 `Save as profile`，前端 SHALL 调 `ssh_save_last_connection` 时 SHALL NOT 携带 password 字段
- **AND** 持久化结果 SHALL 仅含 host / port / username / authMethod 四字段

#### Scenario: Test connection 不切换 active context

- **WHEN** 用户填好表单点击 `Test connection` 且当前 active 是 `Local`
- **THEN** 前端 SHALL 调 `ssh_test_connection` IPC
- **AND** 响应成功后 SHALL 在 inline 区显示"测试成功"提示
- **AND** active context SHALL 仍是 `Local`，未注册新 SSH context

#### Scenario: Connect 成功切换 active context

- **WHEN** 用户填好表单点击 `Connect`
- **THEN** 前端 SHALL 调 `ssh_connect` IPC
- **AND** 响应成功后 ConnectionStatusBadge SHALL 切到 `connected`
- **AND** ContextSwitchOverlay 全屏 loading SHALL 在切换期间短暂出现并在 `context_changed` 事件后退场

#### Scenario: Connect 失败显示 auth chain 诊断

- **WHEN** 用户点击 `Connect` 但鉴权全部失败
- **THEN** 前端 SHALL 从 `ssh_status` 事件 payload 的 `error.attempts[]` 渲染逐源诊断列表
- **AND** 每行 SHALL 含 source 名 + outcome（成功/失败/跳过）+ reason

#### Scenario: Save as profile 持久化（无密码）

- **WHEN** 用户填好表单点击 `Save as profile` 并输入 profile 名 `prod-server`
- **THEN** 前端 SHALL 调 `update_config("ssh", { profiles: [..., { name: "prod-server", host, port, username, authMethod, passwordRequired }] })`
- **AND** 后续 saved profiles 列表 SHALL 渲染该按钮，点击一键填充表单

#### Scenario: Disconnect 按钮回退到 Local

- **WHEN** 当前 active 是 SSH context，用户点击 `Disconnect`
- **THEN** 前端 SHALL 调 `ssh_disconnect` IPC
- **AND** 响应成功后 active context SHALL 切回 `Local`
- **AND** 表单字段保持原值（不清空，方便用户重连）

#### Scenario: Windows 平台显示限制提示

- **WHEN** Connection section 在 Windows 平台渲染
- **THEN** 表单上方 SHALL 显示 inline 提示"v1 Windows 仅支持密码模式或 IdentityFile 直读，命名管道 ssh-agent 计划在 v2 加入"
- **AND** 表单功能照常工作（用户可选 password 模式正常连接）

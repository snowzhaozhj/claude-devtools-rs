## MODIFIED Requirements

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

### Requirement: Validate configuration fields before persistence

系统 SHALL 对传入的配置更新做校验（HTTP 端口范围、regex 模式、文件路径、SSH 字段等），非法值 SHALL 被拒绝并附错误说明，不写入坏状态。

SSH 字段 SHALL 满足：`host` 非空字符串、`port` 整数 1-65535、`username` 非空字符串、`authMethod` 取值 `sshConfig` 或 `password` 之一；profile 的 `name` 非空且在同一 profiles 数组中唯一。任意 SSH 字段非法 SHALL 拒绝整个 update 不作部分写入。

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

#### Scenario: Reject SSH profile with empty host

- **WHEN** 调用方提交 `update_config("ssh", { profiles: [{ name: "p", host: "", port: 22, username: "u", authMethod: "sshConfig", passwordRequired: false }] })`
- **THEN** 更新 SHALL 被拒绝并返回 `validation_error: ssh.profiles[0].host must be non-empty`
- **AND** 已存储 profiles SHALL 保持不变

#### Scenario: Reject SSH profile with port out of range

- **WHEN** 调用方提交某 profile `port = 70000`
- **THEN** 更新 SHALL 被拒绝并返回 `validation_error: ssh.profiles[i].port must be 1-65535`

#### Scenario: Reject duplicate SSH profile name

- **WHEN** 调用方提交 `profiles` 数组中两个条目 `name` 相同
- **THEN** 更新 SHALL 被拒绝并返回 `validation_error: ssh.profiles names must be unique`

#### Scenario: Reject invalid authMethod

- **WHEN** 调用方提交某 profile `authMethod = "kerberos"`
- **THEN** 更新 SHALL 被拒绝并返回 `validation_error: ssh.profiles[i].authMethod must be one of [sshConfig, password]`

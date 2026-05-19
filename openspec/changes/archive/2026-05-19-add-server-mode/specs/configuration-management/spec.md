## ADDED Requirements

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

## MODIFIED Requirements

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

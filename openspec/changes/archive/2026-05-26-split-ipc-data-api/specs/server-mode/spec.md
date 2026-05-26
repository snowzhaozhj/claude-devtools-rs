# server-mode Specification (delta)

## ADDED Requirements

### Requirement: IPC SHALL expose http_server_start / _stop / _status commands

`LocalDataApi` 所在的 Tauri webview IPC 通道 SHALL 暴露 3 个 server-mode 控制 command。这 3 个 command 不属于 `DataApi` trait（与 server lifecycle 强绑定，复用 trait 没有意义），SHALL 在 `src-tauri/src/lib.rs::invoke_handler!` 中直接注册并由 `src-tauri/src/server_mode.rs`（实现期新文件）负责实现。

字段契约（camelCase）：

- **`http_server_start`**：入参 `{ port: number }`（数字 1024–65535），返回 `Result<null, string>`。`Err` 文案 SHALL 携带 specific 类别（端口范围错误 / 端口冲突 / 其它 IO 错误）以便 UI 区分展示。
- **`http_server_stop`**：无入参，返回 `Result<null, string>`。幂等——server 未运行时仍返回 `Ok`。
- **`http_server_status`**：无入参，返回 `{ running: boolean, port: number, lastError: string | null }`。`running` 反映当前 server task 实际状态；`port` 为最近一次成功启动或持久化的端口（即使当前 `running=false`）；`lastError` SHALL 在最近一次启动失败（含自动恢复阶段）时携带错误文案，成功启动后 SHALL 重置为 `null`，让 Settings UI 在挂载时主动查询即可拿到错误原因（不依赖 `http-server-status` event 的 listener 注册时序）。

3 个 command 名 SHALL 出现在以下 5 处保持同步：

1. `src-tauri/src/lib.rs::invoke_handler!`（注册）
2. `crates/cdt-api/tests/ipc_contract.rs::EXPECTED_TAURI_COMMANDS`（contract 列表）
3. `crates/cdt-api/tests/ipc_contract.rs` 内对应 contract test（断言入参 / 返回字段 camelCase）
4. `ui/src/lib/tauriMock.ts::KNOWN_TAURI_COMMANDS`（前端 vitest mock 入口）
5. `ui/src/lib/api.ts`（前端 wrapper 函数声明）

#### Scenario: http_server_start 字段契约

- **WHEN** contract test 模拟前端调用 `http_server_start({ port: 3456 })`
- **THEN** Tauri command handler SHALL 接受 `port` 字段（camelCase 到 snake_case 自动转换）
- **AND** 成功时返回 `null`、失败时返回 `string` 错误文案

#### Scenario: http_server_status 返回字段 camelCase

- **WHEN** contract test 调用 `http_server_status`
- **THEN** 响应 JSON SHALL 含字段 `running: boolean`、`port: number`、`lastError: string | null`（不得为 `is_running` / `port_number` / `last_error` 等 snake_case 形态）

#### Scenario: 3 个 command 名同步 5 处

- **WHEN** ipc_contract test 跑 `EXPECTED_TAURI_COMMANDS` 断言
- **THEN** `http_server_start` / `http_server_stop` / `http_server_status` 三条 SHALL 在断言列表内
- **AND** `KNOWN_TAURI_COMMANDS`（`ui/src/lib/tauriMock.ts`）SHALL 同步含此三条
- **AND** `invoke_handler!`（`src-tauri/src/lib.rs`）SHALL 同步注册此三条

#### Scenario: http_server_status 在 server 未运行时仍可调用

- **WHEN** 前端在 server 未运行时调 `http_server_status`
- **THEN** 响应 SHALL 为 `{ running: false, port: <持久化值或默认 3456> }`
- **AND** **不**得返回错误


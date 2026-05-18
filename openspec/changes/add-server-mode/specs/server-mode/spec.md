## ADDED Requirements

### Requirement: Tauri 桌面应用 SHALL 暴露 server lifecycle IPC 控制

Tauri 桌面应用 SHALL 暴露 3 个 IPC commands 供前端控制本机 HTTP server 的启停与状态查询：`http_server_start(port: u16)` / `http_server_stop()` / `http_server_status() -> { running: bool, port: u16, lastError: string | null }`。`http_server_start` SHALL 先调 `cdt_config::validate_http_port` 校验入参，失败时返回 `Err("port must be in 1024..=65535")`；校验通过 SHALL bind `127.0.0.1:{port}` 启动 server；bind 失败（端口冲突 / 权限不足 / 其它 IO 错误）SHALL 返回 specific `Err` 文案让 UI 提示用户。`http_server_stop` SHALL 优雅关闭已运行的 server 并 join 任务结束；server 未运行时 SHALL 返回 `Ok(())`（幂等）。`http_server_status` SHALL 返回当前 server 状态快照——`lastError` 字段 SHALL 在最近一次启动失败时（含自动恢复失败）携带错误文案、成功启动后 SHALL 重置为 `null`，让 Settings UI 即使错过自动恢复阶段的 emit event 也能在挂载时主动查询到错误原因。

后端 SHALL 用 `tokio::sync::Mutex<Option<ServerHandle>>` 串行化 start/stop 操作，避免用户连点 toggle 时产生 race。每次 `http_server_start` SHALL 在新建 task 前先尝试 abort 现有 handle（若有），再 bind。

3 个 command 名 SHALL 加入 `crates/cdt-api/tests/ipc_contract.rs::EXPECTED_TAURI_COMMANDS` 与 `ui/src/lib/tauriMock.ts::KNOWN_TAURI_COMMANDS`，与 `src-tauri/src/lib.rs::invoke_handler!` 三处保持同步。

#### Scenario: 启动 server 成功并写持久化

- **WHEN** 前端调 `invoke('http_server_start', { port: 3456 })`，端口空闲
- **THEN** 后端 SHALL bind `127.0.0.1:3456`、启动 server task、把 `HttpServerConfig.enabled = true` + `port = 3456` 持久化
- **AND** 该 IPC SHALL 返回 `Ok(())`
- **AND** 后续 `http_server_status` SHALL 返回 `{ running: true, port: 3456 }`

#### Scenario: 启动 server 端口冲突返回 specific 错误

- **WHEN** 前端调 `invoke('http_server_start', { port: 3456 })`，但 `127.0.0.1:3456` 已被其它进程占用
- **THEN** 后端 SHALL 返回 `Err` 含端口号且文案明确为冲突类（如 `"port 3456 is in use"`）
- **AND** `HttpServerConfig.enabled` SHALL 保持 `false`（启动失败不写持久化）
- **AND** 后续 `http_server_status` SHALL 返回 `{ running: false, port: <previous>, lastError: "port 3456 is in use" }`

#### Scenario: 成功启动后 lastError 重置

- **WHEN** 前一次 `http_server_start` 因端口冲突失败（`lastError` 已被设置），用户改 port 再次调 `http_server_start({ port: 3500 })` 成功
- **THEN** `http_server_status` SHALL 返回 `{ running: true, port: 3500, lastError: null }`

#### Scenario: 停止 server 幂等

- **WHEN** 前端调 `invoke('http_server_stop')`，server 当前未运行
- **THEN** 后端 SHALL 返回 `Ok(())`
- **AND** `HttpServerConfig.enabled` SHALL 被写为 `false`（确保持久化与运行状态一致）

#### Scenario: 端口校验失败拒绝启动

- **WHEN** 前端调 `invoke('http_server_start', { port: 80 })`（< 1024）或 `port: 70000`（> 65535）
- **THEN** 后端 SHALL 返回 `Err` 含端口范围说明，**不**得 bind
- **AND** `HttpServerConfig.enabled` 与 `port` SHALL 保持原值

#### Scenario: 串行化避免 race

- **WHEN** 前端在 100ms 内连续调用 `http_server_start` × 2
- **THEN** 后端 SHALL 串行处理：第 2 次 SHALL 先 abort 第 1 次的 handle 再 bind，**不**得在同一时刻持有两个 server handle
- **AND** 最终状态 SHALL 唯一（`running: true` 单一 server）

### Requirement: Tauri 桌面应用启动时 SHALL 按持久化配置自动恢复 server

Tauri `tauri::Builder` 的 `setup` 阶段 SHALL 读取 `HttpServerConfig`，若 `enabled == true` SHALL 自动调启动逻辑（与 IPC `http_server_start` 同一实现，复用同一 `Mutex<Option<ServerHandle>>`）；自动启动失败（端口冲突等）SHALL 仅 `tracing::warn!` 记录 + emit `http-server-status` 事件给前端，**不**得阻塞 app 启动。`enabled == false` SHALL 不启动 server。

应用退出时（Tauri `RunEvent::Exit` 或主窗口关闭触发的 cleanup）SHALL 优雅关闭 server task、释放端口。

#### Scenario: enabled=true 启动时自动恢复

- **WHEN** 用户上次会话已开 server mode、`enabled=true` 持久化，重启 Tauri app
- **THEN** Tauri setup SHALL 自动 bind `127.0.0.1:{persisted_port}` 启动 server
- **AND** 主窗口 webview 加载完成时 SHALL 通过 `http_server_status` 看到 `running: true`

#### Scenario: 自动启动遇端口冲突不阻塞 app

- **WHEN** `enabled=true`、`port=3456`，但 3456 被其它进程占用
- **THEN** Tauri setup 自动启动 SHALL 失败但 **不**阻塞 app 启动
- **AND** SHALL 通过 `tracing::warn!` 记录冲突，并 emit `http-server-status` event 让 Settings UI 显示错误状态
- **AND** server-mode 内部状态 SHALL 写入 `lastError = "port 3456 is in use"` 让晚挂载的 Settings UI 在调用 `http_server_status` 时拿到原因
- **AND** `HttpServerConfig.enabled` SHALL 保持 `true`（用户意图未变，仅运行时未恢复）

#### Scenario: enabled=false 不自动启动

- **WHEN** `HttpServerConfig.enabled=false`
- **THEN** Tauri setup SHALL **不**启动 server
- **AND** `http_server_status` SHALL 返回 `{ running: false, port: <persisted>, lastError: null }`

#### Scenario: 应用退出时关闭 server

- **WHEN** 用户退出 Tauri app（关闭主窗口 + 退出托盘菜单 / `app.exit(0)`）
- **THEN** server task SHALL 被 abort、TCP listener SHALL 释放
- **AND** 端口 SHALL 在 OS 回收后可被其它进程立即 bind（macOS / Linux 内核 SO_REUSEADDR 行为视 OS 而定，本 spec 不强制）

### Requirement: 前端 SHALL 在浏览器 runtime 切换到 HTTP/SSE transport

前端 `ui/src/lib/api.ts` SHALL 通过检测 `window.__TAURI_INTERNALS__` 是否存在判断当前 runtime：存在 → Tauri runtime（保留现有 `invoke` 调用）；不存在 → 浏览器 runtime SHALL 切换到 HTTP transport——所有 IPC command 通过 `fetch('/api/...')` 调用对应 HTTP endpoint，所有事件订阅通过 `EventSource('/api/events')` 订阅 SSE 流。

transport 抽象层 SHALL 集中在一个 wrapper（如 `ui/src/lib/transport.ts`），让现有调用方代码（store / component / 路由）无需感知 transport 切换。

桌面专属 IPC 在浏览器 runtime 下 SHALL 显式抛出 `BrowserUnsupportedError`——清单仅含以下能力（**不**包含通知列表 / 标已读 / CRUD 等数据 API，那些走 HTTP）：

- `check_for_update`（应用内自动更新，依赖 `tauri-plugin-updater`）
- `is_running_under_rosetta`（macOS 翻译检测）
- 前端调用 `getCurrentWindow().setBadgeCount()`（Dock badge）
- 前端调用 `tauri-plugin-notification` 推 OS native toast（仅 Tauri runtime 触发；浏览器 runtime SSE 收到 `new-notification` 时**不**推 OS toast，仅更新应用内通知列表）
- 系统托盘交互（无 IPC 形态，纯 Tauri 端实现，浏览器 runtime 无入口）

调用方 SHALL 按 runtime 隐藏对应 UI 入口（不依赖错误处理兜底）。

资源 URL（图片资产等）：Tauri runtime 当前用 `tauri://localhost/...`，浏览器 runtime SHALL 重写为 `http://localhost:{port}/api/...` 或相对路径 `/api/...`。所有现有 IPC command 在浏览器 runtime 下 SHALL 有对应 HTTP endpoint 可以路由（含 image asset / tool output / subagent trace / pin / hide / triggers / project memory / read memory file / project session prefs 等 lazy 与辅助 command；HTTP 路由完整性见 [[http-data-api]] 的 `Mirror lazy and auxiliary IPC commands` Requirement）。

#### Scenario: 浏览器加载触发 transport 切换

- **WHEN** 用户从 Chrome 浏览器打开 `http://localhost:3456/`
- **THEN** 前端 bundle 加载时 SHALL 检测到 `window.__TAURI_INTERNALS__ === undefined`
- **AND** SHALL 通过 HTTP transport 调用所有 API（如列项目走 `GET /api/projects`）
- **AND** 实时事件 SHALL 通过 `EventSource('/api/events')` 接收

#### Scenario: Tauri runtime 保持 IPC 调用

- **WHEN** Tauri 桌面 app 主窗口 webview 加载前端 bundle
- **THEN** 前端 SHALL 检测到 `window.__TAURI_INTERNALS__` 存在
- **AND** SHALL 通过 `invoke()` 调用所有 IPC（与现有行为一致，不退化）

#### Scenario: 浏览器调用桌面专属 IPC 抛出明确错误

- **WHEN** 浏览器 runtime 下调用 `check_for_update` / `is_running_under_rosetta` / 通知 OS native 推送等桌面专属能力
- **THEN** transport 抽象层 SHALL 抛 `BrowserUnsupportedError`（携带 command 名）
- **AND** UI 层 SHALL 按 runtime 提前隐藏对应入口，**不**让用户能触发该错误

#### Scenario: 浏览器 runtime 资源 URL 重写

- **WHEN** 浏览器 runtime 渲染需要图片资产的 chunk
- **THEN** UI SHALL 用 HTTP 协议 URL（`/api/sessions/.../images/...` 或相对路径）替代 `tauri://localhost/...`
- **AND** 图片 SHALL 正确加载，无 404 / CORS 错误

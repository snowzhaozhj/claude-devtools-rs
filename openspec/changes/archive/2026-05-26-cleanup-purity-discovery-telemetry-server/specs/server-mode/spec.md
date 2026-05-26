# server-mode Specification

## MODIFIED Requirements

### Requirement: Tauri 桌面应用启动时 SHALL 按持久化配置自动恢复 server

桌面应用框架的 `setup` 阶段 SHALL 读取 `HttpServerConfig`，若 `enabled == true` SHALL 自动调启动逻辑（与 IPC `http_server_start` 同一实现，复用同一互斥锁保护的 server handle）；自动启动失败（端口冲突等）SHALL 仅 warn 级日志记录 + emit `http-server-status` 事件给前端，**不**得阻塞 app 启动。`enabled == false` SHALL 不启动 server。

应用退出时（桌面应用退出事件或主窗口关闭触发的 cleanup）SHALL 优雅关闭 server task、释放端口。

#### Scenario: enabled=true 启动时自动恢复

- **WHEN** 用户上次会话已开 server mode、`enabled=true` 持久化，重启桌面应用
- **THEN** 应用 setup SHALL 自动 bind `127.0.0.1:{persisted_port}` 启动 server
- **AND** 主窗口 webview 加载完成时 SHALL 通过 `http_server_status` 看到 `running: true`

#### Scenario: 自动启动遇端口冲突不阻塞 app

- **WHEN** `enabled=true`、`port=3456`，但 3456 被其它进程占用
- **THEN** 自动启动 SHALL 失败但 **不**阻塞 app 启动
- **AND** SHALL 通过 warn 级日志记录冲突，并 emit `http-server-status` event 让 Settings UI 显示错误状态
- **AND** server-mode 内部状态 SHALL 写入 `lastError = "port 3456 is in use"` 让晚挂载的 Settings UI 在调用 `http_server_status` 时拿到原因
- **AND** `HttpServerConfig.enabled` SHALL 保持 `true`（用户意图未变，仅运行时未恢复）

#### Scenario: enabled=false 不自动启动

- **WHEN** `HttpServerConfig.enabled=false`
- **THEN** 应用 setup SHALL **不**启动 server
- **AND** `http_server_status` SHALL 返回 `{ running: false, port: <persisted>, lastError: null }`

#### Scenario: 应用退出时关闭 server

- **WHEN** 用户退出桌面应用（关闭主窗口 + 退出托盘菜单 / 进程退出）
- **THEN** server task SHALL 被 abort、TCP listener SHALL 释放
- **AND** 端口 SHALL 在 OS 回收后可被其它进程立即 bind（macOS / Linux 内核 SO_REUSEADDR 行为视 OS 而定，本 spec 不强制）

### Requirement: 前端 SHALL 在浏览器 runtime 切换到 HTTP/SSE transport

前端 SHALL 通过检测 `window.__TAURI_INTERNALS__` 是否存在判断当前 runtime：存在 → Tauri runtime（保留 `invoke` 调用）；不存在 → 浏览器 runtime SHALL 切换到 HTTP transport——所有 IPC command 通过 `fetch('/api/...')` 调用对应 HTTP endpoint，所有事件订阅通过 `EventSource('/api/events')` 订阅 SSE 流。

transport 抽象层 SHALL 集中在统一 wrapper，让现有调用方代码（store / component / 路由）无需感知 transport 切换。

桌面专属 IPC 在浏览器 runtime 下 SHALL 显式抛出 `BrowserUnsupportedError`——清单仅含以下能力（**不**包含通知列表 / 标已读 / CRUD 等数据 API，那些走 HTTP）：

- `check_for_update`（应用内自动更新，依赖桌面端 updater 插件）
- `is_running_under_rosetta`（macOS 翻译检测）
- 前端调用桌面窗口 API 设置 badge count（Dock badge）
- 前端调用桌面端通知插件推 OS native toast（仅 Tauri runtime 触发；浏览器 runtime SSE 收到 `new-notification` 时**不**推 OS toast，仅更新应用内通知列表）
- 系统托盘交互（无 IPC 形态，纯桌面端实现，浏览器 runtime 无入口）

调用方 SHALL 按 runtime 隐藏对应 UI 入口（不依赖错误处理兜底）。

资源 URL（图片资产等）：Tauri runtime 当前用 `tauri://localhost/...`，浏览器 runtime SHALL 重写为 `http://localhost:{port}/api/...` 或相对路径 `/api/...`。所有现有 IPC command 在浏览器 runtime 下 SHALL 有对应 HTTP endpoint 可以路由（含 image asset / tool output / subagent trace / pin / hide / triggers / project memory / read memory file / project session prefs 等 lazy 与辅助 command；HTTP 路由完整性见 [[http-data-api]] 的 `Mirror lazy and auxiliary IPC commands` Requirement）。

#### Scenario: 浏览器加载触发 transport 切换

- **WHEN** 用户从 Chrome 浏览器打开 `http://localhost:3456/`
- **THEN** 前端 bundle 加载时 SHALL 检测到 `window.__TAURI_INTERNALS__ === undefined`
- **AND** SHALL 通过 HTTP transport 调用所有 API（如列项目走 `GET /api/projects`）
- **AND** 实时事件 SHALL 通过 `EventSource('/api/events')` 接收

#### Scenario: Tauri runtime 保持 IPC 调用

- **WHEN** 桌面应用主窗口 webview 加载前端 bundle
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

### Requirement: IPC SHALL expose http_server_start / _stop / _status commands

桌面应用 IPC 通道 SHALL 暴露 3 个 server-mode 控制 command。这 3 个 command 不属于 `DataApi` trait（与 server lifecycle 强绑定），SHALL 在桌面应用入口注册并由独立的 server-mode 实现模块负责。

字段契约（camelCase）：

- **`http_server_start`**：入参 `{ port: number }`（数字 1024–65535），返回 `Result<null, string>`。`Err` 文案 SHALL 携带 specific 类别（端口范围错误 / 端口冲突 / 其它 IO 错误）以便 UI 区分展示。
- **`http_server_stop`**：无入参，返回 `Result<null, string>`。幂等——server 未运行时仍返回 `Ok`。
- **`http_server_status`**：无入参，返回 `{ running: boolean, port: number, lastError: string | null }`。`running` 反映当前 server task 实际状态；`port` 为最近一次成功启动或持久化的端口（即使当前 `running=false`）；`lastError` SHALL 在最近一次启动失败（含自动恢复阶段）时携带错误文案，成功启动后 SHALL 重置为 `null`，让 Settings UI 在挂载时主动查询即可拿到错误原因（不依赖 `http-server-status` event 的 listener 注册时序）。

3 个 command 名 SHALL 出现在以下 5 处保持同步：

1. 桌面应用入口的 IPC handler 注册
2. IPC contract test 的已知 command 列表
3. IPC contract test 内对应 contract test（断言入参 / 返回字段 camelCase）
4. 前端 vitest mock 的已知 command 列表
5. 前端 API wrapper 函数声明

#### Scenario: http_server_start 字段契约

- **WHEN** contract test 模拟前端调用 `http_server_start({ port: 3456 })`
- **THEN** command handler SHALL 接受 `port` 字段（camelCase 到 snake_case 自动转换）
- **AND** 成功时返回 `null`、失败时返回 `string` 错误文案

#### Scenario: http_server_status 返回字段 camelCase

- **WHEN** contract test 调用 `http_server_status`
- **THEN** 响应 JSON SHALL 含字段 `running: boolean`、`port: number`、`lastError: string | null`（不得为 `is_running` / `port_number` / `last_error` 等 snake_case 形态）

#### Scenario: 3 个 command 名同步 5 处

- **WHEN** ipc_contract test 跑已知 command 列表断言
- **THEN** `http_server_start` / `http_server_stop` / `http_server_status` 三条 SHALL 在断言列表内
- **AND** 前端 vitest mock 已知 command 列表 SHALL 同步含此三条
- **AND** 桌面应用入口 IPC handler 注册 SHALL 同步注册此三条

#### Scenario: http_server_status 在 server 未运行时仍可调用

- **WHEN** 前端在 server 未运行时调 `http_server_status`
- **THEN** 响应 SHALL 为 `{ running: false, port: <持久化值或默认 3456> }`
- **AND** **不**得返回错误

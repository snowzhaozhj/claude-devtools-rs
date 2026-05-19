## MODIFIED Requirements

### Requirement: Expose SSH and context operations

系统 SHALL 暴露列出上下文、切换激活上下文、SSH 连接 / 断开 / 测试、查询 SSH 状态、解析 SSH host alias、列出 SSH config alias、保存 / 读取最近一次连接配置 这些操作。所有操作 SHALL 同时通过 (a) Tauri `invoke_handler!` 命令暴露给桌面 webview 与 (b) HTTP `/api/ssh/*` 与 `/api/contexts/*` 路由暴露给 standalone HTTP 客户端，共享同一 `LocalDataApi` 实现。

Tauri command 命名 SHALL 使用 snake_case，payload 字段 SHALL 使用 camelCase（与既有 IPC 约定一致）。

新增 Tauri command 清单：
- `ssh_connect` —— 入参 `{ host, port?, username?, authMethod, password? }`；返回 `SshConnectionResult { contextId, status, authChain[] }`
- `ssh_disconnect` —— 入参 `{ contextId }`；返回 `Ok`
- `ssh_test_connection` —— 入参同 `ssh_connect`；返回同 `ssh_connect` 但 SHALL NOT 注册新 active context
- `ssh_get_state` —— 入参 `{ contextId? }`；返回 `SshConnectionStatus { contextId, status, error?, authChain? }`
- `ssh_get_config_hosts` —— 入参 `{}`；返回 `Vec<String>`（alias 列表）
- `ssh_resolve_host` —— 入参 `{ alias }`；返回 `SshHostConfig { host, port, user, identityFile?, degraded }`
- `ssh_save_last_connection` —— 入参 `{ host, port, username, authMethod }`（无 password 字段）；返回 `Ok`
- `ssh_get_last_connection` —— 入参 `{}`；返回 `Option<SshLastConnection>`
- `list_contexts` —— 入参 `{}`；返回 `Vec<ContextSummary { id, kind, label, status }>`
- `switch_context` —— 入参 `{ contextId }`；返回 `Ok`
- `get_active_context` —— 入参 `{}`；返回 `ContextSummary`

#### Scenario: Resolve ssh host alias via IPC

- **WHEN** 调用方请求解析一个 alias
- **THEN** 响应 SHALL 含解析后的 hostname、port、user、identity file 路径（或在 not-found 时返回明确错误）

#### Scenario: ssh_connect command payload schema

- **WHEN** 前端调用 `invoke("ssh_connect", { host: "myserver", port: 22, username: "alice", authMethod: "sshConfig" })`
- **THEN** 后端 SHALL 调 `LocalDataApi::ssh_connect` 走真握手
- **AND** 返回 JSON SHALL 含 `contextId` / `status` / `authChain` 三个 camelCase 字段

#### Scenario: ssh_connect authChain serialization shape

- **WHEN** `ssh_connect` 返回值携带 `authChain: [...]`
- **THEN** 每条元素 SHALL 序列化为 `{ "source": { "type": "<variant>", "data"?: ... }, "outcome": { "type": "<variant>", "data"?: ... }, "elapsedMs": <u64> }` 形态（内部标签法 `#[serde(tag = "type", content = "data")]`）
- **AND** `source.type` 取值集 SHALL 与 ssh-remote-context spec 的 `AuthSource` 序列化样例一致（`identityAgent` / `envAgent` / `launchctlAgent` / `onePasswordAgent` / `identityFile` / `defaultKey` / `password`）
- **AND** 字段名一律 camelCase（`elapsedMs`，**非** `elapsed_ms`）

#### Scenario: ssh_connect error.code uses snake_case

- **WHEN** `ssh_connect` 因鉴权全部失败返回结构化错误
- **THEN** 错误 JSON SHALL 含 `code: "ssh_auth_exhausted"` 字段（snake_case，与现有 `ApiError.code` 约定一致）
- **AND** `attempts: [...]` 元素仍按 camelCase 序列化（每条 `AuthAttempt` 的字段名是 `source` / `outcome` / `elapsedMs`）

#### Scenario: ssh_connect request password field is redacted in logs

- **WHEN** 后端通过 `tracing` 记录 `ssh_connect` 请求
- **THEN** 任何日志输出 SHALL NOT 含 password 明文
- **AND** `SshConnectRequest` 的 `Debug` impl SHALL 把 password 字段渲染为 `<redacted>`

#### Scenario: ssh_test_connection does not register active context

- **WHEN** 前端调用 `invoke("ssh_test_connection", { ... })` 成功
- **THEN** 返回值 SHALL 含 `status: "connected"` 但 `list_contexts()` SHALL NOT 把该 host 加入列表
- **AND** SSH session SHALL 在测试结束后立即关闭

#### Scenario: ssh_get_state without contextId returns active

- **WHEN** 前端调用 `invoke("ssh_get_state")`（不带 contextId）
- **THEN** 后端 SHALL 返回当前 active context 的状态

#### Scenario: list_contexts returns local plus all registered SSH hosts

- **WHEN** 前端调用 `invoke("list_contexts")`
- **AND** 已有 1 个 active 的 ssh-host-A context
- **THEN** 返回值 SHALL 含 2 个 `ContextSummary`：`{ id: "local", kind: "local", status: "connected" }` 与 `{ id: "ssh-host-A", kind: "ssh", label: "host-A", status: "connected" }`

#### Scenario: switch_context to local

- **WHEN** 前端调用 `invoke("switch_context", { contextId: "local" })`
- **THEN** 后端 SHALL 切换 active context 为 `Local` 但 SHALL NOT 断开已注册 SSH context（保持 `connected` 状态供后续切回）

#### Scenario: ssh_save_last_connection strips password

- **WHEN** 前端调用 `invoke("ssh_save_last_connection", { host, port, username, authMethod, password: "secret" })`
- **THEN** 后端 SHALL 持久化时 `password` 字段 SHALL NOT 出现在 `~/.claude/claude-devtools-config.json`
- **AND** 即使前端误传 password，配置文件 SHALL 仅含 host/port/username/authMethod 四字段

#### Scenario: HTTP /api/ssh/* routes mirror IPC

- **WHEN** standalone 模式下 HTTP 客户端 `POST /api/ssh/connect` 与 IPC 同形 payload
- **THEN** 后端 SHALL 走与 IPC 相同的 `LocalDataApi::ssh_connect` 实现
- **AND** 响应 JSON schema SHALL 与 IPC 返回值完全一致

### Requirement: Emit push events for file changes and notifications

系统 SHALL 从 main 进程向 renderer 推送以下事件：session 文件变更、todo 文件变更、新通知、SSH 状态变化、context 切换、updater 进度。

桌面（Tauri）host SHALL 在 `setup` 阶段订阅 `FileWatcher::subscribe_files()` 广播，并向前端 webview `emit("file-change", payload)`。Payload SHALL 是 `FileChangeEvent` 的 camelCase 序列化结果（字段 `projectId`、`sessionId`、`deleted`），与其它 IPC payload 命名约定一致。

桌面 host SHALL 在 `setup` 阶段订阅 `LocalDataApi::subscribe_ssh_status()`，并向前端 webview `emit("ssh_status", payload)`。Payload SHALL 是 `SshStatusChange` 的 camelCase 序列化结果（字段 `contextId`、`status`、`error?`、`authChain?`）。同样订阅 `subscribe_context_changed()` emit `context_changed` 事件 payload `{ activeContextId, kind }`。

#### Scenario: New notification while renderer is subscribed

- **WHEN** renderer 已订阅通知事件，期间产出一条新通知
- **THEN** renderer SHALL 在 debounce 窗口内收到一条 push 事件，携带通知 payload

#### Scenario: Tauri 转发 file-change 事件

- **WHEN** `cdt-watch::FileWatcher` 在 100 ms debounce 后产出 `FileChangeEvent { project_id: "p", session_id: "s", deleted: false }`
- **AND** Tauri host 在 `setup` 已 spawn 桥任务订阅 `subscribe_files()`
- **THEN** webview SHALL 通过 `listen("file-change", ...)` 收到 payload `{ projectId: "p", sessionId: "s", deleted: false }`

#### Scenario: file-change payload 是 camelCase

- **WHEN** Tauri 桥任务 emit 一条 `file-change` 事件
- **THEN** 序列化后的 JSON SHALL 使用 camelCase 字段名（`projectId` / `sessionId` / `deleted`），与既有 IPC 类型约定一致

#### Scenario: file-change 桥与通知管线并存

- **WHEN** Tauri host 同时持有 `subscribe_files()`（emit `file-change`）与 `subscribe_detected_errors()`（emit `notification-added`）两个订阅
- **THEN** 两个桥 SHALL 独立运行，文件变更不会因通知 pipeline 的 lag 被丢弃，反之亦然

#### Scenario: ssh_status event broadcast on connect

- **WHEN** 后端 SSH 连接状态从 `connecting` 切到 `connected`
- **AND** Tauri host 在 setup 已 spawn 桥任务订阅 `subscribe_ssh_status()`
- **THEN** webview SHALL 通过 `listen("ssh_status", ...)` 收到 payload `{ contextId: "ssh-host-A", status: "connected" }`
- **AND** payload `error` 与 `authChain` 字段在 success 路径 SHALL 为 `null` 或省略

#### Scenario: ssh_status event carries error detail on failure

- **WHEN** SSH 连接失败导致状态切到 `error`
- **THEN** webview 收到的 `ssh_status` payload SHALL 含 `error: { code: "ssh_auth_exhausted", attempts: [...] }`
- **AND** UI SHALL 能据此渲染逐源诊断

#### Scenario: context_changed event on switch

- **WHEN** 后端从 `Local` context 切到 `ssh-host-A`
- **THEN** webview SHALL 收到一条 `context_changed { activeContextId: "ssh-host-A", kind: "ssh" }` 事件
- **AND** 事件 SHALL 不携带 SSH 状态详情（由独立 `ssh_status` 事件流承担）

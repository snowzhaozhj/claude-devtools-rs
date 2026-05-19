## ADDED Requirements

### Requirement: HTTP server SHALL layer CORS middleware for localhost origins

`cdt_api::http::start_server` SHALL 在构建 router 时 layer 一道 CORS 中间件，仅放行来自 `localhost` 与 `127.0.0.1` 任意端口（含可选 `https://` 协议）的 origin。CORS 实现 SHALL 用 `tower_http::cors::CorsLayer` 配 `AllowOrigin::predicate` 显式判断，匹配规则 SHALL 等价于正则 `^https?://(localhost|127\.0\.0\.1)(:\d+)?$`。

非 localhost origin（如 `https://localhost.evil.com` / `http://example.com`）SHALL 在 CORS preflight 阶段被拒绝（响应 SHALL 不携带 `Access-Control-Allow-Origin`，浏览器自然拦截）。CORS allow-methods SHALL 至少包含 `GET / POST / PATCH / DELETE / OPTIONS`，allow-headers SHALL 包含 `Content-Type`。同源请求（origin 与 server bind 一致）SHALL 不触发 CORS 检查，行为不变。

CORS layer 不引入鉴权或 origin 配置项——任何放宽（LAN 访问 / 自定义 origin allowlist）SHALL 走单独的后续 change 评估。

#### Scenario: localhost origin 通过 CORS

- **WHEN** 浏览器从 `http://localhost:3456` 发起 `GET /api/projects`，附带 `Origin: http://localhost:3456`
- **THEN** 响应 SHALL 携带 `Access-Control-Allow-Origin: http://localhost:3456`（或等价 echo origin 形态）
- **AND** 请求 SHALL 正常处理返回 200 + 项目列表

#### Scenario: 127.0.0.1 origin 通过 CORS

- **WHEN** 浏览器从 `http://127.0.0.1:3456` 发起 `GET /api/projects`，附带 `Origin: http://127.0.0.1:3456`
- **THEN** 响应 SHALL 携带匹配该 origin 的 `Access-Control-Allow-Origin`
- **AND** 请求 SHALL 正常处理

#### Scenario: 非 localhost origin 被 CORS 拒绝

- **WHEN** 浏览器从 `https://localhost.evil.com` 发起跨域请求 `GET /api/projects`，附带 `Origin: https://localhost.evil.com`
- **THEN** 响应 SHALL **不**携带 `Access-Control-Allow-Origin: https://localhost.evil.com`
- **AND** 浏览器 SHALL 阻止 JavaScript 读取响应

#### Scenario: preflight OPTIONS 请求

- **WHEN** 浏览器对 `PATCH /api/config` 发起 preflight `OPTIONS`，origin 为 `http://localhost:3456`
- **THEN** 响应 SHALL 含 `Access-Control-Allow-Methods` 含 `PATCH`、`Access-Control-Allow-Headers` 含 `Content-Type`、状态 SHALL 为 200/204

### Requirement: HTTP server SHALL serve static frontend assets with SPA fallback

`cdt_api::http::start_server` SHALL 接受可选参数 `static_dir: Option<PathBuf>`：传入时 SHALL 在 router 上挂静态文件 fallback handler，提供 `static_dir` 下的静态文件 serve；传入路径无效（不存在 / 非目录）时 SHALL 仅 `tracing::warn!` 警告并跳过 fallback，**不**得 panic 或拒绝启动。

静态文件 fallback SHALL 在 `/api` 路由之后注册（顺序保证 API 优先）。fallback handler 按以下三种情况分流：

1. **磁盘上对应文件存在** → serve 文件内容（按扩展名 guess mime；未识别扩展名 fallback 到 `application/octet-stream`）
2. **navigation 请求**（路径无 `.` 扩展名 / 根路径 `/`）→ 返回 `index.html` 让前端 client-side router 接管
3. **带扩展名但磁盘上不存在的资源**（如 `/assets/missing.js`、`/favicon.ico`）→ SHALL 返回 `404`，**不**得 fallback 到 `index.html`——否则浏览器把 HTML 当 JS 解析爆 parse error，且 `200` 状态会被 CDN / 浏览器缓存导致脏数据持久化（SPA 部署经典坑，本 change 显式规约此行为）

path traversal 防御：fallback handler SHALL 拒绝路径中含 `..` 段（包括 URL-encoded 形态 `%2e%2e` / `%2E%2E`）以及 backslash（`\` 或 `%5c`）的请求。具体状态码可为 `403`（fallback handler 主动拒绝）、`400` / `404`（axum 路由层在 normalize / decode 阶段先拦下）任一——**不变量**是：SHALL NOT 返 `200` + 任何静态文件内容（攻击者 SHALL NOT 凭 traversal 拿到磁盘文件）。

`static_dir = None` 时（如 `cdt-cli` 默认行为或 dev mode）SHALL 不挂 fallback，未命中 API 路由的请求 SHALL 返回 `404`（与本 change 之前行为兼容）。Tauri runtime SHALL 在调用 `start_server` 时根据 `tauri::path::resource_dir()` 计算前端 bundle 路径并传入；具体子路径解析 SHALL 在实施期通过 `cargo tauri build` 实测确定（design.md Open Questions 已记）。

#### Scenario: GET / 返回前端 index.html

- **WHEN** 启动 server 时传入 `static_dir = Some(<path/to/ui/dist>)`，浏览器请求 `GET /`
- **THEN** 响应 SHALL 为 `200`，body SHALL 为前端 `index.html` 内容、`Content-Type: text/html`

#### Scenario: GET 已知静态资源命中 ServeDir

- **WHEN** 浏览器请求 `GET /assets/index-abc123.js`，该文件存在于 `static_dir/assets/`
- **THEN** 响应 SHALL 为 `200`、对应 JS 内容、合适 `Content-Type`

#### Scenario: GET 未知前端路由 fallback 到 index.html

- **WHEN** 浏览器请求 `GET /sessions/some-id`（前端 client-side router 路由，磁盘上无此文件）
- **THEN** 响应 SHALL 为 `200` 返回 `index.html` 让前端接管路由

#### Scenario: GET 带扩展名但磁盘上不存在的资源 SHALL 返 404 不 fallback

- **WHEN** 浏览器请求 `GET /assets/missing.js`、`/favicon.ico`、`/some/path/file.png` 等带扩展名路径，但磁盘上无此文件
- **THEN** 响应 SHALL 为 `404`，body **不**得含 `index.html` 内容
- **AND** 浏览器 SHALL NOT 把 HTML 当成 JS 解析（避免 CDN / 浏览器缓存脏 200 状态）

#### Scenario: path traversal 攻击 SHALL NOT 拿到磁盘文件

- **WHEN** 浏览器请求 `GET /../etc/passwd` / `/foo/../../bar` / `/%2e%2e/etc/passwd` / `/%2E%2E/etc/passwd` / `/foo/%2e%2e/bar` / `/foo%5cbar`（含裸 `..`、URL-encoded `..`、URL-encoded `\` 等形态）
- **THEN** 响应 SHALL NOT 为 `200` 且 SHALL NOT 含任何静态文件内容
- **AND** 状态码可为 `403`（fallback handler 主动拒绝）/ `400` / `404`（axum 路由层先拦下），具体由框架决定

#### Scenario: GET /api/* 不被 ServeDir 拦截

- **WHEN** 浏览器请求 `GET /api/projects`
- **THEN** 该请求 SHALL 走 API handler、**不**得被 ServeDir 拦截或 fallback 到 index.html

#### Scenario: static_dir = None 时无 ServeDir

- **WHEN** 启动 server 时不传 `static_dir`（或传 `None`）
- **THEN** 未命中 API 路由的 `GET /` 请求 SHALL 返回 `404`，行为与本 change 之前一致

#### Scenario: static_dir 路径无效仅警告不阻塞启动

- **WHEN** `static_dir = Some("/nonexistent/path")`
- **THEN** server SHALL 启动成功（仅 `tracing::warn!` 提示路径无效），所有 `/api/*` 路由 SHALL 正常 serve
- **AND** `GET /` 请求 SHALL 返回 `404`

### Requirement: Mirror lazy and auxiliary IPC commands as HTTP endpoints

`cdt-api::http::routes` SHALL 在 `/api` 前缀下镜像目前仅在 IPC 路径暴露的 lazy 与辅助 command，让浏览器 runtime（详 [[server-mode]]）能跑通完整 UI 流程。每个 endpoint 的请求 / 响应 schema SHALL 与对应 IPC command 同形（camelCase 字段、相同 enum tag、相同 `xxxOmitted` 语义）。

实际路由清单（Method + URL）SHALL 至少包含：

- `GET /api/projects/{projectId}/memory` — 镜像 `get_project_memory`，返回 `MemoryFile[]`
- `POST /api/projects/{projectId}/memory-files` — 镜像 `read_memory_file`，body 含 `{ "file": "<relative path>" }`，返回文件内容
- `GET /api/sessions/{rootSessionId}/subagents/{subagentSessionId}/trace` — 镜像 `get_subagent_trace`，返回 trace 数据
- `GET /api/sessions/{rootSessionId}/subagents/{sessionId}/blocks/{blockId}/image` — 镜像 `get_image_asset`，返回 base64 字符串（与 IPC 同形）
- `GET /api/sessions/{rootSessionId}/subagents/{sessionId}/tools/{toolUseId}/output` — 镜像 `get_tool_output`，返回 `ToolOutput`（含 `outputBytes` / `outputOmitted` 语义）
- `POST /api/notifications/triggers` — 镜像 `add_trigger`，body 为 `NotificationTrigger`（caller SHALL 在 body 内提供非空 `id`，与 IPC 路径校验语义一致；server 不自动生成 id）
- `DELETE /api/notifications/triggers/{triggerId}` — 镜像 `remove_trigger`
- `POST /api/projects/{projectId}/sessions/{sessionId}/pin` — 镜像 `pin_session`
- `DELETE /api/projects/{projectId}/sessions/{sessionId}/pin` — 镜像 `unpin_session`
- `POST /api/projects/{projectId}/sessions/{sessionId}/hide` — 镜像 `hide_session`
- `DELETE /api/projects/{projectId}/sessions/{sessionId}/hide` — 镜像 `unhide_session`
- `GET /api/projects/{projectId}/session-prefs` — 镜像 `get_project_session_prefs`，返回 `{ pinned: string[], hidden: string[] }`

每个 handler SHALL 直接委托给 `LocalDataApi` 同名方法，错误映射沿用 `Return safe defaults on lookup failures (current baseline)` Requirement 的 `code → status` 表。

**HTTP payload omit 行为**：lazy endpoint（image asset / tool output）SHALL **不**应用 `OMIT_*` 裁剪——它们本就是 IPC 侧懒加载的真实数据来源，HTTP 路径用户主动请求时 SHALL 拿到完整 payload。`get_session_detail` / `get_sessions_by_ids` 路径维持 `ipc-data-api` spec 现有"HTTP 不应用 omit"语义不变（浏览器 runtime 大会话首屏可能因此较慢，作为已知限制 + 后续 follow-up 评估窗口）。

#### Scenario: GET project memory mirrors IPC

- **WHEN** 浏览器请求 `GET /api/projects/<projectId>/memory`
- **THEN** 响应 SHALL 与 IPC `get_project_memory(<projectId>)` 同形（含 CLAUDE.md 各 scope）

#### Scenario: GET image asset returns base64

- **WHEN** 浏览器请求 `GET /api/sessions/<root>/subagents/<sid>/blocks/<bid>/image`
- **THEN** 响应 SHALL 是 base64 字符串（同 IPC `get_image_asset` 返回类型）
- **AND** SHALL **不**应用任何 `dataOmitted` 裁剪（lazy 端点本就是真实数据源）

#### Scenario: GET tool output preserves outputOmitted semantics

- **WHEN** 浏览器请求 `GET /api/sessions/<root>/subagents/<sid>/tools/<tuid>/output`
- **THEN** 响应 SHALL 与 IPC `get_tool_output` 同形——`outputBytes` 字段保留、`outputOmitted: false`、内层 `text` / `value` 携带完整内容

#### Scenario: POST add trigger persists caller-provided id

- **WHEN** 浏览器 `POST /api/notifications/triggers`，body 为合法 `NotificationTrigger`（含 caller 自行分配的非空 `id`）
- **THEN** 响应 SHALL 返回更新后的完整 `AppConfig` JSON，`notifications.triggers` 含该新 trigger 且 `id` 与 caller 入参一致，与 IPC `add_trigger` 同形
- **AND** 后续 `GET /api/config` 读取的 `notifications.triggers` SHALL 含该新 trigger

#### Scenario: POST pin session 与 DELETE unpin session 互逆

- **WHEN** 浏览器先 `POST /api/projects/<pid>/sessions/<sid>/pin`，再 `GET /api/projects/<pid>/session-prefs`
- **THEN** 响应 `pinned` 数组 SHALL 含 `<sid>`
- **WHEN** 然后 `DELETE /api/projects/<pid>/sessions/<sid>/pin`，再次 GET prefs
- **THEN** `pinned` SHALL 不含 `<sid>`

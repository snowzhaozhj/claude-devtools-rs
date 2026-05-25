## MODIFIED Requirements

### Requirement: Serve projects and sessions over HTTP under /api prefix

系统 SHALL 在 `/api` 前缀下暴露与 IPC data API **同形数据返回**的 HTTP endpoint，覆盖：列项目 / 项目详情 / 项目仓库信息 / 列会话（含分页与按 id 批量两种 variant）/ 会话详情 / 会话 chunk / 会话 metrics / waterfall 数据 / subagent 详情。完整路由表（method + URL + 入参形态）SHALL 与 IPC 等价 command 一一对应（详 design.md `D-Impl-4`）。

`GET /api/sessions/{sessionId}` URL 仅携带 session_id 而**不**携带 project_id。系统 SHALL 在该 handler 内反查所属 project_id 再委托详情 IPC 等价方法。反查失败 SHALL 走 `Return safe defaults on lookup failures` 路径返 404 + `code=not_found`，**不**得返 200 配空 body 或 500。

`POST /api/sessions/batch`（按 id 列表批量取会话详情）入参仅含 session id 列表；某条 id 反查失败时 SHALL 在该位置返回 `metadata.status = "not_found"` 占位条目，整体响应 SHALL 仍为 200。

列会话路由 SHALL 返回**骨架** `PaginatedResponse<SessionSummary>`：每条 SessionSummary 的 sessionId / projectId / timestamp SHALL 为真实值，title / messageCount / isOngoing / gitBranch SHALL 允许为占位值（与 IPC 等价路径行为对齐）。lookup-fast-path 命中条 SHALL 在响应中直接 inline 填回真值（zero 后续 SSE emit）；未命中条的真实 metadata SHALL 通过 SSE `session_metadata_update` 事件异步推送。

#### Scenario: GET 项目 / 会话路由返回与 IPC 同形 payload

- **WHEN** 客户端发起列项目 / 列会话 / 取会话详情 / 批量取会话详情等 GET / POST 请求
- **THEN** 响应 SHALL 与等价 IPC 命令返回同形 JSON（含 chunks / metrics / metadata / 分页结构）
- **AND** 字段名 SHALL 与 IPC 一致用 camelCase

#### Scenario: GET session detail resolves project id internally

- **WHEN** 客户端发起 `GET /api/sessions/<sid>`，对应 jsonl 位于某 project 目录下
- **THEN** 系统 SHALL 内部反查得到 project_id 再走详情 IPC 等价路径
- **AND** 响应 detail 的 projectId / sessionId 字段 SHALL 为真实值

#### Scenario: GET session detail unknown session returns 404

- **WHEN** 客户端发起 `GET /api/sessions/<sid>` 但 sid 在所有 project 目录下都没有对应 jsonl
- **THEN** 响应 SHALL 为 404，body 含 `code: "not_found"` 与 message 引用 sid

#### Scenario: POST sessions batch with mixed-existence ids

- **WHEN** 客户端发起批量取会话详情，body 含部分存在 + 部分不存在的 id
- **THEN** 响应 SHALL 为 200，body 数组与 id 顺序一一对应：存在条为完整 detail（projectId 非空）；不存在条 metadata.status SHALL 为 `"not_found"`、projectId SHALL 为空字符串

#### Scenario: GET paginated sessions returns skeleton with cache-hit inline real values

- **WHEN** 客户端发起列会话路由请求
- **AND** 项目下某些 session 的 lookup-fast-path 命中（metadata cache 等价），其余 cache miss
- **THEN** 路由 handler SHALL 调与 IPC 等价的列会话方法（**不**走同步 fallback 镜像）
- **AND** 响应 body SHALL 是分页骨架：cache 命中条 inline 携带真实 title / messageCount / isOngoing / gitBranch；cache miss 条为占位值
- **AND** cache miss 条 SHALL 在后续后台扫描完成后通过 SSE `session_metadata_update` 事件推送真实值

### Requirement: Serve search endpoints

系统 SHALL 在 `/api` 下暴露与 session 搜索 capability 对应的搜索 endpoint，接受 POST body 形式的 query 参数，返回有序结果。body schema 与 IPC 搜索入参一致，至少包含 query 字符串与可选的 projectId / sessionId 限定字段；响应与 IPC 搜索操作返回同形。

#### Scenario: POST session search

- **WHEN** 客户端发起搜索请求，body 含 query / 可选 project id / 可选 session id
- **THEN** 响应 SHALL 与等价 IPC 搜索操作返回同形

### Requirement: Serve auxiliary, subagent, utility, and validation endpoints

系统 SHALL 暴露与 IPC data API 中所有辅助操作一一对应的 HTTP endpoint，包括 subagent 详情 / trace、仓库分组、worktree sessions、CLAUDE.md 读取、agent configs、路径 / mention 校验、context 切换、SSH、updater。完整路由清单（method + URL + 入参 / 返参形态）SHALL 与 IPC 等价 command 一一对应（详 design.md `D-Impl-4`）。

subagent 详情 / chunk / waterfall 等若干路由当前由会话详情 payload 内联返回；若未来拆出独立 endpoint，SHALL 同步追加路由清单。

#### Scenario: GET subagent detail via session detail payload

- **WHEN** 客户端发起取会话详情请求
- **THEN** 响应 SHALL 含该 session 下所有 subagent 的 chunks 占位、metrics、spawning context；subagent.messages 默认按 IPC 同样的 omit 策略懒加载

#### Scenario: POST path validation 与 GET CLAUDE.md / SSH host alias resolution

- **WHEN** 客户端发起这些辅助操作的 GET / POST 请求
- **THEN** 响应 SHALL 与等价 IPC 命令返回同形

### Requirement: Serve config and notification endpoints

系统 SHALL 暴露读取 / 更新配置以及列出 / 标记通知为已读的 HTTP endpoint，语义与 IPC data API 一致。完整路由（method + URL）SHALL 与 IPC 等价 command 一一对应（详 design.md `D-Impl-4`）。

#### Scenario: PATCH config field

- **WHEN** 客户端发起配置更新 PATCH 请求，body 含合法字段更新
- **THEN** 响应 SHALL 反映新配置，且变更 SHALL 已被持久化

#### Scenario: GET notifications with pagination

- **WHEN** 客户端发起通知列表请求 + limit / offset 分页
- **THEN** 响应 SHALL 是与 IPC 等价命令同形的通知数组（按时间倒序）

#### Scenario: Mark all notifications read

- **WHEN** 客户端发起 mark-all-read 请求
- **THEN** 响应 SHALL 含成功标志，所有通知 SHALL 已被持久化为已读状态

### Requirement: Push events via Server-Sent Events

系统 SHALL 暴露一个 Server-Sent Events endpoint（`GET /api/events`），传递与 IPC push channel 相同的事件流。启动 HTTP server 的进程 SHALL 同时启动事件 producer，覆盖以下信号源：

1. **file-change**：订阅文件 watcher broadcast，每条事件转换为 SSE PushEvent 推送（删除事件 SHALL 同样推送，让客户端能感知删除）
2. **todo-change**：订阅 todo watcher broadcast，每条事件按统一 schema 推送（todo 文件名仅含 session_id；project_id 字段 SHALL 填空字符串占位以保留 schema 一致）
3. **new-notification**：订阅 detected-error broadcast，每条事件序列化后推送
4. **session-metadata-update**：订阅 session metadata broadcast，每条事件按统一 schema 推送，让浏览器 runtime 可复用 IPC 路径的骨架列表 + metadata patch 语义
5. **ssh-status / updater 事件**：当前未提供 broadcast 源；PushEvent 仍保留对应 variant，未来 producer 接通后 SSE 客户端 SHALL 按本 Requirement 描述的同一桥接模式收到

producer 任务对 broadcast Lagged 错误 SHALL 跳过该条；对 Closed 错误 SHALL 退出 loop。所有 producer task 共用同一 events 发送端，但每个 SSE 客户端连接 SHALL 各自获得独立 receiver——broadcast 语义保证多客户端各自**恰好**收到一次事件。

#### Scenario: Browser transport receives session metadata update

- **WHEN** 浏览器 runtime 通过 SSE endpoint 订阅，列会话后台元数据扫描产出一条 metadata 更新
- **THEN** SSE event data SHALL 携带 metadata 更新 type 与 snake_case 原始字段：project_id / session_id / title / message_count / is_ongoing / git_branch
- **AND** 浏览器 transport SHALL 将其归一化为 metadata 更新事件与 camelCase payload：projectId / sessionId / title / messageCount / isOngoing / gitBranch
- **AND** 浏览器 runtime 的 sidebar SHALL 能用该事件 in-place patch 对应 session summary

### Requirement: Return safe defaults on lookup failures (current baseline)

系统 SHALL 对查询失败返回结构化错误响应，response body 形如 `{"code": "<code>", "message": "<...>"}`。code 字符串与 HTTP status 的映射 SHALL 与 IPC 错误代码语义一致：

- `code: "validation_error"` → 400 Bad Request — 输入校验失败（缺字段 / 字段类型错 / 值非法）
- `code: "config_error"` → 400 Bad Request — 配置 JSON 解析失败 / 配置字段值非法
- `code: "not_found"` → 404 Not Found — 请求资源不存在（项目 / 会话 / 通知 id 等）
- `code: "ssh_error"` → 502 Bad Gateway — SSH 远端连接 / 命令执行失败
- `code: "internal"` → 500 Internal Server Error — 处理请求时未捕获异常 / 不变量违反

这是相对原版基线的有意改进——原版返 200 配空数据，本 capability 显式区分状态码并附带机器可读 code。

#### Scenario: GET nonexistent session 返 404

- **WHEN** 客户端请求一个不存在的 session id
- **THEN** 响应 SHALL 为 404，body 含 `code: "not_found"`

#### Scenario: GET sessions for unknown project 返 404

- **WHEN** 客户端请求一个无法解析的 project id 的 sessions
- **THEN** 响应 SHALL 为 404，body 含 `code: "not_found"`

#### Scenario: PATCH config with invalid field value 返 400

- **WHEN** 客户端发起配置更新 body 字段值不符合 enum / 范围约束
- **THEN** 响应 SHALL 为 400，body 含 `code: "config_error"` 或 `code: "validation_error"`

#### Scenario: SSH command fails on remote 返 502

- **WHEN** 客户端发起 SSH 连接 / 命令但远端不可达 / 握手失败
- **THEN** 响应 SHALL 为 502，body 含 `code: "ssh_error"` 与 message 描述底层失败

#### Scenario: Unhandled server exception 返 500

- **WHEN** 处理请求时抛出未捕获异常
- **THEN** 响应 SHALL 为 500，body 含 `code: "internal"`

### Requirement: HTTP server SHALL layer CORS middleware for localhost origins

HTTP server 启动 SHALL 在构建 router 时 layer 一道 CORS 中间件，仅放行来自 localhost 与 127.0.0.1 任意端口（含可选 https 协议）的 origin。匹配规则 SHALL 等价于正则 `^https?://(localhost|127\.0\.0\.1)(:\d+)?$`。

非 localhost origin SHALL 在 CORS preflight 阶段被拒绝（响应 SHALL 不携带 allow-origin 头）。CORS allow-methods SHALL 至少包含 GET / POST / PATCH / DELETE / OPTIONS，allow-headers SHALL 包含 Content-Type。同源请求（origin 与 server bind 一致）SHALL 不触发 CORS 检查。

CORS layer **不**引入鉴权或 origin 配置项——任何放宽（LAN 访问 / 自定义 origin allowlist）SHALL 走单独的后续 change 评估。

#### Scenario: localhost / 127.0.0.1 origin 通过 CORS

- **WHEN** 浏览器从 localhost 或 127.0.0.1 任意端口发起请求，附带匹配 origin
- **THEN** 响应 SHALL 携带匹配该 origin 的 allow-origin 头
- **AND** 请求 SHALL 正常处理

#### Scenario: 非 localhost origin 被 CORS 拒绝

- **WHEN** 浏览器从非 localhost 域名（典型 `localhost.evil.com`）发起跨域请求
- **THEN** 响应 SHALL **不**携带匹配该 origin 的 allow-origin 头
- **AND** 浏览器 SHALL 阻止 JavaScript 读取响应

#### Scenario: preflight OPTIONS 请求

- **WHEN** 浏览器发起 preflight OPTIONS，origin 为 localhost
- **THEN** 响应 SHALL 含合适 allow-methods 与 allow-headers，状态 SHALL 为 200/204

### Requirement: HTTP server SHALL serve static frontend assets with SPA fallback

HTTP server 启动 SHALL 接受可选 static_dir 参数：传入有效路径时 SHALL 在 router 上挂静态文件 fallback handler；传入路径无效（不存在 / 非目录）时 SHALL 仅 warn 级日志并跳过 fallback，**不**得 panic 或拒绝启动。

静态文件 fallback SHALL 在 `/api` 路由之后注册（顺序保证 API 优先）。fallback handler 按以下三种情况分流：

1. **磁盘上对应文件存在** → serve 文件内容（按扩展名 guess mime；未识别扩展名 fallback 到 `application/octet-stream`）
2. **navigation 请求**（路径无 `.` 扩展名 / 根路径 `/`）→ 返回 SPA 入口 HTML 让前端 client-side router 接管
3. **带扩展名但磁盘上不存在的资源** → SHALL 返 404，**不**得 fallback 到 SPA 入口（避免浏览器把 HTML 当 JS 解析爆 parse error，且 200 状态会被 CDN / 浏览器缓存导致脏数据持久化）

path traversal 防御：fallback handler SHALL 拒绝路径中含 `..` 段（包括 URL-encoded 形态）以及 backslash 的请求。具体状态码可为 403 / 400 / 404 任一——**不变量**是：SHALL NOT 返 200 + 任何静态文件内容（攻击者 SHALL NOT 凭 traversal 拿到磁盘文件）。

不传 static_dir 时 SHALL 不挂 fallback，未命中 API 路由的请求 SHALL 返 404。

#### Scenario: GET / 返回 SPA 入口

- **WHEN** 启动 server 时传入有效 static_dir，浏览器请求根路径
- **THEN** 响应 SHALL 为 200，body 为 SPA 入口 HTML、Content-Type 为 text/html

#### Scenario: GET 已知静态资源命中

- **WHEN** 浏览器请求磁盘存在的静态资源
- **THEN** 响应 SHALL 为 200、对应内容、合适 Content-Type

#### Scenario: GET 未知前端路由 fallback 到 SPA 入口

- **WHEN** 浏览器请求前端 client-side router 的路由（无 `.` 扩展名，磁盘上无此文件）
- **THEN** 响应 SHALL 为 200 返回 SPA 入口 HTML

#### Scenario: GET 带扩展名但磁盘上不存在的资源 SHALL 返 404 不 fallback

- **WHEN** 浏览器请求带扩展名路径但磁盘上无此文件
- **THEN** 响应 SHALL 为 404，body **不**得含 SPA 入口 HTML 内容

#### Scenario: path traversal 攻击 SHALL NOT 拿到磁盘文件

- **WHEN** 浏览器请求含 `..` 段（含 URL-encoded 形态）或 backslash 的路径
- **THEN** 响应 SHALL NOT 为 200 且 SHALL NOT 含任何静态文件内容
- **AND** 状态码可为 403 / 400 / 404 任一（具体由框架决定）

#### Scenario: GET /api/* 不被静态 fallback 拦截

- **WHEN** 浏览器请求 `/api/<endpoint>`
- **THEN** 该请求 SHALL 走 API handler、**不**得被静态 fallback 拦截或回到 SPA 入口

#### Scenario: static_dir 不传或路径无效

- **WHEN** 启动 server 时不传 static_dir 或传无效路径
- **THEN** 路径无效场景 SHALL 仅 warn 级日志，不阻塞启动；未命中 API 路由的请求 SHALL 返 404

### Requirement: Mirror lazy and auxiliary IPC commands as HTTP endpoints

HTTP server SHALL 在 `/api` 前缀下镜像目前仅在 IPC 路径暴露的 lazy 与辅助 command，让浏览器 runtime 能跑通完整 UI 流程。每个 endpoint 的请求 / 响应 schema SHALL 与对应 IPC command 同形（camelCase 字段、相同 enum tag、相同 omit 语义）。完整路由清单详 design.md `D-Impl-4`，行为类别覆盖：

- 项目 memory 文件读取
- subagent trace
- 图像资源（SHALL 转 browser-loadable 形态——若底层 IPC 返回 Tauri-only `asset://` URL，HTTP handler SHALL 转为 `data:` URI）
- tool 输出（保留 omit 语义）
- 通知触发器 CRUD（caller SHALL 在 body 内提供非空 id，与 IPC 路径校验语义一致；server 不自动生成 id）
- session pin / hide / 偏好读取

每个 handler SHALL 直接委托给等价 IPC 方法，错误映射沿用 `Return safe defaults on lookup failures` Requirement 的 code → status 表。

**HTTP payload omit 行为**：lazy endpoint（图像资源 / tool 输出）SHALL **不**应用 omit 裁剪——它们本就是 IPC 侧懒加载的真实数据来源，HTTP 路径用户主动请求时 SHALL 拿到完整 payload。会话详情路径维持现有"HTTP 不应用 omit"语义不变。

#### Scenario: GET 项目 memory 镜像 IPC

- **WHEN** 浏览器请求项目 memory 路由
- **THEN** 响应 SHALL 与等价 IPC 命令同形（含 CLAUDE.md 各 scope）

#### Scenario: GET image asset returns browser-loadable data

- **WHEN** 浏览器请求图像资源路由
- **THEN** 响应 SHALL 是浏览器可加载的 `data:` URI 或 base64 字符串
- **AND** SHALL NOT 返回 Tauri-only `asset://` URL
- **AND** SHALL **不**应用任何 omit 裁剪（lazy 端点本就是真实数据源）

#### Scenario: GET tool output preserves omit semantics

- **WHEN** 浏览器请求 tool 输出路由
- **THEN** 响应 SHALL 与等价 IPC 命令同形——保留 outputBytes / outputOmitted 字段、内层 text / value 携带完整内容

#### Scenario: POST add trigger persists caller-provided id

- **WHEN** 浏览器发起新增通知触发器请求，body 为合法触发器（含 caller 自行分配的非空 id）
- **THEN** 响应 SHALL 返回更新后的完整配置 JSON，触发器列表含该新 trigger 且 id 与 caller 入参一致
- **AND** 后续读取配置 SHALL 含该新 trigger

#### Scenario: POST pin session 与 DELETE unpin session 互逆

- **WHEN** 浏览器先 POST pin 再 GET 偏好
- **THEN** 偏好响应中 pinned 数组 SHALL 含该 session id
- **WHEN** 然后 DELETE unpin 再 GET 偏好
- **THEN** pinned 数组 SHALL 不含该 session id

### Requirement: 浏览器 client SHALL 在首次 list_sessions 前订阅 SSE

由于列会话路由改为骨架 + SSE 异步 patch 语义，浏览器 client SHALL 在发起首次列会话请求之前确保 SSE 订阅已进入 OPEN 状态——否则 backend 在 GET response 返回后立即 emit 的 metadata 更新事件会在订阅前发生，导致 metadata patch 永久丢失（直到 file-change 触发 silent refresh 兜底）。

实现 SHALL 在 browser transport 层对所有走"列会话"链路的命令添加 SSE OPEN 前置 await。等待行为：

- 已 OPEN → 立即放行
- CONNECTING → await 单次 onopen 或最大 1000ms 超时
- CLOSED → 触发 reconnect 后 await 新 source onopen 或最大 1000ms 超时
- 超时后 SHALL 放行（不抛错）让首次 fetch 继续

Tauri runtime SHALL NOT 受此 Requirement 影响——Tauri IPC event listener 由原生 API 同步注册，不存在异步 OPEN 等待问题。

#### Scenario: BrowserTransport 等待 SSE OPEN 后才发列会话请求

- **WHEN** 浏览器 runtime 首次调用列会话命令
- **AND** SSE 处于 CONNECTING
- **THEN** transport SHALL await 直至 SSE 进入 OPEN 或 1000ms 超时
- **AND** 仅在 await 返回后 SHALL 发起列会话 fetch

#### Scenario: SSE 已 OPEN 时不阻塞列会话

- **WHEN** 浏览器调用列会话命令且 SSE 已 OPEN
- **THEN** SHALL 立返（不引入额外 latency）

#### Scenario: SSE 1000ms 超时后仍放行 + sse-recovered 自愈

- **WHEN** SSE 由于网络问题 1000ms 内未进入 OPEN
- **THEN** await 入口 SHALL resolve（不抛错）
- **AND** 后续 fetch SHALL 正常发起
- **AND** transport SHALL 记录 timeout 状态，使得后续 SSE 真正进入 OPEN 时（自然成功 / 重连成功）SHALL 给所有已注册 handler emit 一次 sse-recovered 事件，让 UI 触发 silent refresh 重拉一轮 metadata patch
- **AND** 之后立即清空 timeout 标志，避免后续 onopen 再次重复 emit

### Requirement: /api/events SSE 在 broadcast 容量打满时 SHALL 推送 sse_lagged sentinel

SSE handler 在 broadcast 容量打满 + 当前 receiver 跟不上速度时返 Lagged 错误。原实现走"静默吞掉"，UI 永久看不到落地的 metadata patch。系统 SHALL 把 Lagged 转为一条 SSE event，data 字段为 `'{"type":"sse_lagged"}'` 字面量字符串；stream SHALL 继续从最新 PushEvent 接收，**不**退出 stream。

UI 层 browser transport 收到 sse_lagged event SHALL 映射到 sse-lagged event name 派发给所有 handler；订阅方 SHALL 与 sse-recovered 共享同一 silent refresh handler 重拉一轮 metadata。

#### Scenario: 容量打满时 SSE handler 推送 sse_lagged sentinel

- **WHEN** broadcast capacity 被打满 + 当前 SSE receiver 落后导致 Lagged
- **THEN** SSE handler SHALL 推送一条 SSE event，data 字段字符串等于 `'{"type":"sse_lagged"}'`
- **AND** SSE stream SHALL 继续从最新 PushEvent 接收（**不**退出 stream / **不**静默丢弃）
- **AND** 后续真正的 metadata 更新等事件 SHALL 正常推送到 client

#### Scenario: 浏览器 client 收到 sse_lagged 时触发 silent refresh

- **WHEN** 浏览器 client 收到 sse_lagged event
- **THEN** browser transport SHALL 把它映射到 sse-lagged event name 派发给所有 handler
- **AND** 订阅方 SHALL 对当前选中 project 触发 silent refresh 兜底重拉

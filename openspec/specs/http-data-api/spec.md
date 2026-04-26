# http-data-api Specification

## Purpose

把 `ipc-data-api` 暴露的全部数据操作（项目 / 会话 / 搜索 / 配置 / 通知 / 通用工具 / SSH）通过 `/api` 前缀的 HTTP endpoint 镜像出去，并以 Server-Sent Events 推送同一套实时事件流。本 capability 让远端浏览器或第三方客户端不依赖 Tauri runtime 即可消费会话数据。

## Requirements

### Requirement: Serve projects and sessions over HTTP under /api prefix

系统 SHALL 在 `/api` 前缀下暴露与 IPC data API 同形数据返回的 HTTP endpoint，覆盖：列项目、取项目详情、取项目仓库信息、列会话（含分页与按 id 批量两种 variant）、取会话详情、取会话 chunk、取会话 metrics、取 waterfall 数据、取 subagent 详情。

#### Scenario: GET list of projects
- **WHEN** 客户端发起 `GET /api/projects`
- **THEN** 响应 SHALL 是与 IPC list-projects 操作返回同形的 JSON 项目列表

#### Scenario: GET session detail
- **WHEN** 客户端发起 `GET /api/sessions/:id`
- **THEN** 响应 SHALL 含与 IPC 同形的 chunks、metrics、metadata

#### Scenario: GET paginated sessions for a project
- **WHEN** 客户端发起 `GET /api/projects/:projectId/sessions-paginated?pageSize=N&cursor=C`
- **THEN** 响应 SHALL 与 IPC 分页 sessions 返回同形

### Requirement: Serve search endpoints

系统 SHALL 在 `/api` 下暴露与 `session-search` capability 对应的搜索 endpoint，接受 POST body 形式的 query 参数，返回有序结果。

#### Scenario: POST session search
- **WHEN** 客户端发起 `POST /api/search/sessions`，body 含 query、project id、可选 session id
- **THEN** 响应 SHALL 与等价 IPC 搜索操作返回同形

### Requirement: Serve auxiliary, subagent, utility, and validation endpoints

系统 SHALL 暴露与 `ipc-data-api` 中所有辅助操作一一对应的 HTTP endpoint，包括 subagent 详情 / trace、仓库分组、worktree sessions、CLAUDE.md 读取、agent configs、路径 / mention 校验、通用 shell 操作、SSH、updater。

#### Scenario: GET subagent detail
- **WHEN** 客户端发起 `GET /api/subagents/:id/detail`
- **THEN** 响应 SHALL 含该 subagent 的 chunks、metrics、spawning context

#### Scenario: POST path validation
- **WHEN** 客户端发起带文件系统路径的路径校验请求
- **THEN** 响应 SHALL 标明路径是否存在以及是否在允许根之内

### Requirement: Serve config and notification endpoints

系统 SHALL 暴露读取 / 更新配置以及列出 / 标记通知为已读的 HTTP endpoint，语义与 IPC data API 一致。

#### Scenario: PATCH config field
- **WHEN** 客户端发起一次配置更新请求
- **THEN** 响应 SHALL 反映新配置，且变更 SHALL 已被持久化

### Requirement: Push events via Server-Sent Events

系统 SHALL 暴露一个 Server-Sent Events endpoint，传递与 IPC push channel 相同的事件流：`file-change`、`todo-change`、`new-notification`、`ssh-status`、updater 事件。

#### Scenario: SSE client subscribes and receives file change
- **WHEN** SSE 客户端已连接，某 session 文件被修改
- **THEN** 客户端 SHALL 在 debounce 窗口内收到一条 `file-change` 事件，携带 project id 与 session id

#### Scenario: Multiple concurrent SSE clients
- **WHEN** 三个 SSE 客户端已连接，发出一次通知
- **THEN** 每个客户端 SHALL **恰好**收到一次该事件

### Requirement: Return safe defaults on lookup failures (current baseline)

系统 SHALL 对查询失败返回结构化错误响应：缺失资源 `404`，body `{"code":"not_found","message":"..."}`；非法输入 `400`，body `{"code":"validation_error","message":"..."}`。这是相对 TS 基线的有意改进——TS 基线返回 `200` 配 `null` / 空数组。

#### Scenario: GET nonexistent session
- **WHEN** 客户端请求一个不存在的 session id
- **THEN** 响应 SHALL 为 `404`，body 含 `code: "not_found"`

#### Scenario: GET sessions for unknown project
- **WHEN** 客户端请求一个无法解析的 project id 的 sessions
- **THEN** 响应 SHALL 为 `404`，body 含 `code: "not_found"`

#### Scenario: Unhandled server exception
- **WHEN** 处理请求时抛出未捕获异常
- **THEN** 响应 SHALL 为 `500`，body 含 `code: "internal"`

### Requirement: Bind to configured port with graceful fallback

系统 SHALL 把 HTTP server 绑定到应用配置中的端口，若该端口已被占用 SHALL 在启动时记录明确的错误，SHALL NOT 静默改用其它端口。

#### Scenario: Configured port is busy
- **WHEN** 配置端口已被其它进程占用
- **THEN** 启动 SHALL 记录明确错误，SHALL NOT 静默切换端口

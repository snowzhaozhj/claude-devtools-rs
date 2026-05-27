# ipc-data-api Specification

## Purpose

数据查询层在 Tauri 进程内对前端 webview 暴露的所有 IPC 操作契约：项目 / 会话查询、搜索、配置、通知、SSH、agent configs、CLAUDE.md 读取、subagent trace 与 image asset 懒加载、tool output 懒加载、teammate 消息嵌入、session metadata 异步推送、file-change / detected-error 事件广播。本 capability 同时定义首屏 IPC payload 的瘦身策略（`OMIT_*` 系列开关 + `xxxOmitted` flag），让大会话首次打开仍能在 webview 端流畅渲染。

## MODIFIED Requirements

### Requirement: Expose project and session queries

系统 SHALL 在请求 / 响应式 IPC 通道上暴露项目与会话相关数据查询，至少包括：列项目、列项目下 sessions（含分页）、取 session 详情、取 session metrics、取 waterfall 数据、取 subagent 详情。

`get_session_detail` SHALL 在返回 session 详情时集成 subagent 解析：**从主 session 所在 `projects_dir`（即 `~/.claude/projects/` 或 SSH 远端等价路径）下所有 project 目录扫描 `{rootSessionId}/subagents/agent-*.jsonl`（新结构）**，合并去重后填充 `AIChunk.subagents` 字段。旧结构（flat `{project_dir}/agent-*.jsonl`）SHALL 保持只扫描主 `project_dir` 并按首行 `parentUuid` / `sessionId` 字段过滤。若扫描失败或无候选，`subagents` SHALL 为空数组（不报错）。跨目录扫描开关设为 `false` 时 SHALL 退回"只扫主 `project_dir`"的原行为。

**`get_session_detail` 返回的 `SessionDetail.chunks` 中所有 `AIChunk.subagents[i].messages` 数组 MUST 默认被裁剪为空 Vec，且 `messagesOmitted=true`** —— 用于把首屏 IPC payload 控制在原大小约 40%（subagent 嵌套 chunks 全文是大头）。`Process.headerModel` / `Process.lastIsolatedTokens` / `Process.isShutdownOnly` 三个 derived 字段 MUST 在候选转换阶段由 messages 预算后填充，让 SubagentCard header 不依赖完整 `messages` 也能正常渲染。subagent 消息裁剪开关设 false 时 SHALL 退回完整 payload（messages 不裁剪、`messagesOmitted=false`）。

**`get_session_detail` 返回的 `SessionDetail.chunks` 中所有 `ContentBlock::Image.source.data` 字段 MUST 默认被替换为空字符串 `""`，且同时设 `source.dataOmitted=true`** —— 用于把首屏 IPC payload 中内联截图的 base64 字符串裁掉（行为契约见本 spec `Lazy load inline image asset` Requirement）。`source.kind` / `source.media_type` 字段 SHALL 保留（前端渲染时仍需要），仅 `data` 字段被清空。图片数据裁剪开关设 false 时 SHALL 退回完整 base64 payload（`data` 保留原值、`dataOmitted=false`）。该裁剪 SHALL 应用于所有 chunk 类型（UserChunk / AIChunk responses / subagent.messages 内嵌套——但 subagent.messages 默认已被裁剪，仅在回滚 subagent 消息裁剪时才会触及嵌套层）。

**`get_session_detail` 返回的 `SessionDetail.chunks` 中所有 `AIChunk.responses[i].content` 字段 MUST 默认被替换为空文本内容，且同时设 `contentOmitted=true`** —— 用于把首屏 IPC payload 中最大单一字段裁掉。该字段在前端任何代码路径中都不被读取（chunk 显示文本走 `semanticSteps` 的 thinking / text 步骤），裁剪后 UI 渲染零变化。response content 裁剪开关设 false 时 SHALL 退回完整 payload（`content` 携带原值、`contentOmitted=false`）。该裁剪 SHALL 应用于顶层 AIChunk 及 subagent.messages 嵌套层（与图片数据裁剪同模式：在 subagent 消息默认裁剪时嵌套层为 no-op；回滚 subagent 消息裁剪时仍能命中嵌套层）。

**`get_session_detail` 返回的 `SessionDetail.chunks` 中所有 `AIChunk.tool_executions[i].output` 内 `text` / `value` 字段 MUST 默认被替换为空（Text 变空字符串 / Structured 变 Null / Missing 不变），且同时设 `outputOmitted=true`** —— 用于把首屏 IPC payload 中 tool 输出裁掉（行为契约见本 spec `Lazy load tool output` Requirement）。`output` enum 的 variant kind SHALL 保留（前端 ToolViewer 路由仍需要），仅内层 `text` / `value` 被清空。tool output 裁剪开关设 false 时 SHALL 退回完整 payload（`output` 内字段保留原值、`outputOmitted=false`）。该裁剪 SHALL 应用于顶层 AIChunk 及 subagent.messages 嵌套层（与其它裁剪同模式：默认嵌套层 no-op；回滚 subagent 消息裁剪时仍能命中嵌套层）。

**tool output 裁剪路径下 `ToolExecution.outputBytes: Option<u64>` MUST 在清空 output 之前按 variant 记录原始字节长度**（Text → 文本字节长度、Structured → JSON 序列化后字节长度、Missing → 不填保持 `None`），让前端在懒加载之前即可估算 output token 数（按 `outputBytes / 4` 启发式），从而 BaseItem 头部 token 显示 SHALL **在懒加载展开前后保持一致**——不再因 token 计算在裁剪状态返回 0、懒加载后返回真实值而抖动。tool output 裁剪关闭时 `outputBytes` SHALL 保持 `None`（前端 fallback 到直接读文本长度）。解析层 SHALL **不**主动填充 `outputBytes`——该字段仅在 IPC 裁剪层语义有意义。

`list_sessions` 返回的每个 `SessionSummary` MUST 携带 `sessionId` / `projectId` / `timestamp` 的真实值（可直接从目录扫描得出），但 `title` / `messageCount` / `isOngoing` SHALL 允许为占位值（`null` / `0` / `false`）——这些元数据字段的真实值由后端异步扫描后通过 `session-metadata-update` push event 逐条推送（见本 spec `Emit session metadata updates` Requirement）。`get_session_detail` 返回的 `SessionDetail.isOngoing` 仍 MUST 为同步计算后的真实值（因为 detail 已在调用链内完成全文件解析）。

**`isOngoing` 真实值 SHALL 由两路 AND 计算**：(a) 结构性活动栈五信号判定返回 `true`，**且** (b) session JSONL 文件 mtime 距当前时刻 `< 5 分钟`。任一条件不满足时 `isOngoing` MUST 为 `false`。stale 阈值为 5 分钟（对齐原版 TS 实现的同名常量，覆盖 CLI 异常退出时活动栈误判 ongoing 的场景——mtime 兜底纠正）。`list_sessions` 异步扫描路径与 `get_session_detail` 同步路径行为 MUST 一致；HTTP `GET /api/projects/{projectId}/sessions` 路径共用同一元数据提取实现（详见本 spec §"HTTP `list_sessions` 复用 IPC 骨架 + push 实现"），自动适用。stat 失败时 SHALL 保守保留活动栈判定（避免 fs 偶发错误把活跃 session 错判 dead）；时钟回拨导致 mtime > now 时 SHALL 判 not stale（避免未来 mtime 把活跃 session 误判 dead）。

序列化 SHALL 使用 camelCase（`isOngoing`、`messagesOmitted`、`headerModel`、`lastIsolatedTokens`、`isShutdownOnly`、`dataOmitted`、`contentOmitted`、`outputOmitted`、`outputBytes`）。例外：`ImageSource.media_type` 与 `ImageSource.type`（kind）保持 snake_case，与上游 Anthropic JSONL 格式一致——同 `TokenUsage` 例外。

**HTTP `GET /api/projects/{projectId}/sessions` 路径 SHALL 与 IPC `list_sessions` 共用骨架 + push 实现**——即 HTTP handler SHALL 调会话列表查询接口（骨架快返 + 缓存元数据快速路径 + spawn 后台扫描 + 事件广播通道 emit），**不**得调同步全扫描接口。后台扫描产物 SHALL 通过 HTTP 桥接层转换为 SSE 推送到 `/api/events`，浏览器 client 按 `session-metadata-update` event 收到与 IPC 路径同形的 patch。

同步全扫描 trait method SHALL 保留作为 trait 默认 fallback（供未来非 SSE-aware HTTP client 或 CLI 直接 trait 调用使用），但 HTTP route 实现 **不**得再调用它。HTTP 路径同样 SHALL NOT 应用图片 / response content / tool output 裁剪（HTTP 当前无活跃用户、且无对应 asset 协议端点 / 懒拉接口，保留完整 payload 传输）。

#### Scenario: outputBytes filled before trim under OMIT_TOOL_OUTPUT

- **WHEN** tool output 裁剪路径触发处理一个 `ToolExecution`
- **AND** 该 `ToolExecution.output` 是 `Text { text: "abcde" }`（5 字节）
- **THEN** 处理后 `output.text` SHALL 为 `""`、`outputOmitted` SHALL 为 `true`、`outputBytes` SHALL 为 `Some(5)`

#### Scenario: outputBytes for structured uses serialized length

- **WHEN** tool output 裁剪处理 `Structured { value: {"stdout": "ok", "exit": 0} }`
- **THEN** `outputBytes` SHALL 为 `Some(JSON 序列化后的字节长度)`，`output.value` SHALL 为 `Null`

#### Scenario: outputBytes none for missing variant

- **WHEN** tool output 裁剪处理 `output: Missing`
- **THEN** `outputBytes` SHALL 保持 `None`、`output` 不变

#### Scenario: BaseItem token count stable across expand

- **WHEN** 前端 `BaseItem` 渲染一条 `outputOmitted=true` 的 tool 行
- **AND** 用户点击展开触发懒加载，展开后 `output.text` 替换为完整原始内容
- **THEN** 头部 token badge 显示的数字 SHALL **在展开前后相等**（前端在懒加载前从 `outputBytes` 估算、懒加载后从 `outputBytes` 读取——两次结果一致）

#### Scenario: get_session_detail 跨 project_dir 装载 subagent
- **WHEN** caller 调 `get_session_detail(A, S)`，A 是主 `project_id`，S 是 root session id
- **AND** subagent JSONL 物理位于 `project_dir = B`（`B/S/subagents/agent-<subUuid>.jsonl`）
- **THEN** 返回 `SessionDetail.chunks` 内对应 Task tool_use 的 `AIChunk.subagents` SHALL 含 `Process { session_id: <subUuid>, ... }`
- **AND** subagent 关联三阶段 fallback SHALL 正常评估，与"主 project_dir 自带 subagent"等价

#### Scenario: 跨目录扫描开关=false 回滚到原行为
- **WHEN** 跨目录 subagent 扫描开关设为 false
- **AND** subagent JSONL 位于非主 `project_dir`
- **THEN** `get_session_detail` SHALL NOT 装载该 candidate，对应 Task SHALL 保留为未解析（原行为）

#### Scenario: HTTP list_sessions 走骨架而非 sync

- **WHEN** 客户端发起 `GET /api/projects/{projectId}/sessions?pageSize=N&cursor=C`
- **THEN** HTTP handler SHALL 调会话列表查询接口（**不**得调同步全扫描接口）
- **AND** 响应 body SHALL 是骨架 `PaginatedResponse<SessionSummary>`：每条 `SessionSummary` 的 `sessionId` / `projectId` / `timestamp` SHALL 为真实值；`title` / `messageCount` / `isOngoing` / `gitBranch` SHALL 允许为占位值（除非缓存元数据快速路径命中可直接 inline 填回真值）

#### Scenario: HTTP list_sessions 后台扫描产物经 SSE 推送

- **WHEN** HTTP `list_sessions` 返回骨架后，后台元数据扫描任务对 cache miss 的 session 完成扫描并通过事件广播通道发送 update
- **THEN** 该 update SHALL 通过 HTTP 桥接层转换为 `PushEvent::SessionMetadataUpdate { projectId, sessionId, title, messageCount, isOngoing, gitBranch }` 推送到所有 `/api/events` 客户端
- **AND** 浏览器 client 传输层 SHALL 按既有归一化路径转交 `session-metadata-update` 事件给 listener，与 IPC 路径行为一致

### Requirement: Lazy load inline image asset

`get_image_asset(rootSessionId, sessionId, contentBlockIndex, responseIndex) -> String` IPC MUST 返回指定 content block 的完整 base64 字符串。后端 SHALL 按 `sessionId` 定位对应 jsonl，`parse_file` 后在 AI messages 中线性 scan 找 response block → content block → Image 匹配，返回 `source.data`。

缓存策略：首次拉取成功后 SHALL 把 base64 写到本地持久缓存目录（以 `{rootSessionId}/{sessionId}/{responseIndex}_{contentBlockIndex}.b64` 作为 key），后续同参数调用 SHALL 优先从缓存读（跳过 jsonl 解析）。缓存容量上限 2000 entries（LRU 淘汰）。

#### Scenario: 拉取已裁剪的 image base64

- **WHEN** caller 调用 `get_image_asset("root-uuid", "root-uuid", 0, 2)`
- **AND** 对应 jsonl 存在、response[2].content[0] 是 Image block、`dataOmitted=true`
- **THEN** 后端 SHALL 从 jsonl re-parse（或 cache hit）返回完整 base64 `source.data`

#### Scenario: 缓存命中跳过 jsonl 解析

- **WHEN** 同参数第二次调用 `get_image_asset("root-uuid", "root-uuid", 0, 2)`
- **AND** 本地持久缓存目录存在对应 `0_2.b64` 文件
- **THEN** 后端 SHALL 直接从缓存文件返回，不 parse jsonl

#### Scenario: 缓存写入失败 fallback 到 data URL

- **WHEN** 缓存目录不可写（permission denied / 磁盘满）
- **THEN** 响应 SHALL 为 `data:<mediaType>;base64,<完整 base64>` 字符串，前端按 `<img src>` 仍可加载
- **AND** 后端 SHALL 记录警告日志供排查

### Requirement: Session 列表序列化暴露 cwd 字段

`list_sessions` 与 `get_session_detail` 返回的 `Session`（或 `SessionSummary`）IPC payload SHALL 暴露 `cwd?: string` 字段（camelCase）。该字段值来自 session 数据模型中的 `cwd` 字段（详见 `project-discovery` spec `Expose session cwd for downstream display` Requirement），表示该 session jsonl 内首条带 `cwd` 字段消息的 `cwd` 值。

无 cwd 信息（jsonl 不含 `cwd`）时 SHALL 在 payload 中省略该键（序列化时跳过 None 值），**不**得序列化为 `"cwd": null`，以保持老前端 / 老 fixture 兼容。

HTTP 路径（`GET /api/projects/:id/sessions` / `GET /api/projects/:id/sessions/:sid`）SHALL 同步暴露 `cwd` 字段——与 IPC 路径共享会话列表/详情查询实现，自动适用。

#### Scenario: 含 cwd 的 session 在 list_sessions 返回中带 cwd

- **WHEN** `list_sessions(projectId)` 命中一个 jsonl session，其首条消息 `cwd = "/Users/foo/myrepo/.claude/worktrees/feat-x"`
- **THEN** 返回数组对应条目 SHALL 含 `"cwd": "/Users/foo/myrepo/.claude/worktrees/feat-x"`

#### Scenario: 无 cwd 的 session 在 list_sessions 返回中省略 cwd

- **WHEN** `list_sessions(projectId)` 命中一个 jsonl session，所有消息均不含 `cwd` 字段
- **THEN** 返回数组对应条目 SHALL NOT 包含 `cwd` 键
- **AND** 该 session 其它字段（`id` / `lastModified` / `size` / `isPinned`）SHALL 保留

#### Scenario: get_session_detail 元数据带 cwd

- **WHEN** `get_session_detail(projectId, sessionId)` 命中目标 session
- **THEN** `SessionDetail.metadata` 或顶层等价位置 SHALL 含 `"cwd": <value or omitted>`，与 `list_sessions` 同口径

### Requirement: get_session_detail 本地路径以单文件 stat 取元数据

`get_session_detail` 在本地（非 SSH）路径 SHALL 通过单次异步文件 stat 系统调用获取目标 session 的 `lastModified` 与 `size`，**SHALL NOT** 触发跨 project 的全量扫描（即不得调全量扫描或等价的"列举 `~/.claude/projects/` 下所有目录"路径），也 SHALL NOT 为获取 mtime / size 而读取目标 jsonl 之外的任何文件。

session jsonl 文件不存在时，本地路径 SHALL fallback 至现有 subagent 查找路径（沿用现状行为）；fallback 仍不存在时 SHALL 返回 not_found 错误。

远程 SSH 路径行为不变（沿用现有轻量列举 + 单文件元数据获取）。

#### Scenario: 本地打开 session 详情不触发全量扫描

- **WHEN** `get_session_detail("foo-project", "session-1")` 在本地环境调用，`foo-project` 与 `session-1.jsonl` 在 `~/.claude/projects/foo-project/` 下存在
- **THEN** 后端实现 SHALL 仅对 `~/.claude/projects/foo-project/session-1.jsonl` 调一次异步文件 stat
- **AND** SHALL NOT 对 `~/.claude/projects/` 下其它 project 目录调 `read_dir` / `stat`
- **AND** SHALL NOT 调 head-read / 全文读取除目标 jsonl 之外的任何 jsonl

#### Scenario: 目标 jsonl 不存在 fallback 到 subagent 查找

- **WHEN** `get_session_detail("foo-project", "missing-session")` 调用，`missing-session.jsonl` 不在主目录但存在于 `subagents/agent-*.jsonl`
- **THEN** 后端 SHALL 通过 subagent 查找路径定位到 subagent jsonl 并返回其 detail

#### Scenario: 目标 session id 完全不存在返回 not_found

- **WHEN** `get_session_detail("foo-project", "nope")` 调用，`nope.jsonl` 既不在主目录也不在 `subagents/` 下
- **THEN** 后端 SHALL 返回 not_found 错误，**不**触发全量扫描以试图反查

### Requirement: Contract test asserts get_session_detail does not cross project boundary

contract test 层 SHALL 通过 spy 文件系统抽象包装（在测试 wrapper 里记录每个 `read_dir` / head-read / 全文读取 / stat 方法被调次数 + 路径列表），覆盖 `get_session_detail` 的本地路径，断言：调用 `get_session_detail(P, S)` 后，spy 记录的 `read_dir` 调用次数 == 0；head-read 与 stat 的 path 集合 SHALL ⊆ {target jsonl path}（解析 jsonl 内容的 head-read 与目标 stat 允许）；spy 记录的所有 path 都 SHALL NOT 落在 `~/.claude/projects/<P>` 之外的兄弟 project 目录。

该 contract test SHALL 在 IPC contract test 套件内运行（与性能 bench 互补）；本断言 SHALL 在 CI 默认 job 内执行，对"不全扫"行为契约提供机器验证保护。

#### Scenario: spy 文件系统抽象验证不读取兄弟 project

- **WHEN** 测试搭建 `tempdir` 下铺 3 个 project（`P_A` / `P_B` / `P_C`），每个 2 个 session jsonl
- **AND** 调用 `get_session_detail("P_A", "session_1")`
- **THEN** spy 记录的 `read_dir` 调用次数 SHALL 为 0
- **AND** spy 记录的所有 path 中 SHALL NOT 含 `P_B/` 或 `P_C/` 下任何文件
- **AND** head-read / stat 的 path 集合 SHALL ⊆ `{tempdir/P_A/session_1.jsonl}`

### Requirement: ProjectScanner shared read semaphore injection

项目扫描器 SHALL 接受外部注入的共享并发限制器控制 head-read 并发，所有数据查询层内部调用 SHALL 复用同一共享并发限制器实例（容量默认 64）；MUST NOT 在每次 IPC（含 `list_sessions` / `list_group_sessions` / `list_repository_groups`）新建独立并发限制器，否则多 IPC 并发时实际并发上限会变为 `IPC 数 × 64`，违反 CPU 反模式约束。

项目扫描器旧构造器 SHALL 保留为测试便利构造（内部仍新建并发限制器），生产代码 SHALL 调用接受外部注入的构造器。

数据查询层 SHALL 在构造时创建 / 接受共享并发限制器字段，所有内部项目扫描器构造点 SHALL 传入该字段。

#### Scenario: 19 worktree 并发拉骨架共享 semaphore
- **WHEN** `list_group_sessions` 内部并发跑 19 个 worktree 扫描，每个 worktree 含 100 个 session
- **THEN** 同时 in-flight 的 head-read 调用数 SHALL 不超过 64（共享并发限制器容量）
- **AND** SHALL NOT 出现 19 × 64 = 1216 并发的击穿

#### Scenario: 测试代码可用旧便利构造
- **WHEN** 测试代码调用项目扫描器旧构造器
- **THEN** 测试代码无需手动创建并发限制器；编译通过

#### Scenario: 生产代码强制走注入构造器
- **WHEN** 生产代码使用项目扫描器
- **THEN** SHALL 仅使用接受外部并发限制器的构造器，旧构造器 SHALL 仅在测试代码中出现

### Requirement: `ProjectScanCache` 按事件语义分级失效

`LocalDataApi::new_with_watcher(...)` 构造路径 SHALL spawn 后台 task（"unified invalidator"），订阅 `FileWatcher::subscribe_files()` 广播。该 task 对每条 `FileChangeEvent` SHALL 仅根据 `FileChangeEvent` 字段（`project_id` / `session_id` / `deleted` / `project_list_changed`）+ `ProjectScanCache` snapshot lookup 决定**是否**失效 `ProjectScanCache` Local entry。三档判定结果 SHALL **仅**用于 invalidate 决策，**不**再用于填写 `FileChangeEvent.session_list_changed` 字段——后者由 watcher 层负责（详 `file-watching` Requirement `跟踪 session 首见性以填写 revalidation hint`）。本 Requirement 同时 SHALL 把 cache snapshot 视角的"unknown_session"判定结果作为**辅助 hint** 暴露给 unified invalidator emit 路径，与 watcher 字段做并集 OR（详 `Unified invalidator 作为 LocalDataApi.file_tx 唯一生产者` Requirement 的 emit 公式）。

**判定规则（三档，仅决定 invalidate）**：

1. `event.project_list_changed == true` **OR** `event.deleted == true` → 调 `ProjectScanCache::invalidate_local()`，inc counter `project_scan_cache.invalidate.structural`
2. `event.session_id` 非空 **AND** (`ProjectScanCache::has_entry(local_ctx) == true` **OR** `ProjectScanCache::has_in_flight_scan() == true`) **AND** `ProjectScanCache::contains_session_id(local_ctx, &event.project_id, &event.session_id) == false`（cache 已有该 ctx 的 entry 或当前有 in-flight scan 在跑，且 snapshot 不含此 session）→ 同规则 1：`invalidate_local()` + structural counter
3. 其他（普通 JSONL append + watcher 折叠的 subagent 修改 + 空 sid 事件 + cache 无 entry **且**无 in-flight scan 时的任意非 structural 事件）→ **不**调任何失效 API，保留现有 cache，inc counter `project_scan_cache.invalidate.content_append_skipped`

**为何需要规则 2**：`cdt-watch::FileWatcher` 在构造时**预填**当前已存在的 project 目录到 `known_projects` HashSet。已知 project 下新建 session 时 `mark_project_seen` 不会返回 true，watcher 输出 `plc=false, deleted=false`——与"已知 session JSONL 追加"在 `project_list_changed` / `deleted` 字段上**外观完全相同**。仅靠 (plc, deleted) 两 bool 判定会让新 session 最长 `LOCAL_CACHE_TTL = 300s` 不可见。规则 2 用 cache snapshot 反向查询补这个语义缺口决定是否清缓存。watcher 层填写的 `session_list_changed` 字段为 emit 路径直接提供前端 revalidate hint，不依赖 cache 状态，与本规则的 invalidate 决策独立。

**为何需要 `has_entry || has_in_flight_scan` 守护组合**：

- **`has_entry` 单条件不足以防风暴**：lag 路径调 `invalidate_local()` 后 cache 被清空，若不守护，后续普通 append 事件 `contains_session_id` 一律返 false → unknown_session 命中 → 又调 `invalidate_local()` 反复 bump `invalidation_generation` → 在重扫期间 `finish_scan_with_insert` 因 generation mismatch 一直丢弃 snapshot → cache 长期无法 repopulate（持续重扫风暴）。`has_entry` 守护让 cache 空时直接走规则 3 等待业务路径重扫填回。

- **仅 `has_entry` 又会漏掉 in-flight scan 期间结构事件**：cache 空 + 业务路径已经 `begin_scan` 在跑 scan 期间到达"已知 project 下新 session"事件被吞 → generation 不 bump → scan 完成 `finish_scan_with_insert` 旧 snapshot 因 generation 未变成功落地 → 新 session 最长等 TTL 5min 才能看到。

- **联合条件 `has_entry || has_in_flight_scan` 二者兼得**：cache 有 entry 或 scan 在途时走规则 2 判定 bump；cache 空且无 scan 在途时走规则 3 不 bump。**注意**：cache 空且无 scan 在途时本规则**不**清缓存，但此时 watcher 层已经在 `session_list_changed` 字段上承载了"first-seen" 信号，下游 emit 路径仍能让前端正确触发 revalidate（前端 revalidate 路径自然走 cache miss + 重 scan 兜底）。

**对各类真实 fs 事件的语义覆盖**（对应 `cdt-watch::FileWatcher::parse_project_event` 的输出）：

- 新 project 目录创建（`<projects_root>/<pid>` dir-create）→ watcher 输出 `plc=true, sid=""` → 走规则 1（invalidate_local）
- 启动后第一次见某 pid（典型场景：watcher 重启）→ watcher 输出 `plc=true` → 走规则 1（invalidate_local）
- **已知 project 下新 session 首次出现** → watcher 输出 `plc=false, deleted=false` 且 `contains_session_id == false` → 走规则 2（invalidate_local，仅当 has_entry||has_in_flight 时）；watcher 同时填 `session_list_changed=true` 给 emit 路径
- 已知 project 已知 session JSONL 追加（普通 hot path）→ watcher 输出 `plc=false, deleted=false` 且 `contains_session_id == true` → 走规则 3（不清缓存）
- watcher 折叠的 subagent JSONL **修改**（事件 `(pid, sid=父, deleted=false, plc=false)` + `contains_session_id(父 sid) == true`）→ 走规则 3（不清缓存）
- 主 session JSONL 删除 → watcher 输出 `deleted=true` → 走规则 1（invalidate_local）；watcher 同时填 `session_list_changed=true` 给 emit 路径
- watcher 折叠的 subagent JSONL **删除**（事件 `(pid, sid=父, deleted=true, plc=false)`）→ 走规则 1（**false-positive**：事件无法区分主 vs subagent 删除；触发一次重扫即结束，无正确性问题，详 design R6）

**MUST NOT**：

- MUST NOT 扩展或读取 `cdt-core::FileChangeEvent` 中除 `project_id` / `session_id` / `deleted` / `project_list_changed` 之外的其他字段做**判定**输入（`session_list_changed` 字段由 watcher 层填，本规则**仅消费 `event.session_id` 等输入字段**，不依赖 emit 字段做判定输入）
- MUST NOT 在事件回调路径内调任何 fs 操作（文件 stat / 元数据查询 等）—— 完全基于事件字段 + cache snapshot lookup 判定
- MUST NOT 引入 per-project 失效粒度（当前 cache 数据结构无 per-project entry 概念，per-project 重构超本 Requirement scope）
- MUST NOT 让 invalidate 决策影响 `FileChangeEvent.session_list_changed` 字段填写——该字段由 watcher 层独立决定，本规则只产出"是否 invalidate" + "供 emit 路径 OR 兜底的 cache_unknown_hint"

**`ProjectScanCache::contains_session_id` API 契约**：`ProjectScanCache` SHALL 暴露公开方法 `contains_session_id(&self, ctx: &ContextId, project_id: &str, session_id: &str) -> bool`，遍历指定 ctx 对应 entry 的 project 列表，定位 `Project.id == project_id` 后检查 `Project.sessions: Vec<String>` 是否含 `session_id`；ctx 无 entry 或 project 不存在时返回 `false`。复杂度 O(N project × N session_per_project)，复杂度在典型 corpus 下足够低，可在 hot 路径调用。

**`ProjectScanCache::has_entry` API 契约**：`ProjectScanCache` SHALL 暴露公开方法 `has_entry(&self, ctx: &ContextId) -> bool`，返回 `entries` 是否含此 ctx 的 entry。invalidator 在规则 2 判定前 SHALL 先用本方法守护——cache 空时跳过 unknown_session 判定，避免 lag 后被普通 append 事件持续触发 invalidate 导致重扫风暴。

**`ProjectScanCache::has_in_flight_scan` API 契约**：`ProjectScanCache` SHALL 暴露公开方法 `has_in_flight_scan(&self) -> bool`，返回当前 `in_flight_scans > 0`。invalidator 在规则 2 判定前 SHALL 与 `has_entry` 共同 OR 守护——cache 空但有 scan 在途时仍 bump generation，让 in-flight scan 完成回写时识别 race 丢弃 stale snapshot。

**`ProjectScanCache::begin_scan` / `finish_scan_with_insert` / `abort_scan` API 契约**：业务路径 `scan_projects_cached_with` SHALL 用 `begin_scan` 替代裸 `invalidation_generation()` 拿 recorded_generation 同时 `in_flight_scans += 1`；scan 成功时 SHALL 用 `finish_scan_with_insert` 替代 `try_insert`（内部 `in_flight_scans -= 1` + race 校验）；scan 失败时 SHALL 调 `abort_scan` 配对 `begin_scan` 不漏减。这三 API 联合保护 in-flight scan 与 invalidator 之间的 race 协议。

**SSH context entry 不受 file-change 影响 + SSH event 跳过 local cache hint**：watcher 是 Tauri 本地 fs 的硬不变量。invalidator 推算 `ContextId::local(projects_dir)` 决定失效作用域；`ProjectScanCache::invalidate_local()` 实现仅对 `FsKind::Local` entry 生效，SSH entry 仍按既有 TTL 自然过期。SSH `polling_watcher` 通过 `FileWatcher::attach_remote` 喂入同一 watcher broadcast 的事件，进入 unified invalidator 后 SHALL 通过 watcher 来源判定守护——unified invalidator SHALL 调 `FileWatcher::is_local_project(&event.project_id)` 检查 event 的 project_id 是否在 `local_projects_seen` 集合内（该集合由 `parse_project_event` 所有分支在 emit 前通过 `mark_local_origin` 写入，与 `known_projects` 的 first-seen 语义解耦；SSH 事件由远端 polling 直接构造不进 `parse_project_event`），若返回 `false`（即 SSH 事件）则 SHALL 跳过本规则的三档判定，直接走"不 invalidate + emit_session_list_changed_hint=false"——`session_list_changed` 字段已由 SSH polling watcher 在远端事件上对称填好，**不**需要 local cache hint OR 兜底，否则 SSH 普通 size/mtime append（watcher 已填 `false`）会因 `contains_session_id(local_ctx, ssh_pid, ssh_sid)` 永远返 false 让 `emit_session_list_changed_hint=true`，破坏 SSH/Local 字段对称语义并侵蚀 append 降噪收益。详 `file-watching` Requirement `Watch SSH remote project directory via SFTP polling`。`is_local_project` 限制：仅按 project_id 字符串判定，SSH 远端与 local 同名 project 共存时可能误判 local；本 spec 接受此 edge case，根治留 followup（需 watcher 注入 ContextId 做来源排除）。

**`new()` 构造路径不启动该订阅**：`LocalDataApi::new()`（无 watcher 参数）SHALL NOT spawn 此 task；该场景仅依赖被动 generation 校验路径兜底，与 `MetadataCache` / `ParsedMessageCache` 在 `new()` 路径的行为对齐。

**broadcast lag 走保守全失效**：广播接收端 recv 返回 `Err(RecvError::Lagged(_))` 时 SHALL 调 `invalidate_local()` 并 inc counter `project_scan_cache.invalidate.lag_conservative`，因为 lag 期间可能错过 `plc=true` / `deleted=true` 事件且 `ProjectScanCache` 没有 path-level 被动校验机制可兜底。lag 路径下 file_tx emit 行为契约由 `Unified invalidator 作为 LocalDataApi.file_tx 唯一生产者` Requirement 单独承担（synthetic structural event 兜底）。返回 `Err(RecvError::Closed)` 时 SHALL 退出 loop。

> 该 lag 行为与 `parsed-message 缓存按 file-change 广播主动失效` Requirement 的 lag 静默继续策略**有意不一致**：parsed-message cache 在 lookup 时 stat 比对 `FileSignature` 兜底 lag 错过的事件；ProjectScanCache 无类似被动校验，lag 时必须保守清空。

**telemetry counter 注册**：实现 SHALL 在 `cdt-telemetry` 静态白名单中注册以下 3 个 counter：

- `project_scan_cache.invalidate.structural`
- `project_scan_cache.invalidate.content_append_skipped`
- `project_scan_cache.invalidate.lag_conservative`

每条事件 SHALL 按规则结果 inc 对应 counter 各 1 次。

**性能契约**：长时间使用场景（活跃 claude-code 会话每秒多次追加 JSONL）下，`content_append_skipped` 计数 SHALL 远超 `structural`（典型预期 ≥ 95% 走 skipped 分支）；偏离此预期是判定逻辑或 watcher 字段填充偏差的信号。

#### Scenario: 已知 session JSONL 追加 SHALL NOT 失效 cache

- **WHEN** `LocalDataApi` 由 `new_with_watcher` 构造，且 `ProjectScanCache` 已经因前一次 `list_repository_groups` 写入了某 ctx 的 entry，含 project `pa` 和 session `sa`
- **AND** `<projects_root>/pa/sa.jsonl` 被 claude-code 追加新行
- **AND** `FileWatcher` 广播一条对应 `FileChangeEvent { project_id: "pa", session_id: "sa", deleted: false, project_list_changed: false, session_list_changed: false }`（watcher 跟踪集合已含 `(pa, sa)`）
- **THEN** 后台 invalidator MUST 调 `ProjectScanCache::contains_session_id(&local_ctx, "pa", "sa")` 得到 `true`（规则 2 不命中）
- **AND** MUST NOT 调 `invalidate_local`
- **AND** `ProjectScanCache::lookup` 后续仍 SHALL 命中既有 entry（同一 `root_generation` / `context_generation` 下）
- **AND** counter `project_scan_cache.invalidate.content_append_skipped` MUST inc 1

#### Scenario: 已知 project 下新 session 首次出现 SHALL 失效 cache

- **WHEN** `LocalDataApi` 由 `new_with_watcher` 构造，`ProjectScanCache` 已写入某 ctx 的 entry，含已知 project `pa` 与已知 sessions `{sa1, sa2}`（`sa_new` 不在此列表）
- **AND** claude-code 在已知 project `pa` 下创建新 session `sa_new`，写入 `<projects_root>/pa/sa_new.jsonl`
- **AND** `FileWatcher` 广播 `FileChangeEvent { project_id: "pa", session_id: "sa_new", deleted: false, project_list_changed: false, session_list_changed: true }`（watcher 跟踪集合此前不含 `(pa, sa_new)` → first-seen → `session_list_changed=true`）
- **THEN** 后台 invalidator MUST 调 `ProjectScanCache::contains_session_id(&local_ctx, "pa", "sa_new")` 得到 `false`
- **AND** MUST 调 `ProjectScanCache::invalidate_local()`（规则 2 触发）
- **AND** 下一次 `list_repository_groups` SHALL 走 cache miss 并把 `sa_new` 纳入返回值
- **AND** counter `project_scan_cache.invalidate.structural` MUST inc 1

#### Scenario: cache 空 + 新 session 事件 SHALL NOT invalidate（emit 不受影响）

- **WHEN** `LocalDataApi` 由 `new_with_watcher` 构造，`ProjectScanCache::entries` 为空（冷启 / `reconfigure_claude_root` 后 / SSH context 切换让 Local entry 被驱逐），`has_in_flight_scan() == false`
- **AND** `FileWatcher` 广播 `FileChangeEvent { project_id: "pa", session_id: "sa_new", deleted: false, project_list_changed: false, session_list_changed: true }`（watcher 跟踪集合此前不含 `(pa, sa_new)`）
- **THEN** 后台 invalidator MUST NOT 调 `invalidate_local`（规则 2 守护命中：`has_entry == false && has_in_flight_scan == false`）
- **AND** counter `project_scan_cache.invalidate.content_append_skipped` MUST inc 1
- **AND** invalidate 决策的"未触发"SHALL NOT 影响 `Unified invalidator 作为 LocalDataApi.file_tx 唯一生产者` Requirement 定义的 emit 行为——前端仍 SHALL 收到 `session_list_changed=true` 触发兜底 revalidate

#### Scenario: 顶层 dir-create 标 plc=true 时直接走规则 1

- **WHEN** `ProjectScanCache` 已存若干 ctx entry
- **AND** claude-code 创建新 project 顶层目录 `<projects_root>/p_new`
- **AND** `FileWatcher` 广播 `FileChangeEvent { project_id: "p_new", session_id: "", deleted: false, project_list_changed: true, session_list_changed: false }`
- **THEN** 后台 invalidator MUST 仅基于 `event.project_list_changed == true` 走规则 1，调 `invalidate_local()`
- **AND** SHALL NOT 调 `contains_session_id`（事件 `session_id == ""` 触发规则 2 的 `!session_id.is_empty()` 守护跳过）
- **AND** counter `project_scan_cache.invalidate.structural` MUST inc 1

#### Scenario: 删除已知 session JSONL SHALL 失效 cache

- **WHEN** `ProjectScanCache` 已存某 ctx entry 且内含 project `pa` / session `sa`
- **AND** 用户或外部工具删除 `<projects_root>/pa/sa.jsonl`
- **AND** `FileWatcher` 广播 `FileChangeEvent { project_id: "pa", session_id: "sa", deleted: true, project_list_changed: false, session_list_changed: true }`
- **THEN** 后台 invalidator MUST 仅基于 `event.deleted == true` 走规则 1，调 `ProjectScanCache::invalidate_local()`
- **AND** counter `project_scan_cache.invalidate.structural` MUST inc 1

#### Scenario: subagent JSONL 修改 SHALL NOT 失效 cache

- **WHEN** `ProjectScanCache` 已存某 ctx entry，含 project `pa` / 父 session `s_parent`
- **AND** claude-code 写入 `<projects_root>/pa/s_parent/subagents/agent-xyz.jsonl`
- **AND** watcher 折叠到父 session 后广播 `FileChangeEvent { project_id: "pa", session_id: "s_parent", deleted: false, project_list_changed: false, session_list_changed: false }`（subagent 路径 SHALL NOT 进入跟踪集合 → `session_list_changed=false`）
- **THEN** 后台 invalidator MUST 调 `ProjectScanCache::contains_session_id(&local_ctx, "pa", "s_parent")` 得到 `true`
- **AND** MUST NOT 调任何失效 API
- **AND** counter `project_scan_cache.invalidate.content_append_skipped` MUST inc 1
- **AND** `ProjectScanCache::lookup` 后续仍 SHALL 命中既有 entry

#### Scenario: subagent JSONL 删除触发 false-positive invalidate（接受）

- **WHEN** `ProjectScanCache` 已存某 ctx entry，含 project `pa` / 父 session `s_parent`
- **AND** subagent 文件 `<projects_root>/pa/s_parent/subagents/agent-xyz.jsonl` 被删除
- **AND** watcher 折叠到父 session 后广播 `FileChangeEvent { project_id: "pa", session_id: "s_parent", deleted: true, project_list_changed: false, session_list_changed: false }`（subagent 删除路径靠 `deleted=true` 触发刷新，`session_list_changed` 由 watcher 嵌套分支固定填 `false`）
- **THEN** 后台 invalidator MUST 基于 `event.deleted == true` 走规则 1，调 `ProjectScanCache::invalidate_local()`
- **AND** 这是已知的 **false-positive 行为**：事件字段无 path，无法区分主 session 删除 vs subagent 删除；本 spec 显式接受此 false-positive，触发一次 ProjectScanner 重扫的成本可接受
- **AND** counter `project_scan_cache.invalidate.structural` MUST inc 1

#### Scenario: SSH context entry 不受 file-change 影响

- **WHEN** `ProjectScanCache` 已存 SSH ctx entry（由 SSH `polling_watcher` 间接触发或通过其它路径写入）
- **AND** 本地 `FileWatcher` 广播 `FileChangeEvent { project_id: "pa", session_id: "sa", deleted: false, project_list_changed: false, session_list_changed: false }`
- **THEN** unified invalidator MUST 调 `ProjectScanCache::invalidate_local()`，仅对 `FsKind::Local` entry 生效
- **AND** SSH ctx entry SHALL NOT 被失效，按既有 TTL 自然过期

### Requirement: SessionDetail 与高频 DataApi 方法 SHALL 用 typed Rust struct 暴露字段

`SessionDetail` 类型 的 6 个字段（`chunks` / `metrics` / `metadata` / `context_injections` / `injections_by_phase` / `phase_info`）SHALL 用 typed Rust struct（含本 capability 新增的 `SessionDetailMetrics` / `SessionDetailMetadata` 与 已有的 `Chunk` / `ContextInjection` / `ContextPhaseInfo`）持有；`DataApi` trait 中至少以下 5 个高频方法的返回类型 SHALL 是 typed `Result<XxxResponse, ApiError>` 而非 `Result<动态 JSON, ApiError>`：`search` / `get_config` / `update_config` / `get_subagent_trace` / `get_notifications`。typed 化 SHALL **不**改变任何 wire JSON 形状——所有 typed struct 的序列化字段名、camelCase 命名、enum tag（`Chunk.kind` / `ContextInjection.category`）、`xxxOmitted` 标记 SHALL 与本要求被引入之前的 wire 形状逐字节一致。其余 13 个 `Result<动态 JSON, ApiError>` 方法（SSH 子集 / 文件路径子集 / Trigger CRUD / `validate_path`）SHALL 暂留 `Value`，由后续 change 按本 capability 提供的判定准则（`design.md::D2`）逐批 typed 化。

#### Scenario: SessionDetail 6 个字段编译期为 typed

- **WHEN** 调用方在 Rust 代码中按 `let detail: SessionDetail = local_data_api.get_session_detail(...).await?;` 取得 `SessionDetail`
- **THEN** `detail.chunks` SHALL 直接是 `Vec<Chunk>`，`detail.metrics` SHALL 是 `SessionDetailMetrics`，`detail.metadata` SHALL 是 `SessionDetailMetadata`，`detail.context_injections` SHALL 是 `Vec<ContextInjection>`，`detail.injections_by_phase` SHALL 是 `BTreeMap<String, Vec<ContextInjection>>`，`detail.phase_info` SHALL 是 `ContextPhaseInfo`
- **AND** 上述任一字段 SHALL **不**是动态 JSON 值类型
- **AND** 调用方按 `detail.metrics.message_count` 直接访问字段 SHALL 编译通过（不需动态 JSON 解构）

#### Scenario: SessionDetail 序列化 wire 形状不变

- **WHEN** 同样的输入数据分别走 typed 化前与 typed 化后的 `get_session_detail` 实现，并各自做 JSON 序列化
- **THEN** 两次序列化产物 SHALL 在所有 key 名 / value 形状 / 嵌套层次上逐字段一致——具体含 `chunks[*].kind`（`"user"` / `"ai"` / `"system"` / `"compact"`）、`chunks[*].subagents[*].messages` / `messagesOmitted`、`chunks[*].toolExecutions[*].output` / `outputOmitted`、`chunks[*].responses[*].content` / `contentOmitted`、`metrics.message_count`（snake_case 历史 wire，**不**是 `messageCount`，详 `design.md::D5` + `D7`）、`metadata.last_modified` / `metadata.size` / `metadata.cwd`（snake_case 历史 wire）、`contextInjections[*].category`、`injectionsByPhase` 的 key 形状（`String`，由 `phase_number.to_string()` 得出）、`phaseInfo` 内字段
- **AND** IPC contract test 套件现有覆盖 SessionDetail 的所有断言（含 `session_detail_single_phase_injections_by_phase_equals_context_injections` / `session_detail_multi_phase_preserves_phase1_injections` / `session_detail_title_field_round_trip`）SHALL 保持绿

#### Scenario: 5 个高频 DataApi 方法返回 typed

- **WHEN** 调用方在 Rust 代码中按 `let cfg: AppConfig = local_data_api.get_config().await?;`（或 `update_config` / `search` / `get_subagent_trace` / `get_notifications` 同形）取得返回值
- **THEN** 返回类型 SHALL 是 typed struct（`AppConfig` / `SearchSessionsResult` / `Vec<Chunk>` / `GetNotificationsResult`）而非动态 JSON
- **AND** 编译期访问字段（如 `cfg.theme` / `result.results[0].sessionId`）SHALL 通过类型检查
- **AND** JSON 序列化产物 SHALL 与 typed 化前的 hand-built JSON 形状逐字段一致；以下两处 EXCEPTION：
  - `search` empty query 路径：typed 化后形状从 `{query, results}` 扩为 `{query, results, totalMatches, sessionsSearched, isPartial}`（`SearchSessionsResult` 完整字段），属于 bug fix（详 design 决策 D8），新增字段全部为 `0` / `[]` / `false` 默认值，不破坏前端 前端命令面板组件现有 `"totalMatches" in session ? ... : ...` "in" 判定路径
  - `get_sessions_by_ids` not-found fallback 路径：typed 化后 `metadata` 从 `{"status":"not_found"}` 改为 typed default `{"last_modified":null,"size":null,"cwd":null}`（移除 ad-hoc status 带外标记），`chunks` / `phase_info` / `metrics` 从 `null` 改为各自 typed default；前端按 `result.projectId === ""` 判定 not-found（已有信号），详 `design.md::D9`

#### Scenario: 13 个低频方法暂留 Value 是 spec-allowed

- **WHEN** 调用方按 `let resp: serde_json::Value = local_data_api.ssh_connect(...).await?;`（或其他 13 个低频方法之一）取得返回值
- **THEN** 实现 SHALL 仍允许返回 `Result<动态 JSON, ApiError>`，不要求本 change 必须 typed 化
- **AND** 该方法源码处 SHALL 含 `// TODO(typed-ipc-payload): typed 化判定准则见 design.md::D2` 形式的注释链向后续 change

#### Scenario: 前端 SessionDetail TS interface 同步 typed

- **WHEN** 前端 API 类型定义文件中定义 `SessionDetail` interface
- **THEN** `metrics` / `metadata` / `contextInjections` / `injectionsByPhase` 四个字段 SHALL **不**是 `Record<string, unknown>` / `unknown[]`
- **AND** 上述字段 SHALL 引用与 Rust 端 `SessionDetailMetrics` / `SessionDetailMetadata` / `ContextInjection` / `Record<string, ContextInjection[]>` 镜像的 typed TS interface
- **AND** 前端类型检查SHALL 在引入本 typed 后通过


### Requirement: SessionDetailMetrics 与 SessionDetailMetadata 字段定义 SHALL 与历史 snake_case wire 逐字段对齐

新增 typed struct `SessionDetailMetrics` SHALL 含 `message_count: usize` 单字段（**snake_case** rename，与 历史 hand-built `json!({"message_count": ...})` wire 一致）；`SessionDetailMetadata` SHALL 含 `last_modified: Option<String>` / `size: Option<u64>` / `cwd: Option<String>` 三字段（**snake_case** rename，与 历史 wire 一致，全部 nullable）。两个 struct 序列化产物 SHALL 与历史 hand-built JSON 在所有可能输入下逐字段一致——typed 化 SHALL **不**修正 camelCase IPC 契约违规（详 `design.md::D7`，留 followup issue）。

#### Scenario: SessionDetailMetrics 序列化 wire 形状

- **WHEN** 实现按 对 `SessionDetailMetrics { message_count: 42 }` 做 JSON 序列化
- **THEN** 产物 SHALL 是 `{"message_count": 42}`（snake_case，**不**是 `{"messageCount": 42}`）
- **AND** 与历史 `{"message_count": 42}` wire 形状逐字节一致

#### Scenario: SessionDetailMetadata 字段全 nullable + snake_case wire

- **WHEN** 文件系统 `metadata()` 调用失败 / jsonl 中 `cwd` 字段缺失
- **THEN** `SessionDetailMetadata { last_modified: None, size: None, cwd: None }` 序列化 SHALL 产出 `{"last_modified": null, "size": null, "cwd": null}`（snake_case，**不**是 `lastModified`）
- **AND** 与历史 `{"last_modified": null, "size": null, "cwd": null}` wire 形状逐字节一致
- **AND** 前端 SessionDetail 视图按 `detail.metadata.cwd` 消费 SHALL 与改动前行为一致（其余 `last_modified` / `size` 当前前端未消费但 wire 形状仍 SHALL 保留以兼容 HTTP transport / 未来 consumer）


### Requirement: ipc_contract 测试 SHALL 覆盖 typed 字段命名 round-trip

IPC contract test 套件 SHALL 在本 change 后含至少一个新测试（例如 `session_detail_typed_metrics_metadata_round_trip`）覆盖 `SessionDetail` typed 化后的 wire 形状：从 typed struct 出发 JSON 序列化再反序列化回 typed 反序列化回 typed，断言所有字段值不变。

#### Scenario: SessionDetail typed round-trip

- **WHEN** 测试构造 `SessionDetail { chunks: Vec::new(), metrics: SessionDetailMetrics { message_count: 0 }, metadata: SessionDetailMetadata::default(), context_injections: Vec::new(), injections_by_phase: BTreeMap::new(), phase_info: ContextPhaseInfo::default(), is_ongoing: false, title: None, session_id: "s".into(), project_id: "p".into() }`，序列化为 `Value`，再反序列化回 typed
- **THEN** 反序列化产物 SHALL 与原始 `SessionDetail` 字段逐一相等（`PartialEq`）
- **AND** 序列化产物的顶层 key 集合 SHALL 是 `{sessionId, projectId, chunks, metrics, metadata, contextInjections, injectionsByPhase, phaseInfo, isOngoing, title}`（顶层 SessionDetail 是 camelCase）
- **AND** `metrics` / `metadata` 内部字段 SHALL 仍是 snake_case（`message_count` / `last_modified` / `size` / `cwd`），与历史 hand-built wire 一致（详 `design.md::D5` + `D7`）


### Requirement: Unified invalidator 作为 `LocalDataApi.file_tx` 唯一生产者

数据查询层带 watcher 构造路径 SHALL 把 unified cache invalidator 升级为 file_tx 广播通道 的**唯一**生产者，**不**再 spawn 任何独立的 `bridge_task` 把 watcher 文件事件订阅 直接转发到 `file_tx`。invalidator 内部 sync 跑完三档判定（详 `ProjectScanCache 按事件语义分级失效` Requirement）后 SHALL 把 enriched `FileChangeEvent` 通过 `file_tx.send(enriched)` 广播给下游消费者（Tauri host emit / HTTP `spawn_file_bridge` / 其它 `subscribe_file_changes` 调用方）。

SSH 路径 SHALL 通过 FileWatcher 的远端附加接口 接入 watcher broadcast，数据查询层的远端 watcher 附加方法 SHALL NOT 再走 远端 polling watcher 直接生产到 file_tx 路径——SSH event 必须经过同一 unified invalidator enrichment gateway，与 Local event 行为一致。FileWatcher 远端附加接口签名 SHALL 接受调用方注入的 `CancelToken`（替代原内部 `CancelToken::new()`），保留 `RemoteWatcherHandle` 返回值不变；调用方持有 token clone 用于 dead-signal monitor 路径，外部 disconnect 时仍能 cancel SSH polling。

**Emit 时机契约（unified invalidator loop 顺序）**：

1. `rx.recv().await` 收 raw event
2. sync 调 `apply_file_event_to_project_scan_cache(event)` 拿判定结果（返回 `EnrichDecision { invalidated: bool, emit_session_list_changed_hint: bool }`）；该函数内部锁在 sync block 末尾自动释放
3. 构造 `enriched_event = FileChangeEvent { session_list_changed: event.session_list_changed || decision.emit_session_list_changed_hint, ..raw_event }`——OR 公式让 watcher 视角 + cache 视角并集决定字段，最大兜底
4. 调 `file_tx.send(enriched_event)` broadcast emit（**锁已释放**，emit 永不在持锁路径）
5. async 调 `apply_file_event_to_parsed_cache(event).await`（**不**阻塞 emit）

emit MUST 在 step 4 完成（即 sync invalidate 之后，async parsed invalidate 之前）。这保证：(a) 前端拿到 file-change 时 `ProjectScanCache` 状态已是事件后的最新；(b) 前端无需等磁盘 stat I/O 完成；(c) `parsed_cache` 失效路径仍走 async 不阻塞 emit。

**emit 字段 OR 公式语义**：watcher 层填的 `event.session_list_changed` 是判定**主源**（基于 watcher 跟踪集合首见性，详 `file-watching` Requirement `跟踪 session 首见性以填写 revalidation hint`）；`decision.emit_session_list_changed_hint` 是 cache 视角的辅助 hint（值 = "本 event 命中 `ProjectScanCache 按事件语义分级失效` Requirement 规则 2 的 unknown_session 判定条件"）。两源并集 OR 兜底 watcher 重启 / `reconfigure_claude_root` 等让 watcher 跟踪集合重置但 cache 仍有有效 snapshot 的窗口。

**仅 Local event 参与 OR 兜底**：cache hint OR 仅对 Local event 应用——unified invalidator SHALL 调 is_local_project 守护（基于 `local_projects_seen` 集合判定），**仅** `is_local_project=true` 的 event 才查 `apply_file_event_to_project_scan_cache` 取 hint 并参与 OR；SSH event（`is_local_project=false`）SHALL 跳过 cache 查询，`emit_session_list_changed_hint=false` 强制 emit 等于 `event.session_list_changed`。理由：local cache 的 session 存在性查询对 SSH event 永远返 false，若不守护会让 SSH 普通 append 被错误升 `session_list_changed=true`，破坏对称 + 噪声回归。SSH 路径 watcher 字段已由 SSH polling 视角对称填写（详 `file-watching` Requirement `Watch SSH remote project directory via SFTP polling`），无需 OR 兜底。

**反压**：广播通道 send 满时丢旧元素不阻塞，invalidator 自身永远不会被慢 subscriber 阻塞；slow subscriber 引发的 lag 走下游 bridge 的 `Lagged` 兜底（见 `Emit push events for file changes and notifications` Requirement 的 lag 兜底契约）。

**broadcast lag 路径 SHALL emit synthetic structural event**：`rx.recv().await` 返回 `Err(RecvError::Lagged(n))` 时除调 `apply_lag_to_project_scan_cache`（保守 `invalidate_local()` + counter `lag_conservative`）外，SHALL 显式 send 一条 synthetic `FileChangeEvent { project_id: "", session_id: "", deleted: false, project_list_changed: true, session_list_changed: true }` 到 `file_tx`。理由：本路径的 lag 在 watcher 文件事件订阅 上游 receiver 上，下游 数据查询层 file_tx 广播通道 的下游 bridge（src-tauri Tauri host emit / HTTP SSE bridge）的 `RecvError::Lagged` 兜底监听的是 `file_tx`——上游 lag 不会让下游 receiver 同步 lag，下游 bridge 的 sse-lagged 通知路径不会触发，前端连兜底 silent refresh 都收不到。synthetic event 让前端三档守护命中并触发兜底全量 revalidate。返回 `Err(RecvError::Closed)` 时 SHALL 退出 loop。

**Synthetic event 在所有下游 bridge 路径的传播契约**：synthetic event 经 数据查询层 file_tx 广播通道 broadcast 后，下游两路 bridge SHALL 按既有 forward 路径处理，**不**对 synthetic event 做特殊识别 / 过滤：

- **Tauri host bridge**（src-tauri）：SHALL `app.emit("file-change", &payload)` 转发 synthetic event 到 webview，与 real event 行为一致
- **HTTP SSE bridge**（HTTP 客户端 / 浏览器 transport 路径）：SHALL 把 synthetic event 序列化为 `PushEvent::FileChange { ... }` 推到 `/api/events` SSE stream，与 real event 形态一致

**Synthetic event 在前端消费侧的副作用守护**：所有前端 surface（Tauri webview / 浏览器 transport `?http=1`）SHALL 在收到 `payload.projectId === ""` **且** `payload.sessionId === ""` 时跳过 per-session 操作（如 `loadSessions("")` / per-session DOM patch），仅触发"项目列表 / dashboard 全量 revalidate"等顶层兜底刷新。Tauri webview 与浏览器 transport 共用同一前端 handler 链（file-change store 模块），守护实现 SHALL 集中在该 handler 链的入口处或各 surface 自身的回调内，避免跨 transport 漂移。

**MUST NOT**：

- MUST NOT 与额外的 `bridge_task` 并存——unified invalidator 是 `file_tx` 唯一生产者，避免双 producer 引发的事件顺序与重复问题
- MUST NOT 让 SSH `polling_watcher` 直接生产到 数据查询层 file_tx 广播通道 ——必须经过 `FileWatcher::attach_remote` → watcher broadcast → unified invalidator 的统一路径
- MUST NOT 在 emit 路径覆盖 `event.session_list_changed` 字段——OR 公式 SHALL 保留 watcher 已填值，cache hint 仅做 OR 提升

#### Scenario: unified invalidator 是 `file_tx` 唯一生产者

- **WHEN** 数据查询层带 watcher 构造 构造完成，启动 watcher 桥任务
- **THEN** 启动路径 SHALL NOT spawn 任何独立的 `bridge_task` 把 watcher 文件事件订阅 直接转发到 `file_tx`
- **AND** `file_tx` 的所有事件 SHALL 来自 unified invalidator 的 `file_tx.send(enriched_event)` 调用

#### Scenario: SSH 路径走 attach_remote 进入 unified invalidator

- **WHEN** 数据查询层的远端 watcher 附加方法 被调用（SSH 连接上时）
- **THEN** 实现 SHALL 调用 `FileWatcher::attach_remote(sftp, projects_dir, cancel_token)` 让 SSH polling 事件喂入 `watcher.file_tx`，且调用方 SHALL 注入自己持有的 `CancelToken`（用于 dead-signal monitor cancel 路径）
- **AND** SSH event SHALL 经过 unified invalidator 的判定（cache 无 SSH entry 时 invalidate 决策退化为只看 `project_list_changed || deleted`）
- **AND** enriched SSH event SHALL 通过 `file_tx.send` 广播给下游，与 Local event 形态一致；`session_list_changed` 字段已由 SSH polling watcher 在远端事件上填好（详 `file-watching` Requirement `Watch SSH remote project directory via SFTP polling`）
- **AND** 外部 disconnect 触发 `cancel_token.cancel()` 时 SHALL 让 SSH polling task 退出（dead-signal monitor 路径保持原行为）

#### Scenario: emit 顺序在 sync invalidate 之后、async parsed invalidate 之前

- **WHEN** unified invalidator loop 收到一条 raw `FileChangeEvent`
- **THEN** 实现 SHALL 先 sync 调 `apply_file_event_to_project_scan_cache` 拿 `EnrichDecision`
- **AND** 然后 sync 调 `file_tx.send(enriched_event)` emit（锁已释放）
- **AND** 最后 async 调 `apply_file_event_to_parsed_cache(event).await`
- **AND** `file_tx.send` MUST NOT 在 cache lock 临界区内调用

#### Scenario: emit 字段 OR 公式 watcher 主源 + cache hint 兜底

- **WHEN** unified invalidator 收到 `FileChangeEvent { project_id: "pa", session_id: "sa_new", deleted: false, project_list_changed: false, session_list_changed: true }`（watcher 已填 first-seen=true），且 `ProjectScanCache::contains_session_id(&local_ctx, "pa", "sa_new")` 返 `false` 让 `decision.emit_session_list_changed_hint = true`
- **THEN** enriched event 的 `session_list_changed` 字段 SHALL 是 `true || true == true`
- **AND** 通过 `file_tx` emit 给下游

#### Scenario: emit 字段 OR 公式 watcher 已填 false 且 cache hit 也 false

- **WHEN** unified invalidator 收到 `FileChangeEvent { project_id: "pa", session_id: "sa", deleted: false, project_list_changed: false, session_list_changed: false }`（watcher 跟踪集合已含 `(pa, sa)` → first-seen=false），且 `contains_session_id(&local_ctx, "pa", "sa")` 返 `true` 让 `decision.emit_session_list_changed_hint = false`
- **THEN** enriched event 的 `session_list_changed` 字段 SHALL 是 `false || false == false`
- **AND** 该事件 SHALL NOT 触发前端三档守护 revalidate

#### Scenario: emit 字段 OR 公式 watcher 重启窗口期 cache 兜底

- **WHEN** watcher 已重启（`reconfigure_claude_root` 触发）让跟踪集合清空，但 `ProjectScanCache` 仍持有旧 entry（含 project `pa` 与 `sa`）；用户在 `pa` 下追加已知 session `sa.jsonl`
- **AND** watcher 视为 first-seen 填 `session_list_changed=true`（lazy false-positive）
- **THEN** enriched event 的 `session_list_changed` 字段 SHALL 是 `true || (cache contains_session_id 返 true → hint=false) == true`
- **AND** 前端 revalidate 一次（false-positive，cache 视角下其实是已知 session 追加，但 watcher 视角是 first-seen，OR 取并集偏向 emit）

#### Scenario: SSH event 跳过 local cache hint OR

- **WHEN** Local cache 持有 entry（含 project `pa` 与 sessions `{sa1, sa2}`），SSH context 当前 active；远端 SSH polling watcher emit `FileChangeEvent { project_id: "pa-ssh", session_id: "sx", deleted: false, project_list_changed: false, session_list_changed: false }`（SSH 已知 session size/mtime 变化，watcher 字段填 `false`）
- **AND** unified invalidator 调 `FileWatcher::is_local_project("pa-ssh")` 返 `false`（SSH 远端 project_id 不在 local watcher `local_projects_seen` 集合内——SSH 事件由远端 polling 直接构造，不经过 `parse_project_event`，不会被 `mark_local_origin` 写入）
- **THEN** unified invalidator SHALL 跳过 `apply_file_event_to_project_scan_cache` 调用 / 跳过 cache hint 查询，强制 `decision.emit_session_list_changed_hint=false` + `decision.invalidated=false`
- **AND** enriched event 的 `session_list_changed` 字段 SHALL 等于 `event.session_list_changed` 即 `false`
- **AND** 该 SSH append 事件 SHALL NOT 触发前端三档守护 revalidate，保留 append 降噪收益

#### Scenario: Local event 仍应用 cache hint OR

- **WHEN** Local cache 持有 entry（含 project `pa` 但不含 `sa_new`），watcher `local_projects_seen` 集合含 `pa`（`parse_project_event` 已在前一次该 project 下事件 emit 前通过 `mark_local_origin` 写入）；用户新建 `<projects_root>/pa/sa_new.jsonl`，watcher emit `FileChangeEvent { project_id: "pa", session_id: "sa_new", session_list_changed: true }`
- **AND** unified invalidator 调 `FileWatcher::is_local_project("pa")` 返 `true`
- **THEN** unified invalidator SHALL 调 `apply_file_event_to_project_scan_cache` 拿 `EnrichDecision { invalidated: true, emit_session_list_changed_hint: true }`
- **AND** enriched event 的 `session_list_changed` 字段 SHALL 是 `true || true == true`

#### Scenario: lag 路径 SHALL emit synthetic structural event

- **WHEN** unified invalidator 的 `rx.recv().await` 返回 `Err(RecvError::Lagged(n))`
- **THEN** 实现 SHALL 调 `apply_lag_to_project_scan_cache`（保守 `invalidate_local()` + counter `lag_conservative`）
- **AND** SHALL 显式 send 一条 synthetic `FileChangeEvent { project_id: "", session_id: "", deleted: false, project_list_changed: true, session_list_changed: true }` 到 `file_tx`

#### Scenario: synthetic event 经 Tauri host bridge 转发到 webview

- **WHEN** synthetic event 进入 `file_tx` broadcast 后被 Tauri host bridge 接收
- **THEN** bridge SHALL `app.emit("file-change", &payload)` 把 synthetic event 转发给 webview，与 real event 行为一致
- **AND** SHALL NOT 对 synthetic event 做特殊识别 / 过滤
- **AND** webview 前端 handler 收到该 payload 后 SHALL 触发兜底全量 revalidate
- **AND** webview 前端 SHALL 按 `payload.projectId === "" && payload.sessionId === ""` 守护跳过 per-session 操作（`loadSessions("")` / per-session DOM patch），不引发副作用

#### Scenario: synthetic event 经 HTTP SSE bridge 推到浏览器客户端

- **WHEN** synthetic event 进入 `file_tx` broadcast 后被 HTTP SSE bridge 接收
- **THEN** bridge SHALL 把 synthetic event 序列化为 `PushEvent::FileChange` 推到 `/api/events` SSE stream，与 real event 形态一致
- **AND** SHALL NOT 对 synthetic event 做特殊识别 / 过滤
- **AND** 浏览器 transport 收到该 SSE 消息后 SHALL 走与 webview 同一 file-change handler 链
- **AND** 浏览器 transport 路径前端 SHALL 按 `payload.projectId === "" && payload.sessionId === ""` 同款守护跳过 per-session 操作，仅触发顶层 revalidate


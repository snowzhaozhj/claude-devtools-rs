# ssh-remote-context Specification

## Purpose

定义"上下文"抽象（本地 / SSH 远程）以及 SSH 连接的建立、状态查询与拆除规则，使下游 capability（`project-discovery`、`session-parsing`、`session-search`）能够以统一接口同时消费本地和远端的 Claude 会话数据。
## Requirements
### Requirement: Manage local and SSH contexts

系统 SHALL 暴露"上下文"概念，表示会话数据的来源，分两类：`local`（宿主机文件系统）与 `ssh`（远程主机）。系统 SHALL 提供列出上下文、切换当前上下文、查询当前激活上下文的能力。同一时刻 SHALL 仅有一个上下文处于 `active` 状态；连接新 SSH host 时 SHALL 先断开当前 active SSH context（若存在）再切换到新 host。`Local` 上下文 SHALL 始终在 registry 中存在且不可销毁；`Ssh<host>` 上下文 SHALL 在 `ssh_disconnect` 后从 registry 移除。

#### Scenario: Default local context

- **WHEN** 应用启动且无既有 SSH 状态
- **THEN** 当前上下文 SHALL 为 `Local`，绑定本地文件系统 provider

#### Scenario: Switch to SSH context

- **WHEN** 调用方请求切换到一个已建立的 SSH 上下文
- **THEN** 后续 session discovery 与读取 SHALL 走 SSH 文件系统 provider
- **AND** registry SHALL emit 一条 `context_changed` 事件 `{ activeContextId, kind: "ssh" }`

#### Scenario: Connecting new host while another SSH context is active

- **WHEN** active context 是 `ssh-host-A`，调用方请求 `ssh_connect` 到 `host-B`
- **THEN** 系统 SHALL 先调 `disconnect(host-A)`，等其状态切到 `disconnected`
- **AND** 再发起 `host-B` 连接握手，成功后切 active context 为 `ssh-host-B`
- **AND** registry SHALL emit 两条事件：`context_changed { activeContextId: "ssh-host-B" }` 与 `ssh_status { contextId: "ssh-host-A", status: "disconnected" }`

#### Scenario: Local context is indestructible

- **WHEN** 调用方尝试从 registry 移除 `Local` context
- **THEN** 操作 SHALL 被拒绝并返回结构化错误 `code: invalid_operation`
- **AND** registry SHALL 仍保留 `Local` context

### Requirement: Establish and tear down SSH connections

系统 SHALL 通过 SSH 连接到远程主机，连接时 SHALL 在 `~/.ssh/config` 存在的情况下读取主机元数据；SHALL 支持显式断开与应用退出时的优雅断开。连接 SHALL 走 `russh` + `russh-keys` 真协议栈（非占位实现），完成 TCP probe（5s 超时）→ SSH transport 握手 → 鉴权候选链尝试（Requirement: SSH authentication candidate chain）→ SFTP subsystem open（8s 超时）→ remote home probe 五个阶段；总外层硬超时 SHALL 为 25s。任一阶段失败 SHALL 返回结构化 `SshError`（Requirement: Structured SSH error classification）。

#### Scenario: Connect by host alias from ssh config

- **WHEN** 调用方请求连接到 `~/.ssh/config` 中已定义的 alias
- **THEN** 系统 SHALL 先调 `ssh -G <alias>` 子进程解析得到 hostname / user / port / IdentityFile / IdentityAgent
- **AND** 用解析结果建立 `russh::client::connect` TCP + transport
- **AND** 按鉴权候选链尝试到第一个成功源
- **AND** 连接 SHALL 被登记为新的 `Ssh<host>` context，状态切到 `connected`

#### Scenario: Test connection without persisting

- **WHEN** 调用方请求测试连通性（`ssh_test_connection`）
- **THEN** 系统 SHALL 走与 `ssh_connect` 相同的握手流程
- **AND** 成功后 SHALL 立即关闭 SSH session，不向 registry 注册新 context
- **AND** 返回值 SHALL 包含 `authChain[]` 让 UI 可显示"试过哪些候选源"诊断

#### Scenario: Disconnect

- **WHEN** 调用方断开一个已激活的 SSH 上下文
- **THEN** 系统 SHALL 关闭 SFTP channel + SSH transport + TCP socket
- **AND** 该 context 的 polling watcher（若已启动）SHALL 被停止
- **AND** 后续从该上下文的读取 SHALL 以 `code: not_connected` 错误失败
- **AND** 若被断开的是 active context，registry SHALL 自动把 active 切回 `Local`

#### Scenario: Graceful disconnect on app exit

- **WHEN** 应用收到关闭信号（Tauri `WindowEvent::CloseRequested`）
- **AND** 当前有 N 个已注册 SSH context（N >= 1）
- **THEN** 系统 SHALL 对每个 SSH context 并发 `disconnect`，最长等待 3s
- **AND** 应用 SHALL NOT 被某个 context 的 disconnect 阻塞超过该上限

### Requirement: Read sessions and files over SSH with same contract

系统 SHALL 在 SSH 上下文上提供与 local 上下文等价的 `project-discovery`、`session-parsing`、文件读取能力，使下游消费者观察到完全相同的数据形状。`SshFileSystemProvider` SHALL 实现 `cdt-fs::FileSystemProvider` trait 的所有方法（`exists` / `read_to_string` / `read_dir` / `read_dir_with_metadata` / `stat` / `stat_many` / `read_lines_head` / `open_read`），底层走 `russh-sftp` 或等价 SFTP 客户端 API；SHALL NOT 在远端 spawn 任何工作进程，唯一允许在远端执行的命令是 `printf %s "$HOME"` 用于探测 remote home。

`open_read` SHALL 替代旧的 inherent 方法 `open_read_stream`——返回 `Box<dyn AsyncRead + Send + Unpin>` 让调用方不需 downcast 到 `SshFileSystemProvider` 就能流式读。`stat_many` SHALL 实现为 trait default（`futures::future::join_all` 包装 `stat`）；由于底层 `Arc<Mutex<SftpSession>>` 全锁串行，当前 SSH `stat_many` 仍是 N 次串行 RTT（**已知限制**），真正的 SFTP message-id 并发 pipeline 留独立 PR（PR-F，方案 C 路径——保持"无远端 shell 依赖"架构假设）解决。trait API 先就位让 caller 一律调 `stat_many` 而非循环 `stat`。

**`read_dir_with_metadata` SSH override**（change `ssh-batch-readdir-with-metadata` 引入）：`SshFileSystemProvider` SHALL override `read_dir_with_metadata` 直接 delegate 到 `self.read_dir(path)`——底层 SFTP `SSH_FXP_READDIR` reply 1 个 RTT 返完整 dir 内容 + 每个 file entry 的 attrs（size/mtime）；缺 mtime 的 entry SHALL 在 `DirEntry.metadata = None` 状态返给 caller，由上层 batch 校验语义视同 cache mismatch 走 cache wrapper miss 路径补齐——实现 SHALL NOT 在 trait 实现层做 per-entry stat fallback（避免 N+1 RTT 退化）。

**SSH list 路径性能契约**：朴素 per-session 串行 `fs.stat` 验 cache signature 在 SSH 上 `Arc<Mutex<SftpSession>>` 全锁串行 = 50 sessions × 50ms = 2.5s wall，超 sidebar 首屏 < 500ms 预算 5×（详 ipc-data-api change `unify-fs-direct-calls` design D3 + codex 二审 Blocking #1）。本 capability SHALL 让 SSH list 路径走以下三件套：

- **G. cache hit trust**（PR-D 落地）：用户切回已访问过的 SSH host → UI 立刻拿 in-memory cache 内容渲染列表（**0 fs op via `MetadataCache::lookup_trust_cached`**），不等任何 fs.stat RTT
- **D. SkeletonThenStream**（PR-D 落地）：list_sessions SSH 路径与 Local 路径同走 `page_jobs` spawn 模型；首屏返骨架 + cache trust 内容，metadata diff 通过 SSE event 异步推送（取代 PR-A D6 标"PR-E 上移"的 SSH FullEager line 855/1515/1524/1574——PR-D 提前实施算法层 SSH 同入口；PR-E 后续把字段值塞 BackendPolicy struct）。后台 scan 通过 `scan_metadata_for_page` 分流：Local 走 per-session via fs trait；**SSH 走 `scan_metadata_for_page_batched`**（本 change 引入，详 E 段）
- **E. read_dir_with_metadata batch**（本 change `ssh-batch-readdir-with-metadata` 落地）：后台 batch 校验 task SHALL 走 `fs.read_dir_with_metadata(project_dir)` per project（SFTP READDIR reply 含 entry attrs，1 RTT 拿全 dir metadata），对 page_jobs 每条 session SHALL 调 `MetadataCache::lookup_with_known_signature(&ctx, path, &sig)` 直接命中跳 stat；mismatch / 新增 / dir read 失败 SHALL 走原 cache miss 路径（`extract_session_metadata_cached` 内部 stat + scanner）→ 命中条与 mismatch 条都通过 `session_metadata_update` SSE event 推差量

**SSH 大会话 scanner buffer 上限**（PR-D 落地）：scanner（`extract_session_metadata_with_ongoing` / `parse_file_via_fs`）SHALL 通过 `FileSystemProvider::open_read` 拿 `Box<dyn AsyncRead + Send + Unpin>`，再用 `BufReader::with_capacity(SCANNER_BUF_BYTES /* 32 KiB */, reader)` 包装。Buffer 容量钉死 **32 KiB** 与 SFTP packet 上限对齐——`SSH_FXP_READ` reply 单消息上限 32 KiB，64 KiB BufReader 强制每次 fill 跑 2 次底层 SFTP READ 无收益反而多一层 alloc；32 KiB 单 BufReader fill = 单 SFTP READ message。

#### Scenario: List projects on a remote host

- **WHEN** 当前上下文是 SSH，调用方请求项目列表
- **THEN** 返回结果 SHALL 与本地项目列表形状一致，数据源为远程 `<remote_home>/.claude/projects/` 目录

#### Scenario: Read a remote session

- **WHEN** 当前上下文是 SSH，调用方请求会话详情
- **THEN** 系统 SHALL 通过 `FileSystemProvider::open_read` 流式读取远程 JSONL 文件
- **AND** 返回与本地输出形状一致的 chunk 序列

#### Scenario: open_read 是 trait 方法不再是 inherent

- **WHEN** caller 持 `&dyn FileSystemProvider` 句柄指向 `SshFileSystemProvider`
- **THEN** caller SHALL 能直接调 `fs.open_read(path).await?` 拿到 `Box<dyn AsyncRead + Send + Unpin>`
- **AND** SHALL NOT 需要 downcast 到具体 `SshFileSystemProvider` 类型才能流式读

#### Scenario: stat_many 当前是 SSH 已知假 batch

- **WHEN** caller 在 SSH 模式下调 `fs.stat_many(&[p1, p2, ..., p50])`
- **THEN** 实现 SHALL 使用 trait default `join_all`，返回 `Vec<Result<FsMetadata, FsError>>` 顺序对应
- **AND** 由于 SFTP session 全锁，实际执行是 50 次串行 RTT —— 此限制属已知，留 PR-F 解决（方案 C SFTP message-id pipeline）；trait 契约层面 caller SHALL 一律调 `stat_many` 而非循环 `stat`

#### Scenario: Resolve remote home with multiple fallbacks

- **WHEN** 远端 `<home>/.claude/projects` 不存在，但 `/home/<user>/.claude/projects` 或 `/Users/<user>/.claude/projects` 或 `/root/.claude/projects` 存在
- **THEN** 系统 SHALL 按上述顺序探测候选路径并使用第一个存在的
- **AND** 全部不存在时 SHALL 返回 `SshError::RemoteHomeMissing { tried }` 错误，状态切到 `error`，不切换 active context，但 `ssh_get_state` SHALL 保留该 context 的错误状态与已完成的 `authChain` 诊断

#### Scenario: SFTP transient errors are retried

- **WHEN** SFTP 调用返回瞬时错误码（`code=4` / `EAGAIN` / `ECONNRESET` / `ETIMEDOUT` / `EPIPE`）
- **THEN** 系统 SHALL 重试最多 3 次，每次间隔指数退避（75ms × attempt）
- **AND** 仍失败时 SHALL 把错误向上抛给调用方，封装为 `FsError::TransientExhausted { attempts: 3, last_reason }`

#### Scenario: SSH list 路径 hot path cache hit trust（用户感知卡顿消失）

- **WHEN** 用户在 SSH active context 下调 `list_sessions`，cache 中持有该 ContextId 的 entry
- **THEN** UI 渲染路径 SHALL 走 `MetadataCache::lookup_trust_cached(&ctx, path)` 命中返 cache 内容（**0 fs op**：无 `fs.open_read` / `fs.read_to_string` / `fs.stat`）
- **AND** UI 立刻拿 in-memory cache 内容渲染列表（与 Local SkeletonThenStream 路径同入口）
- **AND** 后台 spawn `scan_metadata_for_page_dispatch` task per project 异步校验 cache freshness——SSH backend_kind 路径 SHALL 走 `scan_metadata_for_page_batched`（先 `fs.read_dir_with_metadata(project_dir)` 1 RTT 拿全 dir metadata，对 page_jobs 每条调 `MetadataCache::lookup_with_known_signature` 命中跳 stat）
- **AND** 外部进程改动 → mismatch → 通过 `session_metadata_update` SSE event 推差量给 UI

#### Scenario: SSH list 路径冷启动走 SkeletonThenStream + page_jobs

- **WHEN** 用户首次连 SSH host A 调 `list_sessions`，cache 中无该 ContextId 的 entry
- **THEN** UI 首屏 SHALL 返 SessionSummary 骨架（title=None / message_count=0），不阻塞等待 metadata
- **AND** 入 `page_jobs` 后 spawn `scan_metadata_for_page_dispatch` task；SSH backend 走 batched 路径——首先 `fs.read_dir_with_metadata(project_dir)` 1 RTT 拿全 dir entries metadata，对 page_jobs 每条 lookup `by_name: HashMap<PathBuf, FsMetadata>`；冷启动场景 cache 全 miss → 全部 mismatch → 走 cache wrapper miss 路径 spawn 单条 task（`extract_session_metadata_cached` 内部 `fs.stat` + cache miss 调 `parse_file_via_fs` 走 `fs.open_read`）异步刷新 metadata
- **AND** 每条 metadata 通过 `session_metadata_update` SSE event 推给 UI 增量填充

#### Scenario: SSH ctx 后台 batch 校验 fs op 形态钉死（all-hit）

- **WHEN** SSH context 下 `scan_metadata_for_page_batched` 被 spawn，page_jobs 含 N 条 session（全部 cache hit byte-equal，且 dir metadata 含每条对应 path）
- **THEN** fs op 调用 SHALL 仅含 1 次 `fs.read_dir_with_metadata(project_dir)`（拿全 dir entry metadata）
- **AND** `fs.stat` 调用次数 SHALL = 0；`fs.open_read` 调用次数 SHALL = 0；`fs.read_to_string` 调用次数 SHALL = 0
- **AND** 命中条 SHALL 通过 `session_metadata_update` SSE event 推 cache 现值（`is_ongoing = entry.messages_ongoing`，SSH 跳 stale check 与 `extract_session_metadata_cached` SSH 分支语义一致）
- **AND** 总 fs op = 1，对比 PR-D 既有 per-session 路径 `N stat`（cache hit 路径不 open_read，只 stat 拿 signature）省 N - 1 RTT，典型 50 sessions 50 → 1 RTT

#### Scenario: SSH ctx 后台 batch 校验 fs op 形态（partial-hit）

- **WHEN** page_jobs 含 H 条 cache hit + M 条 cache mismatch / 新增 / dir metadata 缺该 path（H + M = N，其中 mismatch 包含 `DirEntry.metadata = None` 的 mtime_missing 条）
- **THEN** H 条命中条 SHALL 通过 `MetadataCache::lookup_with_known_signature` 命中直 broadcast，不调任何 fs op
- **AND** M 条 mismatch 条 SHALL spawn 单 sub-task 走 `extract_session_metadata_cached` 既有 cache wrapper miss 路径（per-task 1 `fs.stat` + 1 `fs.open_read`），共 M × 2 fs op
- **AND** 总 fs op SHALL = 1（batch read_dir_with_metadata）+ 2M
- **AND** 对比 PR-D 既有 per-session 路径 `N stat + M open_read = N + M`：本路径 1 + 2M，差额 = (1 + 2M) - (N + M) = M - N + 1 = M - H + 1，H > M + 1 时 batch 更优；典型 hit rate 80%（H = 40, M = 10）下 batch 21 ops vs 既有 50 ops 显著节省

#### Scenario: SSH ctx 后台 batch 校验 fs op 形态（all-miss）

- **WHEN** page_jobs 含 N 条全 mismatch（典型冷启动场景：cache 全空）
- **THEN** 总 fs op SHALL = 1（batch read_dir_with_metadata）+ 2N（per-mismatch sub-task）= 1 + 2N
- **AND** 对比 PR-D 既有 per-session 路径 `N stat + N open_read = 2N`：本路径多 1 RTT（read_dir_with_metadata 的 50ms RTT）
- **AND** 此一次 read_dir RTT 是接受 trade-off：换取 partial-hit / all-hit 场景的 N - 1 RTT 节省；冷启动场景用户感知仍走 SkeletonThenStream（hot path 用骨架渲染 + 后台异步刷），1 RTT 的额外 batch 开销在后台不影响首屏

#### Scenario: SSH ctx batch helper 在 dir read 失败时 fallback 到 per-session 路径

- **WHEN** SSH context 下 `scan_metadata_for_page_batched` 调 `fs.read_dir_with_metadata(project_dir)` 返 `Err`（瞬时网络抖动 / dir 被删等）
- **THEN** 函数 SHALL 走 fallback 路径调既有 `scan_metadata_for_page`（per-session via fs trait）继续异步刷新——保证功能正确性，性能退化为 PR-D 既有形态
- **AND** SHALL 通过 `tracing::warn!(target: "cdt_api::perf", ...)` 让运维侧可见

#### Scenario: SSH 同 session 二次 get_tool_output cache hit byte-equal

- **WHEN** 在 SSH context 下首次调 `get_tool_output(root, sid, tu_a)` 完成 cache 写入；session 文件未变后调 `get_tool_output(root, sid, tu_b)`（同 session，不同 tool_use_id）
- **THEN** 第二次调用 SHALL 走 `extract_parsed_messages_cached` 内部 `fs.stat(path)` 拿当前 signature + cache lookup；signature byte-equal 直接 `Arc::clone` 复用 cache `Arc<Vec<ParsedMessage>>`，**SHALL NOT 触发 `parse_file_via_fs` 重 parse**（即 `fs.open_read = 0`）
- **AND** 形态：`fs.stat = 1`、`fs.open_read = 0`、`fs.read_to_string = 0`、`cdt_parse::parse_file_via_fs` 调用次数 = 0
- **AND** Note：纯 0 fs op `ParsedMessageCache::lookup_trust_cached` + 后台 stat 校验的设计与 batch readdir 解耦，留独立 PR（如 PR-E 或后续 PR-D2a）wire 入 get_tool_output / get_image_asset

#### Scenario: SSH 远端 jsonl 真改动后 cache invalidate 走 page_jobs 校验

- **WHEN** 用户在 SSH context 下访问 session A 写入 cache；外部进程（`ssh remote-host > append.jsonl`）追加该 jsonl 内容；用户再次访问 list_sessions
- **THEN** UI 立刻拿旧 cache 内容渲染（hot path 0 fs op，via `lookup_trust_cached`）
- **AND** 同时 spawn `scan_metadata_for_page_dispatch` task per project_dir → SSH backend 走 batched 路径，`fs.read_dir_with_metadata` 拿到新 metadata（mtime/size 已变）；`MetadataCache::lookup_with_known_signature` 比对 mismatch → spawn 单 task 走 cache wrapper miss 路径（`fs.stat` 拿新 signature → `parse_file_via_fs` 走 `fs.open_read` 重 parse）
- **AND** 每条改动通过 `session_metadata_update` SSE event 推 metadata diff 给 UI 增量更新（用户感知"列表先出但 1-2 秒后内容自动 refresh"）

#### Scenario: SSH disconnect 中间态 user-facing IPC 返 not_found 而非降级 Local

- **WHEN** 用户在 SSH context A active 时调用 `get_session_detail(sid)` / `get_tool_output(...)` / `get_image_asset(...)` 等 user-facing IPC handler；调用过程中并发触发 `ssh_disconnect("A")` 让 active context 进入"None active 但旧 SSH provider 仍在 sessions HashMap"中间态
- **THEN** handler 内部 SHALL 通过 `active_fs_and_context_strict()` 拿三元组同快照
- **AND** 该 helper SHALL 返 `Err(ApiError::not_found)` 而非降级到 Local provider（避免 user 在 SSH 视角下意外拿到 Local 同 sid 的内容）
- **AND** 用户后续 reconnect 同 host A，cache 中先前写入的 entry SHALL 复用（与 PR-A spec §"ssh_disconnect 不清 cache" 一致）

#### Scenario: SSH 后台 batch 校验 task 在 ssh_disconnect 时 abort

- **WHEN** 用户在 SSH context A 下调 `list_sessions` spawn 多个后台 `scan_metadata_for_page_dispatch` task（per project_dir，SSH 路径走 `scan_metadata_for_page_batched`）；调用过程中触发 `ssh_disconnect("A")`
- **THEN** 所有该 ssh ctx 下的顶层 batch task SHALL 通过 `LocalDataApi::active_scans` per-key abort handle 被 abort（既有 `abort_scans_for_context` 路径覆盖 backend_kind=Ssh 的所有 entry）
- **AND** 顶层 batch task 内 `JoinSet` 持有的 mismatch sub-task SHALL 随 JoinSet drop 自动 abort（tokio JoinSet 语义），SHALL NOT 再向 `session_metadata_tx` broadcast 旧 ctx update
- **AND** 后续用户切回该 host reconnect，新 batch task 用新快照启动；旧 task 的部分写入 cache（不同 ContextId）SHALL NOT 串扰新 task

#### Scenario: SSH 大会话 scanner BufReader 容量与 SFTP packet 对齐

- **WHEN** SSH context 下 cache miss 后调 `extract_session_metadata_with_ongoing(fs, path)` 或 `parse_file_via_fs(fs, path)` 扫描 5 MB jsonl 文件
- **THEN** 函数体 SHALL 用 `BufReader::with_capacity(32 * 1024, reader)` 包装 `fs.open_read` 返回的 `Box<dyn AsyncRead + Send + Unpin>`
- **AND** 单 BufReader fill = 单 SFTP `SSH_FXP_READ` message（packet 上限 32 KiB）
- **AND** SHALL NOT 出现 64 KiB 或更大的 BufReader（强制底层拆 2× SFTP READ 无收益）
- **AND** SHALL NOT 出现默认 8 KiB BufReader（5 MB jsonl 需 ~640 RTTs 在 SSH 上不可接受）

#### Scenario: SSH cache miss 路径 fs op 形态钉死

- **WHEN** cache miss 触发 SSH 端单 file scan
- **THEN** fs op 调用 SHALL 仅含：1 次 `fs.stat`（前置 signature 拿，由 cache wrapper 自动处理）+ 1 次 `fs.open_read` 拿 reader + N 次 `BufReader::poll_read`（内部分摊到底层 SFTP read，不计入 fs trait 公开 op）
- **AND** SHALL NOT 出现 `fs.read_to_string` 全文调用（该路径会绕过流式状态机，把 5 MB 全装入内存 + alloc 一次大 String）

### Requirement: Report SSH connection status

系统 SHALL 暴露每个已配置 SSH 上下文的连接状态（`disconnected` / `connecting` / `connected` / `error`），错误状态 SHALL 附带可读的错误说明与结构化错误分类。状态 SHALL 通过 `broadcast::Sender<SshStatusChange>` 推送给订阅者（HTTP SSE / Tauri emit 桥），订阅者多次订阅 SHALL 各自独立收到事件。`connecting` 状态 SHALL 携带 `authChain` 进度（已尝试源列表，便于 UI 显示"正在尝试 IdentityFile..."）。

#### Scenario: Query status of a failed context

- **WHEN** 某个 SSH 上下文连接失败
- **THEN** 状态查询 SHALL 返回 `error` 与底层错误信息（`SshError` 序列化结果）
- **AND** 错误信息 SHALL 含 `authChain[]`（每个候选源的 source/outcome/elapsed_ms）

#### Scenario: Status broadcast to multiple subscribers

- **WHEN** 一个 SSH 连接状态从 `connecting` 切到 `connected` 且当前有两个订阅者
- **THEN** 两个订阅者 SHALL 各自**恰好**收到一次 `SshStatusChange { contextId, status: "connected" }`
- **AND** 任一订阅者的滞后 SHALL NOT 影响另一订阅者投递

#### Scenario: Connecting state carries auth chain progress

- **WHEN** 系统正在尝试鉴权候选链的第 3 个候选（IdentityFile）
- **THEN** `ssh_get_state` SHALL 返回 `status: "connecting"` 与 `authChain` 含前 2 个候选的 outcome（已 Skipped / Failed）

### Requirement: SSH authentication candidate chain

系统 SHALL 在 SSH 握手鉴权阶段按以下顺序构建候选源并依次尝试：(1) ssh config `IdentityAgent` 字段（来自 `ssh -G` 解析结果，仅当字段非空且非 `none` 时启用）—— 把字段值视作 unix socket 路径直接连接，**优先于** `SSH_AUTH_SOCK` env 与 IdentityFile 文件直读，与 OpenSSH 行为对齐；(2) `SSH_AUTH_SOCK` 环境变量指向的 unix socket；(3) macOS 平台 `launchctl getenv SSH_AUTH_SOCK` 返回的 socket 路径；(4) 1Password well-known socket，依次尝试 `~/Library/Group Containers/2BUA8C4S2C.com.1password/t/agent.sock` 与 `~/.1password/agent.sock`（仅当候选 (1) 没有显式给出 1Password socket 路径时作为兜底，避免重复尝试同一 agent）；(5) 来自 `ssh -G` 解析的 `IdentityFile` 候选私钥（按列出顺序）；(6) 默认私钥位置 fallback：`~/.ssh/id_ed25519` → `id_rsa` → `id_ecdsa`；(7) 仅当用户在 UI 选择 `password` auth method 时尝试 password 鉴权。每个候选 SHALL 在结果中记录为 `AuthAttempt { source, outcome, elapsedMs }`（camelCase 序列化）。任一候选成功 SHALL 立即停止尝试后续候选；全部失败 SHALL 返回 `SshError::AuthExhausted { attempts }`。系统 SHALL NOT 在 v1 中尝试 Linux gnome-keyring agent / Windows named pipe agent / 加密私钥 passphrase 弹窗——这三类 SHALL 在 v1 中明确标记为不支持。

#### Scenario: IdentityAgent field in ssh config takes precedence

- **WHEN** 用户 `~/.ssh/config` 含 `IdentityAgent ~/Library/Group Containers/2BUA8C4S2C.com.1password/t/agent.sock`
- **AND** 进程环境同时有 `SSH_AUTH_SOCK=/tmp/standard-agent.sock`
- **THEN** 鉴权候选链 SHALL 把 `IdentityAgent` 字段对应的 1Password socket 作为候选 (1) 优先尝试
- **AND** 仅当候选 (1) 失败时才会尝试候选 (2)（env agent）

#### Scenario: macOS Launchpad-launched app uses launchctl SSH_AUTH_SOCK

- **WHEN** 应用从 macOS Launchpad / Dock 启动，进程环境变量无 `SSH_AUTH_SOCK`
- **AND** ssh config 也未指定 `IdentityAgent`
- **AND** `launchctl getenv SSH_AUTH_SOCK` 返回 `/private/tmp/com.apple.launchd.xxx/Listeners`
- **THEN** 鉴权候选链 SHALL 把该路径作为候选 (3) 并尝试连接
- **AND** 即使候选 (1)(2) 失败，候选 (3) 成功也 SHALL 让连接进入 `connected` 状态

#### Scenario: 1Password agent socket discovery

- **WHEN** 用户使用 1Password 管理 SSH 密钥
- **AND** `~/Library/Group Containers/2BUA8C4S2C.com.1password/t/agent.sock` 文件存在
- **THEN** 鉴权候选链 SHALL 把该 socket 作为候选 (3) 尝试

#### Scenario: IdentityFile fallback chain when agent unavailable

- **WHEN** 候选 (1)(2)(3) 全部失败（agent 不可用）
- **AND** `ssh -G <host>` 输出含 `identityfile ~/.ssh/work_key` 与 `identityfile ~/.ssh/personal_key`
- **THEN** 候选链 SHALL 依次尝试 `~/.ssh/work_key` 和 `~/.ssh/personal_key`
- **AND** 每个文件 SHALL 调 `russh-keys::decode_secret_key(content, None)`；返回 passphrase-required 时 SHALL 跳过并记录 `AuthOutcome::Skipped("requires passphrase, use ssh-add")`

#### Scenario: All candidates exhausted

- **WHEN** 所有 7 个候选都失败或被跳过
- **THEN** 系统 SHALL 返回 `SshError::AuthExhausted { attempts }` 含每个候选的详细 outcome
- **AND** UI SHALL 能从 `attempts[]` 渲染"7 个候选都失败：xxx"诊断

#### Scenario: AuthAttempt serialization shape

- **WHEN** `AuthExhausted { attempts }` 通过 IPC 跨边界序列化为 JSON
- **THEN** 每条 `AuthAttempt` SHALL 序列化为 `{ "source": { "type": "<variant>", "data"?: ... }, "outcome": { "type": "<variant>", "data"?: ... }, "elapsedMs": <u64> }` 形态
- **AND** `AuthSource` enum 序列化样例：`{ "type": "identityAgent", "data": "/path/to/agent.sock" }` / `{ "type": "envAgent" }` / `{ "type": "launchctlAgent" }` / `{ "type": "onePasswordAgent", "data": "/path/to/socket" }` / `{ "type": "identityFile", "data": "/Users/alice/.ssh/work_key" }` / `{ "type": "defaultKey", "data": "/Users/alice/.ssh/id_ed25519" }` / `{ "type": "password" }`
- **AND** `AuthOutcome` enum 序列化样例：`{ "type": "success" }` / `{ "type": "failure", "data": "Permission denied" }` / `{ "type": "skipped", "data": "requires passphrase, use ssh-add" }`
- **AND** 字段名 SHALL 是 camelCase（`elapsedMs`，**非** `elapsed_ms`）

#### Scenario: Encrypted private key without agent is skipped not crashed

- **WHEN** 候选 (4) 中某个 IdentityFile 是 passphrase 加密私钥
- **AND** 该候选的 source 不是 agent（直接读文件路径）
- **THEN** 系统 SHALL 跳过该候选并记录 `Skipped("requires passphrase, use ssh-add")`
- **AND** 继续尝试下一个候选，SHALL NOT 弹出 passphrase UI

#### Scenario: Windows v1 limited auth modes

- **WHEN** 当前平台是 Windows
- **THEN** 鉴权候选链 SHALL 跳过候选 (3)（macOS launchctl）和 (4)（1Password 路径）
- **AND** v1 SHALL NOT 尝试 named pipe ssh-agent（`\\.\pipe\openssh-ssh-agent`），即使该 pipe 可用
- **AND** 候选 (1)(2)(5)(6)(7) 仍正常工作（IdentityAgent / env agent / IdentityFile / 默认密钥 / password）

### Requirement: Resolve SSH host alias via `ssh -G`

系统 SHALL 通过 `tokio::process::Command` spawn 系统 `ssh -G <host>` 子进程解析 SSH config 高级特性（`Include` / `Match` / `ProxyJump` / `IdentityAgent` 等）。子进程 SHALL 设置 5s 超时；超时或非零 exit 时 SHALL 降级到 `cdt-ssh::config_parser` 的基本字段解析（仅支持 `Host` / `HostName` / `Port` / `User` / `IdentityFile`）。`SshConfigParser` SHALL 仅承担"列出所有 Host alias"用于 UI combobox 联想，不复刻 SSH config 复杂语法。

`ssh -G` 解析输出 SHALL 提取以下字段并填入 `ResolvedHost`：`hostname` / `port` / `user` / `identityfile`（多个）/ `identityagent` / `proxyjump` / `proxycommand` / `hostkeyalias`。其中 `proxyjump` / `proxycommand` / `hostkeyalias` 是为 `cdt-fs::HostSignature::config_digest` 计算服务的——这三个字段直接影响"是否同一远端机器"判定，cache 不得跨这些差异复用。

退化路径（`config_parser` 兜底）SHALL 把 `proxyjump` / `proxycommand` / `hostkeyalias` 设为 `None`，但**不**阻塞 `HostSignature` 计算——`config_digest` 仍可基于 `hostname` / `port` / `user` / `identityfile` 计算（degraded 模式下 cache 范围略宽，但不会跨 host 串扰）。

#### Scenario: Resolve alias via system ssh -G

- **WHEN** 调用方请求 `ssh_resolve_host("myserver")`
- **AND** 系统有 `ssh` 二进制
- **THEN** 系统 SHALL spawn `ssh -G myserver`，从 stdout 解析得到 hostname / port / user / identityfile / identityagent / **proxyjump / proxycommand / hostkeyalias** 等字段
- **AND** 返回 `ResolvedHost` 含以上**所有**字段（缺失字段为 `None` / 空 Vec）

#### Scenario: Fallback when ssh binary missing or fails

- **WHEN** 系统无 `ssh` 二进制（如 Windows 未启用 OpenSSH client）
- **OR** `ssh -G` 5s 超时 / 非零 exit
- **THEN** 系统 SHALL 降级到 `cdt-ssh::config_parser` 的基本字段解析
- **AND** 返回结果 SHALL 标记 `degraded: true`（UI 可据此显示"高级 SSH config 特性不可用"提示）
- **AND** `proxyjump` / `proxycommand` / `hostkeyalias` SHALL 为 `None`（degraded 模式不解析这些字段）

#### Scenario: HostSignature 在 degraded 模式仍可计算

- **WHEN** `ssh -G` 失败，`ResolvedHost.degraded == true`
- **AND** 调用方通过 `SshConfigDigestInput::from(&resolved_host)` 计算 `HostSignature`
- **THEN** SHALL 成功产 `config_digest`，输入字段中 `proxyjump` / `proxycommand` / `hostkeyalias` 为 `None`
- **AND** SHALL NOT 阻塞 `ssh_connect` 流程

#### Scenario: List all host aliases for UI combobox

- **WHEN** 调用方请求 `ssh_get_config_hosts()`
- **THEN** 系统 SHALL 解析 `~/.ssh/config` 提取所有非通配符 Host alias 列表
- **AND** SHALL NOT spawn `ssh -G`（该接口仅 list，无需高级特性解析）
- **AND** 文件不存在时 SHALL 返回空列表，不报错

### Requirement: Watch remote project directories via SFTP polling

系统 SHALL 在 SSH context 处于 `connected` 状态时启动远端文件变更感知 polling watcher：每 3 秒调用一轮 SFTP `read_dir(<remote_home>/.claude/projects/<project_id>/)` + 对每个 `.jsonl` 文件 `stat` 取 size 与 mtime，与上轮 baseline 比较差异（新增 / size 变化 / 删除）后通过与本地 watcher 相同的 `FileChangeEvent` schema 广播事件。第一次 poll SHALL 不触发任何事件（建 baseline 用）。系统 SHALL 额外每 30 秒运行一次 catch-up 比较作为兜底。SHALL 在 `ssh_disconnect` 时停止 watcher 与释放 SFTP 资源。

#### Scenario: First poll establishes baseline without events

- **WHEN** SSH context 刚切到 `connected` 状态，watcher 启动后第一次 poll
- **AND** 远端项目目录有 5 个 session JSONL 文件
- **THEN** watcher SHALL NOT emit 任何 `FileChangeEvent`
- **AND** 内部 baseline `BTreeMap<PathBuf, FileFingerprint>` SHALL 含 5 个条目

#### Scenario: Subsequent poll detects size change

- **WHEN** 第二次 poll 中某文件 size 从 1024 增长到 2048
- **THEN** watcher SHALL emit 一条 `FileChangeEvent { project_id, session_id, deleted: false }`
- **AND** baseline 中该文件 fingerprint SHALL 被更新

#### Scenario: Polling stops on disconnect

- **WHEN** 用户调 `ssh_disconnect`
- **THEN** 该 context 的 polling task SHALL 在 1s 内退出（cancellation token）
- **AND** SFTP channel SHALL 被关闭

#### Scenario: Watcher tolerates transient SFTP errors

- **WHEN** 某轮 poll 中 `read_dir` 返回瞬时错误（`ETIMEDOUT`）
- **THEN** watcher SHALL 跳过本轮，下一轮（3s 后）再尝试
- **AND** SHALL NOT 因单次失败而停止 watcher 或断开 SSH

### Requirement: Structured SSH error classification

系统 SHALL 把所有 SSH 失败场景归类到结构化 `SshError` enum：`Tcp`（TCP probe 失败）/ `AuthExhausted`（鉴权候选链全部失败）/ `SftpInit`（SFTP subsystem open 失败）/ `RemoteHomeMissing`（远端 `~/.claude/projects` 与多个 fallback 都不存在）/ `Cancelled`（用户主动取消）/ `Timeout`（按 stage 区分：TCP / Auth / SFTP）/ `Config`（SSH config 解析或 `ssh -G` 失败）。每个变体 SHALL 携带充分上下文（`Tcp { host, source }` / `AuthExhausted { attempts }` 等）。SHALL 实现 `serde::Serialize` 让错误能跨 IPC 边界以 JSON 形式传给前端 UI。

文件操作级错误 SHALL 通过 `cdt-fs::FsError` 表达——`SshFileSystemProvider` 实现 `FileSystemProvider` trait 时 SHALL 把 SFTP 错误投影到 `FsError`，包括：

- SFTP `NoSuchFile` → `FsError::NotFound`
- SFTP `PermissionDenied` → `FsError::Io { source: io::Error::new(ErrorKind::PermissionDenied, ...) }`
- 瞬时错误重试耗尽 → `FsError::TransientExhausted { path, attempts, last_reason }`
- SSH 会话断开（操作时 session 已 disconnect / channel closed）→ `FsError::Disconnected { path, reason }`
- 其它永久错误 → `FsError::Io { source: io::Error::other(...) }`

`FsError` SHALL 提供 `is_retryable()` 与 `should_invalidate_cache()` 元方法让 caller 按错误语义决定是否重试 / 是否清 cache。

#### Scenario: TCP probe failure carries host context

- **WHEN** 调用方连接到不可达 host
- **AND** TCP probe 5s 超时
- **THEN** `SshError::Tcp { host: "unreachable.example.com", source: <io::Error> }` SHALL 被返回
- **AND** 序列化后含 `code: "ssh_tcp_failure"` / `host` / `reason` 三个字段

#### Scenario: Auth exhausted carries detailed attempts

- **WHEN** 鉴权候选链全部失败
- **THEN** 错误 SHALL 为 `SshError::AuthExhausted { attempts }` 含每个候选的 `source` / `outcome` / `elapsed_ms`
- **AND** UI SHALL 能从 attempts 渲染逐项诊断（如"env agent: socket 不存在 / launchctl: 返回空 / 1Password: 文件不存在 / id_ed25519: requires passphrase use ssh-add / id_rsa: not found"）

#### Scenario: Cancellation by user

- **WHEN** 用户在 `connecting` 状态点击 UI 取消按钮
- **THEN** 进行中的 `russh::client::connect` future SHALL 被 abort
- **AND** 错误 SHALL 为 `SshError::Cancelled`，状态切到 `disconnected`，不残留半连接资源

#### Scenario: SFTP NoSuchFile 投影到 FsError::NotFound 且不重试

- **WHEN** `SshFileSystemProvider::stat(path)` 远端返 SFTP `NoSuchFile`
- **THEN** 调用方拿到 `FsError::NotFound(path)`
- **AND** `err.is_retryable()` 返 `false`，`err.should_invalidate_cache()` 返 `true`

#### Scenario: SFTP transient 耗尽投影到 TransientExhausted

- **WHEN** SFTP `read_to_string` 连续 3 次返回 `code=4` / `EAGAIN` 等瞬时错误
- **THEN** 调用方拿到 `FsError::TransientExhausted { path, attempts: 3, last_reason: <某个瞬时错误描述> }`
- **AND** `err.is_retryable()` 返 `false`（已经重试过了），`err.should_invalidate_cache()` 返 `false`（远端可能恢复）

#### Scenario: Session disconnect 投影到 Disconnected

- **WHEN** 文件操作时 SSH session 已断开（channel closed / session dropped）
- **THEN** 调用方拿到 `FsError::Disconnected { path, reason }`
- **AND** `err.is_retryable()` 返 `true`（重连后可能恢复），`err.should_invalidate_cache()` 返 `false`

### Requirement: Reconnect lifecycle preserves SFTP session integrity

`LocalDataApi` 在 `ssh_connect` / `switch_context` / `ssh_disconnect` 路径上 SHALL 保证：旧 `RemotePollingWatcher` 在 `SshSessionManager` 做任何 lifecycle 动作（`connect` / `disconnect` / `switch_context`）之前已完成 cancel-and-join，使新调用路径不可能拿到指向已关闭 SftpSession 的旧 `Arc<Mutex<SftpSession>>`。

实施约束（与 PR #171 现有实现一致，本 Requirement 主要为加自动化回归屏障）：

- 三处调用路径 SHALL 持 `ssh_watcher_ops: Mutex<()>` 序列化整段 cancel-then-mutate 操作
- `cancel_remote_watcher(prev_context_id).await` SHALL 在 `ssh_mgr.connect / switch_context / disconnect` 之前调用
- `attach_remote_watcher(new_context_id).await` SHALL 在 `ssh_mgr` 完成插入新 `SshSessionResources` 之后调用，且与 `ssh_shutdown_generation` 双检（shutdown 中途的 attach 被丢弃）
- watcher 归属保持在 `LocalDataApi.remote_watchers`，`SshSessionManager` 不直接管 watcher 生命周期（保持 crate 边界：`cdt-ssh` 不依赖 `cdt-api` 的 broadcast tx）

#### Scenario: 同 host 重连后 list_repository_groups 仍返回远端数据

- **WHEN** 调用方依次执行：`insert_test_ssh_context("ctx-a", fake_provider_v1)` → `list_repository_groups`（断言成功）→ `ssh_disconnect("ctx-a")` → `insert_test_ssh_context("ctx-a", fake_provider_v2)` 同名重新注册 → `list_repository_groups`
- **THEN** 第二次 `list_repository_groups` SHALL 成功返回 `RepositoryGroup`
- **AND** 返回值 SHALL 与 `fake_provider_v2` 提供的 fixture 一致（不复用 v1 的旧数据）
- **AND** 调用过程 SHALL NOT 抛 `Err` 含 `session closed` 字符串

#### Scenario: 切换到新 host 时旧 watcher 先 cancel-and-join 再 mutate

- **WHEN** active context 是 `Ssh<host_a>` 且其 watcher 正在运行
- **AND** 调用方请求 `ssh_connect(host_b)` 切换到新 host
- **THEN** `LocalDataApi::ssh_connect` SHALL 在调 `ssh_mgr.connect` 之前完成 `cancel_remote_watcher("host_a").await`
- **AND** cancel-and-join 完成后才执行 `ssh_mgr.connect`（内部会 disconnect `host_a` 的 SshSessionResources，旧 SftpSession Arc ref count 此时降为 0）
- **AND** `host_b` 上线后任何对 `host_b` provider 的查询 SHALL 拿到 fresh Arc，**不会**返回 `host_a` 的 closed session

### Requirement: Polling watcher exits promptly on cancellation

`RemotePollingWatcher::run_polling_loop` SHALL 在 `cancel_token.cancelled()` 触发时立即跳出主 loop（不等满 `POLL_INTERVAL` 或 `CATCH_UP_INTERVAL`）。当前实现使用 `tokio::select!` 同时 await `cancel_token.cancelled()` 与两个 interval tick，本 Requirement 把这一行为固化为契约。in-flight 的 `sftp.read_dir(...)` 自然完成，cancel 中断点在每次 select 入口；这是 spec `Read sessions and files over SSH with same contract` 的补强。

#### Scenario: cancel 在 sleep 阶段触发时 watcher 立即退出（paused time）

- **WHEN** 测试设置 `tokio::test(start_paused = true)`
- **AND** watcher task 在 `poll_interval.tick()` 的 await 状态
- **AND** 调用方触发 `cancel_token.cancel()`
- **THEN** `tokio::time::timeout(Duration::from_millis(100), watcher.cancel_and_join()).await` SHALL 返回 `Ok(())`（即 join 在 paused-time 维度的 100ms 内完成）
- **AND** 测试**不**通过推进时钟来让 watcher 退出（验证 cancel 本身而非 timer 触发）

#### Scenario: cancel 在 in-flight read_dir 时按现有逻辑退出

- **WHEN** watcher task 正在 await `sftp.read_dir(...)`（远端 SFTP I/O）
- **AND** 调用方触发 `cancel_token.cancel()`
- **THEN** 当前 read_dir 完成后，下一次 `tokio::select!` 入口 SHALL 命中 `cancel_token.cancelled()` 分支并跳出循环
- **AND** 本 Requirement **不**强制中断 in-flight SFTP request（保留 SFTP 协议层的礼貌断开）

### Requirement: `SshSessionManager` 暴露 `HostSignature` 派生的 `ContextId` 查询

系统 SHALL 在 SSH `connect_inner` 的 host alias resolve 阶段（stage 0）完成后，通过 `cdt_fs::SshConfigDigestInput::from(&ResolvedHost)` + `cdt_fs::HostSignature::from_ssh_config_fields(&input)` 计算并缓存当前 SSH context 的 `HostSignature`，存放在 `SshSessionResources.host_signature` 字段；`HostSignature` MUST NOT 在每次 IPC 调用时重新通过 `ssh -G` 子进程 resolve（避免 50-200ms 子进程 spawn overhead）。

`SshSessionManager` SHALL 暴露 `async fn context_id(&self, context_id: &str) -> Option<cdt_fs::ContextId>` 查询方法：

- 入参为已注册 SSH context 的 `context_id` 字符串
- 命中时 SHALL 从 `sessions.lock().await.get(context_id)` 取 `SshSessionResources` 的 `host_signature` 与 `remote_home`，合成 `ContextId::ssh(host_signature.clone(), remote_home.clone())` 返回 `Some(_)`
- 未注册（含已 disconnect / 未连接成功）时 SHALL 返回 `None`
- SHALL NOT 调用 `resolve_host_via_ssh_g` 子进程

`SshSessionManager::insert_test_context` test helper SHALL 接受 `Option<cdt_fs::HostSignature>` 参数；缺省时 SHALL 用 `(host, port, user)` 字符串拼接做 fake SHA-256 digest 构造一个 `HostSignature`，使不同 host 的测试 fixture 自然产不同 digest。

`SshSessionManager` SHALL 暴露原子查询方法 `async fn provider_and_context_id(&self, context_id: &str) -> Option<(SshFileSystemProvider, cdt_fs::ContextId)>`——单次 `sessions` lock 内同时返回 provider 与 `ContextId`，保证二者来自同一快照（codex 二审 commit-stage Blocking → design D3-bis）。调用方 SHALL 用本方法而非独立的 `provider(&str)` + `context_id(&str)` 配对取 fs/ctx，避免 disconnect race 产生 `(SSH provider, Local ctx)` 不自洽组合。

#### Scenario: connect 路径自动计算并存储 `HostSignature`

- **WHEN** `SshSessionManager::connect(request)` 走到 stage 0 完成 `resolve_host_via_ssh_g` 拿到 `ResolvedHost`
- **THEN** SHALL 通过 `SshConfigDigestInput::from(&resolved)` 构造 input
- **AND** SHALL 调 `HostSignature::from_ssh_config_fields(&input)` 计算 digest
- **AND** SHALL 在最终构造 `SshSessionResources` 时填入 `host_signature` 字段
- **AND** SHALL NOT 在后续 IPC / cache lookup 时再次跑 `ssh -G`

#### Scenario: `context_id(&str)` 返回 `ContextId::ssh(...)`

- **WHEN** 调用方对一个已连接的 SSH context 调用 `ssh_mgr.context_id("ssh-host-A").await`
- **THEN** 返回 `Some(ContextId)`，其 `backend_kind == FsKind::Ssh`
- **AND** `host_signature` SHALL 等于 connect 时计算并存储的 `HostSignature`
- **AND** `root_or_home` SHALL 等于 `SshSessionResources.remote_home`

#### Scenario: 未注册 context 返回 `None`

- **WHEN** 调用方对一个未注册（或已 disconnect）的 context_id 调用 `ssh_mgr.context_id(...)`
- **THEN** SHALL 返回 `None`，且 SHALL NOT panic 或 spawn 子进程

#### Scenario: `provider_and_context_id` 原子返回 provider+ctx

- **WHEN** 调用方对已注册 SSH context 调用 `ssh_mgr.provider_and_context_id("ssh-host-A").await`
- **THEN** SHALL 返回 `Some((provider, ctx))`，二者来自同一 `sessions.lock()` 快照
- **AND** `ctx.host_signature` SHALL 等于该 provider 在 connect 时计算并存储的 `HostSignature`
- **AND** `ctx.root_or_home` SHALL 等于 provider 的 `remote_home`
- **WHEN** 调用方对未注册 context 调用同方法
- **THEN** SHALL 返回 `None`，调用方据此 fall-through 到 Local 安全降级

#### Scenario: 同 host reconnect 后 `ContextId` 一致

- **WHEN** 用户先 connect → disconnect → 再 connect 同一 SSH host A（`~/.ssh/config` 未变 AND 两次 connect 均走 `ssh -G` 成功路径）
- **THEN** 两次 connect 后通过 `context_id("ssh-host-A").await` 拿到的 `ContextId` SHALL `==`（`HostSignature.config_digest` 是 resolved ssh config 的纯函数，不含随机或时序成分）
- **AND** 任何用此 `ContextId` 做 key 的 cache entry SHALL 跨 reconnect 复用

#### Scenario: degraded fallback 与 `ssh -G` 路径产 `ContextId` 安全不等（by-design miss）

- **WHEN** 第一次 connect 走 `resolve_host_via_ssh_g` 成功路径，`ResolvedHost` 含 `proxyjump` / `proxycommand` / `hostkeyalias` 字段 → 计算出 `HostSignature` digest A
- **AND** 第二次 reconnect 时 `ssh` 子进程缺失 / `ssh -G` 失败，走 `fallback_via_config_parser` 路径，`ResolvedHost.proxyjump = .proxycommand = .hostkeyalias = None` → 计算出 `HostSignature` digest B
- **THEN** digest A `!=` digest B（不同字段集合 → 不同 SHA-256 输入）
- **AND** 两次 connect 派生的 `ContextId` SHALL NOT `==`
- **AND** 任何用 digest A 做 key 写入的 cache entry SHALL NOT 被 digest B 的 lookup 命中——这是 **by-design safe miss**（degraded 路径对 host 的连接拓扑认知降级，与 ssh -G 路径不等价；落到不同 cache namespace 防止"基于错误连接假设拿到陈旧远端数据"）
- **AND** 用户体感为 reconnect 后 session 列表冷扫一次，UX 多几秒，但绝不串扰数据


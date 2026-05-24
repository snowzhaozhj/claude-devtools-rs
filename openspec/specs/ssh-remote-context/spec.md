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

watcher SHALL 把每轮 poll 失败按错误特征分到三类（在 polling 层 `with_retry` 之后做语义升级，**不**改 `cdt-ssh::SftpClient` trait 错误分类）：

- `Permanent`：错误消息含 `session closed` / `eof` / `broken pipe` / `epipe` / `connection reset` / `econnreset` 任一关键字（不区分 `Other` / `Transient` 来源——`provider::is_transient_io_reason` 把 `broken pipe` / `connection reset` / `epipe` 归 Transient，`with_retry` 3 次后仍是 transport-dead 即视同 channel 真死）
- `Timeout`：错误消息含 `timeout` / `etimedout` / `timed out` / `eagain` / `would block` 任一关键字（来自 `provider::is_transient_io_reason` 列表减去 transport-dead 子集；含 `would block` 即 `std::io::ErrorKind::WouldBlock`，与 EAGAIN 同源——不纳入 timeout 类会让"反复 WouldBlock"序列只能落 OtherTransient 重置计数，与 timeout 漏检对称）
- `OtherTransient`：其它 `Transient` / `Other` / `NoSuchFile` / `PermissionDenied`（含 `Status::Failure` 的 `error_message` 等不带 transport-dead / timeout 关键字的失败）

watcher SHALL 维护两个独立 counter：

- `consecutive_permanent: u32`，阈值 `PERMANENT_FAILURE_THRESHOLD = 3`（≈ 9s 持续 transport 错误）
- `consecutive_timeout: u32`，阈值 `TIMEOUT_FAILURE_THRESHOLD = 6`（≈ 18s 持续 timeout，远高于网络瞬时抖动 1-3s window，远低于用户感知 sidebar 僵死的 60s）

counter 演化规则（codex 二审收紧 reset 规则，避免攻击序列推迟 dead_signal）：

- `Ok` / `OtherTransient`：两 counter 都 SHALL reset 为 0（唯一 reset 入口；只有"channel 真活着"的强证据才清零）
- `Permanent`：仅 `consecutive_permanent += 1`，**不动** `consecutive_timeout`
- `Timeout`：仅 `consecutive_timeout += 1`，**不动** `consecutive_permanent`

任一 counter ≥ 自己阈值时，watcher SHALL `dead_signal.notify_one()` + 跳出主 loop。

理由：早期"互斥重置"规则被 `5T → 1P → 5T → 1P → ...` 攻击序列利用让 timeout 永不达 6；新规则下 dead-向量单调累积，攻击序列只能拖延无法阻止——`5T + 1P` 后下一轮 `1T` 即触发（`timeout=6 ≥ 6`）。

`scan_once` 内 sub-project 子目录 `read_dir` 失败时：

- `NoSuchFile` / `PermissionDenied`：silent skip 该 project（保持现有容错）
- 其它错误经 `classify_failure` 分类——`Permanent` SHALL 让整个 `scan_once` 返 `Err(SftpClientError::*)` escalate 到顶层 counter（避免 sub-project channel-dead 错误被静默吞掉、watcher 误以为 baseline 完整后下轮报"全部 session deleted"事件）；`Timeout` / `OtherTransient` 仍 silent skip 该 project，留下次 catch-up 重试

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

#### Scenario: Watcher tolerates short transient SFTP errors below threshold

- **WHEN** 某轮 poll 中 `read_dir` 返回瞬时 timeout 错误（`Transient("timeout")`）
- **AND** `consecutive_timeout` 累计 < `TIMEOUT_FAILURE_THRESHOLD = 6`
- **THEN** watcher SHALL 跳过本轮，下一轮（3s 后）再尝试
- **AND** SHALL NOT 因单次失败而停止 watcher 或断开 SSH
- **AND** SHALL NOT 触发 `dead_signal`

#### Scenario: Sustained timeout triggers dead_signal at 6 consecutive

- **WHEN** SFTP `read_dir` 连续 6 轮 poll 都返 `Transient("timeout")` 类错误（典型场景：远端 `pkill -STOP sshd` 导致 SFTP 协议层 hang 但 TCP 未断）
- **THEN** watcher SHALL 在第 6 轮后 `dead_signal.notify_one()` 并跳出主 loop
- **AND** 触发 `LocalDataApi` monitor task 走 `perform_polling_self_heal_disconnect` 把 active context 切回 `Local`
- **AND** wall time SHALL ≈ 18s（6 × `POLL_INTERVAL=3s`），远低于 issue #231 报告的"用户走死 SFTP 等 30s curl timeout 才放弃"

#### Scenario: Permanent transport error triggers dead_signal at 3 consecutive

- **WHEN** SFTP `read_dir` 连续 3 轮 poll 都返含 `session closed` / `broken pipe` / `connection reset` 等 transport-dead 关键字的错误（无论来源是 `SftpClientError::Other` 还是 `Transient`）
- **THEN** watcher SHALL 在第 3 轮后 `dead_signal.notify_one()` 并跳出主 loop
- **AND** wall time SHALL ≈ 9s（3 × `POLL_INTERVAL`）

#### Scenario: Timeout counter resets on intervening success

- **WHEN** 5 轮 timeout（`consecutive_timeout = 5`）后下一轮 `read_dir` 成功
- **THEN** `consecutive_timeout` SHALL 立即 reset 为 0
- **AND** 后续即使再来 5 轮 timeout 也 SHALL NOT 触发 `dead_signal`（因为新 streak < 6）

#### Scenario: Permanent and timeout counters accumulate independently (mixed sequence still triggers)

- **WHEN** 5 轮 timeout 后来 1 轮 permanent
- **THEN** `consecutive_timeout` SHALL = 5（不被 permanent 重置）；`consecutive_permanent` SHALL = 1
- **AND** 下一轮 timeout SHALL 让 `consecutive_timeout = 6 ≥ TIMEOUT_FAILURE_THRESHOLD` → 立即触发 `dead_signal`
- **AND** 反向同理：2 轮 permanent + 1 轮 timeout 后 `consecutive_permanent = 2`、`consecutive_timeout = 1`；下一轮 permanent 让 `consecutive_permanent = 3 ≥ PERMANENT_FAILURE_THRESHOLD` → 触发
- **AND** 攻击序列 `5T → 1P → 5T → 1P → ...` SHALL **不能**永远推迟 dead_signal——任一 dead 向量单调累积

#### Scenario: OtherTransient errors do not trigger dead_signal

- **WHEN** 连续 10 轮 poll 返 `Transient("EAGAIN")` 等不含 transport-dead 与 timeout 关键字的错误
- **THEN** 两 counter 都 SHALL reset 为 0（`OtherTransient` 不计任一计数）
- **AND** SHALL NOT 触发 `dead_signal`
- **AND** watcher SHALL 持续运行等下一轮恢复

#### Scenario: Sub-project read_dir permanent error escalates to scan_once failure

- **WHEN** 顶层 `read_dir(<remote_home>/.claude/projects/)` 成功，但其中一个 sub-project `read_dir(<base>/<project_id>/)` 返 `Other("session closed")` 永久错误
- **THEN** `scan_once` SHALL 立即 return `Err(SftpClientError::Other(...))` 而非 silent skip 该 project
- **AND** 外层 polling loop 经 `classify_failure` 把该错误归 `Permanent` → `consecutive_permanent += 1`
- **AND** 连续 3 轮 sub-project permanent 错误 SHALL 触发 `dead_signal`

#### Scenario: Sub-project read_dir timeout / NoSuchFile silent skip 仍保留

- **WHEN** 顶层 `read_dir` 成功，sub-project A 返 `NoSuchFile`，sub-project B 返 `Transient("timeout")`，sub-project C 成功
- **THEN** `scan_once` SHALL 跳过 A 与 B，处理 C 后正常返 `Ok(BTreeMap)`
- **AND** baseline 仅含 C 的条目（A / B 缺失视同未变更，下轮 catch-up 自然重试）
- **AND** 不 escalate 任何错误到外层 counter

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

- **5 处 mutate 入口** SHALL 在 mutate 之前持 `ssh_watcher_ops: Mutex<()>`：`ssh_connect` / `switch_context` / `ssh_disconnect` / `reconfigure_claude_root` / `shutdown_ssh_all`。`shutdown_ssh_all` 实现 SHALL 用 `lock().await` 而非 `try_lock()`——`try_lock` 失败时绕过锁直接 mutate `ssh_mgr` 会破坏与 refresh 路径同锁互斥的前提（详 change `generation-race-audit` codex commit-stage Bug 2 修订）。
- `cancel_remote_watcher(prev_context_id).await` SHALL 在 `ssh_mgr.connect / switch_context / disconnect` 之前调用
- `attach_remote_watcher(new_context_id).await` SHALL 在 `ssh_mgr` 完成插入新 `SshSessionResources` 之后调用，且与 `ssh_shutdown_generation` 双检（shutdown 中途的 attach 被丢弃）
- watcher 归属保持在 `LocalDataApi.remote_watchers`，`SshSessionManager` 不直接管 watcher 生命周期（保持 crate 边界：`cdt-ssh` 不依赖 `cdt-api` 的 broadcast tx）

**bump-first 顺序契约（不变）**：5 处路径 SHALL 在 `ssh_mgr.connect / switch_context / disconnect` / `ssh_mgr.shutdown_all` / 写新 `projects_dir` 这一步 await **之前** 完成 `context_generation.fetch_add(1, SeqCst)`（`reconfigure_claude_root` 同时 bump `root_generation`）。理由：保留这一顺序让任何 in-flight `list_sessions_skeleton` / `build_group_session_page` 在 spawn 时记录的 `expected_context_generation` 立即失效（broadcast 前 check `current != expected` → silent drop）。如果反转为"先 await 后 bump"，await 期间 ssh_mgr 状态部分切换但 generation 未 bump，in-flight scan task 的 broadcast 校验 `current == expected` 仍通过，会向前端串扰旧 ctx 的 metadata update。

**派生 cache 写入的双重校验**：依赖 `worktree_meta_cache` 等"全局 flat-key、随 list_repository_groups 刷新"的派生 cache 的实现 SHALL 在 cache 写入路径前用 `ssh_watcher_ops` 锁与本路径互斥，并在锁内做 (current ContextId == captured ContextId) **AND** (current `context_generation` == captured `context_generation`) 双重校验——任一 mismatch 时 SHALL skip 写入（safe degrade，不污染派生 cache）。详 `ipc-data-api::SessionSummary 增加 worktree 元信息字段` Requirement 的"映射缓存刷新约束"段。理由：bump-first 顺序使 `context_generation` 在 `ssh_mgr.switch_context` 网络 RTT 期间已经领先于实际 ssh_mgr 状态；caller 的 generation pre/post snapshot 都可能整段落在该 window 内（pre = post = bumped 后值），漏判 "context 已切"。**单 ctx-equality** 又无法识别"同 host 快速 disconnect+reconnect 期间 ContextId 等价但 generation bumped 两次"边角；**单 generation-equality** 无法识别"reconfigure_claude_root 改 Local projects_dir 但 ssh_mgr.active 不变"边角。结构性修法是 refresh 路径用同锁同步读 ssh_mgr active + 重建 ContextId（含 Local 时的 projects_dir 字段）+ 二次比较 captured generation 做综合判断。

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

#### Scenario: switch_context bump-first 顺序保留以防 in-flight scan 串扰

- **WHEN** active context = `Ssh<host_a>` 且有一个 in-flight `list_sessions_skeleton` 已 spawn `scan_metadata_for_page` 后台 task（task 持 `expected_context_generation = N`）
- **AND** 调用方触发 `switch_context("local")`
- **THEN** `switch_context` 实现 SHALL 先 `context_generation.fetch_add(1, SeqCst)`（gen N→N+1）再 await `ssh_mgr.switch_context(None)`
- **AND** 后台 task 后续每次 `tx.send(SessionMetadataUpdate)` 前 SHALL load `context_generation`，发现 `N+1 != N` → silent drop update
- **AND** 用户 SHALL NOT 在切到 Local 后看到 host_a 的 metadata broadcast 串扰

#### Scenario: 派生 cache 写入识别 captured ctx 与当前 active 不一致时 skip

- **WHEN** 调用方 task A 触发 `switch_context("local")`，进入 `ssh_mgr.switch_context(None).await` 期间（gen 已 bump 到 N+1，但 ssh_mgr.active_context_id() 仍返 `host_a`）
- **AND** 调用方 task B 并发调 `list_repository_groups()`
- **AND** task B 的内部 `active_fs_and_policy().await` 拿到 captured_ctx = `Ssh<host_a>` + captured_generation = `N+1`
- **WHEN** task B 进入 `refresh_worktree_meta_cache` 路径，先获取 `ssh_watcher_ops` 锁
- **THEN** 锁拿到时 `ssh_mgr.switch_context` 已完成（task A 也持同锁，task A 完成 mutate + 释放锁后 task B 才能拿到）
- **AND** 锁内 `ssh_mgr.active_context_id().await` SHALL 返回 `None`（Local active）；重建的 ContextId = `Local { projects_dir }` 与 captured_ctx = `Ssh<host_a>` mismatch
- **AND** task B SHALL skip refresh + 写 `tracing::debug!`（含 captured/current ctx + 两个 generation 值），不调用 `worktree_meta_cache.write().clear()`
- **AND** task B 仍 SHALL 把 host_a 的 groups 返给 caller（caller 拿 self-consistent 旧数据；下次 IPC 自然刷新到 Local）

#### Scenario: 同 host 快速 disconnect+reconnect 期间生成 generation mismatch 触发 skip

- **WHEN** active context = `Ssh<host_a>`，调用方 task B 进入 `list_repository_groups_inner()` 拿到 captured_ctx = `Ssh<host_a>` + captured_generation = `N`
- **AND** 调用方 task A 在 task B inner 完成之后、wrapper 拿锁之前完成 `ssh_disconnect("host_a")`（gen N→N+1）+ `ssh_connect("host_a")` 同 host 重连（gen N+1→N+2）
- **THEN** task B wrapper 拿锁后 重建的 ContextId = `Ssh<host_a>` 与 captured_ctx 全等（同 HostSignature 派生的 ContextId 相等），但 `current_generation = N+2` ≠ `captured_generation = N` → generation mismatch
- **AND** task B SHALL skip refresh —— 避免 task B inner 用旧 host_a session 拿到的 groups 覆盖新 session 应有的最新 mapping

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

### Requirement: SSH `open_read` 大文件走 K-worker prefetch streaming reader

`SshFileSystemProvider::open_read`（`cdt-fs::FileSystemProvider` trait 实现）对**生产路径**（即 `self.sftp` 字段为 `Some(Arc<SftpSession>)`，由 `SshFileSystemProvider::new` 构造）且**大文件**（`sftp.metadata(path).len() >= SFTP_PIPELINE_MIN_BYTES`，当前钉死 256 KiB）SHALL 返回一个**流式 K-worker prefetch reader**——内部由 K=`SFTP_PIPELINE_MAX_WORKERS`（当前 16）个 tokio task 并发飞独立 SFTP `SSH_FXP_READ`，把读到的 chunk 经有界 channel 推给消费侧，使得 reader 的 peak RSS 与 K 成正比而非与 `file_size` 成正比。

K-worker SHALL 用 **round-robin chunk 分派**：第 `i` 个 chunk（chunk 大小 `SFTP_PIPELINE_CHUNK_BYTES`=32 KiB）由 `worker_id = i % n_workers` 处理；消费侧按 `next_worker = (next_worker + 1) % n_workers` 顺序取——保证消费速度推进的同时所有 K 个 worker 都能持续被 backpressure 释放，wall ≈ `ceil(file_size / chunk_bytes / n_workers)` × RTT 与 PR-F 全量预取 baseline 持平。

**Limited 降级**：K 个 `sftp.open` 用 `futures::future::join_all` 预并发打开时若任一返回 `russh_sftp::client::error::Error::Limited(reason)`（server 端 SFTP `open_handles` 上限），`open_read` SHALL 降级到**单 handle 流式**——优先**复用** `join_all` 已成功返回的第 1 个 `russh_sftp::client::fs::File`（avoid 再开一次 `sftp.open` 撞同样 Limited + avoid 依赖 `File::drop` 同步 close 语义）；该类型实现 `tokio::io::AsyncRead`，直接 `Box::new(file)` 返；wall 退化到 `N × RTT` 但 peak RSS 仍受限单 chunk。partial_handles 仅在所有 K 个 open 都 Limited 时为空，此时 SHALL 显式 `sftp.open(path).await` 再开一次（接受可能继续 Limited 上抛）。降级 SHALL 通过 `tracing::warn!` 记录 path / workers / partial_handle_count / reason 让运维侧可见。

**小文件路径**：生产路径 + `file_size < SFTP_PIPELINE_MIN_BYTES` 仍 SHALL 走单 RTT `sftp.read(path)` 全量预取 + `Cursor::new` 包装——避免 K 个 `sftp.open` 的 spawn overhead 对小文件 wall 无收益反加 latency。

**Fake 测试路径**：`SshFileSystemProvider::with_client` 构造的 `self.sftp == None` 实例 SHALL 走 `SftpClient::read(path)` trait 方法 + `Cursor::new` 包装的原有路径——保留 `CountedFakeRemoteSftp::read_count` 等 op counter 语义，让现有 `crates/cdt-api/tests/perf_ssh_cache_hit.rs` 与 `crates/cdt-api/tests/perf_ssh_scanner_chunked_read.rs` 的 fake 路径断言**无需更新**。

**inherent `open_read_stream` 保留**：`SshFileSystemProvider::open_read_stream` (`pub async fn` 返 `russh_sftp::client::fs::File`) 行为不变——caller 显式调用拿原生 SFTP 句柄路径不受本 change 影响。

#### Scenario: 大文件生产路径返流式 K-worker prefetch reader

- **WHEN** caller 在生产构造（`SshFileSystemProvider::new(ctx, Arc<SftpSession>, remote_home)`）的 provider 上调 `open_read(path)` 且 `sftp.metadata(path).len() >= SFTP_PIPELINE_MIN_BYTES`
- **THEN** 返回的 `Box<dyn AsyncRead + Send + Unpin>` SHALL 是 `PipelinedSftpReader` 包装而非 `std::io::Cursor<Vec<u8>>`
- **AND** reader 实例 SHALL 持有 `n_workers = min(SFTP_PIPELINE_MAX_WORKERS, ceil(size / SFTP_PIPELINE_CHUNK_BYTES)).max(1)` 个 mpsc receiver（每个 capacity = 1）+ `JoinSet<()>` 拥有 K 个 worker task
- **AND** caller 持续 `poll_read` 直到 EOF 期间，进程 peak RSS 增量 SHALL ≤ `n_workers × 2 × SFTP_PIPELINE_CHUNK_BYTES`（最坏：每个 channel 1 个 buffered chunk + 每个 worker 1 个 in-flight chunk）

#### Scenario: 大文件 round-robin chunk 分派保 wall parity

- **WHEN** `PipelinedSftpReader::open` 启动 K worker
- **THEN** 第 `i` 个 chunk SHALL 由 worker `i % n_workers` 处理（worker 0 读 chunks [0, K, 2K, ...]，worker 1 读 chunks [1, K+1, 2K+1, ...]，依此类推）
- **AND** 消费侧 `poll_read` SHALL 按 `next_worker = (next_worker + 1) % n_workers` 严格轮询，确保消费推进直接释放每个 worker 的 backpressure
- **AND** total wall time SHALL ≈ `ceil(n_chunks / n_workers) × RTT`（与 PR-F 全量预取 baseline 持平，不退化为 `n_chunks × RTT` 串行）

#### Scenario: 小文件生产路径走单 RTT 全量预取

- **WHEN** caller 在生产构造的 provider 上调 `open_read(path)` 且 `sftp.metadata(path).len() < SFTP_PIPELINE_MIN_BYTES`
- **THEN** 实现 SHALL 调 `sftp.read(path).await` 拿全 `Vec<u8>`
- **AND** SHALL 返 `Box::new(std::io::Cursor::new(bytes))`
- **AND** SHALL NOT spawn K worker / 不创建 `PipelinedSftpReader`

#### Scenario: Fake 测试路径走 `SftpClient::read` 全量保 op counter 语义

- **WHEN** caller 在 fake 构造（`SshFileSystemProvider::with_client(ctx, Arc<dyn SftpClient>, remote_home)`）的 provider 上调 `open_read(path)`
- **THEN** 实现 SHALL 调 `self.client.read(path).await`（trait 方法）拿全 `Vec<u8>`
- **AND** SHALL 返 `Box::new(std::io::Cursor::new(bytes))`
- **AND** `CountedFakeRemoteSftp::read_count` 在此次调用后 SHALL 增 1（与 PR-F 前 PR-D 时期语义一致）

#### Scenario: `SftpError::Limited` 降级到单 handle 流式且优先复用已开 handle

- **WHEN** `PipelinedSftpReader::open` 内部用 `futures::future::join_all` 并发 K 个 `sftp.open`，收齐 `Vec<Result<File, SftpError>>` 后任一为 `Err(russh_sftp::client::error::Error::Limited(reason))`（server SFTP `open_handles` 限制）
- **THEN** `open_read` SHALL 降级到单 handle 流式：**优先复用 `Vec` 中已成功打开的第 1 个 `File`**（避免再次 `sftp.open` 撞同样 Limited，避免依赖 `russh_sftp::client::fs::File::drop` 的同步 close 语义）
- **AND** 若所有 K 个 `sftp.open` 都 Limited（如 K=1 时罕见场景），SHALL 显式 `sftp.open(path).await` 重试 1 次；若仍 Limited 上抛 `FsError::Io { ErrorKind::Other }`
- **AND** SHALL 把 `Vec` 中其余成功的 `File` 直接 drop 让 russh-sftp 自身释放（接受 best-effort close 的潜在短暂 server 端 handle leak，ms 量级，SSH 连接关闭时彻底释放）
- **AND** SHALL 通过 `tracing::warn!(path, workers, partial_handle_count, reason, ...)` 记录降级事件
- **AND** caller 仍能完整流式读到 EOF，peak RSS 不会超过单 chunk + tokio File 内部 buffer

#### Scenario: 任一 worker channel close 时立即按字节计数判定真 EOF 防 silent truncation

- **WHEN** `PipelinedSftpReader` 内 round-robin 轮到的 `next_worker` 对应 `mpsc::Receiver` 在 `poll_recv` 返 `None`（该 worker 正常退出或 panic 后 sender drop；round-robin 顺序保证此刻 stream 已无该位置的后续 chunk）
- **THEN** consumer SHALL **立即**（不等其它 worker 全 close）比较累计写入字节 `total_bytes_read` 与构造时记录的 `total_bytes_expected`（= `sftp.metadata(path).len()`）
- **AND** 若 `total_bytes_read == total_bytes_expected` → SHALL 返 `Poll::Ready(Ok(()))` 不写入 `ReadBuf`（标准 AsyncRead EOF 语义）；reader 标 `eof = true` 后续 poll_read 持续返 EOF
- **AND** 若 `total_bytes_read < total_bytes_expected` → SHALL 返 `Poll::Ready(Err(io::Error::new(ErrorKind::UnexpectedEof, format!("PipelinedSftpReader closed early: expected {} bytes, got {}", expected, read))))`（防 worker 静默退出 / JoinSet 异常 abort 让 caller 误把短读当 EOF）；reader 标 `error_seen = true` 后续 poll_read 返终态错误
- **AND** SHALL NOT 等所有 K 个 receiver 都 close 才判定（继续等会让 consumer hang 死等其它仍在飞 next-round chunk 的 worker）

#### Scenario: 生产路径分支选择钉死小文件 / 大文件 / fake 三 branch wiring

- **WHEN** 调用 `pub(crate) fn pick_open_read_strategy(has_sftp: bool, size: u64) -> OpenReadStrategy`
- **THEN** `(has_sftp=true, size >= SFTP_PIPELINE_MIN_BYTES)` SHALL 返 `OpenReadStrategy::Streaming { n_workers: usize }`
- **AND** `(has_sftp=true, size < SFTP_PIPELINE_MIN_BYTES)` SHALL 返 `OpenReadStrategy::SmallFileBuffered`
- **AND** `(has_sftp=false, _)` SHALL 返 `OpenReadStrategy::FakeBuffered`（fake 测试路径所有 size 都走 `client.read`）
- **AND** 此分支函数 SHALL 在 `crates/cdt-ssh/src/provider.rs::tests` 内有单元测试覆盖以上 4 个组合，拦截"未来 PR 误把生产大文件 branch 接到 client.read 旧路径"类 wiring 回归

#### Scenario: Worker 内部 SFTP 错误经 channel 传 `io::Error` 给消费侧

- **WHEN** 已构造的 `PipelinedSftpReader` 在某 worker 内 `file.seek(SeekFrom::Start(offset))` 或 `file.read_exact(&mut buf)` 调用返 `Err`
- **THEN** worker SHALL 把错误转换为 `tokio::io::Error::other(format!(...))` 并通过 `tx.send(Err(io_err)).await` 推给对应 receiver
- **AND** worker SHALL 然后 `return`（drop sender，channel close）
- **AND** consumer 的 `poll_read` 收到 `Poll::Ready(Some(Err(io_err)))` 时 SHALL 返 `Poll::Ready(Err(io_err))`
- **AND** SHALL NOT silent drop 错误（如 worker panic 后丢失错误信号让 consumer hang）

#### Scenario: Reader drop 联级 abort 所有 worker

- **WHEN** `PipelinedSftpReader` 持有者 drop reader（典型场景：上游 `parse_file_via_fs` 内部 `BufReader<reader>` 提前结束 / 上游 `tokio::spawn` task 被 abort）
- **THEN** `PipelinedSftpReader::_workers: JoinSet<()>` 字段 drop SHALL 触发所有未完成 worker task 的 `AbortHandle::abort`
- **AND** worker 内任一 `.await` 点（典型 `sftp.read_exact` / `tx.send`）SHALL 在下次 poll 时被 cancellation 返回 abort
- **AND** SHALL NOT 留 orphan task 在 tokio runtime 继续读 SFTP 浪费带宽

#### Scenario: EOF 通过 next round-robin worker channel close + 字节计数表达

- **WHEN** worker 处理完自己分到的最后一个 chunk 并通过 `tx.send(Ok(bytes)).await` 成功推给 consumer
- **THEN** worker SHALL `return`（自然 drop sender）
- **AND** consumer 下次 round-robin 轮询到该 worker 的 receiver 时 `mpsc::Receiver::poll_recv` SHALL 返 `None`
- **AND** consumer SHALL **立即**触发字节计数判定（如上 Scenario "任一 worker channel close 时立即按字节计数判定真 EOF 防 silent truncation"），不等其它 worker close；正常退出场景下此时 `total_bytes_read == total_bytes_expected` → 翻译为 `Poll::Ready(Ok(()))` 不再写入 `ReadBuf`（标准 AsyncRead EOF 语义）

#### Scenario: 大会话 scanner BufReader 接 `PipelinedSftpReader` 不破契约

- **WHEN** `cdt-parse::parse_file_via_fs` 在 SSH 生产路径下调 `fs.open_read(path)` 拿 reader，再 `BufReader::with_capacity(SCANNER_BUF_BYTES, reader)` 包装（容量 32 KiB 与 SFTP packet 上限对齐）
- **THEN** reader 实际是 `PipelinedSftpReader`，每次 `BufReader::fill_buf` SHALL 从 `PipelinedSftpReader::poll_read` 拿到下一个 32 KiB chunk（K-worker prefetch 提前飞 read 已让 chunk 通常在 channel 中就绪）
- **AND** scanner 全文 parse 完成 → `BufReader` drop → `PipelinedSftpReader` drop → JoinSet drop → worker cleanup
- **AND** 与 PR-F 全量预取 baseline 对比：scanner wall ≈ 持平（K-worker 并发数 + chunks_per_worker 不变）；进程 peak RSS 增量 SHALL 从 ≈ file_size 降到 ≈ `n_workers × 2 × 32 KiB`

### Requirement: SSH 远端 memory CRUD 走真实 fs ops

系统 SHALL 在 SSH context 下完整支持 project memory CRUD：`get_project_memory` / `read_memory_file` / `add_memory` / `delete_memory` 四个 IPC method 在 active context 是 `Ssh<host>` 时 SHALL 通过当前 SSH `FileSystemProvider` 调用真实远端 fs ops，**不**得 graceful skip 返 `has_memory: false` / not_found。

`SshFileSystemProvider` SHALL 在 `cdt-fs::FileSystemProvider` trait 上实现 `write_atomic` / `create_dir_all` / `remove_file` 三个方法，行为契约：

- `write_atomic` SHALL 通过底层 SFTP 协议写到 `<path>.tmp.<atomic-seq-hex>.<pid-hex>`，写完调 SFTP rename 覆盖目标 path：
  - 优先走 `posix-rename@openssh.com` SFTP 扩展（`russh-sftp::SftpSession::extensions()` 在 connect 时探测，含此扩展则启用），由 OpenSSH server 提供 POSIX rename(2) 原子覆盖
  - 不支持时降级为两步：先 `client.remove(<target>)` 再 `client.rename(<tmp>, <target>)`——降级路径有极短窗口 reader 可能见 `target missing`，单次写场景 acceptable
  - rename 失败 SHALL best-effort 调 `remove_file(<tmp>)` 清理（清理失败不向上传播）
  - 服务端探测结果 SHALL 在 `SshFileSystemProvider` 内 cache 一次（per session），后续 `write_atomic` 直接读 cache 决策，不每次 connect 重探测
- `create_dir_all` SHALL 通过 SFTP 递归创建目录，对每段父目录先调 `try_exists` 探测，已存在跳过；缺失调 `mkdir` 创建。任何 SFTP rpc 失败 SHALL 走既有 retry 策略（`code=4 / EAGAIN / ECONNRESET / ETIMEDOUT / EPIPE` ≤ 3 次，指数退避 75ms × attempt）
- `remove_file` SHALL 通过 SFTP `SSH_FXP_REMOVE` 删文件；不存在 SHALL 返 `FsError::NotFound(path)`；路径是目录 SHALL 返 `FsError::Io { path, source: <ENOTEMPTY> }`，**不**递归删

`cdt-ssh::SftpClient` trait（位于 `crates/cdt-ssh/src/provider.rs`，**不**是独立 `sftp_client.rs` 文件）SHALL 新增 `write` / `mkdir` / `remove` / `rename` 四个方法，由 `RusshSftpClient` 实现 delegate 到 `russh_sftp::client::SftpSession::write` / `create_dir` / `remove_file` / `rename`。所有写操作 SHALL 与既有 read 操作复用同一 `Arc<dyn SftpClient>` + `Arc<SftpSession>`（**不**再用 `Arc<Mutex<SftpSession>>`，老 Mutex 已在前序 PR 移除——`SftpSession` 公共 API 是 `&self` 方法，message-id 由库内部 channel 维护）。SFTP message-id pipeline 并发支持留 PR-F；本 change 写路径与既有 read 路径同处一队列。

#### Scenario: SSH context 下 get_project_memory 走远端 read_dir + read_to_string

- **WHEN** active context 是 `Ssh<host>`，调用方调 `get_project_memory(project_id)`
- **THEN** 系统 SHALL 通过当前 SSH `FileSystemProvider` 调 `fs.read_dir(<remote_home>/.claude/projects/<base>/memory)` 列举 `.md` 文件
- **AND** 调 `fs.read_to_string(<memory_dir>/MEMORY.md)` 读 index 内容（如存在）
- **AND** 返回的 `ProjectMemory` SHALL 携带远端 layers 真实数据，`hasMemory` SHALL 为 `true`（当 memory 目录存在且含 `.md` 文件）
- **AND** 远端 fake provider 的 `read_dir_count` 与 `read_count` SHALL 各 ≥ 1

#### Scenario: SSH context 下 read_memory_file 走远端 read_to_string

- **WHEN** active context 是 `Ssh<host>`，调用方调 `read_memory_file(project_id, "MEMORY.md")`
- **THEN** 系统 SHALL 通过当前 SSH `FileSystemProvider` 调 `fs.read_to_string(<memory_dir>/MEMORY.md)`
- **AND** 返回的 `MemoryFileContent.content` SHALL 是远端文件内容，`filePath` SHALL 以远端 `<remote_home>` 为根
- **AND** SHALL NOT 返回 `ApiError::not_found` 含 "SSH context" 字样的占位错误（旧 graceful skip 文案）

#### Scenario: SSH context 下 add_memory 走远端 write_atomic + 自动创建 memory 目录

- **WHEN** active context 是 `Ssh<host>`，调用方调 `add_memory(project_id, "feedback_test.md", "content")` 且远端 `<memory_dir>` 当前不存在
- **THEN** 系统 SHALL 调 `fs.create_dir_all(<memory_dir>)` 确保目录就绪
- **AND** SHALL 调 `fs.write_atomic(<memory_dir>/feedback_test.md, content.as_bytes())` atomic 写入文件
- **AND** 写入完成后 SHALL 调 `discover_memory_layers(&*fs, &memory_dir)` 拿新 layers 列表
- **AND** 返回的 `ProjectMemory` SHALL 是写入后的最新状态（`hasMemory: true`，新文件出现在 `layers` 中）
- **AND** 远端 fake provider 的 `mkdir_count` SHALL ≥ 1（首次创建 memory 目录）；`write_count` 与 `rename_count` SHALL 各 ≥ 1（atomic write 对 tmp 文件 + rename）

#### Scenario: SSH context 下 delete_memory 走远端 remove_file

- **WHEN** active context 是 `Ssh<host>`，调用方调 `delete_memory(project_id, "feedback_test.md")` 且远端 memory 目录中存在该文件
- **THEN** 系统 SHALL 调 `fs.remove_file(<memory_dir>/feedback_test.md)`
- **AND** 删除完成后 SHALL 调 `discover_memory_layers(&*fs, &memory_dir)` 拿新 layers 列表
- **AND** 返回的 `ProjectMemory` SHALL 不再包含该文件
- **AND** 远端 fake provider 的 `remove_count` SHALL ≥ 1

#### Scenario: SSH context 下 add_memory 文件名校验拒绝路径穿越

- **WHEN** active context 是 `Ssh<host>`，调用方调 `add_memory(project_id, "../etc/passwd", "...")` 或 `add_memory(project_id, "secret.json", "...")`
- **THEN** 系统 SHALL 返 `ApiError::validation`，文案与 `read_memory_file` 路径穿越 / 非 `.md` 拒绝一致
- **AND** SHALL NOT 调任何远端 fs 写方法（`write_count` / `mkdir_count` / `rename_count` SHALL 全 0）

#### Scenario: SSH 写路径 transient 错误重试

- **WHEN** SFTP `write` / `mkdir` / `rename` / `remove` 任一 rpc 返回 `code=4 / EAGAIN / ECONNRESET / ETIMEDOUT / EPIPE`
- **THEN** 系统 SHALL 重试最多 3 次，每次间隔指数退避（75ms × attempt）
- **AND** 仍失败时 SHALL 把错误向上抛给调用方，封装为 `FsError::TransientExhausted { attempts: 3, last_reason }`

#### Scenario: SSH write_atomic rename 失败 best-effort 清理 tmp

- **WHEN** `SshFileSystemProvider::write_atomic(path, content)` 在写完 tmp 后调 SFTP rename 失败（非 transient，已重试 3 次）
- **THEN** 系统 SHALL 调 `fs.remove_file(<tmp_path>)` best-effort 清理 tmp 文件
- **AND** 清理失败 SHALL 不向上传播 error（rename 失败已是 primary error）
- **AND** 向调用方抛 `FsError::TransientExhausted { attempts: 3, last_reason }` 或对应 SFTP error

### Requirement: Keep SSH transport alive via russh keepalive

系统 SHALL 在每次 `ssh_connect` 建立 russh client 时启用 transport 层 keepalive，配置为每 `SSH_KEEPALIVE_INTERVAL = 15s` 距离上次 server 数据后发一次 `SSH_MSG_GLOBAL_REQUEST keepalive@openssh.com`（`want_reply = true`），由 russh client task 内部 keepalive loop 在累计 `SSH_KEEPALIVE_MAX = 3` 之上的连续未应答 tick 后（实际 `(SSH_KEEPALIVE_MAX + 2) × SSH_KEEPALIVE_INTERVAL = 75s` 总窗口，因 russh 0.52.x 的判断 `alive_timeouts > keepalive_max` 是先比较再增加再发送的 off-by-one）主动关闭 transport。

`crates/cdt-ssh/src/session.rs` SHALL 暴露 `pub const SSH_KEEPALIVE_INTERVAL: Duration` 与 `pub const SSH_KEEPALIVE_MAX: usize`，并 SHALL 通过 `build_client_config()` helper 把这两个常量写进 `russh::client::Config`。`connect_inner` 阶段 2 SHALL 调用 `build_client_config()` 而非 `russh::client::Config::default()` 构造握手；`build_client_config` 里其它字段 SHALL 通过 `..Default::default()` 语法保留 russh 默认（`russh::client::Config` 不实现 `Clone` / `PartialEq`，一致性由构造方式保证而非运行时断言）。

启用该机制的目的：(1) **主作用**：每 15s 让 channel 双向有 SSH msg 流动，防止 server-side `ClientAliveInterval=0`（docker openssh 默认）/ NAT idle / firewall idle 把 channel 静默关闭；(2) **次作用**：让对端硬故障（拔网线 / `sshd` 重启）能在 ~75s 内被 client 主动发现，由 russh 关闭 transport，触发既有 `polling watcher` → `dead_signal` → `perform_polling_self_heal_disconnect` 自愈链路（详 `Requirement: Watch remote project directories via SFTP polling` 与 PR #205 实现）。

本 Requirement 仅约束 client config 入参与 connect 路径调用点；keepalive timeout 真触发后的自愈语义仅在**已 attach polling watcher 的 SSH context** 上生效（典型场景：用户 `ssh_connect` 后立即 `switch_context` 触发 `attach_remote_watcher`）。已 connect 但从未 `switch_context` 也从未触发任何 fs IPC 的纯 idle context 在 transport 被 keepalive 关闭后会保留 stale `SshSessionManager::sessions[ctx]` 直到下一次 fs op，属 v1 已知边界（详 design `Risks/已知未覆盖` 段），不属本 Requirement 必须解决。

#### Scenario: build_client_config enables keepalive with documented constants

- **WHEN** 调用 `build_client_config()`
- **THEN** 返回的 `Arc<russh::client::Config>` SHALL 满足 `keepalive_interval == Some(SSH_KEEPALIVE_INTERVAL)` 且 `keepalive_max == SSH_KEEPALIVE_MAX`
- **AND** 实现 SHALL 通过 `russh::client::Config { keepalive_interval, keepalive_max, ..Default::default() }` 语法构造，确保其它字段从 russh `Default::default()` 继承（不引入额外副作用）

#### Scenario: connect_inner uses build_client_config for handshake

- **WHEN** 调用方触发 `ssh_connect` 进入 `connect_inner` 阶段 2 的 `russh::client::connect_stream`
- **THEN** 传入的 config SHALL 由 `build_client_config()` 产出
- **AND** SHALL NOT 是 `russh::client::Config::default()`
- **AND** transport 握手成功后 russh client task 内部 keepalive loop SHALL 按 `SSH_KEEPALIVE_INTERVAL` 周期运行（由 russh 0.52.x 的内部实现保证，本契约只钉死 client config 入参）

#### Scenario: Keepalive timeout closes transport so polling self-heal can run

- **WHEN** 已建立的 SSH context 处于 active 且有 polling watcher attached
- **AND** 对端不再回复任何 SSH 报文（NAT idle close / `sshd` 被 KILL / iptables 丢包）
- **AND** 累计 `SSH_KEEPALIVE_MAX + 1` 个 tick（约 `(SSH_KEEPALIVE_MAX + 2) × SSH_KEEPALIVE_INTERVAL = 75s` 后）keepalive 仍未收到应答
- **THEN** russh client task SHALL 返回 `russh::Error::KeepaliveTimeout` 并关闭 transport
- **AND** 该 context 上后续 SFTP 调用 SHALL 收到 `session closed` / `broken pipe` 类错误，由 `polling_watcher::is_permanent_sftp_failure` 识别为永久错误
- **AND** 累计 `PERMANENT_FAILURE_THRESHOLD` 次后 `dead_signal.notify_one()` 触发 `perform_polling_self_heal_disconnect`，emit `ContextChanged { active_context_id: None, kind: Local }`，与既有自愈链路一致


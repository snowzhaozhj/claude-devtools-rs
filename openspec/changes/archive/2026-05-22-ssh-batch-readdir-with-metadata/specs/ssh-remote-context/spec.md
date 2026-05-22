## MODIFIED Requirements

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

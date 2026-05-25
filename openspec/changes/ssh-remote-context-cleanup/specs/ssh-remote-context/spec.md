# ssh-remote-context Specification Changes

## MODIFIED Requirements

### Requirement: Manage local and SSH contexts

系统 SHALL 暴露"上下文"概念，表示会话数据的来源，分两类：`local`（宿主机文件系统）与 `ssh`（远程主机）。系统 SHALL 提供列出上下文、切换当前上下文、查询当前激活上下文的能力。同一时刻 SHALL 仅有一个上下文处于 `active` 状态；连接新 SSH host 时 SHALL 先断开当前 active SSH context（若存在）再切换到新 host。`Local` 上下文 SHALL 始终在 registry 中存在且不可销毁；`Ssh<host>` 上下文 SHALL 在断开后从 registry 移除。

#### Scenario: 默认 Local 上下文

- **WHEN** 应用启动且无既有 SSH 状态
- **THEN** 当前上下文 SHALL 为 `Local`，绑定本地文件系统 provider

#### Scenario: 切换到 SSH 上下文

- **WHEN** 调用方请求切换到一个已建立的 SSH 上下文
- **THEN** 后续 session discovery 与读取 SHALL 走 SSH 文件系统 provider
- **AND** registry SHALL emit 一条 `context_changed` 事件 `{ activeContextId, kind: "ssh" }`

#### Scenario: 在已有 SSH 上下文活跃时连接新 host

- **WHEN** active context 是 `ssh-host-A`，调用方请求连接到 `host-B`
- **THEN** 系统 SHALL 先断开 `host-A`，等其状态切到 `disconnected`
- **AND** 再发起 `host-B` 连接握手，成功后切 active context 为 `ssh-host-B`
- **AND** registry SHALL emit 两条事件：`context_changed { activeContextId: "ssh-host-B" }` 与 `ssh_status { contextId: "ssh-host-A", status: "disconnected" }`

#### Scenario: Local 上下文不可销毁

- **WHEN** 调用方尝试从 registry 移除 `Local` context
- **THEN** 操作 SHALL 被拒绝并返回结构化错误 `code: invalid_operation`
- **AND** registry SHALL 仍保留 `Local` context

### Requirement: Establish and tear down SSH connections

系统 SHALL 通过 SSH 连接到远程主机，连接时 SHALL 在 `~/.ssh/config` 存在的情况下读取主机元数据；SHALL 支持显式断开与应用退出时的优雅断开。连接 SHALL 走真协议栈（非占位实现），完成以下五个阶段：TCP probe（5s 超时）→ SSH transport 握手 → 鉴权候选链尝试（详 `Requirement: SSH authentication candidate chain`）→ SFTP subsystem open（8s 超时）→ remote home probe；总外层硬超时 SHALL 为 25s。任一阶段失败 SHALL 返回结构化 `SshError`（详 `Requirement: Structured SSH error classification`）。

#### Scenario: 通过 ssh config alias 连接

- **WHEN** 调用方请求连接到 `~/.ssh/config` 中已定义的 alias
- **THEN** 系统 SHALL 先解析 SSH config 拿到 hostname / user / port / IdentityFile / IdentityAgent
- **AND** 用解析结果建立 TCP + SSH transport
- **AND** 按鉴权候选链尝试到第一个成功源
- **AND** 连接 SHALL 被登记为新的 `Ssh<host>` context，状态切到 `connected`

#### Scenario: 测试连通性不持久化

- **WHEN** 调用方请求测试连通性（`ssh_test_connection`）
- **THEN** 系统 SHALL 走与 `ssh_connect` 相同的握手流程
- **AND** 成功后 SHALL 立即关闭 SSH session，不向 registry 注册新 context
- **AND** 返回值 SHALL 包含 `authChain[]` 让 UI 可显示"试过哪些候选源"诊断

#### Scenario: 断开

- **WHEN** 调用方断开一个已激活的 SSH 上下文
- **THEN** 系统 SHALL 关闭 SFTP channel + SSH transport + TCP socket
- **AND** 该 context 的 polling watcher（若已启动）SHALL 被停止
- **AND** 后续从该上下文的读取 SHALL 以 `code: not_connected` 错误失败
- **AND** 若被断开的是 active context，registry SHALL 自动把 active 切回 `Local`

#### Scenario: 应用退出时优雅断开

- **WHEN** 应用收到关闭信号
- **AND** 当前有 N 个已注册 SSH context（N >= 1）
- **THEN** 系统 SHALL 对每个 SSH context 并发断开，最长等待 3s
- **AND** 应用 SHALL NOT 被某个 context 的断开阻塞超过 3s

### Requirement: Read sessions and files over SSH with same contract

系统 SHALL 在 SSH 上下文上提供与 local 上下文等价的 `project-discovery`、`session-parsing`、文件读取能力，使下游消费者观察到完全相同的数据形状。SSH 文件系统 provider SHALL 实现 `FileSystemProvider` trait 的所有方法（`exists` / `read_to_string` / `read_dir` / `read_dir_with_metadata` / `stat` / `stat_many` / `read_lines_head` / `open_read`），底层走标准 SFTP 协议；SHALL NOT 在远端 spawn 任何工作进程，唯一允许在远端执行的命令是用于探测 remote home 的 `printf` 调用。

`open_read` SHALL 是 trait 方法（而非具体类型上的 inherent 方法）——返回异步流式读取句柄让调用方不需 downcast 到具体 provider 类型就能流式读。`stat_many` SHALL 实现为 trait default（基于 `stat` 的并发包装）；由于底层 SFTP session 全锁串行，当前 SSH `stat_many` 仍是 N 次串行 RTT（**已知限制**），真正的 SFTP message-id 并发 pipeline 留独立后续 change（保持"无远端 shell 依赖"架构假设）解决。trait API 先就位让 caller 一律调 `stat_many` 而非循环 `stat`。

**`read_dir_with_metadata` SSH override**：SSH 文件系统 provider SHALL override `read_dir_with_metadata` 直接 delegate 到 `read_dir(path)`——底层 SFTP READDIR reply 单 RTT 返完整 dir 内容 + 每个 file entry 的 attrs（size/mtime）；缺 mtime 的 entry SHALL 在 metadata = None 状态返给 caller，由上层 batch 校验语义视同 cache mismatch 走 cache wrapper miss 路径补齐——实现 SHALL NOT 在 trait 实现层做 per-entry stat fallback（避免 N+1 RTT 退化）。

**SSH list 路径性能契约**：朴素 per-session 串行 stat 验 cache signature 在 SSH 上受底层 SFTP session 全锁串行约束会显著超 sidebar 首屏预算。本 capability SHALL 让 SSH list 路径走以下三件套：

- **G. cache hit trust**：用户切回已访问过的 SSH host → UI 立刻拿 in-memory cache 内容渲染列表（**0 fs op**），不等任何 fs.stat RTT
- **D. SkeletonThenStream**：list_sessions SSH 路径与 Local 路径同走骨架 spawn 模型；首屏返骨架 + cache trust 内容，metadata diff 通过 SSE event 异步推送。后台 scan 通过统一 dispatcher 分流：Local 走 per-session via fs trait；SSH 走批量 readdir 路径
- **E. read_dir_with_metadata batch**：后台 batch 校验 task SHALL 走 `fs.read_dir_with_metadata(project_dir)` per project（SFTP READDIR reply 含 entry attrs，单 RTT 拿全 dir metadata），对每条 session SHALL 通过 metadata cache 的"已知 signature"接口直接命中跳 stat；mismatch / 新增 / dir read 失败 SHALL 走原 cache miss 路径（cache wrapper 内部 stat + scanner）→ 命中条与 mismatch 条都通过 `session_metadata_update` SSE event 推差量

**SSH 大会话 scanner buffer 上限**：scanner（metadata 提取与 file parse）SHALL 通过文件系统 provider 的 `open_read` 拿异步流式读取句柄，再用 BufReader 包装。Buffer 容量 SHALL 与底层 SFTP READ reply 单消息上限对齐——不强制每次 fill 跑多次底层 READ（无收益反而多一层 alloc），也不使用过小默认值（在 SSH 上 RTT 数过多）。

#### Scenario: 列出远端 host 上的项目

- **WHEN** 当前上下文是 SSH，调用方请求项目列表
- **THEN** 返回结果 SHALL 与本地项目列表形状一致，数据源为远程 `<remote_home>/.claude/projects/` 目录

#### Scenario: 读取远端会话

- **WHEN** 当前上下文是 SSH，调用方请求会话详情
- **THEN** 系统 SHALL 通过文件系统 provider 的 `open_read` 流式读取远程 JSONL 文件
- **AND** 返回与本地输出形状一致的 chunk 序列

#### Scenario: 调用方通过 trait 句柄即可流式读远端文件

- **WHEN** caller 持文件系统 provider 的 trait 对象句柄指向 SSH provider
- **THEN** caller SHALL 能直接调 `fs.open_read(path).await?` 拿到异步流式读取句柄
- **AND** SHALL NOT 需要 downcast 到具体 SSH provider 类型才能流式读

#### Scenario: SSH 批量 stat 退化为顺序 RTT 是已知限制

- **WHEN** caller 在 SSH 模式下对 N 条路径调 `fs.stat_many`
- **THEN** 实现 SHALL 使用 trait default 并发包装，返回顺序对应的结果 vec
- **AND** 由于 SFTP session 全锁，实际执行是 N 次串行 RTT —— 此限制属已知，留后续 change 解决；trait 契约层面 caller SHALL 一律调 `stat_many` 而非循环 `stat`

#### Scenario: 多 fallback 候选解析远端 home

- **WHEN** 远端 `<home>/.claude/projects` 不存在，但 `/home/<user>/.claude/projects` 或 `/Users/<user>/.claude/projects` 或 `/root/.claude/projects` 存在
- **THEN** 系统 SHALL 按上述顺序探测候选路径并使用第一个存在的
- **AND** 全部不存在时 SHALL 返回 `SshError::RemoteHomeMissing { tried }` 错误，状态切到 `error`，不切换 active context，但 `ssh_get_state` SHALL 保留该 context 的错误状态与已完成的 `authChain` 诊断

#### Scenario: SFTP 瞬时错误自动重试

- **WHEN** SFTP 调用返回瞬时错误码（覆盖 SFTP `code=4` / `EAGAIN` / `ECONNRESET` / `ETIMEDOUT` / `EPIPE`，与 polling watcher 的 `OtherTransient` 集合保持对称）
- **THEN** 系统 SHALL 重试最多 3 次，每次间隔指数退避
- **AND** 仍失败时 SHALL 把错误向上抛给调用方，封装为 `FsError::TransientExhausted { attempts: 3, last_reason }`

#### Scenario: 切回已访问 SSH host 列表立刻显示无可感知卡顿

- **WHEN** 用户在 SSH active context 下调 `list_sessions`，cache 中持有该 ContextId 的 entry
- **THEN** UI 渲染路径 SHALL 走 metadata cache 的 "trust cached" 命中接口返 cache 内容（**0 fs op**：无 `fs.open_read` / `fs.read_to_string` / `fs.stat`）
- **AND** UI 立刻拿 in-memory cache 内容渲染列表（与 Local SkeletonThenStream 路径同入口）
- **AND** 后台 spawn 异步 scan task per project 异步校验 cache freshness——SSH backend 路径 SHALL 走 batched 路径（先 `fs.read_dir_with_metadata(project_dir)` 单 RTT 拿全 dir metadata，对每条 session lookup 已知 signature 命中跳 stat）
- **AND** 外部进程改动 → mismatch → 通过 `session_metadata_update` SSE event 推差量给 UI

#### Scenario: SSH list 路径冷启动走 SkeletonThenStream

- **WHEN** 用户首次连 SSH host A 调 `list_sessions`，cache 中无该 ContextId 的 entry
- **THEN** UI 首屏 SHALL 返 SessionSummary 骨架（title=None / message_count=0），不阻塞等待 metadata
- **AND** spawn 后台 scan task；SSH backend 走 batched 路径——先 `fs.read_dir_with_metadata(project_dir)` 单 RTT 拿全 dir entries metadata；冷启动场景 cache 全 miss → 全部 mismatch → 走 cache wrapper miss 路径异步刷新 metadata
- **AND** 每条 metadata 通过 `session_metadata_update` SSE event 推给 UI 增量填充

#### Scenario: SSH 后台 batch 校验 fs op 形态钉死（all-hit）

- **WHEN** SSH context 下 batch 校验 task 被 spawn，包含 N 条 session（全部 cache hit byte-equal，且 dir metadata 含每条对应 path）
- **THEN** fs op 调用 SHALL 仅含 1 次 `fs.read_dir_with_metadata(project_dir)`（拿全 dir entry metadata）
- **AND** `fs.stat` 调用次数 SHALL = 0；`fs.open_read` 调用次数 SHALL = 0；`fs.read_to_string` 调用次数 SHALL = 0
- **AND** 命中条 SHALL 通过 `session_metadata_update` SSE event 推 cache 现值（SSH 跳 stale check 与 SSH 路径 cache wrapper 一致）
- **AND** 总 fs op = 1，对比 per-session 路径 N stat 显著节省

#### Scenario: SSH 后台 batch 校验 fs op 形态（partial-hit）

- **WHEN** 包含 H 条 cache hit + M 条 cache mismatch / 新增 / dir metadata 缺该 path（H + M = N，其中 mismatch 包含 `DirEntry.metadata = None` 的 mtime_missing 条）
- **THEN** H 条命中条 SHALL 通过"已知 signature"接口命中直 broadcast，不调任何 fs op
- **AND** M 条 mismatch 条 SHALL spawn 单 sub-task 走 cache wrapper miss 路径（per-task 1 `fs.stat` + 1 `fs.open_read`），共 2M 次 fs op
- **AND** 总 fs op SHALL = 1（batch read_dir_with_metadata）+ 2M
- **AND** 在大多数 hit 占比的典型负载下，相对 per-session N 次 stat 路径有显著节省

#### Scenario: SSH 后台 batch 校验 fs op 形态（all-miss）

- **WHEN** 包含 N 条全 mismatch（典型冷启动场景：cache 全空）
- **THEN** 总 fs op SHALL = 1（batch read_dir_with_metadata）+ 2N（per-mismatch sub-task）
- **AND** 对比 per-session 路径多一次额外 RTT
- **AND** 此一次额外 RTT 是接受 trade-off：换取 partial-hit / all-hit 场景的 RTT 节省；冷启动场景用户感知仍走 SkeletonThenStream（hot path 用骨架渲染 + 后台异步刷），后台 batch 不影响首屏

#### Scenario: SSH batch helper 在 dir read 失败时 fallback 到 per-session 路径

- **WHEN** SSH context 下 batch 校验 task 调 `fs.read_dir_with_metadata(project_dir)` 返 `Err`（瞬时网络抖动 / dir 被删等）
- **THEN** 函数 SHALL 走 fallback 路径调既有 per-session 扫描继续异步刷新——保证功能正确性，性能退化为既有形态
- **AND** SHALL 写入运维侧可见的结构化警告日志

#### Scenario: SSH 同 session 二次 get_tool_output cache hit byte-equal

- **WHEN** 在 SSH context 下首次调 `get_tool_output(root, sid, tu_a)` 完成 cache 写入；session 文件未变后调 `get_tool_output(root, sid, tu_b)`（同 session，不同 tool_use_id）
- **THEN** 第二次调用 SHALL 走 cached parse 内部 `fs.stat(path)` 拿当前 signature + cache lookup；signature byte-equal 直接复用 cache 中已 parse 的 ParsedMessage 序列，**SHALL NOT 触发文件重 parse**（即 `fs.open_read = 0`）
- **AND** 形态：`fs.stat = 1`、`fs.open_read = 0`、`fs.read_to_string = 0`、parse 调用次数 = 0
- **AND** Note：纯 0 fs op 的 ParsedMessage cache trust + 后台 stat 校验设计与 batch readdir 解耦，留独立后续 change wire 入 get_tool_output / get_image_asset

#### Scenario: SSH 远端 jsonl 真改动后 cache invalidate 走 batch 校验

- **WHEN** 用户在 SSH context 下访问 session A 写入 cache；外部进程追加该 jsonl 内容；用户再次访问 list_sessions
- **THEN** UI 立刻拿旧 cache 内容渲染（hot path 0 fs op，via "trust cached" 接口）
- **AND** 同时 spawn 后台 scan task per project_dir → SSH backend 走 batched 路径，`fs.read_dir_with_metadata` 拿到新 metadata（mtime/size 已变）；"已知 signature"接口比对 mismatch → spawn 单 task 走 cache wrapper miss 路径（`fs.stat` 拿新 signature → `fs.open_read` 重 parse）
- **AND** 每条改动通过 `session_metadata_update` SSE event 推 metadata diff 给 UI 增量更新（用户感知"列表先出但短延迟后内容自动 refresh"）

#### Scenario: SSH disconnect 中间态 user-facing IPC 返 not_found 而非降级 Local

- **WHEN** 用户在 SSH context A active 时调用 `get_session_detail(sid)` / `get_tool_output(...)` / `get_image_asset(...)` 等 user-facing IPC handler；调用过程中并发触发 `ssh_disconnect("A")` 让 active context 进入"None active 但旧 SSH provider 仍在 sessions registry"中间态
- **THEN** handler 内部 SHALL 通过原子取 fs/ctx 接口拿三元组同快照
- **AND** 该 helper SHALL 返 `Err(ApiError::not_found)` 而非降级到 Local provider（避免 user 在 SSH 视角下意外拿到 Local 同 sid 的内容）
- **AND** 用户后续 reconnect 同 host A，cache 中先前写入的 entry SHALL 复用

#### Scenario: SSH 后台 batch 校验 task 在 ssh_disconnect 时 abort

- **WHEN** 用户在 SSH context A 下调 `list_sessions` spawn 多个后台 scan task（per project_dir，SSH 路径走 batched 路径）；调用过程中触发 `ssh_disconnect("A")`
- **THEN** 所有该 ssh ctx 下的顶层 batch task SHALL 通过 active scans registry 的 per-key abort handle 被 abort
- **AND** 顶层 batch task 内部并发任务集合持有的 mismatch sub-task SHALL 随集合 drop 自动 abort，SHALL NOT 再向 metadata broadcast 发出旧 ctx update
- **AND** 后续用户切回该 host reconnect，新 batch task 用新快照启动；旧 task 的部分写入 cache（不同 ContextId）SHALL NOT 串扰新 task

#### Scenario: SSH 大会话 scanner BufReader 容量与 SFTP packet 对齐

- **WHEN** SSH context 下 cache miss 后调 metadata 提取或 file parse 扫描大 jsonl 文件
- **THEN** 实现 SHALL 用与底层 SFTP READ 单消息上限对齐的 BufReader 包装 `fs.open_read` 返回的异步流式读取句柄
- **AND** 单 BufReader fill SHALL = 单底层 SFTP READ message
- **AND** SHALL NOT 用过大 buffer（强制底层拆多次 READ 无收益）
- **AND** SHALL NOT 用过小默认值（大 jsonl 的 RTT 数在 SSH 上不可接受）

#### Scenario: SSH cache miss 路径 fs op 形态钉死

- **WHEN** cache miss 触发 SSH 端单 file scan
- **THEN** fs op 调用 SHALL 仅含：1 次 `fs.stat`（前置 signature 拿，由 cache wrapper 自动处理）+ 1 次 `fs.open_read` 拿 reader + 内部底层 SFTP read（不计入 fs trait 公开 op）
- **AND** SHALL NOT 出现 `fs.read_to_string` 全文调用（该路径会绕过流式状态机，把全文装入内存）

### Requirement: Report SSH connection status

系统 SHALL 暴露每个已配置 SSH 上下文的连接状态（`disconnected` / `connecting` / `connected` / `error`），错误状态 SHALL 附带可读的错误说明与结构化错误分类。状态 SHALL 通过事件订阅 channel 推送给订阅者（HTTP SSE / 桌面端事件桥），订阅者多次订阅 SHALL 各自独立收到事件。`connecting` 状态 SHALL 携带 `authChain` 进度（已尝试源列表，便于 UI 显示"正在尝试 IdentityFile..."）。

#### Scenario: 查询失败上下文的状态

- **WHEN** 某个 SSH 上下文连接失败
- **THEN** 状态查询 SHALL 返回 `error` 与底层错误信息（`SshError` 序列化结果）
- **AND** 错误信息 SHALL 含 `authChain[]`（每个候选源的 source/outcome/elapsed_ms）

#### Scenario: 状态广播给多订阅者

- **WHEN** 一个 SSH 连接状态从 `connecting` 切到 `connected` 且当前有两个订阅者
- **THEN** 两个订阅者 SHALL 各自**恰好**收到一次 `SshStatusChange { contextId, status: "connected" }`
- **AND** 任一订阅者的滞后 SHALL NOT 影响另一订阅者投递

#### Scenario: connecting 状态携带鉴权进度

- **WHEN** 系统正在尝试鉴权候选链的第 3 个候选（IdentityFile）
- **THEN** `ssh_get_state` SHALL 返回 `status: "connecting"` 与 `authChain` 含前 2 个候选的 outcome（已 Skipped / Failed）

### Requirement: SSH authentication candidate chain

系统 SHALL 在 SSH 握手鉴权阶段按以下顺序构建候选源并依次尝试：(1) ssh config `IdentityAgent` 字段（来自 SSH config 解析结果，仅当字段非空且非 `none` 时启用）—— 把字段值视作 unix socket 路径直接连接，**优先于** `SSH_AUTH_SOCK` env 与 IdentityFile 文件直读，与 OpenSSH 行为对齐；(2) `SSH_AUTH_SOCK` 环境变量指向的 unix socket；(3) macOS 平台 `launchctl getenv SSH_AUTH_SOCK` 返回的 socket 路径；(4) 1Password well-known socket，依次尝试 `~/Library/Group Containers/2BUA8C4S2C.com.1password/t/agent.sock` 与 `~/.1password/agent.sock`（仅当候选 (1) 没有显式给出 1Password socket 路径时作为兜底，避免重复尝试同一 agent）；(5) 来自 SSH config 解析的 `IdentityFile` 候选私钥（按列出顺序）；(6) 默认私钥位置 fallback：`~/.ssh/id_ed25519` → `id_rsa` → `id_ecdsa`；(7) 仅当用户在 UI 选择 `password` auth method 时尝试 password 鉴权。每个候选 SHALL 在结果中记录为 `AuthAttempt { source, outcome, elapsedMs }`（camelCase 序列化）。任一候选成功 SHALL 立即停止尝试后续候选；全部失败 SHALL 返回 `SshError::AuthExhausted { attempts }`。系统 SHALL NOT 在 v1 中尝试 Linux gnome-keyring agent / Windows named pipe agent / 加密私钥 passphrase 弹窗——这三类 SHALL 在 v1 中明确标记为不支持。

#### Scenario: ssh config 中的 IdentityAgent 字段优先

- **WHEN** 用户 `~/.ssh/config` 含 `IdentityAgent ~/Library/Group Containers/2BUA8C4S2C.com.1password/t/agent.sock`
- **AND** 进程环境同时有 `SSH_AUTH_SOCK=/tmp/standard-agent.sock`
- **THEN** 鉴权候选链 SHALL 把 `IdentityAgent` 字段对应的 1Password socket 作为候选 (1) 优先尝试
- **AND** 仅当候选 (1) 失败时才会尝试候选 (2)（env agent）

#### Scenario: macOS Launchpad 启动应用使用 launchctl SSH_AUTH_SOCK

- **WHEN** 应用从 macOS Launchpad / Dock 启动，进程环境变量无 `SSH_AUTH_SOCK`
- **AND** ssh config 也未指定 `IdentityAgent`
- **AND** `launchctl getenv SSH_AUTH_SOCK` 返回 `/private/tmp/com.apple.launchd.xxx/Listeners`
- **THEN** 鉴权候选链 SHALL 把该路径作为候选 (3) 并尝试连接
- **AND** 即使候选 (1)(2) 失败，候选 (3) 成功也 SHALL 让连接进入 `connected` 状态

#### Scenario: 1Password agent socket 发现

- **WHEN** 用户使用 1Password 管理 SSH 密钥
- **AND** `~/Library/Group Containers/2BUA8C4S2C.com.1password/t/agent.sock` 文件存在
- **THEN** 鉴权候选链 SHALL 把该 socket 作为候选 (4) 尝试

#### Scenario: agent 不可用时 IdentityFile fallback 链

- **WHEN** 候选 (1)(2)(3) 全部失败（agent 不可用）
- **AND** SSH config 解析结果含 `identityfile ~/.ssh/work_key` 与 `identityfile ~/.ssh/personal_key`
- **THEN** 候选链 SHALL 依次尝试 `~/.ssh/work_key` 和 `~/.ssh/personal_key`
- **AND** 每个文件 SHALL 调用密钥解码方法（不带 passphrase）；返回 passphrase-required 时 SHALL 跳过并记录 `AuthOutcome::Skipped("requires passphrase, use ssh-add")`

#### Scenario: 全部候选耗尽

- **WHEN** 所有 7 个候选都失败或被跳过
- **THEN** 系统 SHALL 返回 `SshError::AuthExhausted { attempts }` 含每个候选的详细 outcome
- **AND** UI SHALL 能从 `attempts[]` 渲染"7 个候选都失败：xxx"诊断

#### Scenario: AuthAttempt 序列化形态

- **WHEN** `AuthExhausted { attempts }` 通过 IPC 跨边界序列化为 JSON
- **THEN** 每条 `AuthAttempt` SHALL 序列化为 `{ "source": { "type": "<variant>", "data"?: ... }, "outcome": { "type": "<variant>", "data"?: ... }, "elapsedMs": <u64> }` 形态
- **AND** `AuthSource` enum 序列化样例：`{ "type": "identityAgent", "data": "/path/to/agent.sock" }` / `{ "type": "envAgent" }` / `{ "type": "launchctlAgent" }` / `{ "type": "onePasswordAgent", "data": "/path/to/socket" }` / `{ "type": "identityFile", "data": "/Users/alice/.ssh/work_key" }` / `{ "type": "defaultKey", "data": "/Users/alice/.ssh/id_ed25519" }` / `{ "type": "password" }`
- **AND** `AuthOutcome` enum 序列化样例：`{ "type": "success" }` / `{ "type": "failure", "data": "Permission denied" }` / `{ "type": "skipped", "data": "requires passphrase, use ssh-add" }`
- **AND** 字段名 SHALL 是 camelCase（`elapsedMs`，**非** `elapsed_ms`）

#### Scenario: 加密私钥无 agent 时跳过不崩

- **WHEN** 候选 (5) 中某个 IdentityFile 是 passphrase 加密私钥
- **AND** 该候选的 source 不是 agent（直接读文件路径）
- **THEN** 系统 SHALL 跳过该候选并记录 `Skipped("requires passphrase, use ssh-add")`
- **AND** 继续尝试下一个候选，SHALL NOT 弹出 passphrase UI

#### Scenario: Windows v1 受限鉴权模式

- **WHEN** 当前平台是 Windows
- **THEN** 鉴权候选链 SHALL 跳过候选 (3)（macOS launchctl）和 (4)（1Password 路径）
- **AND** v1 SHALL NOT 尝试 named pipe ssh-agent（`\\.\pipe\openssh-ssh-agent`），即使该 pipe 可用
- **AND** 候选 (1)(2)(5)(6)(7) 仍正常工作（IdentityAgent / env agent / IdentityFile / 默认密钥 / password）

### Requirement: Resolve SSH host alias via `ssh -G`

系统 SHALL 通过子进程调用 `ssh -G <host>` 解析 SSH config 高级特性（`Include` / `Match` / `ProxyJump` / `IdentityAgent` 等）。子进程 SHALL 设置短超时；超时或非零 exit 时 SHALL 降级到内部 SSH config 基本字段解析（仅支持 `Host` / `HostName` / `Port` / `User` / `IdentityFile`）。SSH config 解析器 SHALL 仅承担"列出所有 Host alias"用于 UI combobox 联想，不复刻 SSH config 复杂语法。

`ssh -G` 解析输出 SHALL 提取以下字段并填入 `ResolvedHost`：`hostname` / `port` / `user` / `identityfile`（多个）/ `identityagent` / `proxyjump` / `proxycommand` / `hostkeyalias`。其中 `proxyjump` / `proxycommand` / `hostkeyalias` 是为 host signature 计算服务的——这三个字段直接影响"是否同一远端机器"判定，cache 不得跨这些差异复用。

退化路径（fallback 解析器兜底）SHALL 把 `proxyjump` / `proxycommand` / `hostkeyalias` 设为 `None`，但**不**阻塞 host signature 计算——signature 仍可基于 `hostname` / `port` / `user` / `identityfile` 计算（degraded 模式下 cache 范围略宽，但不会跨 host 串扰）。

#### Scenario: 通过系统 ssh -G 解析 alias

- **WHEN** 调用方请求 `ssh_resolve_host("myserver")`
- **AND** 系统有 `ssh` 二进制
- **THEN** 系统 SHALL spawn `ssh -G myserver`，从 stdout 解析得到 hostname / port / user / identityfile / identityagent / **proxyjump / proxycommand / hostkeyalias** 等字段
- **AND** 返回 `ResolvedHost` 含以上**所有**字段（缺失字段为 `None` / 空 Vec）

#### Scenario: ssh 二进制缺失或失败时 fallback

- **WHEN** 系统无 `ssh` 二进制（如 Windows 未启用 OpenSSH client）
- **OR** `ssh -G` 子进程超时 / 非零 exit
- **THEN** 系统 SHALL 降级到内部 SSH config 基本字段解析
- **AND** 返回结果 SHALL 标记 `degraded: true`（UI 可据此显示"高级 SSH config 特性不可用"提示）
- **AND** `proxyjump` / `proxycommand` / `hostkeyalias` SHALL 为 `None`（degraded 模式不解析这些字段）

#### Scenario: degraded 模式仍可计算 host signature

- **WHEN** `ssh -G` 失败，`ResolvedHost.degraded == true`
- **AND** 调用方据此 ResolvedHost 计算 host signature
- **THEN** SHALL 成功产 `config_digest`，输入字段中 `proxyjump` / `proxycommand` / `hostkeyalias` 为 `None`
- **AND** SHALL NOT 阻塞 `ssh_connect` 流程

#### Scenario: 列出所有 host alias 供 UI combobox

- **WHEN** 调用方请求 `ssh_get_config_hosts()`
- **THEN** 系统 SHALL 解析 `~/.ssh/config` 提取所有非通配符 Host alias 列表
- **AND** SHALL NOT spawn `ssh -G`（该接口仅 list，无需高级特性解析）
- **AND** 文件不存在时 SHALL 返回空列表，不报错

### Requirement: Watch remote project directories via SFTP polling

系统 SHALL 在 SSH context 处于 `connected` 状态时启动远端文件变更感知 polling watcher：每 3 秒调用一轮 SFTP `read_dir(<remote_home>/.claude/projects/<project_id>/)` + 对每个 `.jsonl` 文件 `stat` 取 size 与 mtime，与上轮快照比较差异（新增 / size 变化 / 删除）后通过与本地 watcher 相同的 `FileChangeEvent` schema 广播事件。第一次 poll SHALL 不触发任何事件（建参照快照用）。系统 SHALL 额外每 30 秒运行一次 catch-up 比较作为兜底。SHALL 在 `ssh_disconnect` 时停止 watcher 与释放 SFTP 资源。

watcher SHALL 把每轮 poll 失败按错误特征分到三类（在 polling 层 重试机制完成后做语义升级，**不**改 SFTP client trait 错误分类）：

- `Permanent`：错误消息含 `session closed` / `eof` / `broken pipe` / `epipe` / `connection reset` / `econnreset` 任一关键字（不区分错误来源——既有 transient 分类把 `broken pipe` / `connection reset` / `epipe` 归 Transient，重试机制多次后仍是 transport-dead 即视同 channel 真死）
- `Timeout`：错误消息含 `timeout` / `etimedout` / `timed out` / `eagain` / `would block` 任一关键字（含 `would block` 即标准库的 `WouldBlock`，与 EAGAIN 同源——不纳入 timeout 类会让"反复 WouldBlock"序列只能落 OtherTransient 重置计数，与 timeout 漏检对称）
- `OtherTransient`：其它 Transient / Other / NoSuchFile / PermissionDenied 等不带 transport-dead / timeout 关键字的失败

watcher SHALL 维护两个独立 counter：

- `consecutive_permanent`，阈值 `PERMANENT_FAILURE_THRESHOLD = 3`（约 9s 持续 transport 错误）
- `consecutive_timeout`，阈值 `TIMEOUT_FAILURE_THRESHOLD = 6`（约 18s 持续 timeout，远高于网络瞬时抖动 1-3s 窗口，远低于用户主观放弃 sidebar 僵死的 60s 阈值）

counter 演化规则（避免攻击序列推迟 dead_signal）：

- `Ok` / `OtherTransient`：两 counter 都 SHALL reset 为 0（唯一 reset 入口；只有"channel 真活着"的强证据才清零）
- `Permanent`：仅 `consecutive_permanent` 自增，**不动** `consecutive_timeout`
- `Timeout`：仅 `consecutive_timeout` 自增，**不动** `consecutive_permanent`

任一 counter 达自己阈值时，watcher SHALL 通过 dead 信号通知 + 跳出主 loop。

理由：早期"互斥重置"规则被 `5T → 1P → 5T → 1P → ...` 攻击序列利用让 timeout 永不达阈值；新规则下 dead-向量单调累积，攻击序列只能拖延无法阻止——`5T + 1P` 后下一轮 `1T` 即触发。

`scan_once` 内 sub-project 子目录 `read_dir` 失败时：

- `NoSuchFile` / `PermissionDenied`：silent skip 该 project（保持现有容错）
- 其它错误经分类——`Permanent` SHALL 让整个 `scan_once` 返 Err escalate 到顶层 counter（避免 sub-project channel-dead 错误被静默吞掉、watcher 误以为快照完整后下轮报"全部 session deleted"事件）；`Timeout` / `OtherTransient` 仍 silent skip 该 project，留下次 catch-up 重试

#### Scenario: 第一次 poll 建参照快照 不触发事件

- **WHEN** SSH context 刚切到 `connected` 状态，watcher 启动后第一次 poll
- **AND** 远端项目目录有 5 个 session JSONL 文件
- **THEN** watcher SHALL NOT emit 任何 `FileChangeEvent`
- **AND** 内部参照快照 SHALL 含 5 个条目（每条 path → fingerprint）

#### Scenario: 后续 poll 检测 size 变化

- **WHEN** 第二次 poll 中某文件 size 增长
- **THEN** watcher SHALL emit 一条 `FileChangeEvent { project_id, session_id, deleted: false }`
- **AND** 快照中该文件 fingerprint SHALL 被更新

#### Scenario: 断开时停止 polling

- **WHEN** 用户调 `ssh_disconnect`
- **THEN** 该 context 的 polling task SHALL 在 1s 内退出（cancellation token）
- **AND** SFTP channel SHALL 被关闭

#### Scenario: watcher 在阈值内容忍短暂 SFTP 瞬时错误

- **WHEN** 某轮 poll 中 `read_dir` 返回瞬时 timeout 错误（`Transient("timeout")`）
- **AND** `consecutive_timeout` 累计未达阈值
- **THEN** watcher SHALL 跳过本轮，下一轮再尝试
- **AND** SHALL NOT 因单次失败而停止 watcher 或断开 SSH
- **AND** SHALL NOT 触发 dead 信号

#### Scenario: 持续 timeout 达阈值后触发 dead 信号

- **WHEN** SFTP `read_dir` 连续 6 轮 poll 都返 `Transient("timeout")` 类错误（典型场景：远端 SFTP 协议层 hang 但 TCP 未断）
- **THEN** watcher SHALL 在第 6 轮后通过 dead 信号通知并跳出主 loop
- **AND** 触发上层 monitor task 走自愈断开把 active context 切回 `Local`
- **AND** wall time SHALL ≈ 18s（6 × 3s POLL_INTERVAL），远低于用户主观放弃 SFTP 超时的 30s 阈值

#### Scenario: 永久 transport 错误达阈值后触发 dead 信号

- **WHEN** SFTP `read_dir` 连续 3 轮 poll 都返含 `session closed` / `broken pipe` / `connection reset` 等 transport-dead 关键字的错误（无论错误变体来源）
- **THEN** watcher SHALL 在第 3 轮后通过 dead 信号通知并跳出主 loop
- **AND** wall time SHALL ≈ 9s（3 × 3s POLL_INTERVAL）

#### Scenario: 中间出现成功时 timeout counter 重置

- **WHEN** 已累计若干轮 timeout 后下一轮 `read_dir` 成功
- **THEN** `consecutive_timeout` SHALL 立即 reset 为 0
- **AND** 后续即使再来同样轮次的 timeout 也 SHALL NOT 触发 dead 信号（因为新 streak 未达阈值）

#### Scenario: 永久与超时 counter 独立累积（混合序列仍能触发）

- **WHEN** 多轮 timeout 后来一轮 permanent
- **THEN** `consecutive_timeout` SHALL 保持不变（不被 permanent 重置）；`consecutive_permanent` SHALL 自增
- **AND** 后续再一轮 timeout SHALL 让 `consecutive_timeout` 达阈值 → 立即触发 dead 信号
- **AND** 反向同理：多轮 permanent + 一轮 timeout 后两 counter 各自累积，下一轮 permanent 让 `consecutive_permanent` 达阈值 → 触发
- **AND** 攻击序列 `5T → 1P → 5T → 1P → ...` SHALL **不能**永远推迟 dead 信号——任一 dead 向量单调累积

#### Scenario: OtherTransient 错误不触发 dead 信号

- **WHEN** 连续多轮 poll 返 `Transient("EAGAIN")` 等不含 transport-dead 与 timeout 关键字的错误
- **THEN** 两 counter 都 SHALL reset 为 0（OtherTransient 不计任一计数）
- **AND** SHALL NOT 触发 dead 信号
- **AND** watcher SHALL 持续运行等下一轮恢复

#### Scenario: sub-project read_dir 永久错误升级到 scan_once 失败

- **WHEN** 顶层 `read_dir(<remote_home>/.claude/projects/)` 成功，但其中一个 sub-project `read_dir(<base>/<project_id>/)` 返永久错误
- **THEN** `scan_once` SHALL 立即 return Err 而非 silent skip 该 project
- **AND** 外层 polling loop 经分类把该错误归 `Permanent` → 永久失败计数自增
- **AND** 连续多轮 sub-project 永久错误达阈值 SHALL 触发 dead 信号

#### Scenario: sub-project read_dir 超时 / NoSuchFile 仍 silent skip

- **WHEN** 顶层 `read_dir` 成功，sub-project A 返 `NoSuchFile`，sub-project B 返 `Transient("timeout")`，sub-project C 成功
- **THEN** `scan_once` SHALL 跳过 A 与 B，处理 C 后正常返 `Ok`
- **AND** 快照仅含 C 的条目（A / B 缺失视同未变更，下轮 catch-up 自然重试）
- **AND** 不 escalate 任何错误到外层 counter

### Requirement: Structured SSH error classification

系统 SHALL 把所有 SSH 失败场景归类到结构化 `SshError` enum：`Tcp`（TCP probe 失败）/ `AuthExhausted`（鉴权候选链全部失败）/ `SftpInit`（SFTP subsystem open 失败）/ `RemoteHomeMissing`（远端 `~/.claude/projects` 与多个 fallback 都不存在）/ `Cancelled`（用户主动取消）/ `Timeout`（按 stage 区分：TCP / Auth / SFTP）/ `Config`（SSH config 解析或 `ssh -G` 失败）。每个变体 SHALL 携带充分上下文（`Tcp { host, source }` / `AuthExhausted { attempts }` 等）。SHALL 实现序列化让错误能跨 IPC 边界以 JSON 形式传给前端 UI。

文件操作级错误 SHALL 通过 `FsError` 表达——SSH 文件系统 provider 实现文件系统 provider trait 时 SHALL 把 SFTP 错误投影到 `FsError`，包括：

- SFTP `NoSuchFile` → `FsError::NotFound`
- SFTP `PermissionDenied` → `FsError::Io { source: <PermissionDenied I/O 错误> }`
- 瞬时错误重试耗尽 → `FsError::TransientExhausted { path, attempts, last_reason }`
- SSH 会话断开（操作时 session 已 disconnect / channel closed）→ `FsError::Disconnected { path, reason }`
- 其它永久错误 → `FsError::Io { source: <other I/O 错误> }`

`FsError` SHALL 提供 `is_retryable()` 与 `should_invalidate_cache()` 元方法让 caller 按错误语义决定是否重试 / 是否清 cache。

#### Scenario: TCP probe 失败携带 host 上下文

- **WHEN** 调用方连接到不可达 host
- **AND** TCP probe 超时
- **THEN** `SshError::Tcp { host: "unreachable.example.com", source: <I/O 错误> }` SHALL 被返回
- **AND** 序列化后含 `code: "ssh_tcp_failure"` / `host` / `reason` 三个字段

#### Scenario: 鉴权耗尽携带详细 attempts

- **WHEN** 鉴权候选链全部失败
- **THEN** 错误 SHALL 为 `SshError::AuthExhausted { attempts }` 含每个候选的 `source` / `outcome` / `elapsed_ms`
- **AND** UI SHALL 能从 attempts 渲染逐项诊断（如"env agent: socket 不存在 / launchctl: 返回空 / 1Password: 文件不存在 / id_ed25519: requires passphrase use ssh-add / id_rsa: not found"）

#### Scenario: 用户主动取消

- **WHEN** 用户在 `connecting` 状态点击 UI 取消按钮
- **THEN** 进行中的连接 future SHALL 被 abort
- **AND** 错误 SHALL 为 `SshError::Cancelled`，状态切到 `disconnected`，不残留半连接资源

#### Scenario: SFTP NoSuchFile 投影到 FsError::NotFound 且不重试

- **WHEN** SSH `stat(path)` 远端返 SFTP `NoSuchFile`
- **THEN** 调用方拿到 `FsError::NotFound(path)`
- **AND** `err.is_retryable()` 返 `false`，`err.should_invalidate_cache()` 返 `true`

#### Scenario: SFTP 瞬时错误耗尽投影到 TransientExhausted

- **WHEN** SFTP `read_to_string` 多次返回瞬时错误码且仍未恢复
- **THEN** 调用方拿到 `FsError::TransientExhausted { path, attempts, last_reason: <某个瞬时错误描述> }`
- **AND** `err.is_retryable()` 返 `false`（已经重试过了），`err.should_invalidate_cache()` 返 `false`（远端可能恢复）

#### Scenario: Session 断开投影到 Disconnected

- **WHEN** 文件操作时 SSH session 已断开（channel closed / session dropped）
- **THEN** 调用方拿到 `FsError::Disconnected { path, reason }`
- **AND** `err.is_retryable()` 返 `true`（重连后可能恢复），`err.should_invalidate_cache()` 返 `false`

### Requirement: Reconnect lifecycle preserves SFTP session integrity

SSH 上下文管理层在 `ssh_connect` / `switch_context` / `ssh_disconnect` 路径上 SHALL 保证：旧 polling watcher 在 SSH session manager 做任何 lifecycle 动作（`connect` / `disconnect` / `switch_context`）之前已完成 cancel-and-join，使新调用路径不可能拿到指向已关闭 SFTP session 的旧句柄。

实施约束（加自动化回归屏障）：

- **5 处 mutate 入口** SHALL 在 mutate 之前持 watcher ops 互斥锁：`ssh_connect` / `switch_context` / `ssh_disconnect` / `reconfigure_claude_root` / `shutdown_ssh_all`。`shutdown_ssh_all` 实现 SHALL 用阻塞获取锁而非 try-lock——try-lock 失败时绕过锁直接 mutate 会破坏与 refresh 路径同锁互斥的前提
- watcher cancel 调用 SHALL 在 `ssh_mgr.connect / switch_context / disconnect` 之前完成
- watcher attach 调用 SHALL 在 `ssh_mgr` 完成插入新 SSH session 资源之后完成，且与 shutdown generation 双检（shutdown 中途的 attach 被丢弃）
- watcher 归属保持在数据 API 层，SSH session manager 不直接管 watcher 生命周期（保持 crate 边界：SSH 库不依赖上层 broadcast 通道）

**bump-first 顺序契约**：5 处路径 SHALL 在 `ssh_mgr.connect / switch_context / disconnect` / `ssh_mgr.shutdown_all` / 写新 `projects_dir` 这一步 await **之前** 完成 context generation 自增（`reconfigure_claude_root` 同时自增 root generation）。理由：保留这一顺序让任何 in-flight 列表骨架查询 / 分页构建任务 在 spawn 时记录的"期望 generation"立即失效（broadcast 前 check `current != expected` → silent drop）。如果反转为"先 await 后 bump"，await 期间 ssh_mgr 状态部分切换但 generation 未 bump，in-flight scan task 的 broadcast 校验仍通过，会向前端串扰旧 ctx 的 metadata update。

**派生 cache 写入的双重校验**：依赖"全局 flat-key、随 list_repository_groups 刷新"的派生 cache 的实现 SHALL 在 cache 写入路径前用 watcher ops 锁与本路径互斥，并在锁内做 (current ContextId == captured ContextId) **AND** (current generation == captured generation) 双重校验——任一 mismatch 时 SHALL skip 写入（safe degrade，不污染派生 cache）。理由：bump-first 顺序使 generation 在 `ssh_mgr.switch_context` 网络 RTT 期间已经领先于实际 ssh_mgr 状态；caller 的 generation pre/post snapshot 都可能整段落在该 window 内（pre = post = bumped 后值），漏判 "context 已切"。**单 ctx-equality** 又无法识别"同 host 快速 disconnect+reconnect 期间 ContextId 等价但 generation 已自增两次"边角；**单 generation-equality** 无法识别"reconfigure_claude_root 改 Local projects_dir 但 ssh_mgr.active 不变"边角。结构性修法是 refresh 路径用同锁同步读 ssh_mgr active + 重建 ContextId（含 Local 时的 projects_dir 字段）+ 二次比较 captured generation 做综合判断。

#### Scenario: 同 host 重连后 list_repository_groups 仍返回远端数据

- **WHEN** 调用方依次执行：注册 fake provider v1 → `list_repository_groups`（断言成功）→ `ssh_disconnect` → 同名重新注册 fake provider v2 → `list_repository_groups`
- **THEN** 第二次 `list_repository_groups` SHALL 成功返回远程仓库组
- **AND** 返回值 SHALL 与 v2 提供的 fixture 一致（不复用 v1 的旧数据）
- **AND** 调用过程 SHALL NOT 抛 Err 含 `session closed` 字符串

#### Scenario: 切换到新 host 时旧 watcher 先 cancel-and-join 再 mutate

- **WHEN** active context 是 `Ssh<host_a>` 且其 watcher 正在运行
- **AND** 调用方请求 `ssh_connect(host_b)` 切换到新 host
- **THEN** 数据 API 层的 `ssh_connect` SHALL 在调 `ssh_mgr.connect` 之前完成对 `host_a` 的 watcher cancel-and-join
- **AND** cancel-and-join 完成后才执行 `ssh_mgr.connect`（内部会断开 `host_a` 的 SSH session 资源，旧 SFTP session 句柄此时引用计数降为 0）
- **AND** `host_b` 上线后任何对 `host_b` provider 的查询 SHALL 拿到 fresh 句柄，**不会**返回 `host_a` 的 closed session

#### Scenario: switch_context bump-first 顺序保留以防 in-flight scan 串扰

- **WHEN** active context = `Ssh<host_a>` 且有一个 in-flight `list_sessions_skeleton` 已 spawn 后台 scan task（task 持期望 generation = N）
- **AND** 调用方触发 `switch_context("local")`
- **THEN** `switch_context` 实现 SHALL 先自增 generation（N→N+1）再 await `ssh_mgr.switch_context(None)`
- **AND** 后台 task 后续每次发出 metadata update 前 SHALL load 当前 generation，发现 `N+1 != N` → silent drop update
- **AND** 用户 SHALL NOT 在切到 Local 后看到 host_a 的 metadata broadcast 串扰

#### Scenario: 派生 cache 写入识别 captured ctx 与当前 active 不一致时 skip

- **WHEN** 调用方 task A 触发 `switch_context("local")`，进入 `ssh_mgr.switch_context(None).await` 期间（generation 已自增但 ssh_mgr.active 仍返 `host_a`）
- **AND** 调用方 task B 并发调 `list_repository_groups()`
- **AND** task B 的内部"取活跃 fs+ctx+policy"接口拿到 captured_ctx = `Ssh<host_a>` + captured_generation = 自增后值
- **WHEN** task B 进入派生 cache 刷新路径，先获取 watcher ops 锁
- **THEN** 锁拿到时 `ssh_mgr.switch_context` 已完成（task A 也持同锁，task A 完成 mutate + 释放锁后 task B 才能拿到）
- **AND** 锁内查询当前 active context SHALL 返回 None（Local active）；重建的 ContextId = `Local { projects_dir }` 与 captured_ctx = `Ssh<host_a>` mismatch
- **AND** task B SHALL skip refresh + 写入结构化调试日志（含 captured/current ctx + 两个 generation 值），不调用派生 cache 的 clear
- **AND** task B 仍 SHALL 把 host_a 的 groups 返给 caller（caller 拿 self-consistent 旧数据；下次 IPC 自然刷新到 Local）

#### Scenario: 同 host 快速 disconnect+reconnect 期间生成 generation mismatch 触发 skip

- **WHEN** active context = `Ssh<host_a>`，调用方 task B 进入 list_repository_groups 内部接口拿到 captured_ctx = `Ssh<host_a>` + captured_generation = `N`
- **AND** 调用方 task A 在 task B inner 完成之后、wrapper 拿锁之前完成 `ssh_disconnect("host_a")`（generation N→N+1）+ `ssh_connect("host_a")` 同 host 重连（generation N+1→N+2）
- **THEN** task B wrapper 拿锁后 重建的 ContextId = `Ssh<host_a>` 与 captured_ctx 全等（同 host signature 派生的 ContextId 相等），但 `current_generation = N+2` ≠ `captured_generation = N` → generation mismatch
- **AND** task B SHALL skip refresh —— 避免 task B inner 用旧 host_a session 拿到的 groups 覆盖新 session 应有的最新 mapping

### Requirement: Polling watcher exits promptly on cancellation

polling 主 loop SHALL 在取消信号触发时立即跳出（不等满 poll interval 或 catch-up interval）。当前实现使用统一异步 select 同时 await 取消信号与两个 interval tick，本 Requirement 把这一行为固化为契约。in-flight 的 SFTP `read_dir` 自然完成，cancel 中断点在每次 select 入口；这是 `Requirement: Read sessions and files over SSH with same contract` 的补强。

#### Scenario: cancel 在 sleep 阶段触发时 watcher 立即退出（paused time）

- **WHEN** 测试设置时间暂停模式
- **AND** watcher task 在 poll interval 的 await 状态
- **AND** 调用方触发取消信号
- **THEN** `cancel_and_join` 在短超时（paused-time 维度）内完成
- **AND** 测试**不**通过推进时钟来让 watcher 退出（验证 cancel 本身而非 timer 触发）

#### Scenario: cancel 在 in-flight read_dir 时按现有逻辑退出

- **WHEN** watcher task 正在 await `sftp.read_dir(...)`（远端 SFTP I/O）
- **AND** 调用方触发取消信号
- **THEN** 当前 read_dir 完成后，下一次 select 入口 SHALL 命中 cancel 分支并跳出循环
- **AND** 本 Requirement **不**强制中断 in-flight SFTP request（保留 SFTP 协议层的礼貌断开）

### Requirement: SSH session manager 暴露 host signature 派生的 ContextId 查询

系统 SHALL 在 SSH 连接的 host alias resolve 阶段完成后，通过 SSH config digest input + host signature 推导接口计算并缓存当前 SSH context 的 host signature，存放在 SSH session 资源的 host_signature 字段；host signature MUST NOT 在每次 IPC 调用时重新通过 `ssh -G` 子进程 resolve（避免子进程 spawn overhead）。

SSH session manager SHALL 暴露 ContextId 查询方法：入参为已注册 SSH context 的 context_id 字符串，命中时 SHALL 从对应 SSH session 资源取 host_signature 与 remote_home 合成 `ContextId::ssh(host_signature, remote_home)` 返回；未注册（含已 disconnect / 未连接成功）时 SHALL 返回空；SHALL NOT 调用 `ssh -G` resolve 子进程。

SSH session manager SHALL 暴露原子查询方法在单次内部状态 lock 内同时返回 SSH 文件系统 provider 与 ContextId，保证二者来自同一快照。调用方 SHALL 用本方法而非独立的 provider + context_id 配对取 fs/ctx，避免 disconnect race 产生 `(SSH provider, Local ctx)` 不自洽组合。

#### Scenario: connect 路径自动计算并存储 host signature

- **WHEN** SSH connect 走到 host alias resolve 完成阶段拿到 ResolvedHost
- **THEN** SHALL 通过 SSH config digest input 接口构造输入
- **AND** SHALL 通过 host signature 推导接口计算 digest
- **AND** SHALL 在最终构造 SSH session 资源时填入 host_signature 字段
- **AND** SHALL NOT 在后续 IPC / cache lookup 时再次跑 `ssh -G`

#### Scenario: context_id 返回 ContextId::ssh

- **WHEN** 调用方对一个已连接的 SSH context 调用 `ssh_mgr.context_id("ssh-host-A").await`
- **THEN** 返回 `Some(ContextId)`，其 backend kind 为 SSH
- **AND** host signature SHALL 等于 connect 时计算并存储的值
- **AND** root or home SHALL 等于 SSH session 资源的 remote_home

#### Scenario: 未注册 context 返回 None

- **WHEN** 调用方对一个未注册（或已 disconnect）的 context_id 调用 `ssh_mgr.context_id(...)`
- **THEN** SHALL 返回 `None`，且 SHALL NOT panic 或 spawn 子进程

#### Scenario: provider_and_context_id 原子返回 provider+ctx

- **WHEN** 调用方对已注册 SSH context 调用 `ssh_mgr.provider_and_context_id("ssh-host-A").await`
- **THEN** SHALL 返回 `Some((provider, ctx))`，二者来自同一内部状态快照
- **AND** ctx 的 host signature SHALL 等于该 provider 在 connect 时计算并存储的值
- **AND** ctx 的 root or home SHALL 等于 provider 的 remote_home
- **WHEN** 调用方对未注册 context 调用同方法
- **THEN** SHALL 返回 `None`，调用方据此 fall-through 到 Local 安全降级

#### Scenario: 同 host reconnect 后 ContextId 一致

- **WHEN** 用户先 connect → disconnect → 再 connect 同一 SSH host A（`~/.ssh/config` 未变 AND 两次 connect 均走 `ssh -G` 成功路径）
- **THEN** 两次 connect 后通过 `context_id("ssh-host-A").await` 拿到的 ContextId SHALL 相等（host signature 是 resolved ssh config 的纯函数，不含随机或时序成分）
- **AND** 任何用此 ContextId 做 key 的 cache entry SHALL 跨 reconnect 复用

#### Scenario: degraded fallback 与 ssh -G 路径产 ContextId 安全不等（by-design miss）

- **WHEN** 第一次 connect 走 `ssh -G` 成功路径，ResolvedHost 含 proxyjump / proxycommand / hostkeyalias 字段 → 计算出 host signature digest A
- **AND** 第二次 reconnect 时 `ssh` 子进程缺失 / `ssh -G` 失败，走 fallback 解析路径，ResolvedHost 的 proxyjump / proxycommand / hostkeyalias 全为 `None` → 计算出 host signature digest B
- **THEN** digest A 不等于 digest B（不同字段集合 → 不同 digest 输入）
- **AND** 两次 connect 派生的 ContextId SHALL NOT 相等
- **AND** 任何用 digest A 做 key 写入的 cache entry SHALL NOT 被 digest B 的 lookup 命中——这是 **by-design safe miss**（degraded 路径对 host 的连接拓扑认知降级，与 `ssh -G` 路径不等价；落到不同 cache namespace 防止"基于错误连接假设拿到陈旧远端数据"）
- **AND** 用户体感为 reconnect 后 session 列表冷扫一次，UX 多几秒，但绝不串扰数据

### Requirement: SSH open_read 大文件走 K-worker prefetch streaming reader

SSH 文件系统 provider 的 `open_read` 实现对**生产路径**（实例由真实 SFTP session 构造）且**大文件**（size 达到流水线阈值）SHALL 返回一个**流式 K-worker prefetch reader**——内部由 K 个并发 task 飞独立 SFTP READ，把读到的 chunk 经有界 channel 推给消费侧，使得 reader 的 peak RSS 与 K 成正比而非与 file_size 成正比。

K-worker SHALL 用 **round-robin chunk 分派**：第 i 个 chunk（chunk 大小为流水线 chunk size）由 `worker_id = i % n_workers` 处理；消费侧按 `next_worker = (next_worker + 1) % n_workers` 顺序取——保证消费速度推进的同时所有 K 个 worker 都能持续被 backpressure 释放，wall 与全量预取参照基准持平。

**Limited 降级**：K 个 SFTP open 用并发包装预并发打开时若任一返回 server 端 SFTP `open_handles` 上限错误，`open_read` SHALL 降级到**单 handle 流式**——优先**复用**已成功返回的第 1 个 file 句柄（avoid 再开一次 open 撞同样 Limited + avoid 依赖 file 句柄 drop 同步 close 语义）；该句柄实现异步读，直接返；wall 退化到 N × RTT 但 peak RSS 仍受限单 chunk。partial_handles 仅在所有 K 个 open 都 Limited 时为空，此时 SHALL 显式再开一次（接受可能继续 Limited 上抛）。降级 SHALL 通过结构化警告日志记录 path / workers / partial_handle_count / reason 让运维侧可见。

**小文件路径**：生产路径 + size 小于流水线阈值仍 SHALL 走单 RTT 全量预取 + Cursor 包装——避免 K 个 SFTP open 的 spawn overhead 对小文件 wall 无收益反加 latency。

**Fake 测试路径**：fake 构造的 provider 实例（无真实 SFTP session）SHALL 走 SFTP client trait 的 `read(path)` 方法 + Cursor 包装的原有路径——保留 fake op counter 语义，让现有测试 fake 路径断言**无需更新**。

**inherent open_read_stream 保留**：provider 暴露的 `open_read_stream` 行为不变——caller 显式调用拿原生 SFTP 句柄路径不受本契约影响。

#### Scenario: 大文件生产路径返流式 K-worker prefetch reader

- **WHEN** caller 在生产构造的 provider 上调 `open_read(path)` 且 size 达到流水线阈值
- **THEN** 返回的异步流式读取句柄 SHALL 是流水线 reader 包装而非 Cursor
- **AND** reader 实例 SHALL 持有 `n_workers = min(流水线最大 worker 数, ceil(size / chunk size)).max(1)` 个 receiver（每个有限 capacity）+ 持有 K 个 worker task 的并发任务集合
- **AND** caller 持续 poll_read 直到 EOF 期间，进程 peak RSS 增量 SHALL ≤ `n_workers × 2 × chunk size`（最坏：每个 channel 1 个 buffered chunk + 每个 worker 1 个 in-flight chunk）

#### Scenario: 大文件 round-robin chunk 分派保 wall parity

- **WHEN** 流水线 reader 启动 K worker
- **THEN** 第 i 个 chunk SHALL 由 worker `i % n_workers` 处理（worker 0 读 chunks [0, K, 2K, ...]，worker 1 读 chunks [1, K+1, 2K+1, ...]，依此类推）
- **AND** 消费侧 poll_read SHALL 按 `next_worker = (next_worker + 1) % n_workers` 严格轮询，确保消费推进直接释放每个 worker 的 backpressure
- **AND** total wall time SHALL ≈ `ceil(n_chunks / n_workers) × RTT`（与全量预取参照基准持平，不退化为 `n_chunks × RTT` 串行）

#### Scenario: 小文件生产路径走单 RTT 全量预取

- **WHEN** caller 在生产构造的 provider 上调 `open_read(path)` 且 size 小于流水线阈值
- **THEN** 实现 SHALL 调 SFTP `read(path)` 拿全量 bytes
- **AND** SHALL 返 Cursor 包装的全量 bytes
- **AND** SHALL NOT spawn K worker / 不创建流水线 reader

#### Scenario: Fake 测试路径走 SFTP client read 全量保 op counter 语义

- **WHEN** caller 在 fake 构造的 provider 上调 `open_read(path)`
- **THEN** 实现 SHALL 调底层 SFTP read 方法拿全量 bytes
- **AND** SHALL 返 Cursor 包装的全量 bytes
- **AND** fake provider 的 read counter 在此次调用后 SHALL 自增 1（与既有 fake 路径语义一致）

#### Scenario: SFTP `Limited` 降级到单 handle 流式且优先复用已开 handle

- **WHEN** 流水线 reader 内部并发 K 个 SFTP open，收齐结果后任一为 server 端 SFTP `open_handles` 限制错误
- **THEN** `open_read` SHALL 降级到单 handle 流式：**优先复用已成功打开的第 1 个 file 句柄**（避免再次 open 撞同样 Limited，避免依赖 file 句柄 drop 的同步 close 语义）
- **AND** 若所有 K 个 open 都 Limited（如 K=1 时罕见场景），SHALL 显式再 open 一次重试；若仍 Limited 上抛 `FsError::Io { ErrorKind::Other }`
- **AND** SHALL 把已成功的其余 file 句柄直接 drop 让底层库自身释放（接受 best-effort close 的潜在短暂 server 端 handle leak，量级毫秒，SSH 连接关闭时彻底释放）
- **AND** SHALL 通过结构化警告日志（path, workers, partial_handle_count, reason）记录降级事件
- **AND** caller 仍能完整流式读到 EOF，peak RSS 不会超过单 chunk + file 内部 buffer

#### Scenario: 任一 worker channel close 时立即按字节计数判定真 EOF 防 silent truncation

- **WHEN** 流水线 reader 内 round-robin 轮到的 next_worker 对应 receiver 在 poll_recv 返 None（该 worker 正常退出或异常退出后 sender drop；round-robin 顺序保证此刻 stream 已无该位置的后续 chunk）
- **THEN** consumer SHALL **立即**（不等其它 worker 全 close）比较累计写入字节 `total_bytes_read` 与构造时记录的 `total_bytes_expected`（取自 size 探测）
- **AND** 若 `total_bytes_read == total_bytes_expected` → SHALL 返 `Poll::Ready(Ok(()))` 不写入 `ReadBuf`（标准异步读取 EOF 语义）；reader 标 EOF 后续 poll_read 持续返 EOF
- **AND** 若 `total_bytes_read < total_bytes_expected` → SHALL 返 `Poll::Ready(Err(I/O 错误：UnexpectedEof，含期望与实际字节数差异说明))`（防 worker 静默退出 / 异常 abort 让 caller 误把短读当 EOF）；reader 标错误状态后续 poll_read 返终态错误
- **AND** SHALL NOT 等所有 K 个 receiver 都 close 才判定（继续等会让 consumer hang 死等其它仍在飞 next-round chunk 的 worker）

#### Scenario: 生产路径分支选择钉死小文件 / 大文件 / fake 三 branch wiring

- **WHEN** 调用 open_read 策略选择函数（输入：是否生产路径、文件 size）
- **THEN** （生产路径，size ≥ 流水线阈值）SHALL 返 `OpenReadStrategy::Streaming { n_workers }`
- **AND** （生产路径，size < 流水线阈值）SHALL 返 `OpenReadStrategy::SmallFileBuffered`
- **AND** （非生产路径，任意 size）SHALL 返 `OpenReadStrategy::FakeBuffered`（fake 测试路径所有 size 都走 client.read）
- **AND** 此分支函数 SHALL 有单元测试覆盖以上 4 个组合，拦截"未来误把生产大文件 branch 接到 client.read 旧路径"类 wiring 回归

#### Scenario: Worker 内部 SFTP 错误经 channel 传 I/O 错误给消费侧

- **WHEN** 已构造的流水线 reader 在某 worker 内 seek 或 read 调用返 Err
- **THEN** worker SHALL 把错误转换为标准 I/O 错误并通过 channel 发送给对应 receiver
- **AND** worker SHALL 然后 return（drop sender，channel close）
- **AND** consumer 的 poll_read 收到错误时 SHALL 返 `Poll::Ready(Err(io_err))`
- **AND** SHALL NOT silent drop 错误（如 worker 异常后丢失错误信号让 consumer hang）

#### Scenario: Reader drop 联级 abort 所有 worker

- **WHEN** 流水线 reader 持有者 drop reader（典型场景：上游 BufReader 提前结束 / 上游 task 被 abort）
- **THEN** reader 内部并发任务集合 drop SHALL 触发所有未完成 worker task 的 abort
- **AND** worker 内任一 await 点 SHALL 在下次 poll 时被 cancellation 返回 abort
- **AND** SHALL NOT 留 orphan task 在异步运行时继续读 SFTP 浪费带宽

#### Scenario: EOF 通过 next round-robin worker channel close + 字节计数表达

- **WHEN** worker 处理完自己分到的最后一个 chunk 并发送给 consumer 成功
- **THEN** worker SHALL return（自然 drop sender）
- **AND** consumer 下次 round-robin 轮询到该 worker 的 receiver 时 poll_recv SHALL 返 None
- **AND** consumer SHALL **立即**触发字节计数判定，不等其它 worker close；正常退出场景下此时累计字节等于期望字节 → 翻译为 `Poll::Ready(Ok(()))` 不再写入 `ReadBuf`（标准异步读取 EOF 语义）

#### Scenario: 大会话 scanner BufReader 接流水线 reader 不破契约

- **WHEN** SSH 生产路径下调 `fs.open_read(path)` 拿 reader，再用 BufReader 包装（容量与 SFTP 单消息上限对齐）
- **THEN** reader 实际是流水线 reader，每次 BufReader 填充 SHALL 从流水线 reader 拿到下一个 chunk（K-worker prefetch 提前飞 read 已让 chunk 通常在 channel 中就绪）
- **AND** scanner 全文 parse 完成 → BufReader drop → 流水线 reader drop → 并发任务集合 drop → worker cleanup
- **AND** 与全量预取参照基准对比：scanner wall 持平；进程 peak RSS 增量 SHALL 从约等于 file_size 降到约等于 `n_workers × 2 × chunk size`

### Requirement: SSH 远端 memory CRUD 走真实 fs ops

系统 SHALL 在 SSH context 下完整支持 project memory CRUD：`get_project_memory` / `read_memory_file` / `add_memory` / `delete_memory` 四个 IPC method 在 active context 是 `Ssh<host>` 时 SHALL 通过当前 SSH 文件系统 provider 调用真实远端 fs ops，**不**得 graceful skip 返 `has_memory: false` / not_found。

SSH 文件系统 provider SHALL 在文件系统 provider trait 上实现 `write_atomic` / `create_dir_all` / `remove_file` 三个方法，行为契约：

- `write_atomic` SHALL 通过底层 SFTP 协议写到带唯一后缀的 tmp 文件（基于原子序列号 + 进程 ID 派生），写完调 SFTP rename 覆盖目标 path：
  - 优先走 `posix-rename@openssh.com` SFTP 扩展（在 connect 时探测远端是否支持，含此扩展则启用），由 OpenSSH server 提供 POSIX rename 原子覆盖
  - 不支持时降级为两步：先删除目标再重命名 tmp 文件——降级路径有极短窗口 reader 可能见 `target missing`，单次写场景 acceptable
  - rename 失败 SHALL best-effort 调 `remove_file(<tmp>)` 清理（清理失败不向上传播）
  - 服务端探测结果 SHALL 在 SSH provider 内 cache 一次（per session），后续 `write_atomic` 直接读 cache 决策，不每次 connect 重探测
- `create_dir_all` SHALL 通过 SFTP 递归创建目录，对每段父目录先调 `try_exists` 探测，已存在跳过；缺失调 mkdir 创建。任何 SFTP rpc 失败 SHALL 走 Scenario "SFTP 瞬时错误自动重试" 定义的 retry 策略
- `remove_file` SHALL 通过 SFTP 删文件 RPC；不存在 SHALL 返 `FsError::NotFound(path)`；路径是目录 SHALL 返 `FsError::Io { path, source: <非空目录 I/O 错误> }`，**不**递归删

SFTP client trait SHALL 新增 `write` / `mkdir` / `remove` / `rename` 四个方法，由生产实现 delegate 到底层 SFTP session 的相应方法。所有写操作 SHALL 与既有 read 操作复用同一 SFTP session（**不**额外加 Mutex 包装——SFTP session 公共 API 是 `&self` 方法，message-id 由库内部 channel 维护）。SFTP message-id pipeline 并发支持留独立后续 change；本契约下写路径与既有 read 路径同处一队列。

#### Scenario: SSH context 下 get_project_memory 走远端 read_dir + read_to_string

- **WHEN** active context 是 `Ssh<host>`，调用方调 `get_project_memory(project_id)`
- **THEN** 系统 SHALL 通过当前 SSH 文件系统 provider 调 `fs.read_dir(<remote_home>/.claude/projects/<base>/memory)` 列举 `.md` 文件
- **AND** 调 `fs.read_to_string(<memory_dir>/MEMORY.md)` 读 index 内容（如存在）
- **AND** 返回的 `ProjectMemory` SHALL 携带远端 layers 真实数据，`hasMemory` SHALL 为 `true`（当 memory 目录存在且含 `.md` 文件）
- **AND** 远端 fake provider 的 read_dir counter 与 read counter SHALL 各 ≥ 1

#### Scenario: SSH context 下 read_memory_file 走远端 read_to_string

- **WHEN** active context 是 `Ssh<host>`，调用方调 `read_memory_file(project_id, "MEMORY.md")`
- **THEN** 系统 SHALL 通过当前 SSH 文件系统 provider 调 `fs.read_to_string(<memory_dir>/MEMORY.md)`
- **AND** 返回的 `MemoryFileContent.content` SHALL 是远端文件内容，`filePath` SHALL 以远端 `<remote_home>` 为根
- **AND** SHALL NOT 返回 `ApiError::not_found` 含 "SSH context" 字样的占位错误（旧 graceful skip 文案）

#### Scenario: SSH context 下 add_memory 走远端 write_atomic + 自动创建 memory 目录

- **WHEN** active context 是 `Ssh<host>`，调用方调 `add_memory(project_id, "feedback_test.md", "content")` 且远端 `<memory_dir>` 当前不存在
- **THEN** 系统 SHALL 调 `fs.create_dir_all(<memory_dir>)` 确保目录就绪
- **AND** SHALL 调 `fs.write_atomic(<memory_dir>/feedback_test.md, content.as_bytes())` atomic 写入文件
- **AND** 写入完成后 SHALL 重新发现 memory layers 拿新 layers 列表
- **AND** 返回的 `ProjectMemory` SHALL 是写入后的最新状态（`hasMemory: true`，新文件出现在 `layers` 中）
- **AND** 远端 fake provider 的 mkdir counter SHALL ≥ 1（首次创建 memory 目录）；write counter 与 rename counter SHALL 各 ≥ 1（atomic write 对 tmp 文件 + rename）

#### Scenario: SSH context 下 delete_memory 走远端 remove_file

- **WHEN** active context 是 `Ssh<host>`，调用方调 `delete_memory(project_id, "feedback_test.md")` 且远端 memory 目录中存在该文件
- **THEN** 系统 SHALL 调 `fs.remove_file(<memory_dir>/feedback_test.md)`
- **AND** 删除完成后 SHALL 重新发现 memory layers 拿新 layers 列表
- **AND** 返回的 `ProjectMemory` SHALL 不再包含该文件
- **AND** 远端 fake provider 的 remove counter SHALL ≥ 1

#### Scenario: SSH context 下 add_memory 文件名校验拒绝路径穿越

- **WHEN** active context 是 `Ssh<host>`，调用方调 `add_memory(project_id, "../etc/passwd", "...")` 或 `add_memory(project_id, "secret.json", "...")`
- **THEN** 系统 SHALL 返 `ApiError::validation`，文案与 `read_memory_file` 路径穿越 / 非 `.md` 拒绝一致
- **AND** SHALL NOT 调任何远端 fs 写方法（write counter / mkdir counter / rename counter SHALL 全 0）

#### Scenario: SSH 写路径 transient 错误重试

- **WHEN** SFTP write / mkdir / rename / remove 任一 rpc 返回瞬时错误码
- **THEN** 系统 SHALL 重试有限次数，每次间隔指数退避
- **AND** 仍失败时 SHALL 把错误向上抛给调用方，封装为 `FsError::TransientExhausted { attempts, last_reason }`

#### Scenario: SSH write_atomic rename 失败 best-effort 清理 tmp

- **WHEN** SSH `write_atomic(path, content)` 在写完 tmp 后调 SFTP rename 失败（非 transient，已重试用尽）
- **THEN** 系统 SHALL 调 `fs.remove_file(<tmp_path>)` best-effort 清理 tmp 文件
- **AND** 清理失败 SHALL 不向上传播 error（rename 失败已是 primary error）
- **AND** 向调用方抛 `FsError::TransientExhausted { attempts, last_reason }` 或对应 SFTP error

### Requirement: Keep SSH transport alive via transport-layer keepalive

系统 SHALL 在每次 `ssh_connect` 建立 SSH client 时启用 transport 层 keepalive，配置为每 `SSH_KEEPALIVE_INTERVAL = 15s` 距离上次 server 数据后发一次 keepalive 请求（要求 reply），由 SSH 库内部 keepalive loop 在累计 `SSH_KEEPALIVE_MAX = 3` 之上的连续未应答 tick 后（实际触发窗口约 `(SSH_KEEPALIVE_MAX + 2) × SSH_KEEPALIVE_INTERVAL = 75s`，因当前 SSH 库实现的 off-by-one 语义比较先做后自增再发送）主动关闭 transport。

实现 SHALL 暴露 `SSH_KEEPALIVE_INTERVAL` 与 `SSH_KEEPALIVE_MAX` 两个常量，并 SHALL 通过统一的 client config 构造 helper 把这两个常量写进 SSH client config。connect 阶段 SHALL 调用该 helper 而非默认 config 构造握手；helper 里其它字段 SHALL 通过结构体更新语法保留 SSH 库默认值（SSH client config 不实现深比较语义，一致性由构造方式保证而非运行时断言）。

启用该机制的目的：(1) **主作用**：每 15s 让 channel 双向有 SSH msg 流动，防止 server-side 默认禁用 keepalive / NAT idle / firewall idle 把 channel 静默关闭；(2) **次作用**：让对端硬故障（拔网线 / SSH 服务重启）能在约 75s 内被 client 主动发现，由 SSH 库关闭 transport，触发既有 polling watcher → dead 信号 → 自愈 disconnect 链路（详 `Requirement: Watch remote project directories via SFTP polling`）。

本 Requirement 仅约束 client config 入参与 connect 路径调用点；keepalive timeout 真触发后的自愈语义仅在**已 attach polling watcher 的 SSH context** 上生效（典型场景：用户 ssh_connect 后立即 switch_context 触发 watcher attach）。已 connect 但从未 switch_context 也从未触发任何 fs IPC 的纯 idle context 在 transport 被 keepalive 关闭后会保留 stale SSH session 资源直到下一次 fs op，属 v1 已知边界，不属本 Requirement 必须解决。

#### Scenario: client config helper 启用 keepalive 两个常量

- **WHEN** 调用 client config 构造 helper
- **THEN** 返回的 SSH client config SHALL 满足 keepalive interval 等于 `SSH_KEEPALIVE_INTERVAL` 且 keepalive 最大容忍 tick 等于 `SSH_KEEPALIVE_MAX`
- **AND** 实现 SHALL 通过结构体更新语法仅显式设置 keepalive interval 与 max 两个字段，确保其它字段从库默认继承（不引入额外副作用）

#### Scenario: connect 路径用 helper 而非默认 config

- **WHEN** 调用方触发 `ssh_connect` 进入 SSH transport 握手阶段
- **THEN** 传入的 config SHALL 由 client config 构造 helper 产出
- **AND** SHALL NOT 是 SSH 库的默认 config 构造
- **AND** transport 握手成功后 SSH 库内部 keepalive loop SHALL 按 `SSH_KEEPALIVE_INTERVAL` 周期运行（由库内部实现保证，本契约只钉死 client config 入参）

#### Scenario: Keepalive timeout 关闭 transport 后 polling 自愈生效

- **WHEN** 已建立的 SSH context 处于 active 且有 polling watcher attached
- **AND** 对端不再回复任何 SSH 报文（NAT idle close / SSH 服务被 KILL / iptables 丢包）
- **AND** 累计 keepalive 未应答 tick 数超过最大容忍
- **THEN** SSH 库 client task SHALL 返回 keepalive 超时错误并关闭 transport
- **AND** 该 context 上后续 SFTP 调用 SHALL 收到 `session closed` / `broken pipe` 类错误，由 polling watcher 的永久错误识别归类
- **AND** 累计达永久失败阈值后通过 dead 信号触发自愈 disconnect，emit `ContextChanged { active_context_id: None, kind: Local }`，与既有自愈链路一致


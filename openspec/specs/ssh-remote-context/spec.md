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

系统 SHALL 在 SSH 上下文上提供与 local 上下文等价的 `project-discovery`、`session-parsing`、文件读取能力，使下游消费者观察到完全相同的数据形状。`SshFileSystemProvider` SHALL 实现 `cdt-discover::FileSystemProvider` trait 的所有方法（`exists` / `read_file` / `read_to_string` / `read_dir` / `stat` / `open_read_stream`），底层走 `russh-sftp` 或等价 SFTP 客户端 API；SHALL NOT 在远端 spawn 任何工作进程，唯一允许在远端执行的命令是 `printf %s "$HOME"` 用于探测 remote home。

#### Scenario: List projects on a remote host

- **WHEN** 当前上下文是 SSH，调用方请求项目列表
- **THEN** 返回结果 SHALL 与本地项目列表形状一致，数据源为远程 `<remote_home>/.claude/projects/` 目录

#### Scenario: Read a remote session

- **WHEN** 当前上下文是 SSH，调用方请求会话详情
- **THEN** 系统 SHALL 通过 SFTP `open_read_stream` 流式读取远程 JSONL 文件
- **AND** 返回与本地输出形状一致的 chunk 序列

#### Scenario: Resolve remote home with multiple fallbacks

- **WHEN** 远端 `<home>/.claude/projects` 不存在，但 `/home/<user>/.claude/projects` 或 `/Users/<user>/.claude/projects` 或 `/root/.claude/projects` 存在
- **THEN** 系统 SHALL 按上述顺序探测候选路径并使用第一个存在的
- **AND** 全部不存在时 SHALL 返回 `SshError::RemoteHomeMissing { tried }` 错误，状态切到 `error`，不切换 active context，但 `ssh_get_state` SHALL 保留该 context 的错误状态与已完成的 `authChain` 诊断

#### Scenario: SFTP transient errors are retried

- **WHEN** SFTP 调用返回瞬时错误码（`code=4` / `EAGAIN` / `ECONNRESET` / `ETIMEDOUT` / `EPIPE`）
- **THEN** 系统 SHALL 重试最多 3 次，每次间隔指数退避（75ms × attempt）
- **AND** 仍失败时 SHALL 把错误向上抛给调用方

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

#### Scenario: Resolve alias via system ssh -G

- **WHEN** 调用方请求 `ssh_resolve_host("myserver")`
- **AND** 系统有 `ssh` 二进制
- **THEN** 系统 SHALL spawn `ssh -G myserver`，从 stdout 解析得到 hostname / port / user / identityfile / identityagent 等字段
- **AND** 返回 `SshHostConfig { host, port, user, identity_file? }` 给调用方

#### Scenario: Fallback when ssh binary missing or fails

- **WHEN** 系统无 `ssh` 二进制（如 Windows 未启用 OpenSSH client）
- **OR** `ssh -G` 5s 超时 / 非零 exit
- **THEN** 系统 SHALL 降级到 `cdt-ssh::config_parser` 的基本字段解析
- **AND** 返回结果 SHALL 标记 `degraded: true`（UI 可据此显示"高级 SSH config 特性不可用"提示）

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


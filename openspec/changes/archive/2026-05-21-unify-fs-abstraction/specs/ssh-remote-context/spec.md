## MODIFIED Requirements

### Requirement: Read sessions and files over SSH with same contract

系统 SHALL 在 SSH 上下文上提供与 local 上下文等价的 `project-discovery`、`session-parsing`、文件读取能力，使下游消费者观察到完全相同的数据形状。`SshFileSystemProvider` SHALL 实现 `cdt-fs::FileSystemProvider` trait 的所有方法（`exists` / `read_to_string` / `read_dir` / `read_dir_with_metadata` / `stat` / `stat_many` / `read_lines_head` / `open_read`），底层走 `russh-sftp` 或等价 SFTP 客户端 API；SHALL NOT 在远端 spawn 任何工作进程，唯一允许在远端执行的命令是 `printf %s "$HOME"` 用于探测 remote home。

`open_read` SHALL 替代旧的 inherent 方法 `open_read_stream`——返回 `Box<dyn AsyncRead + Send + Unpin>` 让调用方不需 downcast 到 `SshFileSystemProvider` 就能流式读。`stat_many` SHALL 实现为 trait default（`futures::future::join_all` 包装 `stat`）；由于底层 `Arc<Mutex<SftpSession>>` 全锁串行，当前 SSH `stat_many` 仍是 N 次串行 RTT（**已知限制**），真正的 SFTP message-id 并发 pipeline 留独立 PR（PR-F）解决。trait API 先就位让 caller 一律调 `stat_many` 而非循环 `stat`。

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
- **AND** 由于 SFTP session 全锁，实际执行是 50 次串行 RTT —— 此限制属已知，留 PR-F 解决；trait 契约层面 caller SHALL 一律调 `stat_many` 而非循环 `stat`

#### Scenario: Resolve remote home with multiple fallbacks

- **WHEN** 远端 `<home>/.claude/projects` 不存在，但 `/home/<user>/.claude/projects` 或 `/Users/<user>/.claude/projects` 或 `/root/.claude/projects` 存在
- **THEN** 系统 SHALL 按上述顺序探测候选路径并使用第一个存在的
- **AND** 全部不存在时 SHALL 返回 `SshError::RemoteHomeMissing { tried }` 错误，状态切到 `error`，不切换 active context，但 `ssh_get_state` SHALL 保留该 context 的错误状态与已完成的 `authChain` 诊断

#### Scenario: SFTP transient errors are retried

- **WHEN** SFTP 调用返回瞬时错误码（`code=4` / `EAGAIN` / `ECONNRESET` / `ETIMEDOUT` / `EPIPE`）
- **THEN** 系统 SHALL 重试最多 3 次，每次间隔指数退避（75ms × attempt）
- **AND** 仍失败时 SHALL 把错误向上抛给调用方，封装为 `FsError::TransientExhausted { attempts: 3, last_reason }`

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

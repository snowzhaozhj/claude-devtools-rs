## 0. 推进进度索引（apply 接续锚点）

按 design.md `Risks / Trade-offs::[Trade-off]一次性大 PR` 拆为 5 个 sub-session 推进。每段完成后 commit 一次，不 push；最后一段统一走发布尾段 N.1-N.4。

| sub-session | 范围 | 状态 | 关键产物 |
|---|---|---|---|
| **Phase A** | tasks 1.1-1.5 / 2.1-2.5 / 3.1-3.11+3.14 | ✅ 已完成（commit `70b371f`） | `cdt-ssh::error/auth/host_resolver/request/polling_watcher` 骨架 + `lib.rs` 重组 + design.md D1b（russh 0.46→0.52） |
| **Phase B1** | tasks 3.12-3.13 / 4.1-4.12 | ✅ 已完成 | `cdt-ssh::session::SshSessionManager` 真握手 5 阶段 + `auth::run_auth_chain` 调度层 |
| **Phase B2** | tasks 5.1-5.8 | ✅ 已完成 | `cdt-ssh::provider::SshFileSystemProvider` 真 SFTP：`SftpClient` trait + 生产 `RusshSftpClient` 包装 `SftpSession` + `with_retry` 3 次指数退避 + 错误分类（NoSuchFile / PermissionDenied / Transient / Other）+ inherent `open_read_stream` 流式；fake 单测注入 15 个 case 覆盖 happy path / permission denied / 瞬时重试成功 / 重试耗尽 / classify 真值表 |
| **Phase B3** | tasks 6.1-6.8 + 8.1-8.4 | ✅ 已完成 | `cdt-ssh::polling_watcher` 3s+30s 轮询 + `cdt-watch::attach_remote` |
| **Phase C** | tasks 7.1-7.7 / 9.1-9.10 / 10.1-10.8 | ✅ 已完成 | `cdt-config::SshConfig` 强类型段 + validation/update/save-last；`cdt-api::LocalDataApi` 接真 `SshSessionManager` + status/context 订阅 + HTTP/Tauri 11 command；Tauri `ssh_status`/`context_changed` emit 桥 + shutdown hook；IPC contract / UI mock command 清单同步 |
| **Phase D** | tasks 11.1-11.5 / 12.1-12.6 | ✅ 已完成 | UI: `lib/api.ts` SSH/context IPC wrapper + `types/ssh.ts` + `connection.svelte.ts` / `context.svelte.ts` stores；Settings `Connection` tab + `WorkspaceIndicator` / `ContextSwitchOverlay` / `ConnectionStatusBadge`；mockIPC + Vitest store/event 覆盖 |
| **Phase E** | tasks 13.1-13.6 / 14.1-14.5 / N.1-N.4 | ⏳ 待开工 | 测试金字塔 + perf 验证 + 集成 smoke + 发布尾段 |

**关键 design 决策**（已落代码 + 已写 tasks.md "实现差异" 注释，无需重新决议）：
- D1b：`russh = 0.52`（非 0.46）+ `russh-sftp = 2`，详见 design.md D1b 修订块
- `SshSessionManager`（真握手，session.rs）与 `SshConnectionManager`（占位，connection.rs）**并存**——Phase C task 9.x 切换 cdt-api 时再删旧的；这避免 Phase B 改坏现有 `cdt-api::ssh_*` 路径
- 阶段 3 鉴权：`run_auth_chain` 是公共 API + 单测覆盖纯调度逻辑；`session.rs::connect_inner` 阶段 3 inline 跑同一调度（因 `&mut Handle` 串行 borrow 与 callback FnMut 冲突）
- `SshError::Tcp` / `SftpInit` 存 `reason: String` 而非 `source: io::Error/russh::Error`（后者不实现 Serialize）

**下次开工**：Read `openspec/changes/port-ssh-remote-browse/{proposal,design,tasks}.md` + `crates/cdt-ssh/src/{lib,session,auth,host_resolver,request,error}.rs` 即可恢复全部 context。

## 1. 依赖脚手架（`cdt-ssh` + workspace lock）

- [x] 1.1 **spike + pin**：在 `crates/cdt-ssh/Cargo.toml` pin `russh = "0.52"`（design.md D1b 修订：0.46 API 与 design 引用形态不符，升至 0.52） + `russh-sftp = "2"`；用 ~50 行 spike 代码（`crates/cdt-ssh/examples/spike.rs`）验证 `russh::client::connect()` + `Handle::authenticate_password` + `authenticate_publickey + PrivateKeyWithHashAlg` + `best_supported_rsa_hash` + `Handle::channel_open_session()` + `Channel::request_subsystem("sftp")` + `russh_sftp::client::SftpSession::new(channel.into_stream())` + `AgentClient::connect(UnixStream)` 全套；spike 通过后选定 `russh-sftp` 路径（社区 wrapper API 完整覆盖 read_dir/metadata/open_with_flags），不走自组装 packet；spike 文件在 task 1.1 末尾删除
- [x] 1.2 同步 `Cargo.lock` 与 `src-tauri/Cargo.lock`：跑 `cargo check --workspace` + `cargo check --manifest-path src-tauri/Cargo.toml`
- [x] 1.3 在 `crates/cdt-ssh/src/error.rs` 替换原 `SshError` 为结构化 enum：`Tcp { host, reason } / AuthExhausted { attempts } / SftpInit { reason } / RemoteHomeMissing { tried } / Cancelled / Timeout { stage } / Config { reason }`，附 `AuthAttempt { source, outcome, elapsed_ms }` + `AuthSource` + `AuthOutcome` + `TimeoutStage`，实现 `serde::Serialize`（`SshError` 走 `tag = "code"` snake_case；`AuthSource`/`AuthOutcome` 走 `tag = "type"` camelCase；字段名 camelCase）与 `thiserror::Error`；`SshError::code` 字段使用 snake_case 与现有 `ApiError.code` 约定一致。**实现差异**：`Tcp/SftpInit` 内部存 `reason: String` 而非 `source: io::Error`，因 `io::Error` / `russh::Error` 不实现 `Serialize`；? 传播点把 source 字符串化即可
- [x] 1.4 把 `crates/cdt-ssh/src/lib.rs` 重新组织：增 `auth.rs`（候选源构建）/ `host_resolver.rs`（`ssh -G` 子进程）/ `polling_watcher.rs`（远端 SFTP polling 骨架，Phase B 填实现）/ `request.rs`（`SshConnectRequest`）；保留并升级 `connection.rs` / `provider.rs` / `config_parser.rs`
- [x] 1.5 `SshConnectRequest::Debug` impl 把 `password` 字段渲染为 `<redacted>`（无 password 时渲染为 `<none>`），避免被 `tracing::info!(?request)` 等模式误打印；新建在 `crates/cdt-ssh/src/request.rs`，Phase C task 9.x 时由 `cdt-api` 替换原简化版

## 2. `cdt-ssh::host_resolver` —— `ssh -G` 子进程委托（D3）

- [x] 2.1 实现 `resolve_host_via_ssh_g(alias: &str) -> Result<ResolvedHost, SshError>`：`tokio::process::Command::new("ssh").args(&["-G", alias]).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null())`，5s 超时（`SSH_G_TIMEOUT`）；显式关闭 stdin 防被 hook 影响（codex 二审 #7）+ `kill_on_drop(true)`
- [x] 2.2 `parse_ssh_g_output(stdout: &str) -> ResolvedHost`：解析 stdout 行 `<key> <value>` 提取 `hostname` / `port` / `user` / `identityfile`（多行 push 顺序保留）/ `identityagent`（非空且非 `none` 时记录路径，含双引号路径剥引号）
- [x] 2.3 失败 / 超时 / 非零 exit / `ssh` 二进制缺失时降级到 `cdt-ssh::config_parser` 现有解析，返回 `ResolvedHost { degraded: true }`；`config_parser` 也找不到 alias 时返回 `host=alias, port=22, degraded=true` 的最小填充（不报错，让 UI 仍可手填）
- [x] 2.4 单测：`parse_ssh_g_output` pure-fn 测试覆盖正常 / 多 IdentityFile / IdentityAgent=none 跳过 / 引号路径 / 空输出 / 未知关键字 6 类；额外加一条 `#[ignore]` 的 live 集成测试（需系统 ssh 二进制，本地 `cargo test --ignored` 跑）
- [x] 2.5 升级 `config_parser.rs::list_hosts`：原实现已支持非通配符 host alias 列表 + `parse_ssh_config_file` 文件不存在 → 空列表；本 phase 无需改动

## 3. `cdt-ssh::auth` —— 鉴权候选链（D2）

- [x] 3.1 定义 `AuthSource` enum：`IdentityAgent(PathBuf) / EnvAgent / LaunchctlAgent / OnePasswordAgent(PathBuf) / IdentityFile(PathBuf) / DefaultKey(PathBuf) / Password`，serde `#[serde(tag = "type", content = "data", rename_all = "camelCase")]`（落 `error.rs`）
- [x] 3.2 定义 `AuthOutcome` enum：`Success / Failure(String) / Skipped(String)`，serde 同上（落 `error.rs`）
- [x] 3.3 定义 `AuthAttempt { source, outcome, elapsed_ms }`，serde camelCase（`elapsedMs`）（落 `error.rs`）
- [x] 3.4 实现 `build_candidates_with_env(host, platform, auth_method, env_auth_sock)` + 公开 wrapper `build_candidates(...)` 读 env 调内层（纯函数版便于单测，规避 workspace `forbid(unsafe_code)` 与 Rust 1.85+ env::set_var unsafe）：按 D2 顺序构建 7 项（macOS / Linux / Windows 平台分支）
- [x] 3.5 候选 1：`IdentityAgent` 字段（来自 `ResolvedHost.identity_agent`），仅当字段非空且非 `none` 时启用；与候选 4（1Password well-known）路径相同时去重（`contains_identity_agent_path`）
- [x] 3.6 候选 2：`env_auth_sock` 非空 + 与候选 1 路径不同时附加 `EnvAgent`
- [x] 3.7 候选 3（macOS only）：`AuthSource::LaunchctlAgent` 占位入候选；真子进程调用（`launchctl getenv SSH_AUTH_SOCK`）放 Phase B 的 `try_authenticate`
- [x] 3.8 候选 4（macOS only）：`one_password_well_known_paths()` 返回 `~/Library/Group Containers/2BUA8C4S2C.com.1password/t/agent.sock` + `~/.1password/agent.sock` 并与候选 1 去重
- [x] 3.9 候选 5：`IdentityFile`（来自 `ResolvedHost.identity_files` 顺序）
- [x] 3.10 候选 6：默认密钥 fallback `~/.ssh/id_ed25519` → `id_rsa` → `id_ecdsa`，与候选 5 去重（`contains_identity_file_path`）
- [x] 3.11 候选 7：password 模式（仅当 `auth_method == Password` 时构建）
- [x] 3.12 实现 `try_authenticate_via_handle(handle, source, username, password)`（落 `session.rs`）：每个 source 包装 `russh::client::Handle::authenticate_*` 调用 + agent 连接 + 私钥 decode + passphrase 跳过逻辑（`Skipped("requires passphrase, use ssh-add")`）；agent 走 `russh::keys::agent::client::AgentClient::connect(UnixStream)`；私钥走 `load_secret_key + PrivateKeyWithHashAlg`；password 走 `authenticate_password`
- [x] 3.13 实现 `run_auth_chain(candidates, try_fn)`（落 `auth.rs`）：依次尝试到第一个成功，记录每个 attempt；全部失败抛 `SshError::AuthExhausted { attempts }`。**实现差异**：因 `&mut Handle` 串行 borrow 与 callback FnMut 冲突，`session.rs::connect_inner` 阶段 3 inline 跑同一调度逻辑（first success 立即 break / AuthExhausted on all-fail），`run_auth_chain` 仍作为公共 API + 单测覆盖纯调度逻辑（6 个 tokio test）
- [x] 3.14 单测覆盖 7 类 build 组合：macOS 链含 launchctl+1Password / Windows 链跳过 / IdentityAgent 优先 EnvAgent / EnvAgent 路径与 IdentityAgent 同路径去重 / 1Password 路径与 IdentityAgent 同路径去重 / IdentityFile 与 DefaultKey id_ed25519 去重 / Password method 末尾 / SshConfig method 不含 Password（共 49 个 cdt-ssh 单测全过）

## 4. `cdt-ssh::connection` —— 真握手（spec ssh-remote-context Requirement: Establish）

- [x] 4.1 在 `cdt-ssh::session::SshSessionManager::connect(request) -> Result<ContextId, SshError>`：5 阶段（TCP probe 5s → russh transport → auth chain → SFTP open 8s → remote home probe），25s 外层硬超时（`OUTER_TIMEOUT`）。**实现差异**：未替换 `SshConnectionManager::register_connection`（保留旧 placeholder 不破坏 cdt-api 现有路径），新建并行 `SshSessionManager` 占据真握手；Phase C task 9.x 时由 cdt-api 切换调用
- [x] 4.2 TCP probe：`tokio::net::TcpStream::connect` + `tokio::time::timeout(TCP_TIMEOUT)`，失败抛 `SshError::Tcp { host, reason }`，超时抛 `SshError::Timeout { stage: Tcp }`
- [x] 4.3 russh transport：`russh::client::connect_stream(config, tcp, RusshClientHandler)`（用已建 TCP stream 节省一次 connect），失败 wrap 为 `SshError::Tcp`
- [x] 4.4 鉴权候选链 inline 调度：`build_candidates(&resolved, Platform::current(), auth_method)` + 串行循环 `try_authenticate_via_handle` + first-success break / `AuthExhausted` on all-fail
- [x] 4.5 SFTP subsystem open（`channel.request_subsystem("sftp") + SftpSession::new(channel.into_stream())`）+ `tokio::time::timeout(SFTP_TIMEOUT)` + `SftpInit { reason }` / `Timeout { Sftp }`
- [x] 4.6 remote home probe：`run_remote_command` 跑 `printf %s "$HOME"`（spec 显式允许的唯一远端命令）拿 real_home，依次试 `<home>/.claude/projects` → `/home/<user>/.claude/projects` → `/Users/<user>/.claude/projects` → `/root/.claude/projects` 4 个 fallback；都不存在抛 `RemoteHomeMissing { tried }`
- [x] 4.7 `disconnect(context_id)`：从 sessions Map remove + drop SftpSession Arc + `handle.disconnect(ByApplication, "", "")` + emit `Disconnected` 状态；若被断开是 active 切回 Local。polling watcher 停止留 task 6.x（polling 还没接入）
- [x] 4.8 `test_connection(request)`：跑同 connect_inner 流程，成功立即 drop 资源，不注册 active context；返 `Vec<AuthAttempt>` 让 UI 显示"试过哪些候选源"诊断
- [x] 4.9 `subscribe_status() -> broadcast::Receiver<SshStatusChange>`：`broadcast::Sender` 容量 128（`STATUS_CHANNEL_CAP`）；payload 含 `contextId / status / authChain / error`，error 状态附 `auth_chain` 进度
- [x] 4.10 `subscribe_context_changed() -> broadcast::Receiver<ContextChanged>`：`switch_context` / 自动切回 Local 时发出 `{ activeContextId, kind: local|ssh }`
- [x] 4.11 graceful disconnect on app exit：`shutdown_all(deadline: Duration)` 用 `futures::future::join_all` 并发断开所有 SSH context + 外层 `tokio::time::timeout(deadline)` 兜底（默认 `SHUTDOWN_TIMEOUT = 3s`）
- [x] 4.12 强制单 active SSH：`connect` 入口检测当前 active 不是新 context_id 时先 `disconnect(prev)`

## 5. `cdt-ssh::provider` —— 真实 `SshFileSystemProvider`（spec Requirement: Read sessions and files over SSH）

- [x] 5.1 替换 placeholder：`SshFileSystemProvider::new(context_id, sftp: Arc<Mutex<SftpSession>>, remote_home: PathBuf)` 持有 SFTP client handle 引用（与 `SshSessionResources` 共享同一 `Arc<Mutex<SftpSession>>`）；额外 `with_client` 构造器接 `Arc<dyn SftpClient>` 用于单测注入 fake
- [x] 5.2 实现 `exists(path)`：SFTP `try_exists` 不存在路径返 `false`，其他错误（含 permission denied）降级 `false` 与 `LocalFileSystemProvider` 对齐；放在 `with_retry` 内容忍瞬时抖动
- [x] 5.3 实现 `read_to_string(path)`：SFTP `read` 全量读 + UTF-8 decode（失败 → `FsError::Utf8`），错误映射走 `map_client_error`
- [x] 5.4 实现 `read_dir(path)`：SFTP `read_dir` 返回 entry 列表，把 `russh-sftp` 的 `DirEntry` 映射为 `cdt-discover::DirEntry`（含 size/mtime metadata）；`.jsonl` 过滤交给上层 scanner
- [x] 5.5 实现 `stat(path)`：SFTP `metadata`，map 到 `FsMetadata { size, mtime }`；不存在路径返 `FsError::NotFound`
- [x] 5.6 实现 `open_read_stream(path)`：inherent method（不进 `FileSystemProvider` trait——避免污染 `LocalFileSystemProvider`），生产路径走 `SftpSession::open` 返回 `russh_sftp::client::fs::File`（实现 `AsyncRead + AsyncSeek`）；测试路径返 `FsError::Unsupported("open_read_stream")`
- [x] 5.7 实现 `with_retry(op, max=3, backoff=75ms*attempt)` helper：覆盖 `StatusCode::Failure`(=4) / `Timeout` / IO 字符串匹配 `EAGAIN` / `would block` / `connection reset` / `econnreset` / `etimedout` / `timed out` / `epipe` / `broken pipe`（russh-sftp 0.2 把 `io::Error` 转 `String`，匹配字符串是无奈但已加注释跟踪后续升级）
- [x] 5.8 单测：fake `SftpClient` impl 覆盖 kind=ssh / exists 正常 / exists 不存在 / read_to_string 正常 / read_to_string permission denied 映射 `FsError::Io { kind: PermissionDenied }` / stat NotFound / stat 正常 / read_dir 多 entry / read_lines_head 正常 / read_to_string 瞬时错误重试 2 次后成功 / read_to_string 超 retry 上限返 Transient / open_read_stream fake 路径 Unsupported / `is_transient` 真值表 / `classify_sftp_error` StatusCode 4 类映射 / `classify_sftp_error` IO 字符串 5 类映射（共 15 个 provider 单测全过）

## 6. `cdt-ssh::polling_watcher` —— 远端 SFTP polling（spec ssh-remote-context Requirement: Watch remote project directories + file-watching ADDED Requirement）

- [x] 6.1 实现 `RemotePollingWatcher::spawn(provider, projects_root, sender, cancel_token)`：3s 间隔 tokio task，持有 `BTreeMap<PathBuf, FileFingerprint { size: u64, mtime: Option<SystemTime> }>` baseline
- [x] 6.2 第一次 poll 不发事件，仅建 baseline
- [x] 6.3 后续 poll diff baseline，对差异 emit `cdt_watch::FileChangeEvent { project_id, session_id, deleted, project_list_changed: false }`；差异判定 SHALL 按 size + mtime 双维度：(a) 新增 (b) size 变化 (c) size 不变 mtime 变化 (d) 删除
- [x] 6.4 mtime 缺失（`mtime = None`）时退化为 size-only fingerprint，并通过 `tracing::debug!(target: "cdt_watch::ssh_polling", "mtime missing")` 记录一次（不 spam）
- [x] 6.5 30s catch-up timer 强制全量 readdir + size + mtime 双维度 diff
- [x] 6.6 瞬时 SFTP 错误跳过本轮，不停 watcher
- [x] 6.7 `cancel_token.cancelled()` 1s 内退出
- [x] 6.8 单测：mock SFTP 客户端模拟 5 类差异（新增 / size 变化 / mtime 变化 / mtime 缺失退化 / 删除）

## 7. `cdt-config` —— SSH 字段持久化（spec configuration-management）

- [x] 7.1 在 `crates/cdt-config/src/types.rs` 新增 `SshConfig { profiles: Vec<SshProfile>, last_connection: Option<SshLastConnection>, auto_reconnect: bool }`（本仓无 `store.rs`，配置类型真相源为 `types.rs`）
- [x] 7.2 `SshProfile { id, name, host, port, username, auth_method, private_key_path }` 与 `SshLastConnection { host, port, username, auth_method, context_id }`，`SshAuthMethod` enum `SshConfig` / `Password`，serde camelCase。**实现差异**：保留旧 `id/private_key_path` 字段兼容已存在配置；未加 `password_required` 布尔字段，password 模式由 `auth_method` 表达且密码不持久化
- [x] 7.3 `default_config()` 设 `ssh: SshConfig { profiles: vec![], last_connection: None, auto_reconnect: false }`
- [x] 7.4 实现 `validate_ssh_config(ssh: &SshConfig) -> Result<(), ConfigError>`：host 非空 / port 1-65535 / username 非空 / auth_method 合法 / profile name 非空 + 唯一
- [x] 7.5 接入 `update_config("ssh", value)` 路径：调 `validate_ssh_config` 全部通过才落盘
- [x] 7.6 实现 `save_ssh_last_connection(...)` helper：从 `SshConnectRequest` 映射，strip password 后落盘
- [x] 7.7 单测：IPC contract 覆盖默认值物化 / 部分字段合并 / validation 失败 / save_last_connection 不含 password；`cargo test -p cdt-config` 全过

## 8. `cdt-watch` —— 远端 watcher 接入（capability: file-watching）

- [x] 8.1 在 `crates/cdt-watch/src/lib.rs` 暴露 `FileWatcher::attach_remote(provider, projects_root, cancel_token)` 方法（替代之前可能的 stub）
- [x] 8.2 内部 spawn `cdt-ssh::polling_watcher`，把发出的 `FileChangeEvent` 喂入既有 broadcast channel（与本地 `notify` 事件流共享）
- [x] 8.3 `attach_remote` 返回 `RemoteWatcherHandle { cancel_token }` 让 connection manager 在 disconnect 时调 `cancel`
- [x] 8.4 单测：远端事件经 broadcast 与本地事件 schema 一致

## 9. `cdt-api::LocalDataApi` —— 接入真握手 + 状态广播（capability: ipc-data-api）

- [x] 9.1 在 `crates/cdt-api/src/ipc/local.rs::ssh_connect` 实现替换：调真 `SshSessionManager::connect(request)`，订阅 status broadcast 由 Tauri bridge 推送至前端。**实现差异**：旧 `SshConnectionManager` placeholder 保留给构造器签名兼容，但 `LocalDataApi` 内部不再使用
- [x] 9.2 `ssh_disconnect` 调真 `disconnect`
- [x] 9.3 `ssh_test_connection` 调 `test_connection`
- [x] 9.4 `ssh_get_state(context_id?)` 默认返回 active 状态；v1 API 为全局 `SshState { activeContextId, contexts }`
- [x] 9.5 `ssh_get_config_hosts` 返回 `ConfigManager::get_ssh_config().profiles`；host alias live 解析仍走 `ssh_resolve_host`
- [x] 9.6 `ssh_resolve_host` 调 `cdt-ssh::host_resolver::resolve_host_via_ssh_g`
- [x] 9.7 `ssh_save_last_connection` / `ssh_get_last_connection` 通过 `ConfigManager::save_ssh_last_connection` / `get_ssh_last_connection`
- [x] 9.8 `list_contexts` / `switch_context` / `get_active_context` 走 `SshSessionManager` 的 registry/state
- [x] 9.9 在 `LocalDataApi` 上加 `subscribe_ssh_status() -> broadcast::Receiver<SshStatusChange>` 与 `subscribe_context_changed()`，并加 `shutdown_ssh_all(deadline)`
- [x] 9.10 升级 `crates/cdt-api/src/http/routes.rs`：保留现有 `/api/ssh/connect` / `/api/ssh/disconnect` / `/api/ssh/resolve-host` 与 `/api/contexts` / `/api/contexts/switch` 5 条路由的 payload schema 兼容（`host` + `hostAlias` alias），并补齐 test/state/config-hosts/last-connection/active-context HTTP 路由

## 10. `src-tauri` —— Tauri command 注册 + capabilities + emit 桥

- [x] 10.1 在 `src-tauri/src/lib.rs` 写 11 条新命令的 wrapper 函数（snake_case 命令名）：`ssh_connect` / `ssh_disconnect` / `ssh_test_connection` / `ssh_get_state` / `ssh_get_config_hosts` / `ssh_resolve_host` / `ssh_save_last_connection` / `ssh_get_last_connection` / `list_contexts` / `switch_context` / `get_active_context`，每条调对应 `LocalDataApi` 方法 + `Result` 序列化；wrapper 函数未 `tracing::info!(?request)` 整个请求结构体，仅 log `host` / `username` / `authMethod` 三个非敏感字段
- [x] 10.2 在 `invoke_handler!` 注册 11 条命令
- [x] 10.3 `setup` 阶段 spawn 桥任务订阅 `LocalDataApi::subscribe_ssh_status()`，对每条事件 `app.emit("ssh_status", payload)`（payload camelCase）
- [x] 10.4 同样订阅 `subscribe_context_changed()` emit `context_changed`
- [x] 10.5 `WindowEvent::CloseRequested` 事件 hook：调 `SshSessionManager::shutdown_all(SHUTDOWN_TIMEOUT)` 经 `LocalDataApi::shutdown_ssh_all`
- [x] 10.6 `src-tauri/capabilities/default.json` 已有默认 command capability；`ssh -G` / `launchctl getenv` 在 Rust 后端子进程执行，不走 Tauri shell plugin，因此本 phase 未新增 shell permission。Tauri 11 command 清单通过 `invoke_handler!` + `EXPECTED_TAURI_COMMANDS` + `KNOWN_TAURI_COMMANDS` 同步约束
- [x] 10.7 IPC contract test：在 `crates/cdt-api/tests/ipc_contract.rs` 加 11 条命令清单、payload schema 断言（字段名 camelCase / enum tag 值）；显式断言 `AuthSource` / `AuthOutcome` / `SshError` 序列化形态与 snake_case code
- [x] 10.8 grep 验证：`grep -r "tracing::.*[?:].*request\\|tracing::.*[?:].*body" src-tauri/src/lib.rs crates/cdt-api/src/ipc/local.rs` 无命中（防 password 被 Debug 派生路径泄露）

## 11. UI —— `lib/api.ts` IPC wrapper + stores

- [x] 11.1 在 `ui/src/lib/api.ts` 加 11 条 IPC wrapper 函数（types from `lib/types/ssh.ts`）+ `Tauri::listen("ssh_status")` / `listen("context_changed")` 订阅 helper
- [x] 11.2 `ui/src/lib/types/ssh.ts` 定义 `SshConnectionStatus` / `SshConnectionResult` / `AuthAttempt` / `ContextSummary` / `SshProfile` / `SshLastConnection` 类型
- [x] 11.3 `ui/src/lib/stores/connection.ts`：Svelte 5 rune store 持有当前连接状态 + auth chain 进度 + 表单字段
- [x] 11.4 `ui/src/lib/stores/context.ts`：rune store 持有 `availableContexts[]` + `activeContextId`，监听 `context_changed` 事件
- [x] 11.5 contextSwitch 逻辑：切换前显示 ContextSwitchOverlay，`context_changed` 后退场

## 12. UI —— Connection Section 与三个组件（capability: settings-ui）

- [x] 12.1 `ui/src/routes/settings/Connection.svelte`：host combobox（联想 + alias 选中后调 `ssh_resolve_host` 自动填充） / port / username / authMethod 单选 / password（条件显示）/ Connect / Test / Save as profile / Disconnect 按钮 / saved profiles 列表 / Windows 平台 inline 提示
- [x] 12.2 在 Settings section 导航加 Connection tab，仅 Tauri 桌面渲染（前端检测 `window.__TAURI_INTERNALS__`）
- [x] 12.3 `ui/src/lib/components/WorkspaceIndicator.svelte`：右下角 fixed 浮动 pill；仅 `availableContexts.length > 1` 时渲染；图标 `lucide-svelte::Wifi` 绿色 + host 名；点击下拉切换 workspace
- [x] 12.4 `ui/src/lib/components/ContextSwitchOverlay.svelte`：全屏半透明 loading；接 `context_changed` 事件后退场
- [x] 12.5 `ui/src/lib/components/ConnectionStatusBadge.svelte`：disconnected/connecting/connected/error 四态映射图标 + 颜色 + 错误悬浮提示
- [x] 12.6 ProjectList / SessionDetail 不感知数据源切换 —— 仅依赖现有 IPC，确认无 hardcode `local`

## 13. 测试

- [x] 13.1 `cargo test -p cdt-ssh`：单元测试覆盖 host_resolver / auth / connection / provider / polling_watcher（86 passed / 2 live ignored）
- [x] 13.2 `cargo test -p cdt-api --test ipc_contract`：11 条新命令 payload schema（83 passed）
- [x] 13.3 `cargo test -p cdt-config`：SSH 字段 validation 全过（123 unit + 8 round_trip passed）
- [x] 13.4 Vitest + mockIPC：connection store / context store / Connection.svelte 表单校验（299 passed / 1 skipped；补 `Connection.test.ts` 端口范围 + alias/test 状态；补 `tauriMock.test.ts` SSH payload args）
- [x] 13.5 Playwright user story：打开 Settings → Connection → 输入 host alias → 看到联想 + 显示状态（`connection-settings.spec.ts` mock IPC 通过，不真连远端）
- [x] 13.6 `just preflight` 全过（fmt / lint / test / spec-validate；OpenSpec 26/26 + IPC command sync OK）

## 14. perf 验证 + 集成 smoke

- [ ] 14.1 idle CPU 实测：`just dev` 跑应用 + 连一个本地 ssh server（或用 docker `linuxserver/openssh-server`），观察 Activity Monitor 远端 polling watcher 启动后 idle CPU 稳态 < 2%；不达标降级为 5s 间隔 + 仅 readdir 维度（更新 D5 + spec Scenario）。**实现差异**：bg session 无法完成 GUI Activity Monitor；自动化已跑 `pnpm --dir ui exec playwright test tests/e2e/connection-settings.spec.ts` 与 `just preflight`，真实 idle CPU smoke 留 PR manual checklist
- [ ] 14.2 远端拉大会话 perf：用本地 docker ssh server 模拟"远端 ~/.claude/projects 含一个 1221 msg session"，跑 `get_session_detail` 记录 wall / user / sys / RSS / user/real ratio，对照本地 baseline 60-74ms 的远端容差是否合理；不合理在 PR 描述里说明。**实现差异**：本机缺 `~/.claude/projects/-perf-fixture-project`，`perf_get_session_detail` release bench 仅完成 smoke（2 passed，内部跳过样本），远端/docker perf 留 PR manual checklist
- [ ] 14.3 macOS Launchpad 启动场景手动验证：从 dmg 安装的 release build → Launchpad 启动 → 进 Connection tab → 连 1Password 管理的密钥 host 成功。**实现差异**：bg session 无 dmg/Launchpad/1Password GUI 环境，留 PR manual checklist
- [ ] 14.4 Windows 兼容验证：`ssh` 二进制存在场景 + 缺失场景 各跑一遍 `ssh_resolve_host`，确认 degraded fallback 工作。**实现差异**：已跑 Windows 静态 grep + `windows-compat-reviewer` 只读审查并修复 home/canonicalize/路径名问题；真实 Windows smoke 留 PR manual checklist
- [x] 14.5 远端 `~/.claude/projects` 不存在场景：mock 远端 home 仅有非 .claude 目录，确认返回 `RemoteHomeMissing { tried: [...] }` 错误且 SSH 仍 connected（`SshError::RemoteHomeMissing` serde 单测 + session home probe 错误路径覆盖）

## N. 发布

- [x] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

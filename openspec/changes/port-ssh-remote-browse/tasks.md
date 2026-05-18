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
- [ ] 3.12 实现 `try_authenticate(client, source) -> Result<(), AuthAttempt>`：每个 source 包装 `russh::client::Handle::authenticate_*` 调用 + agent 连接 + 私钥 decode + passphrase 跳过逻辑（`Skipped("requires passphrase, use ssh-add")`）—— Phase B
- [ ] 3.13 实现外层 `run_auth_chain(client, candidates)`：依次尝试到第一个成功，记录每个 attempt；全部失败抛 `SshError::AuthExhausted { attempts }` —— Phase B
- [x] 3.14 单测覆盖 7 类 build 组合：macOS 链含 launchctl+1Password / Windows 链跳过 / IdentityAgent 优先 EnvAgent / EnvAgent 路径与 IdentityAgent 同路径去重 / 1Password 路径与 IdentityAgent 同路径去重 / IdentityFile 与 DefaultKey id_ed25519 去重 / Password method 末尾 / SshConfig method 不含 Password（共 49 个 cdt-ssh 单测全过）

## 4. `cdt-ssh::connection` —— 真握手（spec ssh-remote-context Requirement: Establish）

- [ ] 4.1 替换 `SshConnectionManager::register_connection` 为 `connect(request) -> Result<ContextId, SshError>`：5 阶段（TCP probe 5s → russh transport → auth chain → SFTP open 8s → remote home probe），25s 外层硬超时
- [ ] 4.2 实现 TCP probe：`tokio::net::TcpStream::connect_timeout`，失败抛 `SshError::Tcp`
- [ ] 4.3 实现 russh transport：`russh::client::connect(config, addr, handler)` + `Handle::wait_for_auth_request`
- [ ] 4.4 接 `auth.rs::run_auth_chain`，失败抛 `AuthExhausted`
- [ ] 4.5 实现 SFTP subsystem open（按 1.1 选型）+ 8s 超时 + `SftpInit` 错误
- [ ] 4.6 实现 remote home probe：发 `printf %s "$HOME"` 唯一在远端跑的命令；尝试 `<home>/.claude/projects` → `/home/<user>/.claude/projects` → `/Users/<user>/.claude/projects` → `/root/.claude/projects` 4 个 fallback；都不存在抛 `RemoteHomeMissing { tried }`
- [ ] 4.7 实现 `disconnect(context_id)`：关闭 SFTP + transport + TCP + 停止该 context 的 polling watcher（cancellation token）
- [ ] 4.8 实现 `test_connection(request)`：跑同 connect 流程，成功立即关闭，不注册 active context
- [ ] 4.9 实现 `subscribe_status() -> broadcast::Receiver<SshStatusChange>`：`broadcast::Sender<SshStatusChange>` 容量 128；`connecting` 状态携带当前 auth chain 进度
- [ ] 4.10 实现 `subscribe_context_changed() -> broadcast::Receiver<ContextChanged>`：在 `switch_context` / 自动切回 Local 时发出
- [ ] 4.11 实现 graceful disconnect on app exit：`shutdown_all(timeout: Duration)` 并发断开所有 SSH context，最长 3s
- [ ] 4.12 强制单 active SSH：连接新 host 前先 `disconnect` 当前 active SSH context

## 5. `cdt-ssh::provider` —— 真实 `SshFileSystemProvider`（spec Requirement: Read sessions and files over SSH）

- [ ] 5.1 替换 placeholder：`SshFileSystemProvider` 持有 SFTP client handle 引用
- [ ] 5.2 实现 `exists(path)`：SFTP `stat` 不存在路径返回 `false`，其他错误透传
- [ ] 5.3 实现 `read_to_string(path)`：SFTP `read_file` 全量读 + UTF-8 decode
- [ ] 5.4 实现 `read_dir(path)`：SFTP `read_dir` 返回 entry 列表（`.jsonl` 过滤交给上层 scanner）
- [ ] 5.5 实现 `stat(path)`：SFTP `stat`，map 到 `cdt-discover::FileMetadata { size, mtime, is_dir }`
- [ ] 5.6 实现 `open_read_stream(path)`：SFTP `open_read` 返回 `AsyncRead` impl（流式读 JSONL，支持大会话）
- [ ] 5.7 实现 `with_retry(op, max=3, backoff=75ms*attempt)` helper：覆盖 `code=4` / `EAGAIN` / `ECONNRESET` / `ETIMEDOUT` / `EPIPE`
- [ ] 5.8 单测：mock SFTP 客户端覆盖正常 + permission denied + 瞬时错误重试 3 类

## 6. `cdt-ssh::polling_watcher` —— 远端 SFTP polling（spec ssh-remote-context Requirement: Watch remote project directories + file-watching ADDED Requirement）

- [ ] 6.1 实现 `RemotePollingWatcher::spawn(provider, projects_root, sender, cancel_token)`：3s 间隔 tokio task，持有 `BTreeMap<PathBuf, FileFingerprint { size: u64, mtime: Option<SystemTime> }>` baseline
- [ ] 6.2 第一次 poll 不发事件，仅建 baseline
- [ ] 6.3 后续 poll diff baseline，对差异 emit `cdt_watch::FileChangeEvent { project_id, session_id, deleted, project_list_changed: false }`；差异判定 SHALL 按 size + mtime 双维度：(a) 新增 (b) size 变化 (c) size 不变 mtime 变化 (d) 删除
- [ ] 6.4 mtime 缺失（`mtime = None`）时退化为 size-only fingerprint，并通过 `tracing::debug!(target: "cdt_watch::ssh_polling", "mtime missing")` 记录一次（不 spam）
- [ ] 6.5 30s catch-up timer 强制全量 readdir + size + mtime 双维度 diff
- [ ] 6.6 瞬时 SFTP 错误跳过本轮，不停 watcher
- [ ] 6.7 `cancel_token.cancelled()` 1s 内退出
- [ ] 6.8 单测：mock SFTP 客户端模拟 5 类差异（新增 / size 变化 / mtime 变化 / mtime 缺失退化 / 删除）

## 7. `cdt-config` —— SSH 字段持久化（spec configuration-management）

- [ ] 7.1 在 `crates/cdt-config/src/store.rs` 新增 `SshConfig { profiles: Vec<SshProfile>, last_connection: Option<SshLastConnection>, auto_reconnect: bool }`
- [ ] 7.2 `SshProfile { name, host, port, username, auth_method: AuthMethodKind, password_required }` 与 `SshLastConnection { host, port, username, auth_method }`，`AuthMethodKind` enum `SshConfig` / `Password`，serde camelCase
- [ ] 7.3 `Config::default()` 设 `ssh: SshConfig { profiles: vec![], last_connection: None, auto_reconnect: false }`
- [ ] 7.4 实现 `validate_ssh(ssh: &SshConfig) -> Result<(), ValidationError>`：host 非空 / port 1-65535 / username 非空 / auth_method 合法 / profile name 非空 + 唯一
- [ ] 7.5 接入 `update_config("ssh", value)` 路径：调 `validate_ssh` 全部通过才落盘
- [ ] 7.6 实现 `save_last_connection(host, port, username, auth_method)` helper：strip password 后落盘
- [ ] 7.7 单测：默认值物化 / 部分字段合并 / 各类 validation 失败 / save_last_connection 不含 password 5 类

## 8. `cdt-watch` —— 远端 watcher 接入（capability: file-watching）

- [ ] 8.1 在 `crates/cdt-watch/src/lib.rs` 暴露 `FileWatcher::attach_remote(provider, projects_root, cancel_token)` 方法（替代之前可能的 stub）
- [ ] 8.2 内部 spawn `cdt-ssh::polling_watcher`，把发出的 `FileChangeEvent` 喂入既有 broadcast channel（与本地 `notify` 事件流共享）
- [ ] 8.3 `attach_remote` 返回 `RemoteWatcherHandle { cancel_token }` 让 connection manager 在 disconnect 时调 `cancel`
- [ ] 8.4 单测：远端事件经 broadcast 与本地事件 schema 一致

## 9. `cdt-api::LocalDataApi` —— 接入真握手 + 状态广播（capability: ipc-data-api）

- [ ] 9.1 在 `crates/cdt-api/src/ipc/local.rs::ssh_connect` 实现替换：调 `SshConnectionManager::connect(request)`，订阅 status broadcast 推送至前端
- [ ] 9.2 `ssh_disconnect` 调真 `disconnect`
- [ ] 9.3 `ssh_test_connection` 调 `test_connection`
- [ ] 9.4 `ssh_get_state(context_id?)` 默认返回 active 状态
- [ ] 9.5 `ssh_get_config_hosts` 调 `cdt-ssh::config_parser::list_hosts`
- [ ] 9.6 `ssh_resolve_host` 调 `cdt-ssh::host_resolver::resolve_host_via_ssh_g`
- [ ] 9.7 `ssh_save_last_connection` / `ssh_get_last_connection` 通过 `ConfigManager::save_last_connection` / `get_last_connection`
- [ ] 9.8 `list_contexts` / `switch_context` / `get_active_context` 走 `SshConnectionManager` 的 registry
- [ ] 9.9 在 `LocalDataApi` 上加 `subscribe_ssh_status() -> broadcast::Receiver<SshStatusChange>` 与 `subscribe_context_changed()`
- [ ] 9.10 升级 `crates/cdt-api/src/http/routes.rs`：保留现有 `/api/ssh/connect` / `/api/ssh/disconnect` / `/api/ssh/resolve-host` 与 `/api/contexts` / `/api/contexts/switch` 5 条路由的 payload schema **不变**（仍接 `SshConnectRequest` / `SwitchContextBody` 等），仅底层接入真握手；本 change 不阻塞 HTTP 端补齐余下 7 条路由（`ssh_test_connection` / `ssh_get_state` / `ssh_get_config_hosts` / `ssh_save_last_connection` / `ssh_get_last_connection` / `list_contexts` HTTP 已有 / `get_active_context`），按需另开 follow-up

## 10. `src-tauri` —— Tauri command 注册 + capabilities + emit 桥

- [ ] 10.1 在 `src-tauri/src/lib.rs` 写 11 条新命令的 wrapper 函数（snake_case 命令名）：`ssh_connect` / `ssh_disconnect` / `ssh_test_connection` / `ssh_get_state` / `ssh_get_config_hosts` / `ssh_resolve_host` / `ssh_save_last_connection` / `ssh_get_last_connection` / `list_contexts` / `switch_context` / `get_active_context`，每条调对应 `LocalDataApi` 方法 + `Result` 序列化；wrapper 函数 SHALL NOT `tracing::info!(?request)` 整个请求结构体（避免 `password` 字段被打印），仅 log `host` / `username` / `authMethod` 三个非敏感字段
- [ ] 10.2 在 `invoke_handler!` 注册 11 条命令
- [ ] 10.3 `setup` 阶段 spawn 桥任务订阅 `LocalDataApi::subscribe_ssh_status()`，对每条事件 `app.emit("ssh_status", payload)`（payload camelCase）
- [ ] 10.4 同样订阅 `subscribe_context_changed()` emit `context_changed`
- [ ] 10.5 `WindowEvent::CloseRequested` 事件 hook：调 `SshConnectionManager::shutdown_all(Duration::from_secs(3))`
- [ ] 10.6 `src-tauri/capabilities/default.json` 加 `ssh:*` IPC scope（11 条命令）+ `core:command:execute` 仅允许 `ssh` 与 `launchctl` 二进制（用于 `ssh -G` 与 `launchctl getenv`）
- [ ] 10.7 IPC contract test：在 `crates/cdt-api/tests/ipc_contract.rs` 加 11 条命令的 payload schema 断言（字段名 camelCase / enum tag 值 / `xxxOmitted` 命名）；显式断言 `AuthSource` / `AuthOutcome` / `SshError` 三个 enum 序列化为 `{ "type": "...", "data"?: ... }` 形态（与 ssh-remote-context spec Scenario "AuthAttempt serialization shape" 对齐），并断言 `SshError::code` 字段使用 snake_case
- [ ] 10.8 grep 验证：`grep -r "tracing::.*[?:].*request" src-tauri/src/lib.rs crates/cdt-api/src/ipc/local.rs` SHALL 无命中（防 password 被 Debug 派生路径泄露），CI 加该 check 兜底

## 11. UI —— `lib/api.ts` IPC wrapper + stores

- [ ] 11.1 在 `ui/src/lib/api.ts` 加 11 条 IPC wrapper 函数（types from `lib/types/ssh.ts`）+ `Tauri::listen("ssh_status")` / `listen("context_changed")` 订阅 helper
- [ ] 11.2 `ui/src/lib/types/ssh.ts` 定义 `SshConnectionStatus` / `SshConnectionResult` / `AuthAttempt` / `ContextSummary` / `SshProfile` / `SshLastConnection` 类型
- [ ] 11.3 `ui/src/lib/stores/connection.ts`：Svelte 5 rune store 持有当前连接状态 + auth chain 进度 + 表单字段
- [ ] 11.4 `ui/src/lib/stores/context.ts`：rune store 持有 `availableContexts[]` + `activeContextId`，监听 `context_changed` 事件
- [ ] 11.5 contextSwitch 逻辑：切换前显示 ContextSwitchOverlay，`context_changed` 后退场

## 12. UI —— Connection Section 与三个组件（capability: settings-ui）

- [ ] 12.1 `ui/src/routes/settings/Connection.svelte`：host combobox（联想 + alias 选中后调 `ssh_resolve_host` 自动填充） / port / username / authMethod 单选 / password（条件显示）/ Connect / Test / Save as profile / Disconnect 按钮 / saved profiles 列表 / Windows 平台 inline 提示
- [ ] 12.2 在 Settings section 导航加 Connection tab，仅 Tauri 桌面渲染（前端检测 `window.__TAURI_INTERNALS__`）
- [ ] 12.3 `ui/src/lib/components/WorkspaceIndicator.svelte`：右下角 fixed 浮动 pill；仅 `availableContexts.length > 1` 时渲染；图标 `lucide-svelte::Wifi` 绿色 + host 名；点击下拉切换 workspace
- [ ] 12.4 `ui/src/lib/components/ContextSwitchOverlay.svelte`：全屏半透明 loading；接 `context_changed` 事件后退场
- [ ] 12.5 `ui/src/lib/components/ConnectionStatusBadge.svelte`：disconnected/connecting/connected/error 四态映射图标 + 颜色 + 错误悬浮提示
- [ ] 12.6 ProjectList / SessionDetail 不感知数据源切换 —— 仅依赖现有 IPC，确认无 hardcode `local`

## 13. 测试

- [ ] 13.1 `cargo test -p cdt-ssh`：单元测试覆盖 host_resolver / auth / connection / provider / polling_watcher
- [ ] 13.2 `cargo test -p cdt-api --test ipc_contract`：11 条新命令 payload schema
- [ ] 13.3 `cargo test -p cdt-config`：SSH 字段 validation 全过
- [ ] 13.4 Vitest + mockIPC：connection store / context store / Connection.svelte 表单校验
- [ ] 13.5 Playwright user story：打开 Settings → Connection → 输入 host alias → 看到联想 + 显示状态（mock IPC 不真连远端）
- [ ] 13.6 `just preflight` 全过（fmt / lint / test / spec-validate）

## 14. perf 验证 + 集成 smoke

- [ ] 14.1 idle CPU 实测：`just dev` 跑应用 + 连一个本地 ssh server（或用 docker `linuxserver/openssh-server`），观察 Activity Monitor 远端 polling watcher 启动后 idle CPU 稳态 < 2%；不达标降级为 5s 间隔 + 仅 readdir 维度（更新 D5 + spec Scenario）
- [ ] 14.2 远端拉大会话 perf：用本地 docker ssh server 模拟"远端 ~/.claude/projects 含一个 1221 msg session"，跑 `get_session_detail` 记录 wall / user / sys / RSS / user/real ratio，对照本地 baseline 60-74ms 的远端容差是否合理；不合理在 PR 描述里说明
- [ ] 14.3 macOS Launchpad 启动场景手动验证：从 dmg 安装的 release build → Launchpad 启动 → 进 Connection tab → 连 1Password 管理的密钥 host 成功
- [ ] 14.4 Windows 兼容验证：`ssh` 二进制存在场景 + 缺失场景 各跑一遍 `ssh_resolve_host`，确认 degraded fallback 工作
- [ ] 14.5 远端 `~/.claude/projects` 不存在场景：mock 远端 home 仅有非 .claude 目录，确认返回 `RemoteHomeMissing { tried: [...] }` 错误且 SSH 仍 connected

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

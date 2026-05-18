## Context

claude-devtools-rs 是 TS 原版 `claude-devtools`（Electron）的 Rust 端口（Tauri 2 + Svelte 5）。TS 原版有完整生产 SSH 远程会话浏览：用户在 Settings → Connection 输入 host alias，应用通过 SFTP 拉远端 `~/.claude/projects` 的 JSONL，UI 复用本地浏览组件，右下角 `WorkspaceIndicator` 浮动 pill 切换 local / ssh-host context。核心代码 `SshConnectionManager.ts`(911 行) + `SshFileSystemProvider.ts`(274 行) + `ServiceContextRegistry.ts`(220 行) + `SshConfigParser.ts`(316 行) + `SshHostResolver.ts`(112 行) + `ssh.ts`(IPC handlers) + `connectionSlice.ts` / `contextSlice.ts` / `ConnectionSection.tsx` / `WorkspaceIndicator.tsx` / `ContextSwitchOverlay.tsx`，技术栈 `ssh2` v1.17（libssh2 binding）+ `ssh-config` v5（解析）+ 系统 `ssh -G <host>` 子进程（高级特性兜底）。

Rust 端口现状：
- `crates/cdt-ssh` 已建但**不依赖任何真 SSH 库**（无 `russh`/`openssh`/`ssh2`），`config_parser.rs` 仅解析最基础的 Host/HostName/User/Port/IdentityFile，`connection.rs` 是状态机（无网络），`provider.rs` 全部返回 `not connected`。
- `openspec/specs/ssh-remote-context/spec.md` 已存在，4 个 Requirement 中：context 切换 + 状态查询已实现；真握手 + SFTP 读未实现（archive change `port-ssh-remote-context` 标 placeholder 留待后续）。
- `crates/cdt-api` HTTP 路由暴露 3 条 `/api/ssh/*`；`DataApi` trait 有 5 个 `ssh_*` / `context_*` 方法；`LocalDataApi` 实现仅做 config 解析 + 状态登记（无真握手）。
- `src-tauri/src/lib.rs::invoke_handler!` **未注册任何 ssh / context 命令**；`src-tauri/capabilities/default.json` 无 ssh scope；UI `grep ssh` 全无。

约束：
- `.claude/rules/perf.md` 规定辅助工具 idle CPU < 2%，远端 polling watcher（3s 间隔）SHALL 验证此阈值。
- `.claude/rules/codex-usage.md` 第 3 节命中本 change（跨 5 个 capability + 新增 IPC 字段 + 性能关键路径 polling + 状态机 + UI 重大重构），design 阶段 codex 二审强制。
- 双 lock 同步：引入 `russh` 系列依赖会同时改 `Cargo.lock` 与 `src-tauri/Cargo.lock`，`just release-check` 正常处理。
- macOS Tauri 应用从 Launchpad 启动时进程环境**没有 `SSH_AUTH_SOCK`**，必须查 `launchctl getenv SSH_AUTH_SOCK`——TS 原版 911 行核心代码的关键场景。

## Goals / Non-Goals

**Goals**：
- macOS 桌面用户开箱即用：从 Launchpad 启动后能用 ssh-agent / 1Password 连接到任意 `~/.ssh/config` 已配 host
- UX 与 TS 原版对齐（Connection tab 字段 / WorkspaceIndicator pill / ContextSwitchOverlay loading）
- 远程会话浏览数据形状与本地一致，下游 `project-discovery` / `session-parsing` / `chunk-building` 无感
- 错误诊断结构化（Rust enum），可观测性融入项目 `tracing` perf target
- 性能：远端 polling 不超 idle CPU 阈值；远端拉大会话目标 wall < 3s（10k msg）
- 安全：密码绝不持久化；passphrase 私钥 v1 强制 agent 模式

**Non-Goals**（v1 显式不做，spec 标 v2 phase）：
- Linux gnome-keyring SSH agent（少数）
- Windows named pipe SSH agent（`\\.\pipe\openssh-ssh-agent`）—— Windows v1 仅 password 模式与 IdentityFile 直读
- 加密私钥 passphrase UI 弹窗
- SSH 自动重连 / TCP keepalive
- 多 host 同时活跃（与 TS 原版一致：连新 host 自动断旧的）
- 远端文件 / sub-session 写操作（项目本身只读浏览）
- 在远端跑命令 / 部署 helper（仅 SFTP，TS 原版唯一在远端跑的命令是 `printf %s "$HOME"` 探测 home，本 port 同样保留）

## Decisions

### D1. SSH 库选型 — `russh` + `russh-keys`（pin 0.46.x），SFTP 客户端 spike 后定

**选择**：使用 `russh` 系列（pure async Rust SSH 实现）作为真协议栈，`Cargo.toml` pin `russh = "0.46"` + `russh-keys = "0.46"`（minor 锁定，patch 自动）。SFTP 客户端在 (a) 社区 wrapper `russh-sftp` 与 (b) `russh::client::Handle::channel_open_session()` 自行组装 SFTP packet 之间二选一——`apply` 第一步用 ~50 行 spike 验证两者 API 后选定（见 OQ1）；任务 1.1 把"先跑 spike → pin 选型 → 才进入后续实现"作为硬约束。

**拒绝的备选**：
- (A) `openssh` crate（subprocess wrapper）：每次 SSH 操作 fork 子进程 ~5-15ms，SFTP 大量小读时累积可观（远端拉 JSONL 命中此场景）；错误诊断弱（解析 stderr 字符串）；Windows 行为受系统 ssh 版本影响；与项目 `tracing::info!(target: "cdt_api::perf", ...)` 体系无法融合；唯一优势"零配置兼容用户已有 SSH 配置"通过 D3（spawn `ssh -G`）部分获得。
- (B) `async-ssh2-tokio`（libssh2 binding）：libssh2 算法支持落后于现代 SSH（如 `ssh-ed25519` 早期版本不全），且 C 库依赖 cross-compile 在 Tauri release 矩阵（macOS / Linux / Windows）会增加构建复杂度。

**理由**：
1. 远端拉大会话（`get_session_detail` 的 SSH 等价）是 port 项目核心场景，`russh` 在内存里直接组装 SFTP packet，零跨进程 IPC overhead；同 SSH session 多 channel 可并发优化（v1 不必用，但保留路径）。
2. 结构化 Rust enum 错误便于 spec Scenario 写测试 + `tracing` 集成。
3. pure Rust 与 Cargo workspace 治理一致（`cargo audit` 可审计，无系统 ssh 版本漂移）。
4. `russh-keys::agent` 模块直接支持 unix socket 与 named pipe 的 ssh-agent 协议。

**风险**：`russh` 不解析 SSH config 高级特性（Include/Match/ProxyJump）— 由 D3 缓解。

### D2. v1 鉴权候选链范围 — 7 项有序尝试（`IdentityAgent` 优先）

**选择**：按以下顺序构建鉴权候选列表，对每项尝试到第一个成功为止；任一失败记录详细原因到 `AuthAttempt[]`。该顺序与 OpenSSH 自身行为对齐——`IdentityAgent` 字段一旦在 ssh config 中显式指定（典型如 1Password 用户写 `IdentityAgent ~/Library/Group Containers/.../agent.sock`），SHALL 优先于 `SSH_AUTH_SOCK` env 与 IdentityFile 文件直读，避免出现"用户明明在 ssh config 配了 1Password agent 却被默认 env agent 抢先"的体感差异（codex 二审 #1）。

1. **`IdentityAgent`（来自 `ssh -G` 解析结果，仅当字段非空且非 `none`）**：把字段值视作 unix socket 路径（macOS 上 1Password 写法如 `~/Library/Group Containers/2BUA8C4S2C.com.1password/t/agent.sock`），SHALL 在尝试 `SSH_AUTH_SOCK` env 之前先连接此 socket。
2. **`SSH_AUTH_SOCK` env**（终端启动场景，~5 行）
3. **macOS `launchctl getenv SSH_AUTH_SOCK`**（macOS 图形启动必备，~30 行；用 `tokio::process::Command` 调用 `launchctl`）
4. **1Password well-known socket**（macOS `~/Library/Group Containers/2BUA8C4S2C.com.1password/t/agent.sock` + `~/.1password/agent.sock`，各试一遍，~20 行；仅当候选 1 没有显式给出 1Password socket 路径时才作为兜底）
5. **`IdentityFile`（来自 `ssh -G` 解析结果）**：依次读取每个候选私钥文件，调 `russh-keys::decode_secret_key(content, None)`；失败（含 passphrase 加密）跳过并记录"requires passphrase, use ssh-add"。
6. **默认密钥 fallback**：`~/.ssh/id_ed25519` → `id_rsa` → `id_ecdsa`，规则同上。
7. **password 模式**：仅在用户在 UI 选择 password auth method 时使用，密码内存仅当前会话，绝不持久化。

**拒绝的备选**：
- (A) 全量复刻 TS 911 行：v1 不必。Linux gnome-keyring（gnome 用户用纯 ssh-agent 即可）/ Windows named pipe agent / passphrase UI 弹窗均标 v2 phase。
- (B) 仅 `russh-keys::connect_env`（最小版）：macOS Launchpad 启动用户体验崩溃（无 `SSH_AUTH_SOCK` env），不可接受。

**理由**：覆盖 macOS Tauri 桌面用户**绝大多数**实际场景；6 项中 3 项是 macOS 关键路径，证明该选型不是"懒"；spec Scenario 一一覆盖。

### D3. SSH config 高级特性 — 委托给系统 `ssh -G <host>`

**选择**：在解析 host alias 时，调用 `tokio::process::Command::output()` spawn `ssh -G <host>`，从 stdout 解析 `hostname` / `port` / `user` / `identityfile` / `identityagent` 等结果字段；不在 cdt-ssh 内复刻 `Include` / `Match` / `ProxyJump` / `ProxyCommand` 解析。`SshConfigParser` 仅承担"列出所有 Host alias"功能（用于 UI combobox 联想）— 这只需简单语法解析即可。

**拒绝的备选**：
- (A) 用 Rust crate 完整解析 SSH config 全部语法：开源生态没有覆盖 `Match` / `ProxyJump` 的成熟 crate；自己写预计 +400 行且 OpenSSH 持续演进，难维护。
- (B) 不支持 `Include` / `Match` / `ProxyJump`：与 TS 原版能力差距过大，是大量 power user 的硬需求。

**理由**：与 TS 原版策略一致（`SshHostResolver.ts:50` 也调 `ssh -G`），把 SSH config 复杂语法的演进委托给 OpenSSH 上游；本仓只承担"alias 列表 + 解析后字段读取"。系统 `ssh` 二进制是 macOS / Linux 默认存在 / Windows 10+ 内置；不存在时降级（没有 ssh-G 解析仍可手动输入字段连接）。

**Risk**：Windows 用户没启用 OpenSSH client → 解析失败 → spec Scenario 标"降级到表单纯字段输入"。

### D4. Context 模式 — 一个活跃 + 多注册

**选择**：完全对齐 TS `ServiceContextRegistry`：
- 一个 `Local` context 永远存在 + 0 ~ N 个 `Ssh<host>` context 注册
- 同一时刻只有一个 `active` context；连接新 host 自动 `disconnect` 旧的（v1 不支持多 host 同时活跃）
- `ActiveContext` enum 已在 `cdt-ssh::connection` 现有代码中定义，复用并扩展（增加 `connecting` 子状态包含 auth chain 进度信息）。

**拒绝的备选**：
- (A) 多 host 同时活跃：TS 原版也不支持，且 UI 设计（一个 ProjectList / 一个 SessionDetail）不支持"同时显示两个 host 数据"，价值低复杂度高。

### D5. 远端文件变更感知 — 3 秒 polling + 30s catch-up（指纹 = size + mtime）

**选择**：与 TS 原版一致：
- 每 3 秒调用 SFTP `readdir` 列举远端 `<remote_home>/.claude/projects/<project_id>/` 下的 `.jsonl` 文件
- 对每个文件调用 SFTP `stat` 取 `size` 与 `mtime`
- 维护 `BTreeMap<PathBuf, FileFingerprint { size: u64, mtime: Option<SystemTime> }>` baseline
- 与上轮 baseline 比较差异：(a) 新文件 → emit；(b) `size` 变化 → emit；(c) `size` 不变但 `mtime` 变化 → emit（覆盖"截断后写到原长度"场景，单纯比 size 漏检 — codex 二审 #2）；(d) 文件不再出现 → emit deletion
- mtime 缺失策略：极少数 SFTP server 不返回 mtime（`mtime = None`），此时 fingerprint 仅依赖 size + 文件名；接受"截断后同长度重写"漏检（v1 在 30s catch-up 周期里大概率仍不能恢复，但远端 Claude 写 JSONL 是 append-only，**实际**不存在截断后同长度重写的场景；spec Scenario 标注此 trade-off）
- 第一次 poll **不**触发事件（建 baseline）
- 30 秒额外 catch-up timer 兜底（防止 SFTP 偶发丢失差异）；catch-up 同样按 size + mtime 双维度比对

**拒绝的备选**：
- (A) `inotify` / `kqueue` 远端转发：远端无标准协议；自己装 helper 进程违背"仅 SFTP"原则。
- (B) 1 秒间隔：远端 idle CPU 会显著上升（项目核心硬约束）；3 秒已与 TS 原版对齐且实测 idle 友好。
- (C) 完全不做 polling：UI 体验严重劣化（用户不能实时看到 Claude 在远端的写入）。

**性能预算**：每 3s 一轮 SFTP `readdir`（~50 文件 → ~5KB 流量 / 100ms wall）+ 各文件 `stat`。idle CPU 实测目标 < 2%（验证方式 `Activity Monitor` + `top -pid <tauri>`）。

**Mitigation**：发现 idle CPU 不达标时降级为 5s 间隔 + 不做单文件 stat（仅 `readdir` size 维度差异）。

### D6. IPC 形态 — 11 条 Tauri command + 2 条 emit 事件

**新增 Tauri `invoke_handler!` 命令**（与 TS IPC channel 对齐 + 命名 snake_case 符合 Rust 约定 + payload camelCase）：

| Command | Payload | Returns |
|---|---|---|
| `ssh_connect` | `{ host, port?, username?, authMethod, password? }` | `SshConnectionResult { contextId, status, authChain[] }` |
| `ssh_disconnect` | `{ contextId }` | `Ok` |
| `ssh_test_connection` | 同 connect | 同 connect 但 SHALL NOT 注册 active context |
| `ssh_get_state` | `{ contextId? }` | `SshConnectionStatus` |
| `ssh_get_config_hosts` | `{}` | `Vec<String>`（alias 列表） |
| `ssh_resolve_host` | `{ alias }` | `SshHostConfig { host, port, user, identityFile? }` |
| `ssh_save_last_connection` | `{ host, port, username, authMethod }` | `Ok` |
| `ssh_get_last_connection` | `{}` | `Option<{ host, port, username, authMethod }>` |
| `list_contexts` | `{}` | `Vec<ContextSummary { id, kind, label, status }>` |
| `switch_context` | `{ contextId }` | `Ok` |
| `get_active_context` | `{}` | `ContextSummary` |

**事件 emit**（broadcast::Sender → Tauri `emit("name", payload)` / HTTP SSE）：
- `ssh_status` payload `{ contextId, status, error? }` —— 状态变更（每 context 独立）
- `context_changed` payload `{ activeContextId, kind }` —— 活跃 context 切换

**HTTP 路由**：现有 `/api/ssh/connect` / `/api/ssh/disconnect` / `/api/ssh/resolve-host` 与 `/api/contexts` / `/api/contexts/switch` 路由 SHALL 保留，**payload schema 不变**（仍接受现有 `SshConnectRequest` / `SwitchContextBody { context_id }` 等结构体），仅底层实现接入真握手 — 不引入 BREAKING（codex 二审 #6）。本 change 暂不在 HTTP 端新增路由（11 条 Tauri command 中余下的 `ssh_test_connection` / `ssh_get_state` / `ssh_get_config_hosts` / `ssh_save_last_connection` / `ssh_get_last_connection` / `list_contexts` / `get_active_context` 七条，HTTP 端按需补，但不阻塞本 change v1）。

**IPC payload 序列化形态规约**（codex 二审 #3）：所有跨 IPC 边界的 enum SHALL 使用 `#[serde(tag = "type", content = "data", rename_all = "camelCase")]` 内部标签法。具体：
- `AuthSource` 序列化样例：`{ "type": "identityAgent", "data": "/path/to/agent.sock" }` / `{ "type": "envAgent" }` / `{ "type": "launchctlAgent" }` / `{ "type": "onePasswordAgent", "data": "/path/to/socket" }` / `{ "type": "identityFile", "data": "/Users/alice/.ssh/work_key" }` / `{ "type": "defaultKey", "data": "/Users/alice/.ssh/id_ed25519" }` / `{ "type": "password" }`
- `AuthOutcome` 序列化样例：`{ "type": "success" }` / `{ "type": "failure", "data": "Permission denied" }` / `{ "type": "skipped", "data": "requires passphrase, use ssh-add" }`
- `SshError` 序列化样例：`{ "code": "ssh_tcp_failure", "host": "...", "reason": "..." }` / `{ "code": "ssh_auth_exhausted", "attempts": [...] }` / `{ "code": "ssh_remote_home_missing", "tried": [...] }` 等——code 字段使用 snake_case 与 `ApiError` 既有 `code` 字段约定一致，不走 internal-tag。
- `AuthAttempt` 字段名 camelCase（`source` / `outcome` / `elapsedMs`）。

IPC contract test SHALL 显式断言上述每种形态。

**拒绝的备选**：
- (A) 单一 `ssh_invoke(operation, payload)` 大杂烩 command：违反 IPC contract test 单测粒度；payload schema 难追踪。
- (B) 在事件 payload 里塞完整 `AuthAttempt[]` chain：首次连接成功路径下 chain 可达 ~10 项，每次状态变更全推过去浪费带宽。chain 仅在 `error` 状态附带；连接成功状态 chain 可省略。

### D7. UI 形态 — Connection tab + WorkspaceIndicator + Overlay + Badge

**Settings → Connection tab**（仅 Tauri 桌面渲染，HTTP standalone 模式 hide）：
- `host` combobox：联想 `~/.ssh/config` Host alias（来自 `ssh_get_config_hosts`），用户也可手输非 alias 的 hostname
- `port`（默认 22 + alias 选中后从 `ssh_resolve_host` 自动填充）
- `username`（同上 alias 自动填充）
- `authMethod` 单选：`sshConfig`（推荐，使用 D2 鉴权链）/ `password`
- `password` 输入框（仅 `password` 模式可见）
- saved profiles 一键填充按钮列表（来自 `ssh.profiles[]` 配置）
- "Connect" / "Test connection" / "Disconnect"（已连接状态显示） / "Save as profile" 按钮
- 连接状态徽章（`ConnectionStatusBadge`）显示当前 SSH context 状态

**WorkspaceIndicator**（右下角浮动 pill，`fixed` 定位）：
- 仅在 `availableContexts.length > 1`（即至少有一个 SSH context 已注册）时显示
- 默认形态：图标（`lucide-svelte::Wifi`，绿色）+ host 名（如 `myserver`）；点击展开下拉切换 workspace
- 切换时触发 `switch_context` + `ContextSwitchOverlay` 全屏 loading

**ContextSwitchOverlay**：
- 切换 context 期间全屏半透明 loading（避免用户看到中间态项目列表闪烁）
- 接收 `context_changed` 事件后退场

**ConnectionStatusBadge**：
- 状态映射图标 / 颜色：`disconnected`（灰 wifi-off）/ `connecting`（黄 spinner）/ `connected`（绿 wifi）/ `error`（红 alert + 错误悬浮提示）

**远程 ProjectList / SessionDetail 复用**：UI 完全不感知数据源是 local 还是 ssh-host，依赖 D4 的 `active context` 切换路由数据请求到正确 provider。

### D8. WorkspaceIndicator 落地哪个 capability —— 不新建，落到 `app-chrome`

**选择**：`WorkspaceIndicator` + `ContextSwitchOverlay` 行为契约写入 `app-chrome`（应用级 chrome shell），不另立 `ui-workspace-switcher` 新 capability。

**拒绝的备选**：
- (A) 新 capability `ui-workspace-switcher`：本 change 已经跨 5 个 capability，再加一个会让 spec delta 更碎。WorkspaceIndicator 与现有 app-chrome（已含 TabBar / Sidebar 等顶层 chrome）天然同层。
- (B) 落到 `sidebar-navigation`：sidebar 是垂直左侧导航，WorkspaceIndicator 是右下角浮动 pill，物理与逻辑都不在同一组件。

**理由**：先按"紧耦合 capability 组"原则落到现有 capability；若 v2 / v3 workspace switcher 行为大幅扩展（比如增加多 host 同时活跃 / 跨 workspace 搜索），届时拆 capability 也来得及。

### D9. 安全 —— 密码不持久化，passphrase 私钥 v1 强制 agent，子进程 stdin 关闭

**选择**：
- 密码字段**绝不**写入 `~/.claude/claude-devtools-config.json`；仅 in-memory 持有，连接断开 / 应用关闭即丢失
- 密码字段**绝不**进入 `tracing` 日志：`SshConnectRequest::Debug` impl SHALL 把 `password` 字段渲染为固定字符串 `<redacted>`；Tauri command wrapper SHALL NOT 调用 `tracing::info!(?request)` / `?body` 形式打印整个结构体（codex 二审 #7）。如必须 log，仅记录 `host` / `username` / `authMethod` 三个非敏感字段。
- `ssh.last_connection` 仅持久化 `host` / `port` / `username` / `authMethod`
- `ssh.profiles[]` 同上字段集；`ssh.profiles[].passwordRequired: bool` 标记是否 password 模式（以便 UI 重新填表时弹密码框）
- passphrase 加密私钥 v1 直接跳过并在 `AuthAttempt` 列表中标"requires passphrase, use ssh-add"，不收集 passphrase（不存在 passphrase UI 弹窗 → 不存在 passphrase 泄露面）
- `~/.ssh/id_*` 等私钥文件**只读**，永远不修改
- spawn 子进程（`ssh -G` / `launchctl getenv`）SHALL 显式 `Stdio::null()` 关闭 stdin、`Stdio::piped()` 收集 stdout、`Stdio::null()` 丢弃 stderr（codex 二审 #7）；防被恶意 hook 或终端控制序列影响 + 防输出被 inherited stderr 染色到主进程日志

**拒绝的备选**：
- (A) OS keychain 持久化密码：跨 macOS / Linux / Windows 三平台 keychain API 不一致，引入额外依赖；TS 原版也未做。
- (B) v1 加 passphrase UI 弹窗：增加攻击面（passphrase 必须在 Rust 进程内存解密私钥），与"v1 强制 agent"理念冲突。

### D10. 错误分类 —— 结构化 `SshError` enum + auth chain diagnostic

**选择**：扩展 `SshError`：
```rust
pub enum SshError {
    Tcp { host: String, source: io::Error },          // TCP probe 失败
    AuthExhausted { attempts: Vec<AuthAttempt> },     // 所有候选都失败
    SftpInit { source: russh::Error },                // SFTP subsystem open 失败
    RemoteHomeMissing { tried: Vec<PathBuf> },        // ~/.claude/projects 在远端找不到
    Cancelled,                                        // 用户主动取消
    Timeout { stage: TimeoutStage },                  // TCP / Auth / SFTP 各阶段超时
    Config { reason: String },                        // SSH config 解析 / ssh -G 失败
}

pub struct AuthAttempt {
    pub source: AuthSource,        // EnvAgent / LaunchctlAgent / OnePasswordAgent / IdentityFile(path) / DefaultKey(path) / Password
    pub outcome: AuthOutcome,      // Success / Failure(reason) / Skipped(reason)
    pub elapsed_ms: u64,
}
```

每次 `ssh_connect` 失败时 `AuthExhausted.attempts` 列出每个候选的尝试结果，便于 UI 与日志给用户清晰诊断（"试过 6 个候选，都失败：env agent socket 不存在 / launchctl 返回空 / 1Password socket 路径不存在 / id_ed25519 requires passphrase, use ssh-add / id_rsa not found / password 未提供"）。

**理由**：避免 TS 原版 `enrichAuthError()`（拼接字符串）的非结构化诊断；结构化错误便于 IPC contract test 断言 + spec Scenario 覆盖每种错误形态。

## Risks / Trade-offs

- **[Risk]** `russh` 协议层 bug 直接影响所有用户 → **Mitigation**：v1 仅做读操作（SFTP read-only），无写路径暴露；锁定 `russh` minor 版本，重大升级前 manual smoke。
- **[Risk]** macOS Tauri 应用 codesigning / sandbox 影响 `launchctl` 子进程调用 → **Mitigation**：`launchctl getenv SSH_AUTH_SOCK` 是只读子命令，不需要特殊 entitlement；用 `tokio::process::Command` 启动且明确不继承 stdin。验证方式：`just dev` 模式 + 已 codesign 的 release dmg 各验一遍。
- **[Risk]** 3 秒 polling 对 idle CPU 影响超阈值 → **Mitigation**：spec Scenario 显式覆盖此预算；apply 阶段必跑 `Activity Monitor` 实测，不达标降级为 5s + 仅 `readdir` 维度。
- **[Risk]** Windows 用户无 OpenSSH client 时 `ssh -G` 解析失败 → **Mitigation**：spec Scenario 覆盖"降级到表单纯字段输入"；UI 在 Windows 显示提示。
- **[Risk]** 远端 `~/.claude/projects` 不存在或权限不足 → **Mitigation**：`SshError::RemoteHomeMissing { tried: [...] }` 列出 fallback 候选路径（`<home>/.claude/projects` / `/home/<user>/.claude/projects` / `/Users/<user>/.claude/projects` / `/root/.claude/projects`）；至少返回空 project list 不崩溃。
- **[Risk]** `russh-keys` 对极少数私钥格式（如 PuTTY .ppk）不支持 → **Mitigation**：v1 文档注明仅支持 OpenSSH 格式（`-----BEGIN OPENSSH PRIVATE KEY-----` / 老 PEM）；用户用 `ssh-keygen -p` 转换。
- **[Trade-off]** 选 `russh` 而非 `openssh` subprocess：失去"零成本兼容用户已有 SSH 配置"的部分能力（passphrase 弹窗等），换来 in-process 性能与可观测性。
- **[Trade-off]** 一次性大 PR（~3000 行 Rust + Svelte）：reviewer 一次看完整故事，但 codex / wait-ci / 自审压力集中。**Mitigation**：apply 阶段按 5 个 sub-session 推进（cdt-ssh 协议 → cdt-api 接入 → Tauri IPC → UI → 集成发布），每个 sub-session 同分支多次 commit，最后一次统一 push 走发布尾段。

## Migration Plan

**v1 落地步骤（同 PR 内顺序）**：
1. 引入依赖：`russh` + `russh-keys` + `russh-sftp` 加到 `crates/cdt-ssh/Cargo.toml`，跑一次 `cargo check --workspace` 确认双 lock 同步。
2. 升级 `crates/cdt-ssh::provider`：`SshFileSystemProvider` 接入真 SFTP（`exists` / `read_file` / `readdir` / `stat` / `open_read_stream`）；mock SFTP 单测覆盖正常 + 错误（Permission denied / Connection lost）路径。
3. 升级 `crates/cdt-ssh::connection`：实现 `SshConnectionManager::connect()` 真握手（D2 鉴权链 + D5 polling watcher 启动 / 停止）；状态广播走 `broadcast::Sender<SshStatusChange>`。
4. 接入 `crates/cdt-api::LocalDataApi::ssh_*`：原 placeholder → 调用真 `SshConnectionManager`；保留向后兼容签名。
5. 注册 Tauri command：`src-tauri/src/lib.rs::invoke_handler!` 加 11 条新命令；`src-tauri/capabilities/default.json` 加 `ssh:*` + `core:command:execute` shell-spawn scope（仅允许 `ssh` 二进制）。
6. UI：先做 `Settings → Connection` tab + `lib/api.ts::ssh_*` wrapper + `lib/stores/connection.ts`；再做 `WorkspaceIndicator` + `ContextSwitchOverlay` + `ConnectionStatusBadge`；最后 `lib/stores/context.ts` + ProjectList / SessionDetail 切换路由。
7. `ConfigStore` 加 `SshConfig` 段（`profiles[]` / `last_connection` / `auto_reconnect`）；validate_trigger 类似的 validator 覆盖 host 非空 / port 1-65535。
8. perf 验证：`Activity Monitor` 实测 idle CPU < 2%；`/perf-bench` skill 跑 `get_session_detail` 远端等价场景对照本地 baseline。
9. 集成测试：Vitest mockIPC 覆盖 connection / context store；Playwright 1 个 user story（打开 Connection tab → 输入 alias → 显示状态）；`just preflight` 全过。

**Rollback 策略**：若 release 后发现严重问题，可通过 v1 顶层 const `ENABLE_SSH_CONNECT: bool = true` 一键禁用 SSH command（false 时 ssh_* 命令直接返回 `error: ssh disabled`，UI 隐藏 Connection tab）。该 const 设默认 `true`；hotfix 需要时 release 0.x.y+1 把它设为 `false`。

## Open Questions

- **OQ1**：`russh-sftp` 还是 `russh::client::Handle::channel_open_session()` 自组装 SFTP packet？前者是社区 wrapper 提供高级 API（已包含 readdir / stat 等），后者底层但完全自控。倾向 `russh-sftp` 减少 ~200 行手写 SFTP packet 代码——apply 第一步用 ~50 行 spike 验证两者 API 后冻结选型 + pin 版本到 `Cargo.toml`，再开始 task 1.2 之后的全量实现。spike 失败（如 `russh-sftp` 与 `russh = 0.46` minor 不兼容）时切换到自组装路径并在 design.md 追加修订决策块 D1b 记录。
- **OQ2**：`launchctl getenv SSH_AUTH_SOCK` 在 macOS sandbox 内是否可调？已知 Tauri 默认非沙箱，但若未来开 App Sandbox entitlement 此调用可能受限。v1 不开沙箱，OQ 留 v2 评估。
- **OQ3**：Polling watcher 在 SSH 连接断开后是否自动重启？v1 决定不重启（连接断开后 active context 自动切回 local，watcher 整体 stop），与 D4 一致。但若用户从 UI"重连"按钮再连同一 host，watcher 是否复用旧 baseline 还是重新建？倾向重新建（避免远端在断开期间被外部修改后差异计算错误）—— spec Scenario 显式覆盖此选择。
- **OQ4**：UI Connection tab 在 Windows 上是否需要"使用密码"warning（因为 v1 不支持 named pipe agent）？倾向显示 inline 提示"Windows 当前 v1 仅支持密码模式或 IdentityFile 直读，命名管道 ssh-agent 计划在 v2 加入"。

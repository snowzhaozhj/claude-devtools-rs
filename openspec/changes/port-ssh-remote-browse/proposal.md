## Why

Rust 端口当前的 SSH 远程会话浏览只是 placeholder：`crates/cdt-ssh` 没有引入任何真 SSH 协议库（无 `russh`/`openssh`/`ssh2`），`SshFileSystemProvider` 全部返回 `not connected` 错误；`cdt-api` 仅暴露 3 条 `/api/ssh/*` HTTP 路由，`src-tauri/src/lib.rs::invoke_handler!` 没有任何 SSH/context 命令；UI 完全空白（`grep ssh` 无结果）。结果就是 Tauri 桌面用户**完全无法**使用 TS 原版（Electron）已生产的 SSH 远端浏览能力——这是 port 项目最显眼的功能缺口之一。

需要把这块 placeholder 升级为真实可用的 SSH 远程会话浏览，对齐 TS 原版用户体验，同时兼顾 Rust 重写带来的可观测性（结构化错误 + `tracing` perf target 融合）与远端拉大会话场景下的 SFTP 吞吐。

## What Changes

- **接入 `russh` + `russh-keys` 真实 SSH 协议栈**：替换 `cdt-ssh::SshFileSystemProvider` 占位，实现真握手 + SFTP `read_file`/`readdir`/`stat`/`exists`/`open_read_stream`。
- **v1 鉴权候选链**（macOS Tauri 桌面优先）：依次尝试 `SSH_AUTH_SOCK` env → `launchctl getenv SSH_AUTH_SOCK`（macOS 图形启动必备）→ 1Password well-known socket（`~/Library/Group Containers/2BUA8C4S2C.com.1password/t/agent.sock` + `~/.1password/agent.sock`）→ `IdentityFile`（来自 `ssh -G` 解析）→ 默认密钥 fallback（`id_ed25519` / `id_rsa` / `id_ecdsa`）→ password 模式。
- **v1 不做（spec 标 v2 phase）**：Linux gnome-keyring / Windows named pipe agent / 加密私钥 passphrase UI 弹窗 / 自动重连 / keepalive。
- **`ssh -G <host>` 子进程委托**：用 `tokio::process::Command` 调用系统 `ssh -G` 解析 `Include` / `Match` / `ProxyJump` / `IdentityAgent` 等高级 SSH config 特性，不在 cdt-ssh 内复刻 SSH config 复杂语法。
- **远端文件变更感知**：3 秒 polling（与 TS 原版一致，远端无 inotify/kqueue），SFTP `readdir` + `stat.size` baseline diff，第一次 poll 不触发事件，30s catch-up timer 兜底。
- **新增 Tauri `invoke_handler!` 命令**：`ssh_connect` / `ssh_disconnect` / `ssh_test_connection` / `ssh_get_state` / `ssh_get_config_hosts` / `ssh_resolve_host` / `ssh_save_last_connection` / `ssh_get_last_connection` / `list_contexts` / `switch_context` / `get_active_context`；事件推送 `ssh_status`（每 context 状态变更）+ `context_changed`（活跃 context 切换）。
- **新增 `src-tauri/capabilities/default.json` SSH scope**：允许 ssh_*/context_* IPC 命令；允许 spawn `ssh` 子进程做 host 解析。
- **UI 新增三组件 + 一个 Settings tab**：`Settings → Connection` tab（host combobox 联想 `~/.ssh/config` + port + user + auth method 二选一 + saved profiles 一键填充）；右下角浮动 `WorkspaceIndicator` pill（绿色 wifi + host 名，多 context 时下拉切换）；切换中 `ContextSwitchOverlay` 全屏 loading；`ConnectionStatusBadge` 状态图标。远程会话列表 / 详情 UI 完全复用本地 `ProjectList` + `SessionDetail`，仅底层数据源切换。
- **`ConfigStore` 字段扩展**：新增 `ssh.profiles[]`（命名保存的连接配置，无密码）/ `ssh.last_connection`（host/port/username/authMethod，无密码）/ `ssh.auto_reconnect`（v1 仅持久化字段，自动重连本身 v2 实现）；密码绝不持久化。
- **错误分类**：`SshError` 升级为结构化分类（`Tcp` / `AuthExhausted` / `SftpInit` / `RemoteHomeMissing` / `Cancelled` / `Timeout` / `Config`），附带 auth chain 诊断信息（每个尝试源的成败原因）。
- 不引入 BREAKING：现有 `cdt-api` HTTP 路由 `/api/ssh/*` 行为保持兼容，仅在底层接入真握手；`ssh-remote-context` 主 spec 的 Scenario 由"placeholder 占位"升级为"真实实现"，调用契约不变。

## Capabilities

### New Capabilities

- 无新增 capability。WorkspaceIndicator / ContextSwitchOverlay 落到现有 `app-chrome` 与 `sidebar-navigation`；Connection tab 落到 `settings-ui`。design.md `D8` 进一步辨析是否值得拆出 `ui-workspace-switcher` 独立 capability。

### Modified Capabilities

- `ssh-remote-context`：把 4 个 Requirement 的 Scenario 由"placeholder 占位"升级为"真实 russh 握手 + SFTP 读"；新增 Requirement 覆盖鉴权候选链、远端 polling 文件变更感知、错误分类、状态事件推送、`ssh -G` host 解析。
- `ipc-data-api`：新增 11 条 `ssh_*` / `context_*` Tauri command（`invoke_handler!` 注册）+ 2 条事件流（`ssh_status` / `context_changed`）；payload schema 严格 camelCase；与现有 `LocalDataApi` IPC 契约保持一致。
- `settings-ui`：新增 `Connection` tab（仅 Tauri 桌面显示，standalone 模式禁用）；表单字段 `host` / `port` / `username` / `authMethod`（`sshConfig` | `password`） / `password`（仅 password 模式可见）；saved profiles 一键填充。
- `file-watching`：在现有本地 `notify` 监听之外，新增 SSH 远端 polling 模式（3s 间隔 + 30s catch-up timer + 第一次 poll 不触发事件）；保持事件接口与本地一致，下游消费者无感。
- `configuration-management`：`ConfigStore` 增加 `ssh` 段（`profiles[]` / `last_connection` / `auto_reconnect`），默认值与持久化语义；密码字段绝不出现在持久化层。

## Impact

- **新依赖**：`russh` + `russh-keys` + `russh-sftp`（或 russh 自带 sftp client，design.md 选定具体 crate）添加到 `crates/cdt-ssh/Cargo.toml`；触发 `Cargo.lock` 与 `src-tauri/Cargo.lock` 双 lock 更新。
- **代码**：
  - `crates/cdt-ssh/src/{lib,connection,provider,config_parser,error}.rs` 全部重写或重大扩展（占位 → 真实实现，预计 +800 ~ +1200 行）。
  - `crates/cdt-api/src/ipc/{traits,local}.rs` 接入真握手；HTTP 路由 `/api/ssh/*` 行为升级（仍保留向后兼容）。
  - `src-tauri/src/lib.rs::invoke_handler!` 注册 11 条新命令 + 2 条事件 emit 路径；`src-tauri/capabilities/default.json` 新增 `ssh:*` 与 `core:command` shell-spawn scope。
  - `ui/src/` 新增 `routes/settings/Connection.svelte` + `lib/components/WorkspaceIndicator.svelte` + `lib/components/ContextSwitchOverlay.svelte` + `lib/components/ConnectionStatusBadge.svelte`；`lib/api.ts` 加 `ssh_*` / `context_*` IPC wrapper；`lib/stores/connection.ts` + `lib/stores/context.ts` 状态 store。
  - `crates/cdt-config/src/store.rs` 新增 `SshConfig` 段。
  - `crates/cdt-watch/src/` 新增 SSH polling watcher 实现（trait 复用，分支按 provider 类型）。
- **测试**：cdt-ssh 加 mock SFTP 单测（trait 注入）；cdt-api 加 IPC contract test 覆盖新命令的字段名 / camelCase；UI 加 Vitest + mockIPC 覆盖 connection store + Connection.svelte 表单；Playwright 至少 1 个 user story 覆盖"打开 Connection tab → 输入 host alias → 显示状态"。
- **性能预算**：新增 polling watcher 每 3s 一次 SFTP `readdir` + 各文件 `stat`，对**已连接**远端长驻；evictions 走原有 watcher trait。need 验证 idle CPU < 2% 阈值（CLAUDE.md `.claude/rules/perf.md::辅助工具系统 CPU 阈值`）。SFTP 大会话首次拉取（`get_session_detail` 远端版）目标 wall < 3s（10k msg），与本地 < 800ms 预算的远端容差。
- **平台兼容**：v1 macOS / Linux 终端启动 + macOS Launchpad 启动全覆盖；Windows 仅 password 模式与 `IdentityFile` 直读（spec 显式标 v2 补 named pipe agent）。
- **安全**：密码绝不持久化；ssh.last_connection 字段无 password；passphrase 私钥 v1 不收集（提示用户用 `ssh-add`）。
- **followups.md**：本 change 关闭 ssh-remote-context capability 下的 2 个 coverage-gap（SSE 增量补全 ssh-status 事件源 / SSH stage-limit 快速搜索），design.md 列具体行号引用。

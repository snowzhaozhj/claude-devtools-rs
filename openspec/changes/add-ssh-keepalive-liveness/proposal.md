## Why

Issue #230 复现：`ssh_connect` 成功 → `/api/repository-groups` 返 42 项 ✓ → 等约 90 秒再 curl → **0 groups**，与此同时 `ssh ... ls` 直连远端正常、`/api/contexts/active` 仍报 `isActive: true kind=ssh`。后端日志反复刷 `cdt_watch::ssh_polling: polling scan failed error=sftp error: session closed` + `cdt_discover::project_scanner: projects root does not exist`。

根因（按层）：

- **Server 层**：docker openssh 默认 `ClientAliveInterval 0` 不主动发心跳；其它部署里 sshd 也通常是 server-side 静默策略
- **网络中间件层**：家用 NAT 表项 idle 30-60s 失效；企业 firewall connection-tracking 表项默认 300s
- **Client 层**：`crates/cdt-ssh/src/session.rs::connect_inner` 阶段 2 用 `russh::client::Config::default()`，**未启用** transport keepalive（`keepalive_interval = None`）

三层任一不主动发心跳就足以让 channel 在闲置一段时间后被悄悄关 / reset / 丢包；本 change 在 client 层补上心跳，让 channel 永远不进入 idle，根治 server / 中间件这两层的不可控 idle 杀手。

短期缓解 PR #205 已在：polling watcher 连续 3 次永久 SFTP 错误 → 调 `ssh_mgr.disconnect`。本 change 走 **transport 层根治**，把"等到 SFTP RPC 撞墙"前移到"主动周期探测"，同时保留短期自愈作为兜底。

## What Changes

- 在 `connect_inner` 用 `build_client_config()` 替代 `client::Config::default()`，显式设置 `keepalive_interval = Some(15s)` + `keepalive_max = 3`（russh 内置：超过 max 个未回应的 keepalive request 后自动关闭 transport，因 russh 0.52.x 的判断 off-by-one，实际触发窗口 = `(keepalive_max + 2) × keepalive_interval = 75s`）
- 新增 `Requirement: Keep SSH transport alive via russh keepalive` 到 `ssh-remote-context` capability，钉死参数与协同语义
- 不引入额外定时任务、不改 SFTP 调用路径、不改 polling watcher 行为；保持 disconnect / reconnect / 错误投影 / `ContextChanged` 广播链路完全一致

## Impact

- Affected specs: `ssh-remote-context`（ADDED Requirement）
- Affected code: `crates/cdt-ssh/src/session.rs`
- 无 IPC 字段变化；无前端改动；无新依赖（russh 已支持）
- 单元测试覆盖：`build_client_config` 返回的 `client::Config` SHALL 携带预期 `keepalive_interval` / `keepalive_max`；grep 验证 `connect_inner` 没有遗留 `Config::default()` 直调
- 无 BREAKING；旧链接行为退化为"和过去一样依赖网络层 keepalive"——升级即生效

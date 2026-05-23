## Context

Issue #230 在 docker openssh + 家用 NAT 真实环境复现："SSH connect 成功 → 闲置 ~90 秒 → SFTP channel 已被对端关 → polling 反复打不开"。同期 `ssh -G ... ls` 直连仍正常，说明 TCP 链路本身可达，是 **SSH session / SFTP channel 级**的 idle 超时关闭。

`crates/cdt-ssh` 用 russh 0.52.1。该版本 `client::Config` 已经把 keepalive 字段做成一等公民：

- `keepalive_interval: Option<Duration>`（默认 `None`）—— 距离上次收到 server 数据这么久就发一个 `SSH_MSG_GLOBAL_REQUEST keepalive@openssh.com`（`want_reply=true`）
- `keepalive_max: usize`（默认 `3`）—— `client::session` 的 keepalive loop 实测 **off-by-one**：判断条件是 `alive_timeouts > keepalive_max`（先比较，再 `alive_timeouts += 1` 再 send），所以 `keepalive_max = 3` 时实际允许 4 个连续未收回的 keepalive，第 5 个 tick 触发 `Error::KeepaliveTimeout`，总窗口 = `(keepalive_max + 2) × keepalive_interval`（详 `~/.cargo/registry/src/.../russh-0.52.1/src/client/mod.rs:1105-1113`）

只要把 `keepalive_interval` 配上，就能：

1. **主作用（防 idle close）**：每 N 秒 client → server 发 keepalive request、server → client 回 reply，双向 SSH msg 持续流动，让对端 / 中间件不会判 idle
   - server side：docker openssh 默认 `ClientAliveInterval 0` 不主动；其它部署 `ClientAliveInterval 300` 等也都不会 trigger（每 N 秒有数据）
   - 中间件：家用 NAT idle 表项（典型 30-60s）/ 企业 firewall connection-tracking 表项（典型 300s）永不 expire
2. **次作用（探活已死 transport）**：拔网线、对面机器重启、`sshd` 收到 `KILL` 这类硬故障时，client 在 `(keepalive_max + 2) × keepalive_interval` 内主动检测到无应答，关闭 transport 并让 client task 报 `KeepaliveTimeout`

短期自愈（PR #205）已经接好"polling watcher 连续 3 次永久 SFTP 错误 → `ssh_mgr.disconnect`"链路；本 change 落 transport 层 keepalive，与短期自愈互补。

## Goals / Non-Goals

**Goals**：

- 新链接默认开 keepalive；任意一台 SSH 主机 idle 不再被对端 / NAT / firewall 静默关 channel
- Channel 真死时（拔网线 / `sshd` 重启）transport 在 `(SSH_KEEPALIVE_MAX + 2) × SSH_KEEPALIVE_INTERVAL` 内自行关闭；下游既有自愈链路（polling 永久错误 → `dead_signal` → `perform_polling_self_heal_disconnect`）正常触发
- 参数集中在 module 顶部常量，未来调参不需改 connect 逻辑

**Non-Goals**：

- 不实现 SFTP 层 application-level ping（`sftp.stat(home)`）—— polling watcher 已经每 3s 跑 `read_dir`，够当 SFTP 层 liveness
- v1 不实现 `client::Handler::disconnected()` 回调主动 notify `dead_signal`—— 当前 `RusshClientHandler` 是无状态 unit struct，把 manager handle / dead_signal 注进 Handler 需要改 connect 流程的所有权布局；watcher-attached 场景已被既有 polling self-heal 路径覆盖（占绝大多数实际使用 — connect 后用户立即 `switch_context` 触发 `attach_remote_watcher`），暂不为"已 connect 但从未 switch"这种小概率分支引入第二条断开通路。详 §Risks "已知未覆盖"
- 不暴露给用户配置 keepalive 频率—— 15s/3 是合理默认，UI 暴露这种网络细节配置是反模式；真有需求再加 `AppConfig.ssh.keepaliveIntervalSecs`
- 不动 `inactivity_timeout`（russh 另一个字段，触发 transport 自动 GC）—— 与 keepalive 是不同语义，keepalive 已经覆盖核心需求；保持 `None` 不引入"30 分钟没动作就关"这种容易误伤长 polling 的策略

## Decisions

### D1：用 russh 内置 keepalive，不自己起 SFTP ping task

候选：

- **(a) `keepalive_interval = Some(15s)` + `keepalive_max = 3`（默认）**：纯配置一行，russh client task 内部串行处理；keepalive 与正常 SFTP traffic 共享 transport，不抢 SFTP channel。**采用**。
- (b) 自己 spawn 周期 `sftp.stat(<remote_home>)` ping task：实现复杂、与 polling watcher 重复（polling 已经每 3s 跑 `read_dir`，本身就是 SFTP-level ping）、还要处理"ping task 与 polling 同时撞死 channel"。
- (c) 复用 polling watcher：polling 是**应用层**周期，存在"未启动 watcher 的 SSH context"（reconnect 中、watcher 已 cancel 但未 disconnect 等中间态），靠它做 transport keepalive 不可靠。

**理由**：transport keepalive 是 SSH session-level 关注点，不是文件系统关注点，归属 russh 配置最干净；零额外 task、零额外 lock、零新组件。

### D2：`keepalive_interval = 15s`、`keepalive_max = 3`（实际 75s 探测窗口）

候选窗口（窗口 = `(keepalive_max + 2) × interval`，详 §Context russh off-by-one）：

- 5s/3 = 35s 窗口：太激进，企业 NAT 偶发抖动会误判
- 10s/3 = 50s 窗口：可选，更激进
- **15s/3 = 75s 窗口**：keepalive 间隔与社区常见 SSH client 配置一致（`~/.ssh/config` `ServerAliveInterval 15` 风格——与 OpenSSH **默认**0 不同，是文档与实践推荐值）；总窗口 75s 略低于 issue #230 实测的 ~90s 触发点，留 15s 余量；闲置时双向流量 ≈ 1 包/15s，每包 < 100B = 平均 ~13B/s，对带宽几乎无感
- 30s/3 = 150s 窗口：与 issue #230 复现的 90s idle close 同量级，无安全余量
- 60s/3 = 300s 窗口：触发 issue 场景

**理由**：本 change 的**主作用**是"每 15s 让 channel 有流量，永远不进入 idle 状态"，**次作用**才是"transport 死亡探测"。15s 间隔是主作用唯一相关的参数，已经覆盖绝大多数 NAT / firewall idle 表（30s+）；75s 探测窗口是次作用 fallback，issue #230 的 90s 是单次实测，真实分布上限通常更低，留 15s 安全余量足够。`keepalive_max = 3` 用 russh 默认避免无谓微调。

### D3：与短期自愈（polling watcher）协同

russh keepalive 触发 transport 关闭后：

1. polling watcher 下一轮 SFTP `read_dir` 立刻报 `session closed` / `broken pipe`（已被 `is_permanent_sftp_failure` 识别，详 `polling_watcher.rs:242-250`）
2. 计满 `PERMANENT_FAILURE_THRESHOLD = 3` 后 `dead_signal.notify_one()`
3. `cdt-api/src/ipc/local.rs::perform_polling_self_heal_disconnect` 接 `dead_signal`，做 generation guard + abort scans + `ssh_mgr.disconnect()` + emit `ContextChanged(None)`

**总检测窗口**：transport 自身 ≤ 75s + polling 计 3 次 ≤ 9s = **≤ 84s**（vs 当前 issue #230 的"无穷大或下次手动操作时"）。

**v1 不接 `Handler::disconnected()` 回调**的理由：避免引入第二条断开通路造成 double-dispatch / `ContextChanged` 重复广播 + 当前 `RusshClientHandler` 是无状态 unit struct，需要把 manager handle / dead_signal 注入到 Handler 才能 fire signal，所有权布局改造成本不值这个小概率边界（详 §Risks "已知未覆盖"）。watcher-attached 场景占实际使用的绝大多数（connect 完用户立即 `switch_context` 是默认 UX 流程），polling self-heal 单一通路已够。

### D4：参数集中在 `crates/cdt-ssh/src/session.rs` 顶部常量

```rust
pub const SSH_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(15);
pub const SSH_KEEPALIVE_MAX: usize = 3;
```

`pub` 让 `cdt-api` / 测试可读取做 wait/sleep 配合；不写成 `AppConfig` 字段 —— v1 不暴露 UI、不持久化，避免引入"keepalive 配错了完全连不上"的支持负担。

### D5：`build_client_config()` helper 函数

把 `russh::client::Config { keepalive_interval, keepalive_max, ..Default::default() }` 抽成 `fn build_client_config() -> Arc<client::Config>`：

- `connect_inner` 调一次
- 单测调一次：`assert_eq!(cfg.keepalive_interval, Some(SSH_KEEPALIVE_INTERVAL))` / `assert_eq!(cfg.keepalive_max, SSH_KEEPALIVE_MAX)`

`russh::client::Config` 不实现 `Clone` 也不实现 `PartialEq`，所以"其它字段与 default 一致"由**构造方式**保证（`..Default::default()` syntax），不靠运行时全字段断言；spec 与单测都按此契约表达。

未来如要改 transport-level 配置（如 algo preferences、`maximum_packet_size`），都集中此处。

## Risks / Trade-offs

- **流量小幅增加**：闲置 SSH context 每 15s 发 1 个 keepalive request + 1 个 reply，每包 < 100B → 平均 ~13B/s；对 SSH 心跳级流量来说可忽略
- **测试覆盖**：keepalive 真实 timeout 行为难单测（需要真 SSH server + 模拟服务端不回复），本 change 仅单测配置值；行为级在 `live_connect_to_local_docker` 已有的 `#[ignore]` 集成测试里手验，回归测试 fixture（docker openssh 临时调小 ClientAliveInterval / iptables 拦 idle 等手段）留 followup
- **与 `inactivity_timeout` 互动**：保持 `inactivity_timeout = None`，所以连接生命周期由用户主动 disconnect / 自愈触发 disconnect 决定，不会被 russh 内部 GC 误关
- **已知未覆盖（v1 接受的小概率漏洞）**：用户 `ssh_connect` 成功**但从未** `switch_context` 也从未触发任何 IPC fs 操作的纯 idle 状态（典型场景：UI 显示 SSH 列表但不切过去用）。keepalive 触发 transport 死后：
  - russh client task 退出，`Handle` 进入 dead 状态
  - 没 polling watcher → 没人 fire `dead_signal` → 没自愈 disconnect
  - `SshSessionManager::sessions[ctx]` 残留 stale entry，`active_context_id()` 仍返 `Some(ctx)`（如果它曾是 active），直到下一次 IPC 操作撞墙
  - 影响面：`ssh_get_state` / `list_contexts` 返 stale `connected` 状态；下次任意 fs op 立刻报 `Disconnected` 并通过既有 cache wrapper / `polling_watcher` 自愈
  - 缓解：v2 可加 `RusshClientHandler::disconnected()` callback + `SshSessionManager::dead_signal`，需要对 `RusshClientHandler` 的所有权布局做小改造（注入 `Weak<SshSessionManager>` + ctx_id），约 0.5 day；不在本 change scope

## Migration Plan

无迁移：纯 client-side 行为修改；旧版本连接的 SSH context 升级后下次 `ssh_connect` 自动启用 keepalive；不改持久化 schema。

## Open Questions

无 blocker；v2 跟进项（无 watcher 场景的 transport 死探活）记入 §Risks "已知未覆盖" 不阻塞本 change apply。

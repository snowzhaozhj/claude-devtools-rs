## ADDED Requirements

### Requirement: Keep SSH transport alive via russh keepalive

系统 SHALL 在每次 `ssh_connect` 建立 russh client 时启用 transport 层 keepalive，配置为每 `SSH_KEEPALIVE_INTERVAL = 15s` 距离上次 server 数据后发一次 `SSH_MSG_GLOBAL_REQUEST keepalive@openssh.com`（`want_reply = true`），由 russh client task 内部 keepalive loop 在累计 `SSH_KEEPALIVE_MAX = 3` 之上的连续未应答 tick 后（实际 `(SSH_KEEPALIVE_MAX + 2) × SSH_KEEPALIVE_INTERVAL = 75s` 总窗口，因 russh 0.52.x 的判断 `alive_timeouts > keepalive_max` 是先比较再增加再发送的 off-by-one）主动关闭 transport。

`crates/cdt-ssh/src/session.rs` SHALL 暴露 `pub const SSH_KEEPALIVE_INTERVAL: Duration` 与 `pub const SSH_KEEPALIVE_MAX: usize`，并 SHALL 通过 `build_client_config()` helper 把这两个常量写进 `russh::client::Config`。`connect_inner` 阶段 2 SHALL 调用 `build_client_config()` 而非 `russh::client::Config::default()` 构造握手；`build_client_config` 里其它字段 SHALL 通过 `..Default::default()` 语法保留 russh 默认（`russh::client::Config` 不实现 `Clone` / `PartialEq`，一致性由构造方式保证而非运行时断言）。

启用该机制的目的：(1) **主作用**：每 15s 让 channel 双向有 SSH msg 流动，防止 server-side `ClientAliveInterval=0`（docker openssh 默认）/ NAT idle / firewall idle 把 channel 静默关闭；(2) **次作用**：让对端硬故障（拔网线 / `sshd` 重启）能在 ~75s 内被 client 主动发现，由 russh 关闭 transport，触发既有 `polling watcher` → `dead_signal` → `perform_polling_self_heal_disconnect` 自愈链路（详 `Requirement: Watch remote project directories via SFTP polling` 与 PR #205 实现）。

本 Requirement 仅约束 client config 入参与 connect 路径调用点；keepalive timeout 真触发后的自愈语义仅在**已 attach polling watcher 的 SSH context** 上生效（典型场景：用户 `ssh_connect` 后立即 `switch_context` 触发 `attach_remote_watcher`）。已 connect 但从未 `switch_context` 也从未触发任何 fs IPC 的纯 idle context 在 transport 被 keepalive 关闭后会保留 stale `SshSessionManager::sessions[ctx]` 直到下一次 fs op，属 v1 已知边界（详 design `Risks/已知未覆盖` 段），不属本 Requirement 必须解决。

#### Scenario: build_client_config enables keepalive with documented constants

- **WHEN** 调用 `build_client_config()`
- **THEN** 返回的 `Arc<russh::client::Config>` SHALL 满足 `keepalive_interval == Some(SSH_KEEPALIVE_INTERVAL)` 且 `keepalive_max == SSH_KEEPALIVE_MAX`
- **AND** 实现 SHALL 通过 `russh::client::Config { keepalive_interval, keepalive_max, ..Default::default() }` 语法构造，确保其它字段从 russh `Default::default()` 继承（不引入额外副作用）

#### Scenario: connect_inner uses build_client_config for handshake

- **WHEN** 调用方触发 `ssh_connect` 进入 `connect_inner` 阶段 2 的 `russh::client::connect_stream`
- **THEN** 传入的 config SHALL 由 `build_client_config()` 产出
- **AND** SHALL NOT 是 `russh::client::Config::default()`
- **AND** transport 握手成功后 russh client task 内部 keepalive loop SHALL 按 `SSH_KEEPALIVE_INTERVAL` 周期运行（由 russh 0.52.x 的内部实现保证，本契约只钉死 client config 入参）

#### Scenario: Keepalive timeout closes transport so polling self-heal can run

- **WHEN** 已建立的 SSH context 处于 active 且有 polling watcher attached
- **AND** 对端不再回复任何 SSH 报文（NAT idle close / `sshd` 被 KILL / iptables 丢包）
- **AND** 累计 `SSH_KEEPALIVE_MAX + 1` 个 tick（约 `(SSH_KEEPALIVE_MAX + 2) × SSH_KEEPALIVE_INTERVAL = 75s` 后）keepalive 仍未收到应答
- **THEN** russh client task SHALL 返回 `russh::Error::KeepaliveTimeout` 并关闭 transport
- **AND** 该 context 上后续 SFTP 调用 SHALL 收到 `session closed` / `broken pipe` 类错误，由 `polling_watcher::is_permanent_sftp_failure` 识别为永久错误
- **AND** 累计 `PERMANENT_FAILURE_THRESHOLD` 次后 `dead_signal.notify_one()` 触发 `perform_polling_self_heal_disconnect`，emit `ContextChanged { active_context_id: None, kind: Local }`，与既有自愈链路一致

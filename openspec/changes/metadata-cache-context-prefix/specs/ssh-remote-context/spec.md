## ADDED Requirements

### Requirement: `SshSessionManager` 暴露 `HostSignature` 派生的 `ContextId` 查询

系统 SHALL 在 SSH `connect_inner` 的 host alias resolve 阶段（stage 0）完成后，通过 `cdt_fs::SshConfigDigestInput::from(&ResolvedHost)` + `cdt_fs::HostSignature::from_ssh_config_fields(&input)` 计算并缓存当前 SSH context 的 `HostSignature`，存放在 `SshSessionResources.host_signature` 字段；`HostSignature` MUST NOT 在每次 IPC 调用时重新通过 `ssh -G` 子进程 resolve（避免 50-200ms 子进程 spawn overhead）。

`SshSessionManager` SHALL 暴露 `async fn context_id(&self, context_id: &str) -> Option<cdt_fs::ContextId>` 查询方法：

- 入参为已注册 SSH context 的 `context_id` 字符串
- 命中时 SHALL 从 `sessions.lock().await.get(context_id)` 取 `SshSessionResources` 的 `host_signature` 与 `remote_home`，合成 `ContextId::ssh(host_signature.clone(), remote_home.clone())` 返回 `Some(_)`
- 未注册（含已 disconnect / 未连接成功）时 SHALL 返回 `None`
- SHALL NOT 调用 `resolve_host_via_ssh_g` 子进程

`SshSessionManager::insert_test_context` test helper SHALL 接受 `Option<cdt_fs::HostSignature>` 参数；缺省时 SHALL 用 `(host, port, user)` 字符串拼接做 fake SHA-256 digest 构造一个 `HostSignature`，使不同 host 的测试 fixture 自然产不同 digest。

#### Scenario: connect 路径自动计算并存储 `HostSignature`

- **WHEN** `SshSessionManager::connect(request)` 走到 stage 0 完成 `resolve_host_via_ssh_g` 拿到 `ResolvedHost`
- **THEN** SHALL 通过 `SshConfigDigestInput::from(&resolved)` 构造 input
- **AND** SHALL 调 `HostSignature::from_ssh_config_fields(&input)` 计算 digest
- **AND** SHALL 在最终构造 `SshSessionResources` 时填入 `host_signature` 字段
- **AND** SHALL NOT 在后续 IPC / cache lookup 时再次跑 `ssh -G`

#### Scenario: `context_id(&str)` 返回 `ContextId::ssh(...)`

- **WHEN** 调用方对一个已连接的 SSH context 调用 `ssh_mgr.context_id("ssh-host-A").await`
- **THEN** 返回 `Some(ContextId)`，其 `backend_kind == FsKind::Ssh`
- **AND** `host_signature` SHALL 等于 connect 时计算并存储的 `HostSignature`
- **AND** `root_or_home` SHALL 等于 `SshSessionResources.remote_home`

#### Scenario: 未注册 context 返回 `None`

- **WHEN** 调用方对一个未注册（或已 disconnect）的 context_id 调用 `ssh_mgr.context_id(...)`
- **THEN** SHALL 返回 `None`，且 SHALL NOT panic 或 spawn 子进程

#### Scenario: 同 host reconnect 后 `ContextId` 一致

- **WHEN** 用户先 connect → disconnect → 再 connect 同一 SSH host A（`~/.ssh/config` 未变 AND 两次 connect 均走 `ssh -G` 成功路径）
- **THEN** 两次 connect 后通过 `context_id("ssh-host-A").await` 拿到的 `ContextId` SHALL `==`（`HostSignature.config_digest` 是 resolved ssh config 的纯函数，不含随机或时序成分）
- **AND** 任何用此 `ContextId` 做 key 的 cache entry SHALL 跨 reconnect 复用

#### Scenario: degraded fallback 与 `ssh -G` 路径产 `ContextId` 安全不等（by-design miss）

- **WHEN** 第一次 connect 走 `resolve_host_via_ssh_g` 成功路径，`ResolvedHost` 含 `proxyjump` / `proxycommand` / `hostkeyalias` 字段 → 计算出 `HostSignature` digest A
- **AND** 第二次 reconnect 时 `ssh` 子进程缺失 / `ssh -G` 失败，走 `fallback_via_config_parser` 路径，`ResolvedHost.proxyjump = .proxycommand = .hostkeyalias = None` → 计算出 `HostSignature` digest B
- **THEN** digest A `!=` digest B（不同字段集合 → 不同 SHA-256 输入）
- **AND** 两次 connect 派生的 `ContextId` SHALL NOT `==`
- **AND** 任何用 digest A 做 key 写入的 cache entry SHALL NOT 被 digest B 的 lookup 命中——这是 **by-design safe miss**（degraded 路径对 host 的连接拓扑认知降级，与 ssh -G 路径不等价；落到不同 cache namespace 防止"基于错误连接假设拿到陈旧远端数据"）
- **AND** 用户体感为 reconnect 后 session 列表冷扫一次，UX 多几秒，但绝不串扰数据

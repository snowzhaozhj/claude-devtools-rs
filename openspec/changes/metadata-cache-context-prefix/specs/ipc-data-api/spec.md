## MODIFIED Requirements

### Requirement: `extract_session_metadata` 按 `FileSignature` 缓存

`LocalDataApi` SHALL 持有一个内部 LRU 缓存（不使用全局单例），以 `(ContextId, PathBuf)` 复合 key 记录上一次扫描时的 `(FileSignature, title, message_count, messages_ongoing, git_branch)`。其中 `ContextId` 由 `cdt_fs::ContextId` 定义（详 `openspec/specs/fs-abstraction/spec.md` §`ContextId` 三元组作为 cache key 前缀），Local 模式下 SHALL 为 `ContextId::local(claude_root)`，SSH 模式下 SHALL 为 `ContextId::ssh(host_signature, remote_home)`。`FileSignature` MUST 至少包含：

- `mtime`：文件最后修改时间
- `size`：文件字节数
- `identity`：文件身份 —— Unix `(dev, ino)`；Windows 与其它平台退化为空（详 design D1f）

**等价性是 best-effort**：在常规 append-only 写入路径下，`FileSignature` 字段 byte-equal 即视为文件未变。inode reuse + mtime/size 三维同时撞车的极端场景可能假命中，由后续任何文件变化的 file-change 自然恢复。

再次调用相同 `(ContextId, path)` 时 SHALL 先通过 `FileSystemProvider::stat(path)` 抓取 `FsMetadata` 并经 `FileSignature::from_fs_metadata(&meta)` 构造 signature；若 byte-equal 等于缓存记录 THEN MUST 直接返回基于缓存数据合成的 `SessionMetadata`，**不**再 line-by-line 重读全文件；否则正常扫描并把结果写回缓存。SHALL NOT 直接调用 `tokio::fs::metadata(path)` —— stat 必须通过 `FileSystemProvider` 抽象走当前 active context 的 provider，保证 Local context 命中 `LocalFileSystemProvider`、SSH context 命中 `SshFileSystemProvider`，避免跨 context fs 实例错配。

由于 `is_ongoing` 字段含 `is_file_stale(path)` 时间敏感判定，缓存 MUST 仅缓存"基于消息序列结构"的 `messages_ongoing` 中间值（即 `cdt_analyze::check_messages_ongoing` 的结果），而 `is_ongoing = messages_ongoing && !is_session_stale(signature.mtime, SystemTime::now())` MUST 在每次 lookup 时根据当前 wall clock 实时计算合成——不得直接缓存 `is_ongoing` 终态。

缓存 SHALL 在以下任一条件下走 cache miss：

- 缓存中无该 `(ContextId, path)` 组合（即使同 path 不同 ContextId 也 miss）
- `mtime` / `size` / `identity` 任一不一致
- `FileSystemProvider::stat` 返回 `Err(_)`（任意 `FsError` variant）

缓存容量 SHALL 上限 2000 entries（全局总和，跨 `ContextId` 共享同 pool），按 LRU 淘汰；命中时 MUST 把命中 key bump 到队首避免冷热混淆。

#### Scenario: 相同 `(ContextId, path)` `FileSignature` 不变命中缓存

- **WHEN** 调用 metadata 缓存 wrapper 且 `FileSystemProvider::stat` 拿到的 `FileSignature` 与缓存记录字段 byte-equal 等于缓存记录
- **THEN** MUST 直接返回基于缓存数据合成的 `SessionMetadata`，且 SHALL NOT 再调用 `tokio::io::AsyncBufReadExt::lines` 读全文件
- **AND** SHALL NOT 调用 `tokio::fs::metadata` —— stat 全程经过 `FileSystemProvider`

#### Scenario: mtime 不一致触发重扫

- **WHEN** 调用 metadata 缓存 wrapper 且 `FileSystemProvider::stat` 拿到的 `mtime` 与缓存记录不同
- **THEN** MUST 走原有 line-by-line 全文件扫描路径，并以新 `FileSignature` 与新结果覆盖缓存

#### Scenario: 文件被 rename 替换（inode 变化）触发重扫（仅 Unix）

- **WHEN** 调用 metadata 缓存 wrapper 且 `FileSystemProvider::stat` 拿到的 `identity`（Unix `(dev, ino)`）与缓存记录不同 —— 即便 mtime 与 size 巧合相同
- **THEN** MUST 走 cache miss 分支重新扫描
- Windows 与其它平台 identity 退化为 `None`，此 Scenario 由 mtime/size 维度兜底（best-effort，详 design D1f）

#### Scenario: 缓存命中后实时重算 stale 状态

- **WHEN** 缓存命中（`(ContextId, path)` key 等 + `FileSignature` 一致），且缓存条目的 `messages_ongoing = true`，且当前 wall clock 距 `mtime` 已超过 `STALE_SESSION_THRESHOLD`（5 分钟）
- **THEN** 返回的 `SessionMetadata.is_ongoing` MUST 为 `false`（`messages_ongoing && !stale = true && !true = false`）；缓存 SHALL NOT 因此被 invalidate（`FileSignature` 仍正确反映文件未变，下次访问还能复用其它字段）

#### Scenario: 文件 size 变小触发重扫

- **WHEN** 调用 metadata 缓存 wrapper 且 `FileSystemProvider::stat` 拿到的 `size` 比缓存记录小
- **THEN** MUST 走 cache miss 分支重新扫描

#### Scenario: stat 失败时走 cache miss

- **WHEN** 调用 metadata 缓存 wrapper 但 `FileSystemProvider::stat(path)` 返回 `Err(_)`
- **THEN** MUST 走原路径（由内部 `open_read`/`File::open` 自身决定返回空 `SessionMetadata`），且 SHALL NOT 把空结果写入缓存

#### Scenario: 缓存超过容量按 LRU 淘汰

- **WHEN** 缓存已达 2000 entries 时再调用一个新 `(ContextId, path)` 组合
- **THEN** MUST 淘汰当前最久未访问的条目后再写入新条目，缓存大小始终 ≤ 2000
- **AND** 容量上限是跨 `ContextId` 全局总和，不按 context 拆分配额

#### Scenario: 缓存命中时把 key bump 到队首

- **WHEN** lookup 在缓存中命中 `(ContextId, path)`
- **THEN** MUST 把该 `(ContextId, path)` 组合的 LRU 位置移到队首（最新访问），后续淘汰循环中该 key 不会被冷热顺序错误淘汰

#### Scenario: Local 与 SSH 同字面 path 不串扰

- **WHEN** Local context 写入 cache `(ContextId::local(local_root), "/foo/s.jsonl")`，随后切换到 SSH context 后查询同字面 path `(ContextId::ssh(host_sig, remote_home), "/foo/s.jsonl")`
- **THEN** 查询 SHALL miss —— 不同 `ContextId` 即不同 cache key，即使 path 字面相同
- **AND** Local entry SHALL NOT 被 SSH 查询误命中或覆盖
- **AND** 切回 Local 后再查同 Local key SHALL hit（cache 仍持有该 entry）

#### Scenario: 不同 SSH host 之间不串扰

- **WHEN** 用户连 SSH host A 写入 cache，再切到 SSH host B 查询同字面 path
- **AND** host A 与 host B 的 `HostSignature.config_digest` 不同
- **THEN** host B 的查询 SHALL miss —— `ContextId` 不等

### Requirement: metadata 缓存 ownership 由 `LocalDataApi` 持有

`LocalDataApi` SHALL 通过一个 `Arc<std::sync::Mutex<MetadataCache>>` 字段持有缓存实例。所有构造器（`new` / `new_with_xxx`）MUST 初始化为空 cache。**禁止**用全局 `OnceLock` / `static` 单例 ——多个 `LocalDataApi` 实例（HTTP server + Tauri IPC 各自构造）必须各自独立持有 cache，互相不共享。

`LocalDataApi` SHALL 提供 `async fn active_fs_and_context(&self) -> (Arc<dyn cdt_fs::FileSystemProvider>, PathBuf, cdt_fs::ContextId)` inherent 方法以支撑 `(ContextId, PathBuf)` cache key 拓扑：

- 函数内部 SHALL 单次读 `ssh_mgr.active_context_id().await` 决定走 SSH 还是 Local 分支
- SSH 分支：若同一 active context 的 provider 仍存在（`ssh_mgr.provider(&id).await` 命中），SHALL 返回 `(Arc::new(provider), provider.remote_home(), ssh_mgr.context_id(&id).await)`；任一 lookup miss 时 SHALL 安全降级到 Local 分支（disconnect 中间态等并发场景，详 design D3 / D3-bis）
- Local 分支：SHALL 返回 `(cdt_fs::local_handle(), projects_dir, ContextId::local(projects_dir))`，fs 与 ctx **来自同一快照**
- `switch_context` / `ssh_connect` / `ssh_disconnect` 实现 SHALL NOT 主动清空 `metadata_cache`，且 SHALL NOT 持有外部 `current_context_id` 字段（依靠 ContextId Hash/Eq 隔离 + LRU 自然淘汰，与 `openspec/specs/fs-abstraction/spec.md` §"fs-related cache 必须采用'单实例 + ContextId key 前缀'拓扑"第 4 条一致）

`LocalDataApi` MUST NOT 持 `fs: Arc<dyn FileSystemProvider>` 字段或等价静态注入——避免死字段污染（design D2）；cache callsite 一律走 `active_fs_and_context()` 拿当前 active provider。

`extract_session_metadata` 自身 MUST 保留为纯函数（不持 cache），缓存查询 wrapper（`extract_session_metadata_cached(cache, fs, ctx, path)` 与 `try_lookup_cached_metadata(cache, fs, ctx, path)`）MUST 作为内部辅助函数，由 `LocalDataApi` 的方法或 `scan_metadata_for_page` 调用。两个 wrapper 的签名 SHALL 接受 `fs: &dyn FileSystemProvider` 与 `context_id: &ContextId` 参数，**禁止**在 wrapper 内部直接调用 `tokio::fs::metadata` 或硬编码 fs provider 类型。

**本 change scope 边界**（design D8）：cache stat 路径 SHALL 走 `FileSystemProvider::stat`；cache miss 后的**扫描路径**（line-by-line `BufReader::lines`）本 change **保留** `tokio::fs::File::open`，**不**强制切 `fs.open_read`——因为现有 SSH callsite（`list_sessions_skeleton` SSE inline emit / `get_session_detail` SSH 分支）不经过 metadata cache wrapper，cache wrapper 当前有效调用面 = Local context only。完整 SSH 接入 + scanner 切 `fs.open_read` 留 PR-D 处理。

#### Scenario: 多个 `LocalDataApi` 实例独立持有 cache

- **WHEN** 测试或运行时构造两个 `LocalDataApi` 实例 A 与 B
- **THEN** A 的 `metadata_cache` 与 B 的 `metadata_cache` MUST 是独立 `Arc<Mutex<MetadataCache>>` 实例，A 中的缓存写入 SHALL NOT 影响 B 中的 lookup 结果

#### Scenario: `extract_session_metadata` 保持纯函数签名

- **WHEN** 现有调用方（含单元测试 `extract_*`）直接调用 `extract_session_metadata(path)`
- **THEN** 该函数签名 MUST 保持 `pub async fn extract_session_metadata(path: &Path) -> SessionMetadata`，不接受 cache / fs / context_id 参数；行为与本 change 之前完全一致（line-by-line 全文件扫描）

#### Scenario: `LocalDataApi` 不持 `fs` 字段也不持 `current_context_id` 字段

- **WHEN** 检查 `LocalDataApi` struct 定义
- **THEN** SHALL NOT 含 `fs: Arc<dyn cdt_fs::FileSystemProvider>` 字段（design D2 禁止死字段）
- **AND** SHALL NOT 含 `current_context_id: Mutex<ContextId>` 或等价显式字段（design D3 禁止 fs/ctx 不一致并发窗口）
- **AND** SHALL 提供 `async fn active_fs_and_context(&self)` inherent 方法就地合成 fs 与 ctx

#### Scenario: `active_fs_and_context` 让 fs 与 ctx 来自同一快照

- **WHEN** 调用方在任意时刻调 `local_api.active_fs_and_context().await`
- **THEN** 返回的 `(fs, projects_dir, ctx)` 三元组 SHALL 自洽：fs.kind() == Local 时 ctx == `ContextId::local(projects_dir)`；fs.kind() == Ssh 时 ctx 的 `host_signature` 等于该 SSH provider 在 connect 时计算的 `HostSignature`
- **AND** SHALL NOT 存在"fs 是 Local provider 但 ctx 是 SSH ContextId"或反之的不一致组合（design D3-bis 安全降级）

#### Scenario: `ssh_connect` 强制 disconnect 旧 active 期间 cache 不被串扰

- **WHEN** 在 SSH context A 已 active 时调 `ssh_connect(host_B_request)`，触发 `SshSessionManager::connect` 内部"强制 disconnect 旧 active"流程（`session.rs:323-326`）
- **AND** 在 disconnect 旧 active 与 connect 新 host 之间的并发窗口内，另一个 IPC 调用走 `active_fs_and_context()`
- **THEN** 该并发调用 SHALL 拿到自洽的 `(Local fs, Local ctx)` 或 `(SSH B fs, SSH B ctx)`，而 SHALL NOT 拿到混合的 `(Local fs, SSH A ctx)`

#### Scenario: `ssh_disconnect` 不清 cache

- **WHEN** 调用 `ssh_disconnect("ssh-host-A")`
- **THEN** SHALL NOT 清空 cache 中该 ContextId 的 entry
- **WHEN** 用户随后 `ssh_connect` 同 host A（reconnect 后 `host_signature` 相同）
- **THEN** 同 `ContextId::ssh(host_signature, remote_home)` 的 cache entry SHALL 立即可用（无需冷扫）

#### Scenario: 本 change 不强制切 scanner 路径（design D8 scope 边界）

- **WHEN** cache lookup miss 后触发 `extract_session_metadata_with_ongoing` 内的扫描路径
- **THEN** 本 change SHALL 保留 `tokio::fs::File::open` + `BufReader::lines` 扫描实现，未强制切 `FileSystemProvider::open_read`
- **AND** SHALL 在 design.md D8 + tasks.md follow-up 显式记录"PR-D 完成 scanner 切 fs.open_read + SSH callsite 接入 cache wrapper"，作为未来 spec 演进锚点

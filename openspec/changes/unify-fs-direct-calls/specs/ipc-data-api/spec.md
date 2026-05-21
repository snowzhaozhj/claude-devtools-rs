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

**cache miss 后的扫描路径** MUST 通过 `FileSystemProvider::open_read(path)` 拿到 `Box<dyn AsyncRead + Send + Unpin>` 包装到 `BufReader::with_capacity(SCANNER_BUF_BYTES, reader)` 后逐行解析（`SCANNER_BUF_BYTES` SHALL 为 32 KiB 与 SFTP `SSH_FXP_READ` packet 上限对齐；详 design D5）；SHALL NOT 直接调用 `tokio::fs::File::open` 等 fs trait 之外的 fs 入口。这条 SHALL 取代本 capability 之前由 PR-B（`metadata-cache-context-prefix`）的 scope 边界保留 `tokio::fs::File::open` 的 spec 注解。

#### Scenario: 相同 `(ContextId, path)` `FileSignature` 不变命中缓存

- **WHEN** 调用 metadata 缓存 wrapper 且 `FileSystemProvider::stat` 拿到的 `FileSignature` 与缓存记录字段 byte-equal 等于缓存记录
- **THEN** MUST 直接返回基于缓存数据合成的 `SessionMetadata`，且 SHALL NOT 再调用 `tokio::io::AsyncBufReadExt::lines` 读全文件
- **AND** SHALL NOT 调用 `tokio::fs::metadata` —— stat 全程经过 `FileSystemProvider`

#### Scenario: mtime 不一致触发重扫

- **WHEN** 调用 metadata 缓存 wrapper 且 `FileSystemProvider::stat` 拿到的 `mtime` 与缓存记录不同
- **THEN** MUST 走 line-by-line 全文件扫描路径——但 SHALL 通过 `FileSystemProvider::open_read` 而非 `tokio::fs::File::open`，并以新 `FileSignature` 与新结果覆盖缓存

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

#### Scenario: scanner 通过 `FileSystemProvider::open_read` 切 dyn AsyncRead

- **WHEN** cache miss 后触发 `extract_session_metadata_with_ongoing(fs, path)` 扫描路径
- **THEN** 函数体 SHALL 调 `fs.open_read(path).await` 拿 `Box<dyn AsyncRead + Send + Unpin>`，并用 `BufReader::with_capacity(SCANNER_BUF_BYTES /* 32 KiB */, reader)` 包装后逐行 `next_line().await` 喂 `IsOngoingStateMachine` / 标题 / 计数 / git branch 状态机
- **AND** 函数体 SHALL NOT 直接调用 `tokio::fs::File::open` 或任何 `tokio::fs::*` 入口
- **AND** SSH context 下扫描路径的 fs op 计数 SHALL 为 1 个 `open_read`（不论文件大小，BufReader 内部分多次 `poll_read` 但都通过同一 `Box<dyn AsyncRead>`）

### Requirement: metadata 缓存 ownership 由 `LocalDataApi` 持有

`LocalDataApi` SHALL 通过一个 `Arc<std::sync::Mutex<MetadataCache>>` 字段持有缓存实例。所有构造器（`new` / `new_with_xxx`）MUST 初始化为空 cache。**禁止**用全局 `OnceLock` / `static` 单例 ——多个 `LocalDataApi` 实例（HTTP server + Tauri IPC 各自构造）必须各自独立持有 cache，互相不共享。

`LocalDataApi` SHALL 提供 `async fn active_fs_and_context(&self) -> (Arc<dyn cdt_fs::FileSystemProvider>, PathBuf, cdt_fs::ContextId)` inherent 方法（relaxed，cache 内部路径 / 测试 helper 用）以及 `async fn active_fs_and_context_strict(&self) -> Result<(Arc<dyn cdt_fs::FileSystemProvider>, PathBuf, cdt_fs::ContextId), ApiError>` inherent 方法（strict，user-facing IPC handler 用，SSH disconnect 中间态返 `not_found`）以支撑 `(ContextId, PathBuf)` cache key 拓扑：

- 函数内部 SHALL 单次读 `ssh_mgr.active_context_id().await` 决定走 SSH 还是 Local 分支
- SSH 分支：若同一 active context 的 provider 仍存在（`ssh_mgr.provider_and_context_id(&id).await` 命中），SHALL 返回 `(Arc::new(provider), provider.remote_home(), context_id)`；任一 lookup miss 时 relaxed 变体 SHALL 安全降级到 Local 分支（disconnect 中间态等并发场景，详 PR-B design D3 / D3-bis），strict 变体 SHALL 返 `Err(ApiError::not_found)`
- Local 分支：SHALL 返回 `(cdt_fs::local_handle(), projects_dir, ContextId::local(projects_dir))`，fs 与 ctx **来自同一快照**
- `switch_context` / `ssh_connect` / `ssh_disconnect` 实现 SHALL NOT 主动清空 `metadata_cache`，且 SHALL NOT 持有外部 `current_context_id` 字段（依靠 ContextId Hash/Eq 隔离 + LRU 自然淘汰，与 `openspec/specs/fs-abstraction/spec.md` §"fs-related cache 必须采用'单实例 + ContextId key 前缀'拓扑"第 4 条一致）

`LocalDataApi` MUST NOT 持 `fs: Arc<dyn FileSystemProvider>` 字段或等价静态注入——避免死字段污染（PR-B design D2）；cache callsite 一律走 `active_fs_and_context()` / `active_fs_and_context_strict()` 拿当前 active provider。

`extract_session_metadata` 自身 MUST 保留为 path-only 公开函数（兼容现有调用方 / 单测），内部以 `cdt_fs::local_handle()` 包装；SSH-aware 入口 SHALL 通过 `extract_session_metadata_with_ongoing(fs, path)` 暴露给 cache wrapper。缓存查询 wrapper（`extract_session_metadata_cached(cache, fs, ctx, path)` 与 `try_lookup_cached_metadata(cache, fs, ctx, path)`）MUST 作为内部辅助函数，由 `LocalDataApi` 的方法或 `scan_metadata_for_page` 调用。两个 wrapper 的签名 SHALL 接受 `fs: &dyn FileSystemProvider` 与 `context_id: &ContextId` 参数，**禁止**在 wrapper 内部直接调用 `tokio::fs::metadata` 或硬编码 fs provider 类型。

**SSH callsite 接入 cache wrapper**（本 change 落地）：`list_sessions_skeleton` / `build_group_session_page` 在 SSH active context 下 SHALL 走与 Local context 完全一致的 cache wrapper 调用——**不再**走 PR-B 临时保留的 inline `if is_remote { return None }` 早退路径。

SSH list 路径**hot path（用户感知）SHALL 走 cache hit trust**：UI 立刻拿 in-memory cache 内容渲染列表（0 fs op），不等 fs.stat RTT；后台 spawn `scan_metadata_for_page` task 异步校验 cache freshness，每条改动通过 SSE event 推差量给 UI 增量更新（与 Local 现有 SkeletonThenStream 体验一致）。**朴素 per-session 串行 fs.stat 路径 SHALL NOT 出现在 SSH list hot path 用户感知阻塞段** —— `Arc<Mutex<SftpSession>>` 全锁串行（PR-A D3 已知假 batch）会让 50 sessions × 50ms = 2.5s 直接超 sidebar 首屏预算。

SSH 后台校验路径（本 segment 实现）：spawn `scan_metadata_for_page` per project，内部 per-session 调 `extract_session_metadata_cached`（`fs.stat` 拿 signature → cache mismatch → `parse_file_via_fs` 走 `fs.open_read` 重 parse）。`fs.read_dir_with_metadata` per-project N→1 stat batch 优化（SFTP READDIR reply 自带 entry attrs）SHALL 留 PR-D2 follow-up 实施。

fs op 计数：

- Hot path cache hit（用户切回已访问 SSH host）：UI 渲染 fs op = 0（via `MetadataCache::lookup_trust_cached`）；后台 `scan_metadata_for_page` per session `fs.stat` 校验 signature，mismatch 时 `fs.open_read` 重 parse
- Cold start 首次连 SSH：UI 立即返 SessionSummary 骨架（title=None / message_count=0）；spawn `scan_metadata_for_page` per session 异步刷新，metadata 通过 SSE 推差量；典型每 session ~50-100ms RTT（per-session 串行），真消除卡顿留 PR-D2 batch + PR-F SFTP message-id pipeline

新加 cache helpers（PR-D2 follow-up wire 入 batch 路径用，本 segment 仅 `MetadataCache::lookup_trust_cached` 在 SSH list hot path 使用）：
- `MetadataCache::lookup_with_known_signature(&mut self, ctx, path, sig) -> Option<MetadataCacheEntry>`：用调用方提供的 signature 直接查 cache，跳过内部 stat（PR-D2 用：batch wrapper 先 read_dir_with_metadata 拿全 dir sig 后逐条 lookup）
- `MetadataCache::lookup_trust_cached(&mut self, ctx, path) -> Option<MetadataCacheEntry>`：hot path cache hit trust，不校验 signature 直接返 entry（**本 segment 已在 SSH list_sessions_skeleton inner + outer + build_group_session_page 使用**）

#### Scenario: 多个 `LocalDataApi` 实例独立持有 cache

- **WHEN** 测试或运行时构造两个 `LocalDataApi` 实例 A 与 B
- **THEN** A 的 `metadata_cache` 与 B 的 `metadata_cache` MUST 是独立 `Arc<Mutex<MetadataCache>>` 实例，A 中的缓存写入 SHALL NOT 影响 B 中的 lookup 结果

#### Scenario: `extract_session_metadata` 保持 path-only 公开签名

- **WHEN** 现有调用方（含单元测试 `extract_*`）直接调用 `extract_session_metadata(path)`
- **THEN** 该函数签名 MUST 保持 `pub async fn extract_session_metadata(path: &Path) -> SessionMetadata`，内部 SHALL 用 `cdt_fs::local_handle()` 包装到 `extract_session_metadata_with_ongoing(fs, path)`

#### Scenario: `LocalDataApi` 不持 `fs` 字段也不持 `current_context_id` 字段

- **WHEN** 检查 `LocalDataApi` struct 定义
- **THEN** SHALL NOT 含 `fs: Arc<dyn cdt_fs::FileSystemProvider>` 字段（design D2 禁止死字段）
- **AND** SHALL NOT 含 `current_context_id: Mutex<ContextId>` 或等价显式字段（design D3 禁止 fs/ctx 不一致并发窗口）
- **AND** SHALL 提供 `active_fs_and_context()` (relaxed) + `active_fs_and_context_strict()` (strict) 两个 inherent 方法

#### Scenario: `active_fs_and_context` 让 fs 与 ctx 来自同一快照

- **WHEN** 调用方在任意时刻调 `local_api.active_fs_and_context().await`
- **THEN** 返回的 `(fs, projects_dir, ctx)` 三元组 SHALL 自洽：fs.kind() == Local 时 ctx == `ContextId::local(projects_dir)`；fs.kind() == Ssh 时 ctx 的 `host_signature` 等于该 SSH provider 在 connect 时计算的 `HostSignature`
- **AND** SHALL NOT 存在"fs 是 Local provider 但 ctx 是 SSH ContextId"或反之的不一致组合（PR-B design D3-bis 安全降级）

#### Scenario: `ssh_connect` 强制 disconnect 旧 active 期间 cache 不被串扰

- **WHEN** 在 SSH context A 已 active 时调 `ssh_connect(host_B_request)`，触发 `SshSessionManager::connect` 内部"强制 disconnect 旧 active"流程（`session.rs:323-326`）
- **AND** 在 disconnect 旧 active 与 connect 新 host 之间的并发窗口内，另一个 IPC 调用走 `active_fs_and_context()`
- **THEN** 该并发调用 SHALL 拿到自洽的 `(Local fs, Local ctx)` 或 `(SSH B fs, SSH B ctx)`，而 SHALL NOT 拿到混合的 `(Local fs, SSH A ctx)`

#### Scenario: `ssh_disconnect` 不清 cache

- **WHEN** 调用 `ssh_disconnect("ssh-host-A")`
- **THEN** SHALL NOT 清空 cache 中该 ContextId 的 entry
- **WHEN** 用户随后 `ssh_connect` 同 host A（reconnect 后 `host_signature` 相同）
- **THEN** 同 `ContextId::ssh(host_signature, remote_home)` 的 cache entry SHALL 立即可用（无需冷扫）

#### Scenario: SSH 路径 hot path cache hit trust（取代 PR-B 早退 + 取代朴素串行 stat）

- **WHEN** `list_sessions_skeleton` / `build_group_session_page` 在 SSH active context 下执行 page 处理且 cache 中持有该 ContextId 的 entry
- **THEN** UI 渲染路径 SHALL 调 `cache.lookup_trust_cached(&ctx, path)` 直接拿 cache 内容（0 fs op），不等 fs.stat 校验
- **AND** SHALL NOT 出现 `if is_remote { return None }` 类 cache lookup 早退（取代 PR-B `metadata-cache-context-prefix` design D8 的临时 scope 边界）
- **AND** SHALL NOT 出现 per-session 串行 `fs.stat` 校验（朴素方案在 SSH 上 50×50ms = 2.5s 超 sidebar 预算 5×；codex 二审 Blocking #1）

#### Scenario: SSH 后台 scan task 走 `scan_metadata_for_page` + SSE 推差量

- **WHEN** UI 渲染完 cache hit trust 内容后（或 cache miss 时启动）
- **THEN** SHALL spawn `scan_metadata_for_page` 后台 task per project；task 内部 per-session 调 `extract_session_metadata_cached`（`fs.stat` + cache miss 调 `parse_file_via_fs` 走 `fs.open_read`）
- **AND** 每条改动通过 `session_metadata_update` SSE event 推差量给 UI 增量更新
- **AND** task SHALL NOT 阻塞 list_sessions IPC 响应——hot path 走 cache trust 立即返回；后台 task 异步更新通过 SSE channel
- **AND** task SHALL 注册 abort handle 到 `LocalDataApi::active_scans` map；`ssh_disconnect` / `switch_context` 时所有 in-flight scan task SHALL 通过 `abort_scans_for_context` 被 abort（codex 二审 H2 修订 + design D3-bis）
- **AND** 本 segment **不**实现 `fs.read_dir_with_metadata` per-project N→1 stat batch 优化；scan 仍是 per-session 串行（PR-D2 follow-up：把 batch + `MetadataCache::lookup_with_known_signature` 上层加进 `scan_metadata_for_page` 复用 SFTP READDIR reply 自带 entry attrs）

#### Scenario: 冷启动 SSH list_sessions（cache 无 entry）

- **WHEN** 用户首次连 SSH host A 调 `list_sessions`，cache 中无该 ContextId 的 entry
- **THEN** UI 立即返 SessionSummary 骨架（title=None / message_count=0）走 SkeletonThenStream
- **AND** 入 page_jobs 后 spawn `scan_metadata_for_page` per project，内部 per-session 调 `extract_session_metadata_cached`（`fs.stat` + `parse_file_via_fs` 走 `fs.open_read`）
- **AND** SHALL 通过 `session_metadata_update` SSE event 推骨架 + 增量 metadata 给 UI；前端按 SkeletonThenStream 模式先渲染骨架后填充 metadata
- **AND** 本 segment 不走 `fs.read_dir_with_metadata` per-project batch（PR-D2 follow-up）；当前 per-session 串行 ~50-100ms RTT/session，真消除留 PR-F SFTP message-id pipeline

### Requirement: `get_tool_output` 与 `get_image_asset` 走 parsed-message LRU 缓存

`LocalDataApi` SHALL 持有一个内部 parsed-message LRU 缓存（不使用全局单例），以 `(cdt_fs::ContextId, PathBuf)` 二元组为 key（**MUST** 把 `ContextId` 作为 key 的第一成员；裸 `PathBuf` 作 key **MUST NOT** 出现），缓存值为 `(FileSignature, Arc<Vec<ParsedMessage>>)` 二元组。`get_tool_output` 与 `get_image_asset` MUST 在调用 `cdt_parse::parse_file(...)` / `parse_file_via_fs(...)` 之前先查该缓存，命中时 MUST 直接复用缓存中的 `Arc<Vec<ParsedMessage>>`、SHALL NOT 重读 JSONL 全文件，亦 SHALL NOT 重新执行 line-by-line parse。

`FileSignature` 等价性 MUST 与 `MetadataCache` 同源（即 `crates/cdt-api/src/cache_signature.rs::FileSignature` 的 `(mtime, size, identity)` 三元组，identity 在 Unix 上为 `(dev, ino)`，Windows 与其它平台退化为 `None`），best-effort 语义与 `extract_session_metadata` 按 `FileSignature` 缓存 Requirement 完全一致。

stat 路径 MUST 走 `FileSystemProvider::stat`（而非 `tokio::fs::metadata`）；构造 `FileSignature` MUST 走 `FileSignature::from_fs_metadata(&FsMetadata)`（而非 deprecated `from_metadata(&std::fs::Metadata)`）。

缓存 SHALL 在以下任一条件下走 cache miss：

- 缓存中无该 `(context_id, path)` key
- stat 拿到的 `FileSignature` 与缓存记录任一字段不一致
- stat 失败

miss 路径 MUST 调用 `cdt_parse::parse_file_via_fs(fs, path)`（fs trait 适配版本，内部走 `fs.open_read(path)` + 流式 BufReader）：成功时把结果包装为 `Arc::new(messages)`，与新 `FileSignature` 一起写入缓存；解析失败时 SHALL NOT 写入缓存（避免 negative cache 引入新失效边界），由 caller 走原有错误兜底（`get_image_asset` 返回 `empty_data_uri()`、`get_tool_output` 返回 `ToolOutput::Missing`）。

`get_tool_output` 在命中缓存后 MUST 在 `Arc<Vec<ParsedMessage>>` 上重新调用 `cdt_analyze::build_chunks(&messages)` 完成 tool_use_id 匹配——本 change 不缓存 `build_chunks` 结果，仅缓存 parse 一层（详 change `parsed-message-lru-cache` design D2/D6 决策）。

缓存容量 SHALL 上限 50 entries，按 LRU 淘汰；容量按全局计算（**所有 `ContextId` 共享同一上限**，不按 context 拆配额）；命中时 MUST 把命中 key bump 到队首避免冷热混淆。

**SSH callsite 接入 cache wrapper**（本 change 落地）：`get_tool_output` / `get_image_asset` 在 SSH active context 下 SHALL 走与 Local context 完全一致的 cache wrapper 调用——**不再**走 PR-C 临时保留的 inline `fs.read_to_string + parse_jsonl_content` 路径。

**fs op 形态**（与 metadata cache 同型）：
- Cache hit byte-equal（同 session 二次访问，signature 一致）：`extract_parsed_messages_cached` 走 `fs.stat(path)` 拿当前 signature + cache lookup；hit 时 `Arc::clone` 复用 `Arc<Vec<ParsedMessage>>`，**SHALL NOT 触发 `parse_file_via_fs` 重 parse**。形态：`fs.stat = 1` + `fs.open_read = 0`
- Cache miss：1 个 `fs.stat` + 1 个 `fs.open_read`（`parse_file_via_fs` 内部 BufReader 32 KiB 分多次 `poll_read`，与 SFTP packet 上限对齐）
- Note：纯 0 fs op `ParsedMessageCache::lookup_trust_cached` + 后台 `fs.stat` 校验的设计留 PR-D2 follow-up（本 segment 已添加 helper 函数 + ADR `#[allow(dead_code)]`）

**call site 起始处一次性快照**：`get_tool_output` / `get_image_asset` SHALL 在函数入口调 `active_fs_and_context_strict()` 拿三元组同快照，user-facing IPC handler SSH disconnect 中间态 SHALL 返 `not_found` 而非 silently degrade（与 PR-C D8-bis-fix 一致）。

新加 cache helpers（与 metadata cache 同型，**PR-D2 follow-up wire 入** get_tool_output / get_image_asset hot path）：
- `ParsedMessageCache::lookup_with_known_signature(&mut self, ctx, path, sig) -> Option<Arc<Vec<ParsedMessage>>>`：用调用方提供的 signature 直接查
- `ParsedMessageCache::lookup_trust_cached(&mut self, ctx, path) -> Option<Arc<Vec<ParsedMessage>>>`：hot path 不校验 signature

#### Scenario: `get_tool_output` 命中缓存时不重读 JSONL

- **WHEN** 调用方第一次调 `get_tool_output(root, sid, tool_use_id_a)`，cache 写入对应 session 的 JSONL parse 结果
- **AND** 同一 session 文件未变（`FileSignature` 一致），调用方再次调 `get_tool_output(root, sid, tool_use_id_b)`
- **THEN** 第二次调用 MUST 直接从缓存读取 `Arc<Vec<ParsedMessage>>`，SHALL NOT 调用 `cdt_parse::parse_file_via_fs(...)` 重读 JSONL 全文件
- **AND** 缓存条目的 `Arc` 引用计数 SHALL 通过 `Arc::clone` 共享而非整个 `Vec<ParsedMessage>` 数据复制

#### Scenario: `get_image_asset` 命中缓存时不重读 JSONL

- **WHEN** 调用方第一次调 `get_image_asset(root, sid, block_id_a)`，cache 写入对应 session 的 JSONL parse 结果
- **AND** 同一 session 文件未变，调用方再次调 `get_image_asset(root, sid, block_id_b)`
- **THEN** 第二次调用 MUST 直接从缓存读取 `Arc<Vec<ParsedMessage>>`，SHALL NOT 调用 `cdt_parse::parse_file_via_fs(...)` 重读 JSONL 全文件

#### Scenario: 同 session 在 `get_tool_output` 与 `get_image_asset` 之间共享缓存

- **WHEN** 调用方先调 `get_tool_output(root, sid, tu)` 完成 cache 写入
- **AND** 同 session 文件未变，调用方再调 `get_image_asset(root, sid, block_id)`
- **THEN** `get_image_asset` MUST 命中同一缓存条目（同 `(ContextId, path)` key），SHALL NOT 重新 parse JSONL

#### Scenario: `FileSignature` 不一致走 cache miss

- **WHEN** 调用 `get_tool_output` / `get_image_asset` 时 stat 拿到的 `FileSignature` 与缓存记录任一字段（mtime / size / identity）不一致
- **THEN** MUST 走 cache miss 分支，调 `parse_file_via_fs(...)` 重新解析全文件，并以新 `FileSignature` + 新结果覆盖缓存

#### Scenario: parse 失败时 SHALL NOT 写入缓存

- **WHEN** 调用 `get_tool_output` / `get_image_asset` 时 cache miss，但 `parse_file_via_fs(...)` 返回 `Err`
- **THEN** MUST 走 caller 的原有错误兜底路径（`get_image_asset` 返回 `empty_data_uri()`、`get_tool_output` 返回 `ToolOutput::Missing`），且 SHALL NOT 把空 `Vec` 或任何条目写入缓存

#### Scenario: stat 失败时走 cache miss 且不写入

- **WHEN** 调用 `get_tool_output` / `get_image_asset` 时 `FileSystemProvider::stat(path)` 失败
- **THEN** MUST 走原 caller 错误兜底路径，SHALL NOT 把任何条目写入缓存

#### Scenario: 缓存超过容量按 LRU 淘汰

- **WHEN** 缓存已达 50 entries 时再调 `get_tool_output` / `get_image_asset` 触发一个新 `(context_id, path)` key 写入
- **THEN** MUST 淘汰当前最久未访问的条目后再写入新条目，缓存大小始终 ≤ 50

#### Scenario: 缓存命中时把 key bump 到队首

- **WHEN** lookup 在缓存中命中 `(context_id, path)` key
- **THEN** MUST 把该 key 的 LRU 位置移到队首（最新访问），后续淘汰循环中该 key 不会被冷热顺序错误淘汰

#### Scenario: cache key 在 `(ContextId, PathBuf)` tuple 下 Local 与 SSH 同字面 path 不串扰

- **WHEN** 对同一个 `ParsedMessageCache` 实例先用 Local ctx + path P 写入 entry A，再用 SSH ctx + 同字面 path P 写入 entry B
- **THEN** cache MUST 同时持有两个独立 entry（key 分别为 `(ContextId::local(local_root), P)` 与 `(ContextId::ssh(host_signature, remote_root), P)`），SHALL NOT 串扰命中
- **AND** 用 Local ctx 查询 MUST 只命中 Local entry，用 SSH ctx 查询 MUST 只命中 SSH entry

#### Scenario: SSH 路径同走 cache wrapper（取代 PR-C inline 早退）

- **WHEN** `get_tool_output` / `get_image_asset` 在 SSH active context 下被调用且 session JSONL 远端有效
- **THEN** 起始处 SHALL 调 `self.active_fs_and_context_strict().await?` 拿 `(fs, projects_dir, ctx)` 三元组同快照（SSH disconnect 中间态返 `not_found` 而非 silently degrade）
- **AND** SHALL 经过 `extract_parsed_messages_cached(&cache, &*fs, &ctx, path)`——与 Local 分支同走一条入口
- **AND** SHALL NOT 出现 `if is_remote { fs.read_to_string + parse_jsonl_content }` 类 inline 早退（取代 PR-C `parsed-message-cache-context-prefix` design D6 的临时 scope 边界）
- **AND** hot path cache hit（同 session 二次访问）路径 UI 走 `cache.lookup_trust_cached` 0 fs op；后台 spawn task 走 `fs.stat` 校验 signature
- **AND** cache miss 路径产 1 个 `fs.stat` + 1 个 `fs.open_read`（由 `parse_file_via_fs` 内部完成，BufReader 32 KiB 与 SFTP packet 对齐）

### Requirement: parsed-message 缓存 ownership 由 `LocalDataApi` 持有

`LocalDataApi` SHALL 通过一个 `Arc<std::sync::Mutex<ParsedMessageCache>>` 字段持有缓存实例。所有构造器（`new` / `new_with_watcher` / 任何后续 `new_with_xxx`）MUST 初始化为空 cache。**禁止**用全局 `OnceLock` / `static` 单例 ——多个 `LocalDataApi` 实例（HTTP server + Tauri IPC 各自构造）必须各自独立持有 cache，互相不共享。

构造器扩展（如本 change 引入的 cache 注入路径）MUST 遵循"`new()` / `new_with_watcher()` 签名不变 + 链式 `with_xxx` 或新 `new_with_xxx`"模式（CLAUDE.md `LocalDataApi 构造器扩展` 硬约束）；本 change SHALL 仅在 `LocalDataApi` 现有 `new()` / `new_with_watcher()` 内部初始化新字段，**不**改这两个构造器的参数签名。

`switch_context` / `ssh_connect` / `ssh_disconnect` 三个方法 SHALL NOT 主动清空 parsed-message cache —— 不同 `ContextId` 的 entry 自然不命中（依赖 `(ContextId, PathBuf)` key 的 Hash/Eq 隔离），signature 校验照常工作；reconnect 同 host 时（`host_signature` 等价 → 同 `ContextId`）可复用旧 entry。本 change 让 SSH callsite 真正接入 cache wrapper 后，cache 内 SSH ctx entry 会随用户使用自然累积，PR-C 当时"运行时仅 Local entry"的边界不再适用。

#### Scenario: 多个 `LocalDataApi` 实例独立持有 parsed-message cache

- **WHEN** 测试或运行时构造两个 `LocalDataApi` 实例 A 与 B
- **THEN** A 的 parsed-message cache 与 B 的 parsed-message cache MUST 是独立 `Arc<Mutex<ParsedMessageCache>>` 实例，A 中的缓存写入 SHALL NOT 影响 B 中的 lookup 结果

#### Scenario: 不改 `new()` / `new_with_watcher()` 签名

- **WHEN** 既有调用方（集成测试 / `src-tauri/src/lib.rs` 等）按现有签名调用 `LocalDataApi::new(scanner, config_mgr, notif_mgr, ssh_mgr)` 或 `LocalDataApi::new_with_watcher(scanner, config_mgr, notif_mgr, ssh_mgr, watcher, projects_dir)`
- **THEN** 这两个构造器签名 MUST 保持不变；parsed-message cache 字段 MUST 在构造器内部初始化为空 `ParsedMessageCache`

#### Scenario: switch_context / ssh_connect / ssh_disconnect 不清 parsed-msg cache

- **WHEN** 用户在 Local context 下写入 parsed-msg cache 若干 entry（key 形如 `(ContextId::local(_), _)`）
- **AND** 用户调用 `ssh_connect` 切到 SSH context、再调用 `ssh_disconnect` 切回 Local
- **THEN** 在这两次调用前后，cache 中 Local entry SHALL 保留（未被主动清空）；后续 Local context 下 lookup 同 key 仍命中
- **AND** reconnect 同一 SSH host（`host_signature` 等价）时，cache 中持有的同 SSH ContextId entry SHALL 保留可复用——本 change 让 SSH callsite 接入 cache wrapper 后此 reconnect 复用路径真正生效

#### Scenario: cache miss 路径走 `parse_file_via_fs` 而非 `tokio::fs::File::open`

- **WHEN** parsed-message cache miss 后触发 fallback parse
- **THEN** SHALL 调 `cdt_parse::parse_file_via_fs(fs, path)` 而非 `cdt_parse::parse_file(path)` 旧版
- **AND** `parse_file_via_fs` 内部 SHALL 走 `fs.open_read(path).await?` 拿 `Box<dyn AsyncRead + Send + Unpin>` 后用 `tokio::io::BufReader` 包装逐行解析，SHALL NOT 直接 `tokio::fs::File::open`
- **AND** 旧版 `parse_file(path)` SHALL 保留作为兼容入口（内部以 `cdt_fs::local_handle()` 包装到 `parse_file_via_fs`）以便单测 / 不接 fs trait 的 caller 平滑过渡

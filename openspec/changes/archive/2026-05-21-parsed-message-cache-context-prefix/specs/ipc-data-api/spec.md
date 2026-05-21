## MODIFIED Requirements

### Requirement: `get_tool_output` 与 `get_image_asset` 走 parsed-message LRU 缓存

`LocalDataApi` SHALL 持有一个内部 parsed-message LRU 缓存（不使用全局单例），以 `(cdt_fs::ContextId, PathBuf)` 二元组为 key（**MUST** 把 `ContextId` 作为 key 的第一成员；裸 `PathBuf` 作 key **MUST NOT** 出现），缓存值为 `(FileSignature, Arc<Vec<ParsedMessage>>)` 二元组。`get_tool_output` 与 `get_image_asset` MUST 在调用 `cdt_parse::parse_file(...)` 之前先查该缓存，命中时 MUST 直接复用缓存中的 `Arc<Vec<ParsedMessage>>`、SHALL NOT 重读 JSONL 全文件，亦 SHALL NOT 重新执行 line-by-line parse。

`FileSignature` 等价性 MUST 与 `MetadataCache` 同源（即 `crates/cdt-api/src/cache_signature.rs::FileSignature` 的 `(mtime, size, identity)` 三元组，identity 在 Unix 上为 `(dev, ino)`，Windows 与其它平台退化为 `None`），best-effort 语义与 `extract_session_metadata` 按 `FileSignature` 缓存 Requirement 完全一致。

stat 路径 MUST 走 `FileSystemProvider::stat`（而非 `tokio::fs::metadata`）；构造 `FileSignature` MUST 走 `FileSignature::from_fs_metadata(&FsMetadata)`（而非 deprecated `from_metadata(&std::fs::Metadata)`）。

缓存 SHALL 在以下任一条件下走 cache miss：

- 缓存中无该 `(context_id, path)` key
- stat 拿到的 `FileSignature` 与缓存记录任一字段不一致
- stat 失败

miss 路径 MUST 调用 `parse_file(path)`：成功时把结果包装为 `Arc::new(messages)`，与新 `FileSignature` 一起写入缓存；解析失败时 SHALL NOT 写入缓存（避免 negative cache 引入新失效边界），由 caller 走原有错误兜底（`get_image_asset` 返回 `empty_data_uri()`、`get_tool_output` 返回 `ToolOutput::Missing`）。

`get_tool_output` 在命中缓存后 MUST 在 `Arc<Vec<ParsedMessage>>` 上重新调用 `cdt_analyze::build_chunks(&messages)` 完成 tool_use_id 匹配——本 change 不缓存 `build_chunks` 结果，仅缓存 parse 一层（详 change `parsed-message-lru-cache` design D2/D6 决策）。

缓存容量 SHALL 上限 50 entries，按 LRU 淘汰；容量按全局计算（**所有 `ContextId` 共享同一上限**，不按 context 拆配额）；命中时 MUST 把命中 key bump 到队首避免冷热混淆。

**本 change scope 边界**：cache wrapper 当前**有效**调用面 = Local context only。`get_tool_output` / `get_image_asset` 的 SSH 分支 SHALL 继续走 inline `fs.read_to_string + cdt_parse::parse_jsonl_content`，**不**经过 cache wrapper——SSH 接入 cache wrapper 是 PR-D 的工作（与 PR-D 同时 把 `parse_file` 内部 `tokio::fs::File::open` 切到 `FileSystemProvider::open_read` 一起进行）。

#### Scenario: `get_tool_output` 命中缓存时不重读 JSONL

- **WHEN** 调用方第一次调 `get_tool_output(root, sid, tool_use_id_a)`，cache 写入对应 session 的 JSONL parse 结果
- **AND** 同一 session 文件未变（`FileSignature` 一致），调用方再次调 `get_tool_output(root, sid, tool_use_id_b)`
- **THEN** 第二次调用 MUST 直接从缓存读取 `Arc<Vec<ParsedMessage>>`，SHALL NOT 调用 `cdt_parse::parse_file(...)` 重读 JSONL 全文件
- **AND** 缓存条目的 `Arc` 引用计数 SHALL 通过 `Arc::clone` 共享而非整个 `Vec<ParsedMessage>` 数据复制

#### Scenario: `get_image_asset` 命中缓存时不重读 JSONL

- **WHEN** 调用方第一次调 `get_image_asset(root, sid, block_id_a)`，cache 写入对应 session 的 JSONL parse 结果
- **AND** 同一 session 文件未变，调用方再次调 `get_image_asset(root, sid, block_id_b)`
- **THEN** 第二次调用 MUST 直接从缓存读取 `Arc<Vec<ParsedMessage>>`，SHALL NOT 调用 `cdt_parse::parse_file(...)` 重读 JSONL 全文件

#### Scenario: 同 session 在 `get_tool_output` 与 `get_image_asset` 之间共享缓存

- **WHEN** 调用方先调 `get_tool_output(root, sid, tu)` 完成 cache 写入
- **AND** 同 session 文件未变，调用方再调 `get_image_asset(root, sid, block_id)`
- **THEN** `get_image_asset` MUST 命中同一缓存条目（同 `(ContextId, path)` key），SHALL NOT 重新 parse JSONL

#### Scenario: `FileSignature` 不一致走 cache miss

- **WHEN** 调用 `get_tool_output` / `get_image_asset` 时 stat 拿到的 `FileSignature` 与缓存记录任一字段（mtime / size / identity）不一致
- **THEN** MUST 走 cache miss 分支，调 `parse_file(...)` 重新解析全文件，并以新 `FileSignature` + 新结果覆盖缓存

#### Scenario: parse 失败时 SHALL NOT 写入缓存

- **WHEN** 调用 `get_tool_output` / `get_image_asset` 时 cache miss，但 `parse_file(...)` 返回 `Err`
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

- **WHEN** 单元测试或 PR-D 接入 SSH cache 之后，对同一个 `ParsedMessageCache` 实例先用 Local ctx + path P 写入 entry A，再用 SSH ctx + 同字面 path P 写入 entry B
- **THEN** cache MUST 同时持有两个独立 entry（key 分别为 `(ContextId::local(local_root), P)` 与 `(ContextId::ssh(host_signature, remote_root), P)`），SHALL NOT 串扰命中
- **AND** 用 Local ctx 查询 MUST 只命中 Local entry，用 SSH ctx 查询 MUST 只命中 SSH entry
- **AND** **本 change scope 注**：当前 IPC callsite（`get_tool_output` / `get_image_asset` 的 SSH 分支）走 inline 不查 cache（详后续 Scenario "SSH callsite 仍走 inline 不查 cache"），所以运行时 cache 内仅 Local ctx entry；本 Scenario 由单元测试直接对 `ParsedMessageCache` 公开 API 写入两个 ctx 来覆盖，为 PR-D 接入 SSH 时无破坏迁移做准备

#### Scenario: 本 change scope: SSH callsite 仍走 inline 不查 cache

- **WHEN** 当前实现在 SSH context 下调用 `get_tool_output` / `get_image_asset`
- **THEN** SHALL 走 inline `fs.read_to_string + cdt_parse::parse_jsonl_content` 路径，**不**经过 `extract_parsed_messages_cached` cache wrapper
- **AND** 本 change scope **不**要求 SSH callsite 接入 cache wrapper；该接入留待后续 PR-D（同时把 `parse_file` 内部 `tokio::fs::File::open` 切到 `FileSystemProvider::open_read`）
- **AND** 即便本 scope 下 SSH 不写入 cache，spec 仍 SHALL 要求 cache key 类型为 `(ContextId, PathBuf)`——为 PR-D 接入 SSH 时无破坏迁移做准备

### Requirement: parsed-message 缓存按 file-change 广播主动失效

`LocalDataApi::new_with_watcher(...)` 构造路径 SHALL 在 spawn 自动通知管线的同时，额外 spawn 一个后台 task，订阅 `FileWatcher::subscribe_files()` 广播，对每条 `FileChangeEvent` 按 `projects_dir / project_id / "{session_id}.jsonl"` 推算出 cache key 的 `PathBuf` 部分。

**ContextId 推算**：该后台 task SHALL 在构造时一次性合成 `let ctx = cdt_fs::ContextId::local(projects_dir.clone());`（**watcher 是 Tauri 本地 fs 的硬不变量**，永远不会触发远端 SSH 文件事件），循环内每次事件复用同一个 ctx clone 与推算出的 path 一起作为 cache key 传入 `remove_if_signature_mismatch` / `remove`。

**stat 校验语义**：收到事件后 task MUST 先 `cdt_fs::local_handle().stat(&path).await` 拿当前 `FileSignature`（通过 `FileSignature::from_fs_metadata(&FsMetadata)` 构造），与 cache 中记录的 signature 比对：
- 两者一致 → SHALL NOT 移除（视为 spurious watcher 事件——典型场景：CI 上 inotify 启动期对刚创建的 watch dir 偶发"无内容变化"事件、metadata-only touch、跨平台 backend 行为差异等。若无 stat 比对会错杀仍有效的 cache，导致下次 hot path 不必要重 parse）
- 两者不一致 → MUST `remove(&ctx, &path)` 让下次 lookup 重 parse
- `FileSystemProvider::stat` 失败（文件被删 / 权限）→ MUST `remove(&ctx, &path)` 保守剔除——反正下次 hot path lookup 也会 stat fail 走原兜底（`empty_data_uri()` / `ToolOutput::Missing`），提前清掉不影响正确性

该失效路径与 `FileChangeEvent.deleted` 字段无关——文件被删 / 改 / 新建都同样进入"stat → 比对 signature → 决定 remove"流程。

`LocalDataApi::new()` 构造路径（无 watcher）SHALL NOT 启动该订阅 task；此场景仅依赖被动 `FileSignature` 失效路径兜底——与 `MetadataCache` 在 `new()` 路径下的行为对齐。

broadcast lag（`broadcast::Receiver::recv` 返回 `Err(RecvError::Lagged)`）时 SHALL 静默继续 loop——lag 仅代表事件激增，下次 lookup 由被动 `FileSignature` mismatch 兜底，不影响正确性。channel close（`Err(RecvError::Closed)`）时 task SHALL 退出。

#### Scenario: 文件真改后 file-change 广播主动 invalidate

- **WHEN** `LocalDataApi` 由 `new_with_watcher` 构造且缓存中已有 `<projects_dir>/<encoded_project>/<sid>.jsonl` 的 parsed-message 条目（key 为 `(ContextId::local(projects_dir), path)`）
- **AND** session JSONL 文件被追加 / 重写（mtime+size 变化）
- **AND** `FileWatcher` 广播一条对应 `FileChangeEvent`
- **THEN** 后台 invalidate task MUST 先 stat 拿当前 `FileSignature`、与 cache 记录比对、发现不一致后 remove 该 `(ContextId::local(projects_dir), path)` 对应的条目，使下一次 `get_tool_output` / `get_image_asset` 走 cache miss + 重 parse

#### Scenario: spurious file-change 事件 SHALL NOT 错杀有效 cache

- **WHEN** 缓存中已有某 session 的 parsed-message 条目
- **AND** `FileWatcher` 因 backend 行为发出了一条 `FileChangeEvent`，但目标文件内容 / mtime / size 实际未变（典型 CI inotify 启动期 spurious 事件）
- **THEN** invalidate task MUST stat 拿当前 `FileSignature` 与 cache 记录比对，发现两者一致后 SHALL NOT remove 条目；后续 lookup MUST 仍命中 cache

#### Scenario: 文件被删时 stat 失败走保守 remove

- **WHEN** 缓存中已有某 session 的 parsed-message 条目
- **AND** `FileWatcher` 广播 `FileChangeEvent { ..., deleted: true }` 之后文件已不存在
- **THEN** invalidate task 的 `FileSystemProvider::stat(&path)` 失败，task MUST 调 `remove(&ctx, &path)` 保守剔除条目

#### Scenario: `new()` 构造不启动 invalidate 订阅

- **WHEN** `LocalDataApi` 由 `new(scanner, config_mgr, notif_mgr, ssh_mgr)` 构造（无 watcher 参数）
- **THEN** SHALL NOT spawn 任何订阅 `FileWatcher::subscribe_files()` 的后台 task；parsed-message cache 仅依赖被动 `FileSignature` 失效

#### Scenario: invalidator 用 Local ContextId 推算 cache key

- **WHEN** Local callsite（`get_tool_output` / `get_image_asset` 在 Local context 下）以 key `(ContextId::local(projects_dir), path)` 写入 cache 一个 entry
- **AND** `FileWatcher` 随后广播一条对应 `FileChangeEvent`、文件内容已变（mtime+size 变化）
- **THEN** invalidator 推算的 ContextId MUST 等于 `ContextId::local(projects_dir)`（与 Local callsite 写入的 key 一致），并成功 remove 该 entry
- **AND** 如果用户在 invalidator spawn 之后（运行时）切换到 SSH context，invalidator 的 ContextId 推算 SHALL 保持为构造时的 `ContextId::local(projects_dir)`（**watcher 是 Local 视角的硬绑定**，runtime 切 SSH 不影响 invalidator 行为）

### Requirement: parsed-message 缓存 ownership 由 `LocalDataApi` 持有

`LocalDataApi` SHALL 通过一个 `Arc<std::sync::Mutex<ParsedMessageCache>>` 字段持有缓存实例。所有构造器（`new` / `new_with_watcher` / 任何后续 `new_with_xxx`）MUST 初始化为空 cache。**禁止**用全局 `OnceLock` / `static` 单例 ——多个 `LocalDataApi` 实例（HTTP server + Tauri IPC 各自构造）必须各自独立持有 cache，互相不共享。

构造器扩展（如本 change 引入的 cache 注入路径）MUST 遵循"`new()` / `new_with_watcher()` 签名不变 + 链式 `with_xxx` 或新 `new_with_xxx`"模式（CLAUDE.md `LocalDataApi 构造器扩展` 硬约束）；本 change SHALL 仅在 `LocalDataApi` 现有 `new()` / `new_with_watcher()` 内部初始化新字段，**不**改这两个构造器的参数签名。

`switch_context` / `ssh_connect` / `ssh_disconnect` 三个方法 SHALL NOT 主动清空 parsed-message cache —— 不同 `ContextId` 的 entry 自然不命中（依赖 `(ContextId, PathBuf)` key 的 Hash/Eq 隔离），signature 校验照常工作；reconnect 同 host 时（`host_signature` 等价 → 同 `ContextId`）可复用旧 entry。

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
- **AND** 即使 reconnect 同一 SSH host（`host_signature` 等价），cache 中可能存在的同 SSH ContextId entry SHALL 保留可复用（**虽然本 change scope 内 SSH callsite 不写入 cache，但 spec 钉死此契约为 PR-D 启用 SSH cache 后铺路**）

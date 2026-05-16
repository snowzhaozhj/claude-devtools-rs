## ADDED Requirements

### Requirement: `get_tool_output` 与 `get_image_asset` 走 parsed-message LRU 缓存

`LocalDataApi` SHALL 持有一个内部 parsed-message LRU 缓存（不使用全局单例），以 JSONL 文件 `PathBuf` 为 key，缓存值为 `(FileSignature, Arc<Vec<ParsedMessage>>)` 二元组。`get_tool_output` 与 `get_image_asset` MUST 在调用 `cdt_parse::parse_file(...)` 之前先查该缓存，命中时 MUST 直接复用缓存中的 `Arc<Vec<ParsedMessage>>`、SHALL NOT 重读 JSONL 全文件，亦 SHALL NOT 重新执行 line-by-line parse。

`FileSignature` 等价性 MUST 与 `MetadataCache` 同源（即 `crates/cdt-api/src/cache_signature.rs::FileSignature` 的 `(mtime, size, identity)` 三元组，identity 在 Unix 上为 `(dev, ino)`，Windows 与其它平台退化为 `None`），best-effort 语义与 `extract_session_metadata` 按 `FileSignature` 缓存 Requirement 完全一致。

缓存 SHALL 在以下任一条件下走 cache miss：

- 缓存中无该 path
- stat 拿到的 `FileSignature` 与缓存记录任一字段不一致
- stat 失败

miss 路径 MUST 调用 `parse_file(path)`：成功时把结果包装为 `Arc::new(messages)`，与新 `FileSignature` 一起写入缓存；解析失败时 SHALL NOT 写入缓存（避免 negative cache 引入新失效边界），由 caller 走原有错误兜底（`get_image_asset` 返回 `empty_data_uri()`、`get_tool_output` 返回 `ToolOutput::Missing`）。

`get_tool_output` 在命中缓存后 MUST 在 `Arc<Vec<ParsedMessage>>` 上重新调用 `cdt_analyze::build_chunks(&messages)` 完成 tool_use_id 匹配——本 change 不缓存 `build_chunks` 结果，仅缓存 parse 一层（详 change `parsed-message-lru-cache` design D2/D6 决策）。

缓存容量 SHALL 上限 50 entries，按 LRU 淘汰；命中时 MUST 把命中 key bump 到队首避免冷热混淆。

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
- **THEN** `get_image_asset` MUST 命中同一缓存条目，SHALL NOT 重新 parse JSONL

#### Scenario: `FileSignature` 不一致走 cache miss

- **WHEN** 调用 `get_tool_output` / `get_image_asset` 时 stat 拿到的 `FileSignature` 与缓存记录任一字段（mtime / size / identity）不一致
- **THEN** MUST 走 cache miss 分支，调 `parse_file(...)` 重新解析全文件，并以新 `FileSignature` + 新结果覆盖缓存

#### Scenario: parse 失败时 SHALL NOT 写入缓存

- **WHEN** 调用 `get_tool_output` / `get_image_asset` 时 cache miss，但 `parse_file(...)` 返回 `Err`
- **THEN** MUST 走 caller 的原有错误兜底路径（`get_image_asset` 返回 `empty_data_uri()`、`get_tool_output` 返回 `ToolOutput::Missing`），且 SHALL NOT 把空 `Vec` 或任何条目写入缓存

#### Scenario: stat 失败时走 cache miss 且不写入

- **WHEN** 调用 `get_tool_output` / `get_image_asset` 时 `tokio::fs::metadata(path)` 失败
- **THEN** MUST 走原 caller 错误兜底路径，SHALL NOT 把任何条目写入缓存

#### Scenario: 缓存超过容量按 LRU 淘汰

- **WHEN** 缓存已达 50 entries 时再调 `get_tool_output` / `get_image_asset` 触发一个新 path 写入
- **THEN** MUST 淘汰当前最久未访问的条目后再写入新条目，缓存大小始终 ≤ 50

#### Scenario: 缓存命中时把 key bump 到队首

- **WHEN** lookup 在缓存中命中 `path`
- **THEN** MUST 把该 path 的 LRU 位置移到队首（最新访问），后续淘汰循环中该 path 不会被冷热顺序错误淘汰

### Requirement: parsed-message 缓存按 file-change 广播主动失效

`LocalDataApi::new_with_watcher(...)` 构造路径 SHALL 在 spawn 自动通知管线的同时，额外 spawn 一个后台 task，订阅 `FileWatcher::subscribe_files()` 广播，对每条 `FileChangeEvent` 按 `projects_dir / project_id / "{session_id}.jsonl"` 推算出 cache key。

**stat 校验语义**：收到事件后 task MUST 先 `tokio::fs::metadata(&path)` 拿当前 `FileSignature`，与 cache 中记录的 signature 比对：
- 两者一致 → SHALL NOT 移除（视为 spurious watcher 事件——典型场景：CI 上 inotify 启动期对刚创建的 watch dir 偶发"无内容变化"事件、metadata-only touch、跨平台 backend 行为差异等。若无 stat 比对会错杀仍有效的 cache，导致下次 hot path 不必要重 parse）
- 两者不一致 → MUST `remove(path)` 让下次 lookup 重 parse
- `tokio::fs::metadata` 失败（文件被删 / 权限）→ MUST `remove(path)` 保守剔除——反正下次 hot path lookup 也会 stat fail 走原兜底（`empty_data_uri()` / `ToolOutput::Missing`），提前清掉不影响正确性

该失效路径与 `FileChangeEvent.deleted` 字段无关——文件被删 / 改 / 新建都同样进入"stat → 比对 signature → 决定 remove"流程。

`LocalDataApi::new()` 构造路径（无 watcher）SHALL NOT 启动该订阅 task；此场景仅依赖被动 `FileSignature` 失效路径兜底——与 `MetadataCache` 在 `new()` 路径下的行为对齐。

broadcast lag（`broadcast::Receiver::recv` 返回 `Err(RecvError::Lagged)`）时 SHALL 静默继续 loop——lag 仅代表事件激增，下次 lookup 由被动 `FileSignature` mismatch 兜底，不影响正确性。channel close（`Err(RecvError::Closed)`）时 task SHALL 退出。

#### Scenario: 文件真改后 file-change 广播主动 invalidate

- **WHEN** `LocalDataApi` 由 `new_with_watcher` 构造且缓存中已有 `<projects_dir>/<encoded_project>/<sid>.jsonl` 的 parsed-message 条目
- **AND** session JSONL 文件被追加 / 重写（mtime+size 变化）
- **AND** `FileWatcher` 广播一条对应 `FileChangeEvent`
- **THEN** 后台 invalidate task MUST 先 stat 拿当前 `FileSignature`、与 cache 记录比对、发现不一致后 remove 该 path 对应的条目，使下一次 `get_tool_output` / `get_image_asset` 走 cache miss + 重 parse

#### Scenario: spurious file-change 事件 SHALL NOT 错杀有效 cache

- **WHEN** 缓存中已有某 session 的 parsed-message 条目
- **AND** `FileWatcher` 因 backend 行为发出了一条 `FileChangeEvent`，但目标文件内容 / mtime / size 实际未变（典型 CI inotify 启动期 spurious 事件）
- **THEN** invalidate task MUST stat 拿当前 `FileSignature` 与 cache 记录比对，发现两者一致后 SHALL NOT remove 条目；后续 lookup MUST 仍命中 cache

#### Scenario: 文件被删时 stat 失败走保守 remove

- **WHEN** 缓存中已有某 session 的 parsed-message 条目
- **AND** `FileWatcher` 广播 `FileChangeEvent { ..., deleted: true }` 之后文件已不存在
- **THEN** invalidate task 的 `tokio::fs::metadata(&path)` 失败，task MUST 调 `remove(path)` 保守剔除条目

#### Scenario: `new()` 构造不启动 invalidate 订阅

- **WHEN** `LocalDataApi` 由 `new(scanner, config_mgr, notif_mgr, ssh_mgr)` 构造（无 watcher 参数）
- **THEN** SHALL NOT spawn 任何订阅 `FileWatcher::subscribe_files()` 的后台 task；parsed-message cache 仅依赖被动 `FileSignature` 失效

### Requirement: parsed-message 缓存 ownership 由 `LocalDataApi` 持有

`LocalDataApi` SHALL 通过一个 `Arc<std::sync::Mutex<ParsedMessageCache>>` 字段持有缓存实例。所有构造器（`new` / `new_with_watcher` / 任何后续 `new_with_xxx`）MUST 初始化为空 cache。**禁止**用全局 `OnceLock` / `static` 单例 ——多个 `LocalDataApi` 实例（HTTP server + Tauri IPC 各自构造）必须各自独立持有 cache，互相不共享。

构造器扩展（如本 change 引入的 cache 注入路径）MUST 遵循"`new()` / `new_with_watcher()` 签名不变 + 链式 `with_xxx` 或新 `new_with_xxx`"模式（CLAUDE.md `LocalDataApi 构造器扩展` 硬约束）；本 change SHALL 仅在 `LocalDataApi` 现有 `new()` / `new_with_watcher()` 内部初始化新字段，**不**改这两个构造器的参数签名。

#### Scenario: 多个 `LocalDataApi` 实例独立持有 parsed-message cache

- **WHEN** 测试或运行时构造两个 `LocalDataApi` 实例 A 与 B
- **THEN** A 的 parsed-message cache 与 B 的 parsed-message cache MUST 是独立 `Arc<Mutex<ParsedMessageCache>>` 实例，A 中的缓存写入 SHALL NOT 影响 B 中的 lookup 结果

#### Scenario: 不改 `new()` / `new_with_watcher()` 签名

- **WHEN** 既有调用方（集成测试 / `src-tauri/src/lib.rs` 等）按现有签名调用 `LocalDataApi::new(scanner, config_mgr, notif_mgr, ssh_mgr)` 或 `LocalDataApi::new_with_watcher(scanner, config_mgr, notif_mgr, ssh_mgr, watcher, projects_dir)`
- **THEN** 这两个构造器签名 MUST 保持不变；parsed-message cache 字段 MUST 在构造器内部初始化为空 `ParsedMessageCache`

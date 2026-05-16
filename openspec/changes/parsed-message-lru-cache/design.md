## Context

`crates/cdt-api/src/ipc/local.rs` 两条 hot path：

- `get_image_asset(root, sid, block_id)`：`parse_file(jsonl).await` → 找 image block → 落盘 / data URI
- `get_tool_output(root, sid, tool_use_id)`：`parse_file(jsonl).await` → `build_chunks(...)` → 线性扫 `tool_executions` 找匹配 id

典型用户行为：打开一个 session detail → 滚动 → 展开 5 个 Bash tool / 看 5 张截图。当前每次展开 / 取图都触发**全文件**重 parse，10k 消息会话耗 200–400ms / 次 + `build_chunks` 100–200ms / 次（`get_tool_output`），主线程 IPC handler 反复占用。

仓内已有的同款模式：`MetadataCache`（`crates/cdt-api/src/ipc/session_metadata.rs`）按 `(jsonl_path, FileSignature)` 缓存 `SessionMetadata`，由 `LocalDataApi` 持有 `Arc<std::sync::Mutex<MetadataCache>>`，已在 `multi-session-cpu-cache` change 验证可行。本 change 直接复用该模式覆盖 `parse_file` 结果。

## Goals / Non-Goals

**Goals:**

- `get_tool_output` / `get_image_asset` 在 cache 命中时 SHALL 跳过 JSONL parse，整体响应 < 5ms（按目前 cache lookup + stat 量级）
- cache 失效语义与 `MetadataCache` 一致：按 `FileSignature`（mtime + size + Unix inode）失效，且接 `FileWatcher` 广播主动剔除
- 不改 `LocalDataApi::new()` / `new_with_watcher()` 签名（CLAUDE.md 硬约束：`LocalDataApi` 构造器扩展用 `new_with_xxx()` / `with_xxx()`）
- 不改对外 IPC 协议任何字段（payload 字节无变化）

**Non-Goals:**

- 不替换 `MetadataCache`：metadata 路径已有专用 cache（粒度不同：metadata 只存 title / count / branch 等少量摘要字段），不强行合并
- 不引入跨进程 / 跨会话持久化 cache（in-process LRU 已足够覆盖 dev 使用场景；持久化是另一个 follow-up）
- 不缓存 `build_chunks` 结果：`build_chunks` 输出依赖 `ParsedMessage` + 调用方上下文，缓存粒度更复杂；先缓存 parse 一层让 `build_chunks` 拿 `&Vec<ParsedMessage>` 重算（100–200ms → 仍是收益的大头）
- 不强制双闸门（capacity + byte cap）：单 capacity 50 上限已能控制最坏 ~500 MB；byte cap 留为 follow-up

## Decisions

### D1: cache key = `(jsonl_path, FileSignature)`，模式与 MetadataCache 完全对齐

**选项 A（采用）**：复用 `crates/cdt-api/src/cache_signature.rs::FileSignature`（`mtime + size + identity(dev, ino on Unix)`）+ `PathBuf` key，与 `MetadataCache` 完全对齐——同一个 `multi-session-cpu-cache` design D1 系列决策的承袭。

**选项 B**：自己设计 cache key（如纯 mtime+size 不含 inode）。

**理由**：选 A。已有同款模式被验证 + 已有 fixture + 已有跨平台测试（`cached_miss_when_inode_changes_via_rename`）；选 B 没必要重新决策。Windows 退化（inode → `None`）由 `FileSignature` 内部消化，不影响本 cache。

### D2: cache value = `Arc<Vec<ParsedMessage>>`

**选项 A（采用）**：value 为 `Arc<Vec<ParsedMessage>>`，命中时 `clone()` `Arc` 几乎零成本，caller 拿到只读共享视图。

**选项 B**：value 为 `Vec<ParsedMessage>`，命中时 `.clone()` 整个 Vec —— 大会话 10k×结构体复制即数十 MB，违背本 change 的性能目标。

**选项 C**：value 为 `OnceCell<Arc<Vec<ParsedMessage>>>` 或类似惰性句柄。

**理由**：选 A。`Arc` clone 是原子计数 + 指针拷贝（< 100ns），完全够用；选 B 直接破功；选 C 让 cache 内部并发 parse 同一文件去重，需要更大改动（每条 entry 一把 Mutex），但本 change hot path 每次调用都已在 tokio task 内 `await`，多并发同时 miss 同一文件的概率极低（用户串行展开 tool），先 A 跑通，C 留作 follow-up。

### D3: capacity = 50，单闸门

**选项 A（采用）**：`PARSED_MESSAGE_CACHE_CAPACITY = 50`，仅按条目数 LRU 淘汰。

**选项 B**：双闸门（capacity 50 + byte cap 256 MB）。

**理由**：选 A 先行。

- dev 典型同时打开会话 < 10 个；UI 侧 `tabStore` 不会主动持有 100+ 个 detail，cache 上限主要防"用户翻沙箱 / 切大量会话"造成无界增长
- 50 × 单条 Arc<Vec<ParsedMessage>> 上界粗估 500 MB（典型 10k 消息 ~10 MB Rust struct），是**理论**上界——实际 dev 会话普遍 < 1k 消息 < 1 MB，工作集稳态 50–100 MB
- byte cap 需要每次 insert 时枚举所有 entry 估算字节，复杂度上升；先用条目 LRU 跑，后续 perf bench 真发现内存吃紧再加 byte cap（follow-up）
- 注意：`MetadataCache` 用 200 容量是因为 entry 极小（< 1 KB）；本 cache entry 量级千倍以上，所以 capacity 调低

### D4: 失效 = file-watcher 广播主动剔除 + signature mismatch 被动 miss

**选项 A（采用）**：双路失效——
- **主动**：`new_with_watcher` 构造下，spawn task 订阅 `watcher.subscribe_files()`，每条 `FileChangeEvent` 按 `jsonl_path = projects_dir / encoded_project / "{session_id}.jsonl"` 推算 key，从 cache 中 remove（match 原版 / `MetadataCache` 模式）。**只**在 `LocalDataApi::new_with_watcher(...)` 路径下生效，因为 `new()` 路径无 watcher（测试 / HTTP server / 集成测试默认）
- **被动**：每次 lookup 都先 stat 当前文件 `FileSignature`，与 cache entry signature 不等就 miss + 重 parse + 写入（覆盖旧条目）

**选项 B**：只靠被动 signature 判定，不接 file-watcher。

**理由**：选 A。仅被动判定时，**每次** lookup 都付 stat syscall 成本（虽然便宜，~50µs），active session 下 user 展开 tool 时刚好碰到 watcher debounce window 内的写入也能立即剔除（避免 user 看到 stale tool output 一段时间）；选 B 在 active session 下窗口期 stale 风险更大。注意 active session 写 JSONL 很频繁（每条消息 append），watcher 100ms debounce 之内可能两次 active emit—签名 mismatch 一定能补刀，watcher 只是"更早一步剔除"。

### D5: hot path 改造模式——helper 函数封装"先查 cache → miss 时 parse + insert"

**选项 A（采用）**：把"获取该 session 的 `Arc<Vec<ParsedMessage>>`"封装为 `LocalDataApi` 私有 async helper（如 `parsed_messages_cached(&self, jsonl_path: &Path) -> Arc<Vec<ParsedMessage>>`），让 `get_tool_output` / `get_image_asset` 各自调一句即可。命中返回 `Arc::clone(&entry.messages)`；miss 调 `parse_file(...)`、构造 `Arc::new(parsed)`、写入 cache、返回。

**选项 B**：在 hot path 里手动 lock + lookup + miss + insert，每条 hot path 各写一份。

**理由**：选 A。helper 集中处理 stat / lock / `FileSignature` 比对 / parse 失败兜底（返回空 `Arc<Vec<_>>` 还是 propagate `ApiError`？—— 见 D6），便于一处加 tracing；选 B 重复代码两份易漂移。

### D6: parse 失败时 cache 行为

**选项 A（采用）**：parse 失败时**不**写入 cache，让 caller 拿到失败信号 fall-back 到原有错误处理（`get_image_asset` 返回 `empty_data_uri()`、`get_tool_output` 返回 `ToolOutput::Missing`）。

**选项 B**：parse 失败时写入"空 Vec"作为 negative cache 防止短时间内反复重 parse。

**理由**：选 A。`parse_file` 失败通常意味着文件 IO 错误（被删 / 权限 / 中途截断）—— 用 file-watcher invalidate 路径自然恢复（被删触发 deleted 事件，重写触发 size 变化）；选 B 引入 negative cache 边界更复杂（什么时候清？多久 negative？），收益边际。

### D7: 缓存按值不按 reference 返回 `Arc<Vec<ParsedMessage>>` 给 caller

caller 拿到 `Arc<Vec<ParsedMessage>>` 后，对 `&Vec<ParsedMessage>` 调 `build_chunks(&messages)`（注意：`cdt_analyze::build_chunks` 当前签名是 `(&[ParsedMessage]) -> Vec<Chunk>`，传 `&*arc_vec` 即可）；不需要修改 `cdt_analyze::build_chunks` 签名。`get_image_asset` 同理对 `find_image_block_in_messages(&*arc_vec, ...)` 调用。

### D8: mod 命名 + 可见性

新增 `crates/cdt-api/src/ipc/parsed_message_cache.rs`，在 `crates/cdt-api/src/ipc/mod.rs` 加 `pub(crate) mod parsed_message_cache;`（与 `pub(crate)` 风格的 `session_metadata` 内部入口对齐）。`ParsedMessageCache` struct 与 `extract_parsed_messages_cached(...)` 函数都 `pub(crate)`——本 cache 不暴露到 crate 外。

### D9: 失效订阅 task 的 lifetime / projects_dir 推算

`new_with_watcher` 已经在调内 spawn 通知管线，把"订阅 watcher + invalidate cache"挂在同一构造里再 spawn 一个 task：

```rust
let cache_for_invalidate = parsed_msg_cache.clone();
let projects_dir_for_invalidate = projects_dir.clone();
let mut rx = watcher.subscribe_files();
tokio::spawn(async move {
    while let Ok(evt) = rx.recv().await {
        // evt.project_id + evt.session_id → jsonl_path
        let base_dir = cdt_discover::path_decoder::extract_base_dir(&evt.project_id);
        let jsonl_path = projects_dir_for_invalidate
            .join(base_dir)
            .join(format!("{}.jsonl", evt.session_id));
        cache_for_invalidate
            .lock()
            .expect("parsed message cache mutex poisoned")
            .remove(&jsonl_path);
    }
});
```

`broadcast::recv` 返回 `Err(Lagged)` 时静默继续（与 `subscribe_files` 上游一致：lag 只代表"短时间事件激增"，下次 lookup 会被被动 signature 判定兜底，不影响正确性）。

### D9b: 反转决策 —— `FileChangeEvent` 是否带 deleted

读后端实现：`FileChangeEvent` 字段是 `project_id` / `session_id` / `deleted: bool`（见 `crates/cdt-watch/`）。无论 `deleted` 是 `true` 还是 `false`，cache invalidate 都是"remove 该 path entry"——不需要区分；下次 lookup 时若文件已删，stat 失败走 fall-through（不写 cache），与 `MetadataCache` 路径一致。

### D9c: 反转决策 —— invalidate task 必须 stat + signature 比对再 remove

D9 原方案是"收到事件 → 直接 `cache.remove(path)`"。第一轮 CI 暴露此方案缺陷：

- CI（Linux/inotify）watcher 启动期对刚创建的 watch dir 会偶发一条 "spurious" `FileChangeEvent`，但文件内容 / mtime / size 实际未变
- 直接 `remove` 会错杀仍有效的 cache 条目，导致下一次 hot path lookup 不必要重 parse —— 不影响正确性但浪费 CPU
- 测试 `cache_persists_without_file_change`（验证"无文件改动时 cache 持久"语义）因此在 CI 上 race fail

修订：invalidate task SHALL 先 `tokio::fs::metadata(&path)` 拿当前 `FileSignature` 与 cache 中记录比对：
- **一致** → 视为 spurious 事件，不 remove
- **不一致** → 真正改动，remove
- **stat 失败**（文件被删）→ 保守 remove

代价：每条 file-change 事件多一次 stat syscall（~50µs）。换来 spurious 事件不再错杀有效 cache、且无需任何"启动期 sleep"魔法数字。新增 `ParsedMessageCache::remove_if_signature_mismatch(path, current_sig) -> bool` 把比对原子化在 lock 内。spec delta `parsed-message 缓存按 file-change 广播主动失效` 已同步反映新语义（含 Scenarios "文件真改后 file-change 广播主动 invalidate" / "spurious file-change 事件 SHALL NOT 错杀有效 cache" / "文件被删时 stat 失败走保守 remove"）。

## Risks / Trade-offs

- **[内存最坏上界] 极端场景 cache 占用 ~500 MB** → cap 设 50；后续 perf bench 真观察到吃紧再加 byte cap（follow-up）。日常 dev 工作集 50–100 MB
- **[Arc 共享 + 单 Mutex 写入并发]** → 命中路径 `Arc::clone` 不影响其它 read；miss 路径同 path 并发可能重复 parse（仅多花一次 CPU，不破坏正确性）。如未来 perf 真受影响走 D2 选项 C
- **[file-watcher 失效路径只覆盖 `new_with_watcher` 构造]** → `new()` 构造（无 watcher）仅靠被动 signature mismatch；HTTP / 集成测试场景 active session 写入到 cache 命中之间有 stat 窗口，与 `MetadataCache` 一致，可接受
- **[invalidate task 的 path 推算与 hot path key 必须一致]** → 两边都走 `projects_dir / extract_base_dir(project_id) / "{session_id}.jsonl"`；hot path 实际用 `locate_session_jsonl(...)` 可能涉及 subagent 路径（`<root>/subagents/agent-<sub>.jsonl`），但 `FileChangeEvent` 只覆盖主 session JSONL 路径——subagent JSONL 变化目前 file-watcher 不感知（详 `cdt-watch` 行为），所以 subagent 路径 cache 项**只**靠被动 signature 失效，与现状对齐
- **[parse_file 在 hot path 之外的调用方不享 cache]** → `get_session_detail` 早期阶段已经各自 parse；本 change 不重构所有调用方避免改动放大。如未来发现 `get_session_detail` 也是 cache 候选，单独 follow-up

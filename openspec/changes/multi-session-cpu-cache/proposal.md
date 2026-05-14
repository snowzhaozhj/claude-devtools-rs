## Why

多个 Claude Code 会话同时活跃时（每个 session 的 `.jsonl` 都在被 append），用户报告 CPU 占用经常飙高。诊断后定位到两条**完全绕过前端 250ms 节流**的后端独立链路：

1. **`NotificationPipeline`**：每条 `file-change` 事件直接 `parse_file()` 全 JSONL 文件 + 跑所有 trigger regex。N 个 session 各自每秒触发数次 → N×全文件 parse。
2. **`list_sessions` 后台元数据扫描**：每次 `list_sessions` 都对**整页** session 跑一遍 `extract_session_metadata`（line-by-line 全文件读 + 整 `Vec<ParsedMessage>` 加载）。Sidebar 每次 `file-change` 触发 `loadSessions(silent)` → 整页 N 个 session 全文件重扫，即便只有一个 session 真有变化。

两处共同特征：**都没有 (mtime, size) 缓存**——同一个文件没变也会反复重 parse。

- 引入跨平台 `FileSignature { mtime, size, identity }` 抽象。Unix 上 identity 是 `(dev, ino)`，作为 cache key 防止"同 mtime+size 不同 inode 文件被 rename 替换"假命中（codex 异构二审 D1b 修订）。Windows 与其它平台退化为 `None`（仅依赖 mtime+size），因 stable Rust 不暴露 Windows `file_index` API（D1f 修订）
- 给 `NotificationPipeline` 加 `(project_id, session_id) → FileSignature` 缓存：`process_file_change` 入口先 stat，`FileSignature` 一致则跳过整个 parse + detect 流程
- 给 `LocalDataApi` 加 `metadata_cache: Arc<Mutex<MetadataCache>>` 字段（**不**用全局 `OnceLock` 单例）；新增 `extract_session_metadata_cached(cache, path)` wrapper，文件未变时直接返回缓存的元数据；`extract_session_metadata` 自身保持纯函数签名不变（codex 异构二审 D3b 修订）
- 缓存键失效：`FileSignature` 任一字段（mtime / size / identity）变化即重算；文件被 truncate / rename 替换也走重算分支
- 缓存容量上限：notifier 200 entries / metadata 200 entries；命中时 bump 到队首避免冷热顺序错误淘汰
- **行为接近等价（best-effort）**：在常规 append-only 写入路径下，`FileSignature` 命中即视为文件未变；inode reuse + mtime/size 三维撞车的极端场景可能假命中，由后续 file-change 自然恢复（D1d）

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `notification-triggers`：补充缓存等价契约（WHEN file-change 事件指向的 JSONL 的 `FileSignature` 字段 byte-equal 等于缓存条目 THEN 跳过 parse_file 与 detect_errors）
- `ipc-data-api`：补充 metadata 缓存等价契约（WHEN `extract_session_metadata` 的 source file 的 `FileSignature` 字段 byte-equal 等于缓存条目 THEN 直接返回缓存结果，不再 line-by-line 重扫；缓存 ownership 由 `LocalDataApi` 持有，不使用全局单例）

## Impact

- 受影响代码：`crates/cdt-api/src/notifier.rs`、`crates/cdt-api/src/ipc/session_metadata.rs`
- IPC 字段、Tauri command 协议、前端 store：**零变化**
- 测试：现有 notifier / session_metadata 单测全部应通过；新增缓存命中 / miss / 失效边界测试
- 依赖：可能引入 `lru` crate 或自实现简易 LRU（容量小、淘汰频率低、`HashMap` + `VecDeque` 即可，倾向自实现避免新依赖）
- 性能预期：多 session 活跃场景下，notifier CPU 直降至接近 0（每次 file-change 仅一次 stat）；list_sessions 后台扫描 CPU 降至 O(变化的 session 数) 而非 O(整页 session 数)

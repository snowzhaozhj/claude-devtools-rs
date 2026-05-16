## Why

当前 `get_tool_output` / `get_image_asset` 两条 hot path 每次被调时都 `parse_file(jsonl)` 重新解析整个 JSONL 文件（200–400ms / 大会话），`get_tool_output` 之后还要再 `build_chunks(...)` 一遍（100–200ms）。同一 session 内用户连续展开 5 个 tool / 滚到 5 张图，意味着同一 JSONL 被全文 parse 5 次，纯重复 CPU 与内存分配。

`SessionMetadata` 路径已经有 `MetadataCache`（按 `(jsonl_path, FileSignature)`）解决了 metadata 重扫问题；本 change 用相同模式把 `parse_file(...)` 的结果也缓存起来，让这两条 hot path 命中缓存时直接复用 parse 结果，节省 200–400ms / call。

## What Changes

- 新增 `ParsedMessageCache`：按 `(jsonl_path, FileSignature)` 缓存 `Arc<Vec<ParsedMessage>>`，LRU 上限 50 条
- `LocalDataApi` 持有 `Arc<std::sync::Mutex<ParsedMessageCache>>`；用 `new_with_xxx()` 模式扩展构造器，**不**改 `new()` / `new_with_watcher()` 已有签名
- `get_tool_output`、`get_image_asset` 改走"先查 cache → miss 时 parse + insert → 命中时返回 `Arc<Vec<ParsedMessage>>`"
- 接 `FileWatcher::subscribe_files()` 广播：file-change 事件按 `jsonl_path` key 主动 invalidate cache entry（与 `MetadataCache` 的失效语义对齐）
- 单测覆盖：hit / miss / file changed（size + rename inode 两路）/ stat 失败 fall-through / LRU evict
- `perf_get_session_detail` bench 扩展"连续展开 5 个 tool"场景，对比 before / after

## Capabilities

### New Capabilities
<!-- 无 -->

### Modified Capabilities
- `ipc-data-api`: 给 `get_tool_output` / `get_image_asset` 引入 parsed-message LRU cache 行为契约；并对 cache 失效来源（file-change 广播）给出 SHALL 约束

## Impact

- 影响代码
  - `crates/cdt-api/src/ipc/`：新建 `parsed_message_cache.rs`；`local.rs` 改两条 hot path + 构造器
  - `crates/cdt-api/src/ipc/mod.rs`：导出新模块（内部 `pub(crate)` 即可）
  - `src-tauri/src/lib.rs`：若改 LocalDataApi 构造链则同步（**目标**是用 `with_xxx` 链式构造保持现有 `LocalDataApi::new(...)` 调用点不变）
- 不影响公共 IPC 协议字段（纯后端 cache，IPC payload 字节不变）
- 内存预算：cache 上限 50 条 `Arc<Vec<ParsedMessage>>`；典型大会话 ~10k 消息 / 5–10 MB Rust struct → 上限 50×10 MB = 500 MB 是理论上界，实际 dev 同开 < 10 个会话约 50–100 MB；double-cap（capacity + 可选 byte cap）由 design 决策
- 不引入新外部依赖

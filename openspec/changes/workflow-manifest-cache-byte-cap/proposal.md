## Why

`WorkflowManifestCache`（`cdt-api/src/ipc/workflow_manifest.rs`）的三个 `HashMap`（`entries` / `journal_entries` / `script_entries`）经 `Arc` 长驻 `LocalDataApi`，**均无 byte / cardinality cap**，违反 `.claude/rules/perf.md`「内存类」反模式「cache 仅设 count cap 不设 byte cap → 必须 `current_bytes: AtomicUsize` + `max_bytes` 双闸门」。

这是 pre-existing 架构隐患（非某 PR 引入）：`entries`（`WorkflowItem`，含 agents/phases）/ `journal_entries`（合成 agents）本就无界；PR #564 给 `script_entries` 每条新增 ≤ 32 KB preview String，放大了关注点。同 capability 的 `MetadataCache` / parsed-message cache 早已是双闸门 LRU，本 cache 是同 capability 内唯一漏网者。修复对齐既有 cache 拓扑，消除内存失控可能（issue #565）。

## What Changes

- 三个 `HashMap<PathBuf, *CacheEntry>` 各自改为 `lru::LruCache<PathBuf, *CacheEntry>`，每个 cache 独立配 count cap + byte cap 双闸门。
- 命中时 LRU bump 到队首；任一上限触发从 LRU 端淘汰（保留至少 1 条，与 `SearchTextCache` 一致）。
- 签名 mismatch（文件变化）时移除旧条目并扣减 byte 计数（已有 `get` 语义不变，仅补 byte 记账）。
- 行为对外**透明**：淘汰后下次同 path 走 miss 重读盘，结果不变——仅内存上界从无界变有界。
- 不改任何 IPC 字段 / 序列化格式 / public command 签名。

## Capabilities

### New Capabilities
（无）

### Modified Capabilities
- `ipc-data-api`: 给 `WorkflowManifestCache` 的三个内部 cache 增加「count cap + byte cap 双闸门 LRU 淘汰」行为契约（新增 Requirement），与同 capability 既有 `MetadataCache` / parsed-message LRU scenario 拓扑一致。既有「script 按文件签名缓存」「journal 按 FileSignature 缓存」scenario 的命中语义不变。

## Impact

- **代码**：`crates/cdt-api/src/ipc/workflow_manifest.rs`（`WorkflowManifestCache` struct + get/insert 三组方法 + 新增 byte 估算函数 + 单测）。
- **依赖**：`cdt-api` 新增 `lru` workspace 依赖（`cdt-discover` 已用，workspace 根已声明）。
- **Perf**：内存上界从无界 → 三 cache 各自有界；稳态命中路径零增量（多一次 LRU bump，O(1)）；无 IPC payload / 算法复杂度变化。
- **行为**：对前端 / IPC 透明，无可感知变化（淘汰仅触发重读，结果一致）。

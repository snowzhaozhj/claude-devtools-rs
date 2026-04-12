## Context

`session-search` 是第 6 个 port 的 capability，归属 `cdt-discover` crate。上游依赖（`cdt-parse` 的噪声分类、`cdt-discover` 的项目扫描和文件系统抽象）已全部就绪。TS 实现行为与 spec 完全匹配，无 impl-bug，但有一个 coverage-gap（SSH stage-limit）已补入 spec。

搜索的核心数据流：JSONL 文件 → 解析 → 噪声过滤 → 文本提取 → 缓存 → 查询匹配 → 结果排序。

## Goals / Non-Goals

**Goals:**
- 实现 spec 6 条 Requirement 的全部 Scenario
- 三级搜索 scope：单 session、单 project、全局
- mtime-based LRU 缓存避免重复解析
- SSH 分阶段搜索（[40, 140, 320] 文件分批 + 4.5s 超时 + 最少 8 结果提前返回）
- 大小写不敏感匹配 + 上下文预览片段（前后各 50 字符）

**Non-Goals:**
- 全文搜索引擎（不用 tantivy 等，简单 `str::contains` 足够）
- Regex 搜索（TS 也没有，spec 没要求）
- 搜索结果高亮渲染（属于 UI 层）
- 搜索结果持久化（内存 LRU 即可）

## Decisions

### 1. 模块划分（3 个新文件 + 类型扩展）

| 文件 | 职责 |
|------|------|
| `search_extract.rs` | 从 `ParsedMessage` 提取可搜索文本，复用 `cdt-parse` 的噪声分类 |
| `search_cache.rs` | `SearchTextCache`：`HashMap` + LRU 驱逐，key 为文件路径，mtime 校验 |
| `session_search.rs` | `SessionSearcher`：三级搜索入口，SSH stage-limit 逻辑 |

搜索结果类型（`SearchResult`、`SearchSessionsResult`）放 `cdt-core`，因为 `cdt-api` 需要序列化它们。

**备选方案**：把搜索放到独立 crate `cdt-search`。否决理由：session-search 强依赖 `cdt-discover` 的 `ProjectScanner` 和 `FileSystemProvider`，放同 crate 避免循环依赖。

### 2. LRU 缓存用 `lru` crate 还是自实现

选择 **`lru` crate**（crates.io 下载量最高的 LRU 实现，零依赖）。自实现不值得——标准 LRU 不需要任何定制行为。容量默认 1000 条，与 TS 一致。

### 3. 可搜索文本提取策略

复用 TS 的 AI-buffer flushing 模型：
- 用户消息：提取全部文本内容（text blocks 拼接）
- AI 消息：累积连续 assistant 消息为 buffer，遇到 user/system/compact 时 flush，只取 buffer 中**最后一条** assistant 消息的**最后一个 text block**
- 排除：hard-noise（`cdt_parse::noise::is_hard_noise`）、sidechain（`message_type == "sidechain"`）、system/compact 消息本身

这与 ChunkBuilder 的分类逻辑一致但更轻量——不构建完整 chunk，只提取文本。

### 4. SSH stage-limit 实现

通过 `SearchConfig` 结构体传入参数（`stage_limits`、`time_budget`、`min_results`、`concurrency`），而非硬编码常量。默认值与 TS 一致：
- `stage_limits: [40, 140, 320]`
- `time_budget: Duration::from_millis(4500)`
- `min_results: 8`
- `concurrency: 3`（SSH）/ `16`（local）

非 SSH 模式下忽略 stage-limit，一次扫描所有文件。

### 5. 匹配算法

`str::to_lowercase().contains()` —— 与 TS 的 `indexOf` 等价。不引入 unicode normalization（TS 也没有）。上下文预览取匹配位置前后各 50 个 `char`（不是字节），按 char boundary 截断。

## Risks / Trade-offs

- **[Risk] LRU 缓存在高并发下的竞争** → 用 `tokio::sync::Mutex<LruCache>` 包装；搜索是读多写少，锁粒度足够（每次 get/put 持锁极短）
- **[Risk] 大型 session 文件的内存占用** → 缓存只存提取后的文本片段（每 session ~KB 级），不是原始 JSONL（MB 级）
- **[Trade-off] 不用 `cdt_analyze::chunk::builder` 做文本提取** → 避免引入 `cdt-analyze` 依赖；代价是 buffer flushing 逻辑有少量重复，但提取器远比 ChunkBuilder 简单

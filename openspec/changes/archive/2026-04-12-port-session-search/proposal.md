## Why

`session-search` 是用户在 UI 中定位历史对话内容的核心能力。当前 Rust 端已完成 session-parsing 和 project-discovery，搜索所需的上游基础设施就绪，可以开始 port。TS 实现行为与 spec 完全匹配（无 impl-bug），但 SSH 分阶段限制搜索的 coverage-gap 需要补进来。

## What Changes

- 在 `cdt-discover` crate 中新增 `session_search` 模块，实现三级搜索 scope（单 session、单 project、全局）
- 实现 `SearchTextCache`：基于 mtime 的 LRU 缓存，避免重复解析 JSONL
- 实现可搜索文本提取：排除 hard-noise / sidechain / 系统消息，只索引用户消息全文和 AI buffer 最后一条文本输出
- 实现 SSH 分阶段搜索：按 `[40, 140, 320]` 文件数分阶段，超时或达到最小结果数时提前返回 `is_partial=true`
- 大小写不敏感匹配 + 上下文预览片段

## Capabilities

### New Capabilities

（无新增 capability）

### Modified Capabilities

- `session-search`：spec 当前已覆盖全部核心行为；followups.md 指出 SSH stage-limit 是 coverage-gap，spec 已在 baseline 时补入 `Support staged-limit search over SSH contexts` requirement，本 port 按 spec 实现

## Impact

- **代码**：`cdt-discover` 新增 `session_search.rs`（搜索器）、`search_cache.rs`（LRU 缓存）、`search_extract.rs`（文本提取）
- **依赖**：`cdt-parse`（噪声分类）、`cdt-core`（消息类型）。可能引入 `lru` crate 做 LRU cache
- **API 面**：`SearchResult`、`SearchSessionsResult` 类型定义在 `cdt-core`，供 `cdt-api` 下游消费

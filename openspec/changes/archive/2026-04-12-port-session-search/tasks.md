## 1. 依赖准备

- [x] 1.1 在 workspace `Cargo.toml` `[workspace.dependencies]` 中添加 `lru = "0.14"`
- [x] 1.2 在 `crates/cdt-discover/Cargo.toml` 引入 `lru = { workspace = true }`

## 2. cdt-core：搜索结果类型

- [x] 2.1 新增 `crates/cdt-core/src/search.rs`，定义以下类型（均 `#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]`）：
  - `SearchHit`：`message_uuid: String`、`offset: usize`、`preview: String`、`message_type: String`
  - `SessionSearchResult`：`session_id: String`、`project_id: String`、`session_title: String`、`hits: Vec<SearchHit>`、`total_matches: usize`
  - `SearchSessionsResult`：`results: Vec<SessionSearchResult>`、`total_matches: usize`、`sessions_searched: usize`、`query: String`、`is_partial: bool`
- [x] 2.2 在 `crates/cdt-core/src/lib.rs` 导出 `pub mod search; pub use search::*;`

## 3. cdt-discover：可搜索文本提取

- [x] 3.1 新增 `crates/cdt-discover/src/search_extract.rs`，定义 `SearchableEntry { uuid: String, text: String, message_type: String }` 和 `fn extract_searchable_entries(messages: &[ParsedMessage]) -> (Vec<SearchableEntry>, String)`（返回可搜索条目 + session title）
- [x] 3.2 实现 AI-buffer flushing 模型：连续 assistant 消息累积为 buffer，遇 user/system/compact 时 flush；flush 时只取 buffer 最后一条 assistant 消息的最后一个 text block
- [x] 3.3 排除 hard-noise（`cdt_parse::noise::is_hard_noise`）和 sidechain 消息
- [x] 3.4 用户消息提取全部 text blocks 拼接；session title 取第一条用户消息前 100 字符

## 4. cdt-discover：LRU 缓存

- [x] 4.1 新增 `crates/cdt-discover/src/search_cache.rs`，定义 `SearchTextCache`：内部 `lru::LruCache<PathBuf, CacheEntry>`（`CacheEntry` 含 `entries: Vec<SearchableEntry>`、`session_title: String`、`mtime_ms: u64`）
- [x] 4.2 实现 `get(&mut self, path: &Path, current_mtime_ms: u64) -> Option<&CacheEntry>`：mtime 匹配则返回缓存，不匹配则移除并返回 `None`
- [x] 4.3 实现 `put(&mut self, path: PathBuf, entry: CacheEntry)`
- [x] 4.4 默认容量 1000，可通过 `SearchTextCache::with_capacity(cap)` 自定义

## 5. cdt-discover：搜索器

- [x] 5.1 新增 `crates/cdt-discover/src/session_search.rs`，定义 `SessionSearcher` 持有 `Arc<Mutex<SearchTextCache>>` + `FileSystemProvider` 引用
- [x] 5.2 实现 `search_session_file(project_id, session_id, file_path, query, max_results) -> Result<SessionSearchResult>`：读文件 → 检查缓存 → 提取 → 匹配 → 返回 hits
- [x] 5.3 匹配逻辑：`text.to_lowercase().contains(&query_lower)`，找到后提取 offset + 前后各 50 char 预览
- [x] 5.4 实现 `search_sessions(project_id, query, max_results, config: SearchConfig) -> Result<SearchSessionsResult>`：列出 project 下所有 `.jsonl` 文件 → 按 mtime 倒序 → 逐个搜索 → 汇总
- [x] 5.5 实现 SSH stage-limit：`SearchConfig` 含 `is_ssh: bool`、`stage_limits: Vec<usize>`、`time_budget: Duration`、`min_results: usize`；SSH 模式下按 stage 分批搜索，达标后提前返回 `is_partial=true`
- [x] 5.6 在 `crates/cdt-discover/src/lib.rs` 导出新模块和公共类型

## 6. 测试

- [x] 6.1 `search_extract.rs` 单测：用户消息提取全文、AI buffer flush 只取最后 text block、hard-noise 被排除、sidechain 被排除、session title 截取前 100 字符
- [x] 6.2 `search_cache.rs` 单测：cache hit、cache miss（mtime 变化）、LRU 驱逐超容量条目
- [x] 6.3 `session_search.rs` 集成测试（用 tempdir 写 fixture JSONL）：
  - Scenario "Query matches text in multiple messages"
  - Scenario "Query matches nothing"
  - Scenario "Case-insensitive match"
  - Scenario "Project with 100 sessions and query matching 5"（缩小为 10 sessions / 4 匹配）
  - Scenario "Search term appears only inside a hard-noise system-reminder"
  - Scenario "Second search on same session after first"（验证缓存命中）
- [x] 6.4 SSH stage-limit 测试：手动模拟 `stage_limits=[2]` + `min_results=1`，验证搜索在第一阶段结束后提前返回

## 7. 质量校验

- [x] 7.1 `cargo clippy -p cdt-discover -p cdt-core --all-targets -- -D warnings` 零警告
- [x] 7.2 `cargo fmt --all` 无变更
- [x] 7.3 `cargo test -p cdt-discover -p cdt-core` 全绿（含既有测试零回归）
- [x] 7.4 `cargo test --workspace` 全绿（200 测试）
- [x] 7.5 更新根 `CLAUDE.md` Capability→crate 表中 `session-search` 行为 `done ✓`

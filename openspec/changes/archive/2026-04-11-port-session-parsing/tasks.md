## 1. cdt-core 类型骨架

- [x] 1.1 在 `crates/cdt-core/Cargo.toml` 里加 `serde`、`serde_json`、`thiserror`、`chrono`（均走 workspace 依赖）
- [x] 1.2 新建 `crates/cdt-core/src/message.rs`，定义 `ParsedMessage`、`MessageContent`（untagged enum：`Text` / `Blocks`）、`ContentBlock`（tagged enum：`Text` / `Image` / `ToolUse` / `ToolResult` / `Thinking` / `Unknown`）、`ToolCall`、`ToolResult`、`TokenUsage`、`MessageCategory`（含 `HardNoiseReason` 子枚举）
- [x] 1.3 给关键字段加 `#[serde(rename_all = "camelCase")]`、`#[serde(default)]`、`Option<T>`（次要字段），时间戳用 `chrono::DateTime<Utc>`
- [x] 1.4 `crates/cdt-core/src/lib.rs` 导出 `pub mod message;` 并 `pub use message::*;`
- [x] 1.5 为 `ParsedMessage` / `ContentBlock` 写反序列化单测（legacy string content、现代 blocks content、未知 block type 走 `Unknown`、tool_use/tool_result 字段映射、missing-optional-field 兜底）
- [x] 1.6 `cargo test -p cdt-core` 全绿，`cargo clippy -p cdt-core --all-targets` 无 warning

## 2. cdt-parse 解析器

- [x] 2.1 在 `crates/cdt-parse/Cargo.toml` 里加 `cdt-core`、`serde_json`、`thiserror`、`tokio`、`tokio-stream`、`futures`、`tracing`（均走 workspace 依赖）
- [x] 2.2 新建 `crates/cdt-parse/src/error.rs`，定义 `ParseError` thiserror 枚举（`Io` / `MalformedLine { line, source }` / `SchemaMismatch { line, reason }`）
- [x] 2.3 新建 `crates/cdt-parse/src/noise.rs`，实现 hard-noise 分类器：`classify(raw: &serde_json::Value, content: &MessageContent) -> MessageCategory`，覆盖 baseline spec 列出的全部 hard-noise 场景（system/summary/file-history-snapshot/queue-operation、synthetic assistant、仅 local-command-caveat、仅 system-reminder、空 command-output、interrupt marker）
- [x] 2.4 新建 `crates/cdt-parse/src/parser.rs`，实现 `pub fn parse_entry(line: &str) -> Result<Option<ParsedMessage>, ParseError>`：先 `serde_json::from_str::<serde_json::Value>()`，根据 `type` 字段路由到对应反序列化分支，再调用 `noise::classify` 填充 `category`，提取 `tool_use` → `tool_calls`、`tool_result` → `tool_results`
- [x] 2.5 新建 `crates/cdt-parse/src/dedupe.rs`，实现 `dedupe_by_request_id(msgs: Vec<ParsedMessage>) -> Vec<ParsedMessage>`：从后往前扫 + `HashSet<String>`，仅对 `MessageCategory::Assistant` 且 `request_id.is_some()` 的条目去重，保留 **最后一次出现** 的那一条；其它消息原样通过；保持相对顺序
- [x] 2.6 新建 `crates/cdt-parse/src/file.rs`，实现 `pub async fn parse_file(path: impl AsRef<Path>) -> Result<impl Stream<Item = ParsedMessage>, ParseError>`：用 `tokio::fs::File` + `BufReader::new(...).lines()` 逐行读，对每行调用 `parse_entry`，遇到 `MalformedLine` 走 `tracing::warn!(file = %path, line = n, error = %e)` 并继续；收集到 `Vec` 后调用 `dedupe_by_request_id`，最后用 `tokio_stream::iter` 转 `Stream`
- [x] 2.7 `crates/cdt-parse/src/lib.rs` 用 `pub use` 暴露 `parse_entry` / `parse_file` / `ParseError`，保持内部模块 `pub(crate)`

## 3. 测试：覆盖 baseline spec 的全部 scenario

- [x] 3.1 `crates/cdt-parse/tests/parse_entry.rs`：覆盖"Assistant message with tool_use blocks"、"User message with tool_result blocks"、"Compact summary boundary"、"Legacy string content"、"Current block array content"、"Synthetic assistant placeholder"、"Interrupt marker"
- [x] 3.2 `crates/cdt-parse/tests/parse_file.rs`：覆盖"Large session file"（生成 ≥10k 行的临时文件，断言流式不 OOM + 顺序正确）、"Malformed line in middle of file"、"Empty file"、"Two adjacent malformed lines"
- [x] 3.3 `crates/cdt-parse/tests/dedupe.rs`：覆盖"Two entries with same requestId"、"Three entries with the same requestId, interleaved with other messages"、"Non-assistant messages with a requestId"
- [x] 3.4 `crates/cdt-parse/tests/api_consistency.rs`：对同一份输入分别走 `parse_entry` 循环和 `parse_file`（跳过 dedup 分支），断言两者产出的 `ParsedMessage` 列表字段完全一致
- [x] 3.5 固定 fixture 数据放 `crates/cdt-parse/tests/fixtures/*.jsonl`，尽量从 `../claude-devtools` 抓取脱敏片段

## 4. 集成与质量闸

- [x] 4.1 `cargo build --workspace` 通过
- [x] 4.2 `cargo test -p cdt-core -p cdt-parse` 全绿
- [x] 4.3 `cargo clippy -p cdt-core -p cdt-parse --all-targets -- -D warnings` 无 warning
- [x] 4.4 `cargo fmt --all -- --check` 通过
- [x] 4.5 人工 review：`tracing::warn!` 调用点是否都带 `file` 与 `line` 字段；`ParseError` 是否有 `unwrap()` / `expect()` 漏网；`cdt-core` 是否真的没引入 tokio
- [x] 4.6 在 `openspec/followups.md` 的 session-parsing 区块标注 impl-bug (`requestId 去重未被调用`) 与 coverage-gap (`malformed JSON 单测`) 已在本 port 修正

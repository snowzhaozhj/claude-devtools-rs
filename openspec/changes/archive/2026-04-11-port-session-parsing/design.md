## Context

`session-parsing` 是数据层入口：负责把 Claude Code 生成的 `.jsonl` 会话文件变成下游可消费的 `ParsedMessage` 流。TS baseline 的实现住在 `../claude-devtools/src/main/services/parsing/SessionParser.ts`，Rust 侧要在 `cdt-parse` crate 里重做，核心类型放在 `cdt-core`。

当前仓库里 `cdt-core` 与 `cdt-parse` 都只有空的 `lib.rs`，workspace 已经把 `serde`/`serde_json`/`thiserror`/`tokio`/`futures`/`chrono`/`tracing` 版本锚在根 `Cargo.toml`。本次 port 是"数据层第一块骨牌"，决定了后续所有 crate 看到的核心类型形态；一旦落地，下游 12 个能力就都以 `cdt_core::ParsedMessage` 为起点扩展。

会话文件的几个关键形状（来自 TS 源码与 spec 交叉）：
- 每行一条 JSON；常见字段：`type`（user/assistant/system/summary/file-history-snapshot/queue-operation）、`uuid`、`parentUuid`、`timestamp`（ISO8601）、`cwd`、`gitBranch`、`isSidechain`、`isMeta`、`requestId`、`message.content`、`message.usage`、`message.model`。
- `content` 可能是字符串（legacy）或块数组，块类型包含 `text`、`image`、`tool_use`、`tool_result`、`thinking`。
- 单文件可达 100MB+，必须流式处理。

## Goals / Non-Goals

**Goals:**

- 用 Rust 惯用方式实现 baseline spec `session-parsing` 的 5 个 Requirement，并让每个 `#### Scenario` 都有对应的测试。
- 在 `cdt-core` 里确定一套对下游 12 个能力都友好的公共类型：`ParsedMessage`、`ContentBlock`、`ToolCall`、`ToolResult`、`TokenUsage`、`MessageCategory`。
- 修正 TS `deduplicateByRequestId` 从未被调用的 impl-bug —— 本 port 的 dedup 必须在 `parse_file` 路径上真正生效。
- 提供两种入口：同步 `parse_entry(line: &str)`（纯 CPU，便于单测 & 复用于 watch 增量解析）和异步 `parse_file(path)`（基于 `tokio::fs` + `AsyncBufReadExt::lines`，Stream 输出）。
- malformed JSON 行按 spec 行为处理：跳过 + `tracing::warn!` + 继续下一行。

**Non-Goals:**

- 不做 chunk 构建、tool execution 配对、Task→subagent 匹配、context 追踪 —— 这些是下游独立 port 的能力。
- 不做 session 搜索 / 项目发现 / 文件监视 —— 也是独立 port。
- 不做 UI 层渲染决策。
- 不实现 SSH 远端路径解析 —— `parse_file` 现阶段只读本地 `Path`，SSH 由后续 `ssh-remote-context` port 提供 `FileSystemProvider` trait 后再改造。
- 不提供 1:1 的 TS API 兼容层 —— Rust 这边接口按惯用 Rust 设计，调用方按类型迁移。

## Decisions

### 1. 类型层分层：`cdt-core` 放共享类型，`cdt-parse` 放解析器

**决定**：`ParsedMessage` 及其组成部分（`ContentBlock`、`ToolCall`、`ToolResult`、`TokenUsage`、`MessageCategory` 枚举）全部定义在 `cdt-core::message` 模块里；`cdt-parse` 只负责"把一行 JSONL → `ParsedMessage`"的转换逻辑与流式读取。

**理由**：下游 `cdt-analyze`、`cdt-discover`、`cdt-watch`、`cdt-api` 都会消费这些类型，但它们不会直接调用 `cdt-parse`（比如 `cdt-watch` 只转发文件事件）。把类型抽到 core，可以避免 `cdt-parse` 变成所有 crate 的根依赖；同时 `cdt-core` 保持 **sync-only、无 tokio** 的承诺（`.claude/rules/rust.md`）。

**替代方案**：把类型留在 `cdt-parse`，让下游 `use cdt_parse::ParsedMessage`。否决，因为这会让每个下游 crate 都必须编译出 `cdt-parse`（含 tokio），即便它只需要类型。

### 2. 序列化入口：serde + `serde_json::from_str`，带 `#[serde(rename_all = "camelCase")]`

**决定**：`ParsedMessage` 和所有子结构用 `#[derive(Deserialize, Serialize, Debug, Clone)]`，字段以 Rust 惯用的 snake_case 命名，并在结构体头上加 `#[serde(rename_all = "camelCase")]` 对齐 TS 的 camelCase 字段。`content` 用 `#[serde(untagged)]` 的 `enum MessageContent { Text(String), Blocks(Vec<ContentBlock>) }` 同时兼容 legacy / 现代格式。`ContentBlock` 用 `#[serde(tag = "type", rename_all = "snake_case")]` 的 enum，variants 覆盖 `text` / `image` / `tool_use` / `tool_result` / `thinking`，未知 type 用 `#[serde(other)]` 分支 `Unknown` 兜底，保持向前兼容。

**理由**：serde 的 tagged/untagged enum 组合几乎是所有 TS union type 的标准 Rust 映射，既类型安全又让 matcher 层写起来自然。`Unknown` 兜底分支避免未来 Anthropic 扩展 block 类型时硬性 break。

**替代方案**：用 `serde_json::Value` 懒解析，延后到消费层再分支。否决 —— 把 JSON 噪音带到下游会让 chunk-building 之类的纯逻辑 crate 也得关心 JSON 细节。

### 3. 双入口 API：`parse_entry` 同步 + `parse_file` 异步 Stream

**决定**：

```rust
// cdt-parse/src/lib.rs
pub fn parse_entry(line: &str) -> Result<Option<ParsedMessage>, ParseError>;
pub async fn parse_file(path: impl AsRef<Path>) -> Result<MessageStream, ParseError>;
// MessageStream: impl Stream<Item = ParsedMessage>
```

`parse_entry` 返回 `Option`：`None` 表示这行是合法 JSON 但属于需要过滤的条目（例如 hard-noise 不需要抛出）——实际上 hard-noise 不会在 `parse_entry` 里被 drop，而是在返回的 `ParsedMessage.category` 字段上标记，`None` 仅在"这行 JSON 结构能解析但不是 message 条目"时使用（例如 file-history-snapshot 这类可以短路的类型；TS 里也直接跳过）。对于真正损坏的行，返回 `Err(ParseError::MalformedLine)`；`parse_file` 内部 catch 该 error，`tracing::warn!` 后继续下一行，对上层表现为 Stream 静默跳过。

**理由**：同步单行接口让单测不需要起 tokio runtime；异步文件接口让 `cdt-watch` 在文件增量事件里复用 `parse_entry`，无需再读整文件。

**替代方案**：只暴露异步 API。否决 —— 纯 CPU 解析没必要被 runtime 绑住，违反 `.claude/rules/rust.md` 的"双入口"建议。

### 4. requestId 去重：`parse_file` 聚合阶段做二遍扫描

**决定**：`parse_file` 内部两阶段流水线：

1. 流式读行 → `parse_entry` → 收集到 `Vec<ParsedMessage>`（保留文件顺序）。
2. 在内存里跑一遍 `dedupe_by_request_id`：对 `category=Assistant` 且 `request_id.is_some()` 的条目，保留同 `requestId` **最后出现的完整条目**（通过从后往前扫 + `HashSet` 标记已见 id 实现），其它类型消息原样通过。

最后把 `Vec` 转成 `Stream`（`tokio_stream::iter`）。这违背了"纯流式"的承诺（需要把文件全放进内存），但这是 spec 语义使然：必须先看到文件尾才知道哪个 requestId 是"最后完整的一条"。TS 的 `deduplicateByRequestId` 做的也是同样的全量扫描，只是没被调用。

**理由**：一个典型 session `.jsonl` 即便超过 100MB，解析后的 `ParsedMessage` 列表仍是可承受的内存量（百万级条目/GB 级）；要做真正的 streaming dedup 只有"放弃语义正确性"或"读取两遍文件"两种选择，前者违反 spec，后者在 100MB 场景下 IO 翻倍。我们选正确性。

**Spec 澄清**：baseline spec 原文 "keeping the last complete entry" 的"complete"意思不明确（是"最后一条按时间序的完整记录"？还是"最后一条没有 parse error 的记录"？）。本次 port 的 MODIFIED delta 把 scenario 改写为"保留 requestId 最后一次出现时对应的 ParsedMessage（按文件顺序）"，消除歧义。

**替代方案 A**：流式去重，看到下一条同 requestId 时替换前一条的引用。否决 —— 下游可能已经消费掉前一条了，没法撤销。
**替代方案 B**：读取文件两遍，第一遍收集 id→最终 offset 映射，第二遍流式产出。备选，后续大文件分析下可切换；本期不做。

### 5. Hard-noise 分类：返回 `MessageCategory` 枚举而非 drop

**决定**：`ParsedMessage.category: MessageCategory`，枚举 variants 包含 `User`、`Assistant`、`System`、`Compact`、`HardNoise(HardNoiseReason)`。`HardNoiseReason` 细分出 `SystemEntry`、`SyntheticAssistant`、`LocalCommandCaveatOnly`、`SystemReminderOnly`、`EmptyCommandOutput`、`InterruptMarker` 等，以便下游 chunk-building 能精准过滤而不必重跑分类器。

**理由**：spec 明确要求"标记为 hard noise"而非"删除"——下游的某些调试/检查视图可能仍想看到这些条目（例如统计 noise 比例）。把分类而非过滤作为 parser 的输出符合"parser 做结构、analyzer 做语义"的分层。

### 6. 错误类型：`parse` crate 自己的 `ParseError` thiserror 枚举

```rust
// cdt-parse/src/error.rs
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("malformed JSON at line {line}: {source}")]
    MalformedLine { line: usize, source: serde_json::Error },
    #[error("unexpected schema at line {line}: {reason}")]
    SchemaMismatch { line: usize, reason: String },
}
```

`parse_entry` 返回 `Result<Option<ParsedMessage>, ParseError>`（无 line 号上下文时 `line = 0` 或提供一个 `parse_entry_at(line, ...)` 变体，后者内部加上行号）。`parse_file` 内部记 warn 并继续；不向上抛。

### 7. 时间戳：`chrono::DateTime<Utc>`

workspace 已经锚住了 `chrono = { features = ["serde"] }`，直接用 `DateTime<Utc>`，serde 自动从 ISO8601 字符串反序列化。避免 `String` 字段以后又得在下游重新 parse。

## Risks / Trade-offs

- **[Risk] `parse_file` 会把整份 `Vec<ParsedMessage>` 放进内存（为了 dedup 正确性）** → Mitigation：100MB JSONL 解析后进 struct 的典型开销在几百 MB 量级，对桌面应用可接受；未来真遇到超大文件，切换到"两遍扫描"方案（Decision 4 Alt B）。
- **[Risk] serde untagged enum 在 `MessageContent` 上的失败信息不友好** → Mitigation：明确定义 `MessageContent::Text` 与 `MessageContent::Blocks` 两个 variant，并在反序列化失败时走 `ParseError::SchemaMismatch` 路径给出行号。
- **[Risk] 未知 `ContentBlock` type 被 `Unknown` variant 吞掉后，下游看不见** → Mitigation：在 `parse_entry` 里 `tracing::debug!(block_type = %ty, "unknown content block, preserved as Unknown")`，便于将来 Anthropic 新增 block 时发现。
- **[Risk] `Deserialize` 对缺失字段过于宽松导致 silently 数据丢失** → Mitigation：关键字段（`uuid`、`type`）声明为非 `Option`；次要字段（`cwd`、`gitBranch`、`isSidechain`、`isMeta`）用 `Option<T>` 或 `#[serde(default)]`，并在测试里专门覆盖 missing-field 场景。
- **[Trade-off] hard-noise 不在 parser 层过滤掉** → 下游每个消费方都得自己 match `MessageCategory` 然后决定是否跳过；但这换来的是 parser 的语义纯粹，下游能按需保留 noise 用于调试/统计。
- **[Trade-off] 两阶段（收集 Vec → 再 Stream）失去了真流式的承诺** → 但 baseline spec 的"Large session file"场景只要求"不把原始 JSONL 一次性 slurp 进内存"，本设计满足这一点（读行时是流式，只是解析结果聚合后才 dedup + 下发）。

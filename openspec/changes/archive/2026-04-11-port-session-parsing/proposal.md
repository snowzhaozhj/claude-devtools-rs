## Why

`session-parsing` 是整个数据层的入口：把 Claude Code 的 JSONL 会话文件流式解析成 `ParsedMessage`，是后续 chunk-building、tool-execution-linking、context-tracking 等 12 个能力的共同前置依赖。没有这一步，其它任何 port 都无法落地，因此它是 Rust 重写的第一个 port 目标。

与此同时，TS 实现存在一个已确认的 impl-bug（requestId 去重函数存在但从未被调用），本次 port 顺便修正，不复刻。

## What Changes

- 在 `cdt-parse` crate 里以 Rust 惯用方式实现 session-parsing 能力，覆盖 baseline spec 的全部 5 个 Requirement（流式 JSONL、ParsedMessage 生成、legacy/现代 content 两种格式、requestId 去重、hard-noise 分类）。
- 在 `cdt-core` crate 里引入共享核心类型：`ParsedMessage`、`ContentBlock`、`ToolCall`、`ToolResult`、`TokenUsage`、`MessageCategory`。这些类型会被下游所有 port 复用，故放在 core。
- 对外提供两套解析入口：同步单行 `parse_entry(line) -> Result<ParsedMessage, _>` 和异步文件级 `parse_file(path) -> impl Stream<Item = ParsedMessage>`（符合 `.claude/rules/rust.md` 中"双入口"的约定）。
- **修正 impl-bug**：`deduplicateByRequestId` 语义一定要在 `parse_file` 聚合路径里真正生效（followups.md §session-parsing 第一条）。
- **MODIFIED Requirement**：把 `Deduplicate streaming entries by requestId` 的 scenario 语义由"keeping the last complete entry"澄清为"保留 requestId 最后出现的完整条目（按文件顺序）"，消除 TS 里"last complete"的歧义，便于 Rust 单测覆盖。
- 补上 TS 侧缺失的 malformed JSONL 用例（followups.md 标记为 coverage-gap），以 scenario-level 测试形式落地。
- **不包含**：chunk 构建、tool-execution 链接、subagent 解析、context 追踪 —— 这些是下游能力，后续独立 port。

## Capabilities

### New Capabilities
<!-- 无：session-parsing 已经存在于 openspec/specs/ -->

### Modified Capabilities
- `session-parsing`: 以 Rust 实现替代 TS baseline，并澄清 requestId 去重 scenario 的措辞以消除歧义；同时修正 TS 中"去重函数未被调用"的 impl-bug。

## Impact

- **新代码**：`crates/cdt-core/src/message.rs`（共享类型）、`crates/cdt-parse/src/lib.rs` + `parser.rs` + `dedupe.rs` + `noise.rs`（解析实现）、对应的单元 + 集成测试。
- **依赖新增**：`cdt-core` 加 `serde`、`serde_json`、`thiserror`；`cdt-parse` 加 `tokio`（含 `io-util`、`fs` features）、`tokio-stream`、`futures`、`tracing`。所有版本走 workspace root `[workspace.dependencies]`。
- **下游影响**：`cdt-analyze`、`cdt-discover`、`cdt-watch` 等下游 crate 在后续 port 中会 `use cdt_core::{ParsedMessage, ...}`，本次 port 需要确定这些类型的公共 API 形态。
- **无破坏**：此前 workspace 里没有任何 session-parsing 代码，纯新增。

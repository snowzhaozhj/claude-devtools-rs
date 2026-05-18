## Why

`openspec/followups.md::tool-execution-linking` 章节下三条 [spec-gap] / [coverage-gap] 在 Rust port 推进过程中已经被 Rust 实现覆盖到位，但 spec 与 followups 状态没同步：

1. **重复 tool_use_id 处理**（followups L68-71）：`crates/cdt-analyze/src/tool_linking/pair.rs::pair_tool_executions` 在 assistant 侧已实现"重复 tool_use id 时 keep first + `tracing::warn!` + `duplicates_dropped += 1`"分支（pair.rs:36-43），单测 `duplicate_tool_use_id_warns_and_keeps_first` 覆盖。但 `openspec/specs/tool-execution-linking/spec.md::Pair tool_use with tool_result by id` Requirement 仅写了 user 侧 `Duplicate result ids` 一个 scenario（针对 tool_result 重复），**缺**针对 tool_use 重复的 scenario。

2. **SendMessage summary 4 个 branch**（followups L73-76）：`crates/cdt-analyze/src/team/summary.rs::format_send_message` 已实现四个 branch：`shutdown_response`（approve true/false）、`broadcast`、`default + recipient + body`、`default 无 recipient`；现有单测 `send_message_shutdown_approved` / `_broadcast` / `_to_recipient` 覆盖。spec `Format readable summaries for team coordination tools` Requirement 仅写一个 `SendMessage with recipient and body` scenario，**缺**其它三个 branch 的 scenario。

3. **Task→subagent 三阶段 fallback**（followups L78-81）：spec `Resolve Task subagents with three-phase fallback matching` Requirement + 6 个 Scenario 已完整覆盖 result-based / description-based / positional 三阶段 + 跨 project_dir candidate 装载 + orphan / 等量 check 失败的兜底，与 Rust `crates/cdt-analyze/src/tool_linking/resolver.rs` 实现一致。**spec 已写齐**，仅需把 followups 第三条标 ✅ 完成状态同步。

本 change 不改动任何 Rust / TS 代码，仅做"实现已对，spec 没写全"的纯 spec 同步 + followups 状态收尾。

## What Changes

- **MODIFIED** `tool-execution-linking::Pair tool_use with tool_result by id` Requirement：补 1 个 scenario `Duplicate tool_use ids`，覆盖 assistant 侧重复 tool_use id 时 keep first + warn + `duplicates_dropped += 1` 行为。
- **MODIFIED** `tool-execution-linking::Format readable summaries for team coordination tools` Requirement：补 4 个 scenario：
  - `SendMessage shutdown_response approve=true → "Shutdown approved"`
  - `SendMessage shutdown_response approve=false → "Shutdown denied"`
  - `SendMessage broadcast type → "Broadcast: <truncated>"`
  - `SendMessage default type without recipient → truncate(type)`
- **MODIFIED** `openspec/followups.md::tool-execution-linking` 章节三条状态：把 [spec-gap] 重复 tool_use_id / [spec-gap] SendMessage summary / [coverage-gap] 三阶段 fallback 三条标 ✅ 已修，附引用本 change 名、对应 Rust 函数路径、对应 spec scenario 名。

无 IPC 字段 / Tauri command 协议改动。无 `LocalDataApi` 公开方法签名改动。无 Rust / TS 源码改动。

## Capabilities

### New Capabilities
- 无

### Modified Capabilities
- `tool-execution-linking`：仅 spec scenario 补全，行为契约的"实质"未变（实现一直是这样），是"把已存在的真实行为写进 spec"

## Impact

**代码**：无源码改动。

**spec**：
- `openspec/specs/tool-execution-linking/spec.md`：MODIFIED 两个已有 Requirement，加 5 个 scenario（archive 时由 `openspec archive` sync 回主 spec）
- `openspec/followups.md`：tool-execution-linking 章节三条状态行追加 ✅ 已修标记

**性能**：纯文档同步，无任何运行时影响。

**用户可见**：无（不动代码）。

**风险**：极低。spec 描述的行为本就是 Rust 当前实现，scenario 补全只是把"实现已对"明文化为"spec 已覆盖"，未来回归改动若违反这些 scenario 会被现有单测拦下。

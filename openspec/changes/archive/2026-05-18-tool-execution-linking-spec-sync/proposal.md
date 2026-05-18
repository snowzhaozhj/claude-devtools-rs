## Why

`openspec/followups.md::tool-execution-linking` 章节下三条 [spec-gap] / [coverage-gap] 在 Rust port 推进过程中已经被 Rust 实现覆盖到位，但 spec 与 followups 状态没同步：

1. **重复 tool_use_id 处理**（followups L68-71）：`crates/cdt-analyze/src/tool_linking/pair.rs::pair_tool_executions` 在 assistant 侧已实现"重复 tool_use id 时 keep first + `tracing::warn!` + `duplicates_dropped += 1`"分支（pair.rs:36-43），单测 `duplicate_tool_use_id_warns_and_keeps_first` 覆盖。但 `openspec/specs/tool-execution-linking/spec.md::Pair tool_use with tool_result by id` Requirement 仅写了 user 侧 `Duplicate result ids` 一个 scenario（针对 tool_result 重复），**缺**针对 tool_use 重复的 scenario。

2. **SendMessage summary 4 个 branch**（followups L73-76）：`crates/cdt-analyze/src/team/summary.rs::format_send_message` 已实现四个 branch：`shutdown_response`（approve true/false）、`broadcast`、`default + recipient + body`、`default 无 recipient`；现有单测 `send_message_shutdown_approved` / `_broadcast` / `_to_recipient` 覆盖。spec `Format readable summaries for team coordination tools` Requirement 仅写一个 `SendMessage with recipient and body` scenario，**缺**其它三个 branch 的 scenario。

3. **Task→subagent 三阶段 fallback**（followups L78-81）：spec `Resolve Task subagents with three-phase fallback matching` Requirement + 6 个 Scenario 已完整覆盖 result-based / description-based / positional 三阶段 + 跨 project_dir candidate 装载 + orphan / 等量 check 失败的兜底，与 Rust `crates/cdt-analyze/src/tool_linking/resolver.rs` 实现一致。**spec 已写齐**，仅需把 followups 第三条标 ✅ 完成状态同步。

本 change 原计划"纯 spec 同步无代码改动"，N.3 codex 二审反转该决策（design.md D6b）：spec 新加的 5 个 scenario 中 4 个无单测覆盖、followups 又引用了不存在的单测名 `duplicate_tool_use_id_warns_and_keeps_first`。按 `crates/CLAUDE.md::Spec fidelity` 硬约束"每个 SHALL 至少一个测试"，本 change 同步在 `crates/cdt-analyze/` 内补 5 个单测把契约真正落地。

## What Changes

- **MODIFIED** `tool-execution-linking::Pair tool_use with tool_result by id` Requirement：补 1 个 scenario `Duplicate tool_use ids`，覆盖 assistant 侧重复 tool_use id 时 keep first + warn + `duplicates_dropped += 1` 行为。
- **MODIFIED** `tool-execution-linking::Format readable summaries for team coordination tools` Requirement：在引言段加 effective type 默认值 `"message"` 措辞 + 补 5 个 scenario：
  - `SendMessage shutdown_response approve true → "Shutdown approved"`
  - `SendMessage shutdown_response approve false or missing → "Shutdown denied"`
  - `SendMessage broadcast type → "Broadcast: <truncated>"`
  - `SendMessage default type without recipient → truncate(type)`
  - `SendMessage missing type without recipient uses default literal → "message"`
- **ADDED** `crates/cdt-analyze/src/tool_linking/pair.rs::tests::duplicate_tool_use_id_warns_and_keeps_first` 单测落地 `Duplicate tool_use ids` scenario。
- **ADDED** `crates/cdt-analyze/src/team/summary.rs::tests` 4 个新单测 `send_message_shutdown_denied_explicit_false` / `_shutdown_missing_approve` / `_default_type_without_recipient` / `_missing_type_without_recipient_uses_message_default` 落地新 SendMessage scenario。
- **MODIFIED** `openspec/followups.md::tool-execution-linking` 章节三条状态：把 [spec-gap] 重复 tool_use_id / [spec-gap] SendMessage summary / [coverage-gap] 三阶段 fallback 三条标 ✅ 已修，附引用本 change 名、对应 Rust 函数路径、对应 spec scenario 名。**删除**原 D6 留的 "default 无 recipient 单测缺失" [coverage-gap] 条目（已被本 change 补上的单测覆盖）。

无 IPC 字段 / Tauri command 协议改动。无 `LocalDataApi` 公开方法签名改动。无 TS 源码改动。Rust 改动仅限 `crates/cdt-analyze/src/{tool_linking/pair,team/summary}.rs::tests` 模块新增 5 个 `#[test]` fn，无 production 代码 / 公开 API 变化。

## Capabilities

### New Capabilities
- 无

### Modified Capabilities
- `tool-execution-linking`：仅 spec scenario 补全，行为契约的"实质"未变（实现一直是这样），是"把已存在的真实行为写进 spec"

## Impact

**代码**：仅 `crates/cdt-analyze` 测试模块新增 5 个 `#[test]` fn（pair.rs +1 / summary.rs +4），无 production 代码 / 公开 API / IPC 协议改动。

**spec**：
- `openspec/specs/tool-execution-linking/spec.md`：MODIFIED 两个已有 Requirement，加 6 个 scenario（archive 时由 `openspec archive` sync 回主 spec）
- `openspec/followups.md`：tool-execution-linking 章节三条状态行追加 ✅ 已修标记，**删除** D6 原留的"default 无 recipient 单测缺失" [coverage-gap]（被本 change 内单测覆盖）

**性能**：纯文档同步，无任何运行时影响。

**用户可见**：无（不动代码）。

**风险**：极低。spec 描述的行为本就是 Rust 当前实现，scenario 补全只是把"实现已对"明文化为"spec 已覆盖"，未来回归改动若违反这些 scenario 会被现有单测拦下。

## ADDED Requirements

### Requirement: Expose subagent messages total count

`Process` / `SubagentProcess` 序列化 IPC payload MUST 含 `messagesTotalCount: u32` 字段（Rust 端字段名 `messages_total_count`，`#[serde(rename = "messagesTotalCount")]`），记录 subagent JSONL 内**裁剪前**的完整 `Vec<Chunk>` 长度（`cand.messages.len()`）。该字段 SHALL 在 `OMIT_SUBAGENT_MESSAGES=true`（默认裁剪路径）与 `OMIT_SUBAGENT_MESSAGES=false`（回滚路径）下行为一致——始终等于 subagent session build_chunks 后的 chunk 数。

该字段是前端 SubagentCard 在 `messagesOmitted=true` 下的唯一"messages 数量是否变化"的版本指纹来源；前端 SHALL 用 `(isOngoing, endTs, messagesTotalCount)` 三元组判定 trace 版本，版本递增即代表 subagent 内部有新 chunk 写入。

`messages_total_count` MUST 在 `candidate_to_process` 阶段（`cdt-analyze::tool_linking::resolver`）由 `cand.messages.len() as u32` 填充——与 `header_model` / `last_isolated_tokens` / `is_shutdown_only` 同阶段。IPC 层在 `apply_subagent_messages_omit` 之前 SHALL 保证该字段已填，避免裁剪 messages 后再读 length 永远是 0。

#### Scenario: messagesTotalCount in OMIT default path

- **WHEN** `OMIT_SUBAGENT_MESSAGES=true`，`Process` 由 subagent session 含 7 个 chunk 的 candidate 构造
- **THEN** IPC 序列化 JSON SHALL 含 `"messagesTotalCount": 7`、`"messagesOmitted": true`、`"messages": []`

#### Scenario: messagesTotalCount in rollback path

- **WHEN** `OMIT_SUBAGENT_MESSAGES=false`，同一 candidate 构造 `Process`
- **THEN** IPC 序列化 JSON SHALL 含 `"messagesTotalCount": 7`、`"messagesOmitted": false`、`"messages": <length=7>`

#### Scenario: messagesTotalCount 反映 ongoing subagent 内部增长

- **WHEN** 同一 subagent session 经两次 `get_session_detail`：第一次扫描时含 5 chunk，第二次扫描时（中间有 file-change 触发）含 8 chunk
- **THEN** 两次 IPC 响应中对应 `Process.messagesTotalCount` SHALL 分别为 `5` 与 `8`；前端可据此版本递增判定需要重拉 trace

#### Scenario: 嵌套 subagent 各自暴露 messagesTotalCount

- **WHEN** subagent A 的 messages 内嵌套含一条 subagent B 的引用，`get_subagent_trace` 返回 A 的 trace 含 B 的 `Process` 占位
- **THEN** A 与 B 的 `Process` MUST 各自携带独立的 `messagesTotalCount`，B 的值等于其自身 JSONL build_chunks 后的 chunk 数

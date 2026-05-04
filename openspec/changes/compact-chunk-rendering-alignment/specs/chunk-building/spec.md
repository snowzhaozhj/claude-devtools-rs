## ADDED Requirements

### Requirement: CompactChunk carries optional derived metadata

`CompactChunk`（`cdt-core::chunk::CompactChunk`）SHALL 提供两个可选派生字段，承载本会话内由 `ContextPhaseInfo` 关联出的 compaction 元数据：

- `tokenDelta: Option<CompactionTokenDelta>` —— compact 边界对应的 token 数差值（含 `preCompactionTokens` / `postCompactionTokens` / `delta`），来源为 `ContextPhaseInfo::compaction_token_deltas[chunk_uuid]`
- `phaseNumber: Option<u32>` —— compact 之后第一个 `AIChunk` 所属的 phase 编号，来源为 `ContextPhaseInfo::ai_group_phase_map[next_ai_chunk.responses[0].uuid]`

两个字段均 SHALL 用 `#[serde(default, skip_serializing_if = "Option::is_none")]`——`None` 时序列化省略字段，让老 fixture / 老前端兼容。

`cdt-analyze::chunk::builder` 在 emit `CompactChunk` 时 MUST 把这两个字段填 `None`——builder 算法层不依赖 `ContextPhaseInfo`，保持 `chunk-building` capability 既有契约（chunk emission 算法的输入仅是 `ParsedMessage` 流）。两个字段的真实值由组装层（`ipc-data-api`）后置填充，对应 spec delta 见 capability `ipc-data-api`。

#### Scenario: Builder emits CompactChunk with derived fields as None

- **WHEN** `cdt-analyze::chunk::builder` 处理一条 `is_compact_summary == true` 的 `ParsedMessage`
- **THEN** emit 的 `CompactChunk` SHALL 包含 `tokenDelta: None` AND `phaseNumber: None`
- **AND** 既有 `summaryText` / `uuid` / `timestamp` / `metrics` 字段 SHALL 与既有 Requirement `Emit CompactChunks at compaction boundaries` 描述一致（不被本字段加影响）

#### Scenario: CompactChunk serializes with optional fields omitted when None

- **WHEN** 一个 `tokenDelta: None` AND `phaseNumber: None` 的 `CompactChunk` 被序列化为 JSON
- **THEN** 输出的 JSON object SHALL **不包含** `tokenDelta` / `phaseNumber` key（由 `skip_serializing_if = "Option::is_none"` 控制）
- **AND** 反序列化 JSON object 时缺这两个 key SHALL 等价于 `tokenDelta: None` AND `phaseNumber: None`（由 `serde(default)` 控制）

#### Scenario: CompactChunk serializes derived fields as camelCase when present

- **WHEN** `CompactChunk { tokenDelta: Some(delta), phaseNumber: Some(3), .. }` 被序列化为 JSON
- **THEN** 输出 JSON SHALL 包含 key `tokenDelta`（驼峰，非 `token_delta`）AND `phaseNumber`（驼峰，非 `phase_number`）
- **AND** `tokenDelta` value 为 `{"preCompactionTokens": ..., "postCompactionTokens": ..., "delta": ...}`，对齐既有 `CompactionTokenDelta` 的 camelCase 序列化

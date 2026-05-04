## ADDED Requirements

### Requirement: CompactChunk carries optional derived metadata

`CompactChunk`（`cdt-core::chunk::CompactChunk`）SHALL 提供两个可选派生槽位，让 IPC 组装层后置填充 compaction 元数据：

- `tokenDelta: Option<CompactionTokenDelta>` —— compact 边界对应的 token 数差值（含 `preCompactionTokens` / `postCompactionTokens` / `delta`）
- `phaseNumber: Option<u32>` —— 该 compact 在 chunks 中的 phase 编号

两个字段的**派生算法与数据来源**由 capability `ipc-data-api` 的 Requirement `Expose CompactChunk derived metadata in SessionDetail` 定义——派生层从 chunks 自身（邻接 AI 的 last/first response usage 与 chunks 顺序 compact ordinal）独立计算，**不**依赖 `ContextPhaseInfo`。本 capability 仅声明 `CompactChunk` 提供这两个 optional 槽位。

两个字段均 SHALL 用 `#[serde(default, skip_serializing_if = "Option::is_none")]`——`None` 时序列化省略字段，让老 fixture / 老前端兼容。

`cdt-analyze::chunk::builder` 在 emit `CompactChunk` 时 MUST 把这两个字段填 `None`——builder 算法层接收 `ParsedMessage` 流并 emit Chunk，**不**依赖任何 phase / token 派生数据源，保持 `chunk-building` capability 既有契约（chunk emission 算法行为不变）。两个字段的真实值由 IPC 组装层（`cdt-api`）在 chunks 全部产出后基于 chunks 自身派生填充。

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

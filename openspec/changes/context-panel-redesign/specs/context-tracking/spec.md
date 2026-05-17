## MODIFIED Requirements

### Requirement: Expose context stats to display surfaces

系统 SHALL 通过稳定的数据结构暴露每 turn context 统计、按类累计 token、phase 历史，使 UI badge、hover 细分、完整 context panel 可消费。`ContextInjection.aiGroupId` 字段 SHALL 与同一 `AIChunk.chunkId` 字节级相等（共享同一 ID 形态 `ai:<base>:<n>`），使 UI 可直接用 `aiGroupId` 在 DOM 中按 `data-chunk-id` 锚点定位对应 AIChunk，无需任何客户端映射层。

#### Scenario: Query context stats for a specific turn

- **WHEN** 调用方请求第 N 个 turn 的 context 统计
- **THEN** 结果 SHALL 包含 `tokensByCategory`、total token、当前活跃 phase id、该 turn 的底层 injection 列表

#### Scenario: aiGroupId equals the corresponding AIChunk chunkId

- **WHEN** 一个 turn 的 AI group 对应 `AIChunk { chunk_id: "ai:abc-uuid:0", responses: [...] }`，且该 turn 产出至少一条 `ContextInjection`（如 `ToolOutputInjection` / `ThinkingTextInjection` / `UserMessageInjection`）
- **THEN** 所有由该 turn 产出的 injection 的 `aiGroupId` SHALL 等于 `"ai:abc-uuid:0"`（与 `AIChunk.chunk_id` 字节级相等）
- **AND** 即使同会话内出现 `chunk_id` 冲突由 `next_ai_chunk_id` 递增解决（如 `"ai:abc-uuid:1"`），对应 turn 的 injection `aiGroupId` SHALL 同步使用递增后的值

#### Scenario: Empty-response AIChunk reuses its synthesized chunk_id

- **WHEN** 某 AI group 对应 `AIChunk { responses: [], chunk_id: "ai:empty:0" }`（`next_ai_chunk_id` 已为空 response 生成稳定 ID）
- **THEN** 该 turn 产出的 injections SHALL 复用 `chunk_id` 的值（`"ai:empty:0"`）
- **AND** SHALL NOT 回退到 `responses[0].uuid` 或 `ai-<turn_index>` 等旧形态

## ADDED Requirements

### Requirement: Expose per-phase injections and phase metadata via SessionDetail IPC

系统 SHALL 在 `get_session_detail` IPC 返回的 `SessionDetail` 中暴露完整的 phase 元数据 (`phaseInfo: ContextPhaseInfo`) 与按 phase 切分的累计 injections (`injectionsByPhase: Map<phaseNumber, ContextInjection[]>`)，使前端 Phase Selector 在 compact 之后仍能展示**已被 reset 掉**的旧 phase 的 injections。`injectionsByPhase[N]` 的内容 SHALL 取自 `stats_map[phases[N-1].last_ai_group_id].accumulated_injections`（每 phase 末尾 backfill 的完整列表）；当某 phase 在 stats_map 中无对应条目时（如尾部空 phase），`injectionsByPhase[N]` SHALL 为空数组。原 `contextInjections` 字段 SHALL 保持向后兼容（值等于 `injectionsByPhase[最大 phaseNumber]`），让旧前端继续工作。

#### Scenario: Single-phase session exposes phaseInfo and injectionsByPhase

- **WHEN** 一个无 compact 的会话调用 `get_session_detail`
- **THEN** `SessionDetail.phaseInfo.phases.length == 1`
- **AND** `SessionDetail.injectionsByPhase` SHALL 含恰好一个 key `"1"`，其值等于 `SessionDetail.contextInjections`

#### Scenario: Multi-phase session preserves Phase 1 injections after compact

- **WHEN** 会话序列为 `[AI_1（含 1 个 mentioned-file injection）, compact, AI_2（含 1 个 tool-output injection）]`
- **THEN** `SessionDetail.injectionsByPhase["1"]` SHALL 含该 mentioned-file injection
- **AND** `SessionDetail.injectionsByPhase["2"]` SHALL 含该 tool-output injection
- **AND** `SessionDetail.contextInjections` SHALL 等于 `injectionsByPhase["2"]`（即 latest phase）

#### Scenario: phaseInfo round-trips through Tauri IPC with camelCase

- **WHEN** `SessionDetail` 经 serde_json 序列化为 IPC payload
- **THEN** 顶层字段名 SHALL 为 `phaseInfo` 与 `injectionsByPhase`（camelCase）
- **AND** `phaseInfo.phases[i]` 内字段 SHALL 为 `phaseNumber` / `firstAiGroupId` / `lastAiGroupId` / `compactGroupId`
- **AND** 反序列化回 `SessionDetail` SHALL 与原值字节级相等

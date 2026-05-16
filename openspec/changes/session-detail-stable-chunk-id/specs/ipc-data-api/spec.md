## ADDED Requirements

### Requirement: Stable chunk identifiers in SessionDetail

`get_session_detail` 返回的 `SessionDetail.chunks` 中每个 `Chunk` SHALL 暴露 `chunkId` 字段（camelCase 序列化），且同一次返回内所有 `chunkId` MUST 唯一。同一 session 文件内容未变化时，重复调用 `get_session_detail(projectId, sessionId)` MUST 返回相同顺序、相同 `chunkId` 的 chunks。`UserChunk` / `SystemChunk` / `CompactChunk` 的 `chunkId` SHALL 使用自身消息 `uuid`；`AIChunk` 的 `chunkId` MUST 由后端构建阶段基于稳定消息身份生成，并在重复 assistant response uuid 时通过 occurrence ordinal 等稳定后缀消歧，MUST NOT 只使用 `responses[0].uuid` 或数组 index。

#### Scenario: 重复 assistant response uuid 仍生成唯一 chunkId

- **WHEN** 一个 session 在 compact/replay 后产生两个 `AIChunk`，且两个 chunk 的 `responses[0].uuid` 相同
- **THEN** `get_session_detail` 返回的两个 `AIChunk.chunkId` SHALL 不相同
- **AND** 两个 `AIChunk.chunkId` SHALL 都保留该 shared response uuid 作为稳定身份来源的一部分

#### Scenario: 未变化 session 重复调用时 chunkId 稳定

- **WHEN** 同一 `projectId` / `sessionId` 对应的 session JSONL 文件内容未变化
- **AND** caller 连续两次调用 `get_session_detail(projectId, sessionId)`
- **THEN** 两次返回的 `chunks.map(chunk => chunk.chunkId)` SHALL 完全相同

#### Scenario: 非 AI chunk 使用自身 uuid

- **WHEN** `get_session_detail` 返回 `UserChunk` / `SystemChunk` / `CompactChunk`
- **THEN** 每个 chunk 的 `chunkId` SHALL 等于该 chunk 的 `uuid`

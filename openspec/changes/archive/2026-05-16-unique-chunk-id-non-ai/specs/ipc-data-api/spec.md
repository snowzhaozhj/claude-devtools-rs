## MODIFIED Requirements

### Requirement: Stable chunk identifiers in SessionDetail

`get_session_detail` 返回的 `SessionDetail.chunks` 中每个 `Chunk` SHALL 暴露 `chunkId` 字段（camelCase 序列化），且同一次返回内所有 `chunkId` MUST 唯一。同一 session 文件内容未变化时，重复调用 `get_session_detail(projectId, sessionId)` MUST 返回相同顺序、相同 `chunkId` 的 chunks。`UserChunk` / `SystemChunk` / `CompactChunk` 的 `chunkId` SHALL 以自身消息 `uuid` 为基底——同一次返回内首次出现该 `uuid` 的 chunk MUST 直接使用裸 `uuid` 作为 `chunkId`，后续出现该 `uuid` 的 chunk MUST 通过 occurrence ordinal 等稳定后缀（例如 `<uuid>:1`、`<uuid>:2` ...）消歧。`AIChunk` 的 `chunkId` MUST 由后端构建阶段基于稳定消息身份生成，并在重复 assistant response uuid 时通过 occurrence ordinal 等稳定后缀消歧，MUST NOT 只使用 `responses[0].uuid` 或数组 index。无论哪种 chunk kind，后端在分配 `chunkId` 时 MUST 校验整次返回内尚未出现该 candidate（例如基于一个 build 阶段的全局已分配集合），命中冲突时 MUST 继续递增 ordinal 后缀直到不撞——以兜底 uuid 自身恰好形如 `<base>:<n>` / `ai:<base>:<n>` 等极端上游输入下"`<uuid>` 与 `<uuid>:<n>` 跨形态撞车"以及 "user/system/compact 与 AI 跨命名空间撞车" 的 corner case，确保整体 `chunkId` 集合 MUST 唯一。

#### Scenario: 重复 assistant response uuid 仍生成唯一 chunkId

- **WHEN** 一个 session 在 compact/replay 后产生两个 `AIChunk`，且两个 chunk 的 `responses[0].uuid` 相同
- **THEN** `get_session_detail` 返回的两个 `AIChunk.chunkId` SHALL 不相同
- **AND** 两个 `AIChunk.chunkId` SHALL 都保留该 shared response uuid 作为稳定身份来源的一部分

#### Scenario: 未变化 session 重复调用时 chunkId 稳定

- **WHEN** 同一 `projectId` / `sessionId` 对应的 session JSONL 文件内容未变化
- **AND** caller 连续两次调用 `get_session_detail(projectId, sessionId)`
- **THEN** 两次返回的 `chunks.map(chunk => chunk.chunkId)` SHALL 完全相同

#### Scenario: 非 AI chunk 首次出现使用自身 uuid

- **WHEN** `get_session_detail` 返回 `UserChunk` / `SystemChunk` / `CompactChunk`，且其消息 `uuid` 在同一次返回的其余 chunk 中**未**出现过
- **THEN** 该 chunk 的 `chunkId` SHALL 等于其自身 `uuid`

#### Scenario: 重复 user uuid 仍生成唯一 chunkId

- **WHEN** 同一 sessionId 的 JSONL 在 `claude --bg` 启动子会话等场景下出现两条 `uuid` 相同的 user 消息
- **AND** `get_session_detail` 为这两条消息分别构造 `UserChunk`
- **THEN** 两个 `UserChunk.chunkId` SHALL 不相同
- **AND** 两个 `UserChunk.chunkId` SHALL 都保留该共享 `uuid` 作为稳定身份来源的一部分
- **AND** 整体 `chunks.map(chunk => chunk.chunkId)` MUST 唯一，前端 `{#each ... as chunk (chunk.chunkId)}` MUST NOT 触发 duplicate key 错误

#### Scenario: uuid 与 `<uuid>:<n>` 后缀形态撞车时仍唯一

- **WHEN** 同一次 `get_session_detail` 返回内既有 `uuid == "abc"` 的 chunk，又有另一条 `uuid == "abc:1"` 的 chunk
- **AND** `uuid == "abc"` 的 chunk 第二次出现（按消歧规则生成 candidate `"abc:1"`）
- **THEN** 后端 MUST 校验 `"abc:1"` candidate 已被首条 `uuid == "abc:1"` chunk 占用
- **AND** MUST 继续递增 ordinal（产 `"abc:2"`）直到 candidate 未被占用
- **AND** 整体 `chunks.map(chunk => chunk.chunkId)` MUST 唯一

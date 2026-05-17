## MODIFIED Requirements

### Requirement: Stable chunk identifiers in SessionDetail

`get_session_detail` 返回的 `SessionDetail.chunks` 中每个 `Chunk` SHALL 暴露 `chunkId` 字段（camelCase 序列化），且同一次返回内所有 `chunkId` MUST 唯一。同一 session 文件内容未变化时，重复调用 `get_session_detail(projectId, sessionId)` MUST 返回相同顺序、相同 `chunkId` 的 chunks。

**统一 `chunkId` 形态**（本 change 引入）：所有 `Chunk` 类型（`AIChunk` / `UserChunk` / `SystemChunk` / `CompactChunk`）的 `chunkId` MUST 形如 `<base>:<n>`（`n` 从 0 起的十进制整数）。`AIChunk` 的 `base` MUST 取 `responses[0].uuid`（空 responses 时 fallback 字面量 `"empty"`）；`UserChunk` / `SystemChunk` / `CompactChunk` 的 `base` MUST 取自身消息 `uuid`。**MUST NOT** 使用裸 `<uuid>` 形态（即使首次出现也必须带 `:0` 后缀），**MUST NOT** 使用 `ai:` / `user:` / `sys:` / `compact:` 等类型前缀——chunk 类型由 `Chunk::kind` 字段区分，**不**靠 `chunkId` 字面前缀。

**Collision-free 兜底**：后端在分配 `chunkId` 时 MUST 维护一个跨所有 chunk 类型共享的 build 阶段全局已分配集合（`HashSet<String>`），命中冲突时 MUST 继续递增 ordinal 后缀 `n` 直到 candidate 未被占用——以兜底 uuid 自身恰好形如 `<base>:<n>` 等极端上游输入下"跨形态撞车"以及"跨类型撞车"的 corner case，确保整体 `chunkId` 集合 MUST 唯一。

#### Scenario: 所有 chunk 首次出现使用 `<uuid>:0`

- **WHEN** `get_session_detail` 返回 `UserChunk` / `SystemChunk` / `CompactChunk` / `AIChunk`，且其 base（`uuid` 或 `responses[0].uuid`）在同一次返回的其余 chunk 中**未**出现过
- **THEN** 该 chunk 的 `chunkId` SHALL 等于 `format!("{base}:0")`
- **AND** SHALL NOT 等于裸 `base`（无后缀）
- **AND** SHALL NOT 含 `ai:` / `user:` / `sys:` / `compact:` 等类型前缀

#### Scenario: 重复 assistant response uuid 仍生成唯一 chunkId

- **WHEN** 一个 session 在 compact/replay 后产生两个 `AIChunk`，且两个 chunk 的 `responses[0].uuid` 相同（值 `"dup"`）
- **THEN** `get_session_detail` 返回的两个 `AIChunk.chunkId` SHALL 分别为 `"dup:0"` 与 `"dup:1"`
- **AND** 整体 `chunks.map(chunk => chunk.chunkId)` MUST 唯一

#### Scenario: 未变化 session 重复调用时 chunkId 稳定

- **WHEN** 同一 `projectId` / `sessionId` 对应的 session JSONL 文件内容未变化
- **AND** caller 连续两次调用 `get_session_detail(projectId, sessionId)`
- **THEN** 两次返回的 `chunks.map(chunk => chunk.chunkId)` SHALL 完全相同

#### Scenario: 重复 user uuid 仍生成唯一 chunkId

- **WHEN** 同一 sessionId 的 JSONL 在 `claude --bg` 启动子会话等场景下出现两条 `uuid` 相同的 user 消息（值 `"u-dup"`）
- **AND** `get_session_detail` 为这两条消息分别构造 `UserChunk`
- **THEN** 两个 `UserChunk.chunkId` SHALL 分别为 `"u-dup:0"` 与 `"u-dup:1"`
- **AND** 整体 `chunks.map(chunk => chunk.chunkId)` MUST 唯一，前端 `{#each ... as chunk (chunk.chunkId)}` MUST NOT 触发 duplicate key 错误

#### Scenario: uuid 与 `<uuid>:<n>` 后缀形态撞车时仍唯一

- **WHEN** 同一次 `get_session_detail` 返回内既有 `uuid == "abc"` 的 user chunk，又有另一条 `uuid == "abc:1"` 的 user chunk
- **AND** `uuid == "abc"` 的 chunk 第二次出现（按统一规则 candidate 应为 `"abc:1"`，但已被 `uuid == "abc:1"` 首次出现产出的 `"abc:1:0"` 之前的 candidate 占用）
- **THEN** 后端 MUST 校验 candidate 是否已被占用
- **AND** MUST 继续递增 ordinal 直到 candidate 未被占用（实际产 `"abc:0"` / `"abc:1:0"` / `"abc:1"` 三条互不撞）
- **AND** 整体 `chunks.map(chunk => chunk.chunkId)` MUST 唯一

#### Scenario: AI chunk 与 user chunk 跨类型不撞

- **WHEN** 同一次 `get_session_detail` 返回内有一条 `AIChunk`（`responses[0].uuid == "x"`）和一条 `UserChunk`（`uuid == "x"`）
- **THEN** 两个 chunk 的 `chunkId` 候选都是 `"x:0"`，全局集合检测冲突
- **AND** 后到的 chunk MUST 递增到 `"x:1"`
- **AND** 两个 `chunkId` SHALL 不相同

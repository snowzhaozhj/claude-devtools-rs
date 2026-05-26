# chunk-building Specification (delta)

## ADDED Requirements

### Requirement: Expose CompactChunk derived metadata in SessionDetail

`get_session_detail` 返回的 `SessionDetail.chunks` 中所有 `CompactChunk` SHALL 携带由 chunks 自身派生填充的两个可选字段（数据形态契约见 capability `chunk-building` 的 Requirement `CompactChunk carries optional derived metadata`）：

- `tokenDelta: Option<CompactionTokenDelta>`
- `phaseNumber: Option<u32>`

派生算法 SHALL 在 IPC 组装层（`cdt-api` 内 `SessionDetail` 构造路径）实现，**不**修改 `cdt-analyze::chunk::builder` 算法层、**不**依赖 `ContextPhaseInfo`。派生函数 signature SHALL 是 `apply_compact_derived(chunks: &mut [Chunk], enabled: bool)`，输入仅 chunks 序列与回滚开关。

具体规则：

- **`phaseNumber`**：派生函数内维护 `compact_counter: u32 = 1`，按 chunks 顺序遍历，每遇 `Chunk::Compact(c)` 就 `compact_counter += 1`，立即赋 `c.phase_number = Some(compact_counter)`。对齐原版 `groupTransformer.ts:295-303` 与 `cdt-analyze::context::session.rs:101` 的"compact 触发新 phase"语义
- **`tokenDelta`**：对每个 `Chunk::Compact(c)` at index `i`，独立查 chunks 自身：
  - `last_ai_before` = `chunks[..i]` 中最后一个 `Chunk::Ai`
  - `first_ai_after` = `chunks[i+1..]` 中第一个 `Chunk::Ai`
  - `pre_tokens` = `last_ai_before` 的 last response 的 `usage` 各字段总和（`input_tokens + output_tokens + cache_read_input_tokens + cache_creation_input_tokens`）；`responses` 全 `usage = None` 时 `pre_tokens = None`
  - `post_tokens` = `first_ai_after` 的 first response 的 `usage` 总和；同上 fallback
  - 若 `pre_tokens` 与 `post_tokens` 都有值 → `c.token_delta = Some(CompactionTokenDelta { pre_compaction_tokens: pre, post_compaction_tokens: post, delta: post as i64 - pre as i64 })`；任一缺值 → `c.token_delta = None`
  - 该算法对齐原版 `groupTransformer.ts:305-315` 的 `findLastAiBefore` + `findFirstAiAfter`，对**连续 compact** 给每个 compact 独立计算（虽然连续 compact 中所有 compact 的 `last_ai_before` / `first_ai_after` 命中同一对 AI，结果相同——这是与原版一致的行为）

序列化 SHALL 使用 camelCase（`tokenDelta` / `phaseNumber`）。`None` 时按 `#[serde(default, skip_serializing_if = "Option::is_none")]` 省略字段。

派生函数 SHALL 接收 `enabled: bool` 参数：调用方在生产代码传顶部 `const COMPACT_DERIVED_ENABLED: bool = true`（统一回滚点），测试代码可直接传 `false` 验回滚路径。`enabled = false` 时派生函数 SHALL 直接返回，不写入任何 `tokenDelta` / `phaseNumber`。

派生 SHALL 在 `get_session_detail` 共享路径（IPC 与 HTTP detail 共用同一组装入口）内调用一次。`list_sessions` / `list_sessions_sync` 等返回 `SessionSummary`（无 chunks）的入口 SHALL 不调用派生。

#### Scenario: Token delta computed from neighboring AI chunks

- **WHEN** session chunks 序列为 `[AIChunk(last response usage total = 30000), CompactChunk(uuid="c-1"), AIChunk(first response usage total = 5000)]`
- **WHEN** `get_session_detail` 返回该 session
- **THEN** `CompactChunk(c-1).tokenDelta` SHALL 为 `Some(CompactionTokenDelta { preCompactionTokens: 30000, postCompactionTokens: 5000, delta: -25000 })`
- **AND** 序列化 JSON SHALL 包含 `"tokenDelta":{"preCompactionTokens":30000,"postCompactionTokens":5000,"delta":-25000}`

#### Scenario: Token delta None when no AI before compact

- **WHEN** session chunks 序列为 `[UserChunk, CompactChunk(uuid="c-1"), AIChunk(...)]`（compact 之前无 AIChunk）
- **WHEN** `get_session_detail` 返回该 session
- **THEN** `CompactChunk(c-1).tokenDelta` SHALL 为 `None`
- **AND** 序列化 JSON SHALL **不包含** `tokenDelta` key

#### Scenario: Token delta None when no AI after compact

- **WHEN** session chunks 序列为 `[AIChunk(...), CompactChunk(uuid="c-1")]`（compact 在 chunks 末尾，之后无 AIChunk）
- **WHEN** `get_session_detail` 返回该 session
- **THEN** `CompactChunk(c-1).tokenDelta` SHALL 为 `None`

#### Scenario: Token delta None when neighboring AI lacks usage data

- **WHEN** session chunks 序列为 `[AIChunk(responses 全部 usage=None), CompactChunk(uuid="c-1"), AIChunk(...)]`
- **WHEN** `get_session_detail` 返回该 session
- **THEN** `CompactChunk(c-1).tokenDelta` SHALL 为 `None`（pre_tokens 无法计算）

#### Scenario: Consecutive compacts share identical token delta

- **WHEN** session chunks 序列为 `[AIChunk(last response usage total = 30000), CompactChunk(uuid="c-1"), CompactChunk(uuid="c-2"), AIChunk(first response usage total = 5000)]`
- **WHEN** `get_session_detail` 返回该 session
- **THEN** `CompactChunk(c-1).tokenDelta` SHALL 等于 `CompactChunk(c-2).tokenDelta`（都是 `Some(CompactionTokenDelta { 30000, 5000, -25000 })`，因为两个 compact 的 `last_ai_before` 与 `first_ai_after` 命中同一对 AI；对齐原版 `groupTransformer.ts:305-315` 的 `findLastAiBefore`/`findFirstAiAfter` 独立查询语义，**不会**因 cdt-analyze 内部 `current_phase_compact_group_id` 覆盖问题让 c-1 拿到 None）

#### Scenario: Phase number assigned by compact ordinal

- **WHEN** session chunks 序列含 `[UserChunk, AIChunk(...), CompactChunk(uuid="c-1"), AIChunk(...)]`（chunks 中的第 1 个 compact）
- **WHEN** `get_session_detail` 返回该 session
- **THEN** `CompactChunk(c-1).phaseNumber` SHALL 为 `Some(2)`（compact_counter 从 1 起，遇到 c-1 自增到 2）

#### Scenario: Consecutive compacts each get its own phase number

- **WHEN** session chunks 序列含 `[..., CompactChunk(uuid="c-1"), CompactChunk(uuid="c-2"), AIChunk(...)]`
- **WHEN** `get_session_detail` 返回该 session
- **THEN** `CompactChunk(c-1).phaseNumber` SHALL 为 `Some(2)` AND `CompactChunk(c-2).phaseNumber` SHALL 为 `Some(3)`

#### Scenario: Phase number stable when compact at end of chunks

- **WHEN** session chunks 序列为 `[AIChunk(...), CompactChunk(uuid="c-1"), AIChunk(...), CompactChunk(uuid="c-2")]`
- **WHEN** `get_session_detail` 返回该 session
- **THEN** `CompactChunk(c-1).phaseNumber` SHALL 为 `Some(2)` AND `CompactChunk(c-2).phaseNumber` SHALL 为 `Some(3)`（派生不依赖 compact 之后是否有 AIChunk）

#### Scenario: Compact followed only by user and system chunks

- **WHEN** session chunks 序列为 `[AIChunk(...), CompactChunk(uuid="c-1"), UserChunk, SystemChunk]`（compact 之后仅 User/System，无 AIChunk）
- **WHEN** `get_session_detail` 返回该 session
- **THEN** `CompactChunk(c-1).phaseNumber` SHALL 为 `Some(2)`（phaseNumber 派生与"compact 之后必须 AIChunk"无关）
- **AND** `CompactChunk(c-1).tokenDelta` SHALL 为 `None`（tokenDelta 需要 first_ai_after，不存在时 None）

#### Scenario: Rollback flag disables derivation

- **WHEN** 调用派生函数 `apply_compact_derived(chunks, enabled = false)`
- **AND** `chunks` 中含若干 `CompactChunk` 与相邻 `AIChunk` 含完整 usage
- **THEN** 处理后所有 `CompactChunk.tokenDelta` SHALL 为 `None` AND `phaseNumber` SHALL 为 `None`
- **AND** 该 Scenario SHALL 可在单元测试中独立断言（派生函数接收 `enabled: bool` 参数而非依赖运行时不可改的 `const`）

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


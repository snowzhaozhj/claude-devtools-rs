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

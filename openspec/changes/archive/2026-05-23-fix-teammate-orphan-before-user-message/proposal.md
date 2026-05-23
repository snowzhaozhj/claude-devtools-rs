## Why

teammate session 详情页在特定 JSONL 序列下漏渲染第一条 teammate-message：当 teammate-message 后紧跟一条被 hard-noise 过滤的 assistant（典型 `<synthetic>` model + `isApiErrorMessage=true` 的 API error 占位），再跟一条真实 user 消息时，`pending_teammates` 没有归宿——`chunk_loop` 在 user 分支调 `flush_buffer`，buffer 空时提前 return 不消费 `pending_teammates`，于是 `UserChunk` 先被 emit，第一条 teammate-message 被压到下一个 AIChunk 里，**渲染顺序倒置**（teammate prompt 跑到了用户输入之后）。

例：sessionId=`6290f9d4-c982-4ec8-89c7-5c6de88fad1a`（kb-shortcuts team frontend teammate session），第一条 teammate prompt "你是 kb-shortcuts team 的 frontend teammate..."（22:08）丢失在详情页第一屏，sidebar 摘要却展示了它（因为 sidebar 标题取 first user message text 不过滤 teammate 标签）——sidebar 与详情页数据不一致。

## What Changes

- **MODIFIED `chunk-building`**：新增 Scenario「Teammate message before non-AI user message produces standalone empty-AI chunk」覆盖 orphan-leading teammate 边界。
- **算法行为**：`chunk_loop` 在产 `UserChunk` / `SystemChunk` / `CompactChunk` 之前（即任何"flush 触发点"），若 `pending_teammates` 非空且 assistant buffer 为空，SHALL 先 emit 一条 `responses` 为空、`teammate_messages` 非空的 `AIChunk` 收容这些 teammate，再处理当前 chunk。链接（`reply_to_tool_use_id`）按既有规则在仅含 self 的 chain 上跑，多数情况配不上目标（root teammate-message 通常无 SendMessage 前驱），保持 `None` 即可。
- **既有 Scenario 不变**：teammate-message 在正常 assistant turn 内或末尾的行为契约不动。
- **回滚开关 `EMBED_TEAMMATES`**：true 路径下新增此 flush；false 路径仍直接 `continue` 不变。

## Capabilities

### Modified Capabilities

- `chunk-building`：新增 Scenario 覆盖 teammate-message 在第一个真实 user 消息之前、且其间所有 assistant 都被过滤时的标准化 emit 行为。Requirement「Embed teammate messages into AIChunk」既有 4 条规则保持不变，第 4 条 trailing 兜底语义不变；新增第 5 条规则覆盖 mid-stream user-boundary flush 边界。

## Impact

- `crates/cdt-analyze/src/chunk/builder.rs`：`flush_buffer` 行为分叉（buffer 空 + pending_teammates 非空时改为 emit empty-responses AIChunk 而非 early return）；新增 helper `emit_orphan_teammates_chunk` 或在 flush_buffer 内联处理。
- `crates/cdt-core/src/chunk.rs::AIChunk`：契约上允许 `responses: Vec::new()`——既有 `chunk_id` base fallback `"empty"` 已支持空 responses，但需在 spec 与 ipc-data-api capability 显式注明（本 change 内只改 chunk-building spec，ipc-data-api 现状已兼容）。
- 前端 `ui/src/routes/SessionDetail.svelte::aiModel(chunk)`：已有 `responses.length > 0` 守卫，empty responses 落 fallback `"Claude"`；其余 `chunk.responses[...]` 访问均在 `{#if chunk.responses.length > 0}` 或 reduce 等空安全路径下，无需改动。
- 测试新增：`crates/cdt-analyze/src/chunk/builder.rs` 单元测试覆盖 5 种 trace（teammate→user / teammate→hard-noise→user / teammate→teammate→user / multi teammate→hard-noise→user / 既有 trailing 兜底回归）；`crates/cdt-api/tests/ipc_contract.rs` 增 round-trip 测试确认 `AIChunk { responses: [], teammate_messages: [..] }` 可序列化且前端 type 兼容。
- 兼容性：序列化层 `AIChunk.responses` 不带 `skip_serializing_if`，改动后前端会收到 `responses: []`——已知前端能消费；`ipc_contract` 测试守住该形态。

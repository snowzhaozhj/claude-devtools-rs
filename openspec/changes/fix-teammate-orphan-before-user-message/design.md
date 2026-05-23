## Context

`chunk-building` capability 既有 Requirement「Embed teammate messages into AIChunk」定义了 4 条规则：teammate-message 进 `pending_teammates`，下一次 `flush_buffer` 时注入新构造的 `AIChunk.teammate_messages`；主循环结束兜底追加到最后一个已 emit 的 AIChunk。但**未覆盖 mid-stream 边界**——teammate-message 后到达的"会产生 user-side chunk（`UserChunk` / `SystemChunk` / `CompactChunk`）"的消息**触发的 `flush_buffer` 在 buffer 为空时提前 return**（`crates/cdt-analyze/src/chunk/builder.rs:627-631`），`pending_teammates` 不消费、不归宿、被压到下一个真实 AIChunk。

实际触发条件（命中真实数据，sessionId=`6290f9d4-c982-4ec8-89c7-5c6de88fad1a`）：
1. session 第一条非元数据消息是 teammate-message（lead 给 teammate 的角色 prompt）；
2. 紧跟一条 `<synthetic>` model + `isApiErrorMessage=true` 的 assistant API error 占位 → parser 标 `HardNoiseReason::SyntheticAssistant` 跳过；
3. 用户接着发"继续"等真实 user message。

→ chunk emit 顺序变成 `[UserChunk("继续"), AIChunk(含第一条 teammate-message + 后续 frontend assistant 内容)]`，但第一条 teammate-message 的 timestamp 早于 "继续"——视觉顺序倒置，sidebar 摘要（first-user-message text 不过滤 teammate 标签）与详情页（chunk-building 过滤 teammate user）数据不一致。

## Goals / Non-Goals

**Goals:**
- chunk-building 在所有"产生 user-side chunk"路径前 SHALL 标准化 emit `pending_teammates`，使 teammate-message 的 emit 顺序严格遵循 timestamp 早于后续 user-side chunk。
- 对前端 / IPC / 历史 fixture 的破坏最小（不引入新 Chunk 类型、不改 IPC tag enum 值、不动主 spec Purpose）。
- 保持「teammate-message 不产 UserChunk」既有契约；teammate 仍嵌入 AIChunk.teammate_messages（同一 chunk 类型）。

**Non-Goals:**
- 不修复 sidebar 摘要把 teammate-message 当 first-user-message 取标题的行为（这是另一个 followup，由 `cdt-discover::search_extract` 决定，与详情页渲染解耦）。
- 不改 ipc-data-api capability 的字段语义；`AIChunk.responses` 已经是 `Vec<AssistantResponse>` 不强制非空，本 change 只显式承认空数组的合法性而非引入新约束。
- 不重构 `flush_buffer` 调用方——所有调用点（普通 user / Compact / SystemChunk / Slash / Interruption）都期望"flush 把 pending_teammates 处理掉"，本 change 让单一函数行为对齐期望。
- 不解决 trailing teammate 兜底（drain_trailing_teammates 已覆盖，回归测试守住）。

## Decisions

### D1：用 responses 为空的 AIChunk 收容 orphan teammate（不引入新 Chunk variant）

让 `flush_buffer` 在 `buffer.is_empty() && !pending_teammates.is_empty()` 时**也 emit 一条 `AIChunk`**，其 `responses: Vec::new()` / `semantic_steps: Vec::new()` / `tool_executions: Vec::new()` / `subagents: Vec::new()` / `slash_commands: <既有 pending_slashes>` / `metrics: ChunkMetrics::zero()` / `duration_ms: None`，仅 `teammate_messages` 非空。

**Alternatives considered:**

- **A. 新增 `Chunk::Teammate(TeammateChunk)` variant**：类型上最纯（teammate-only chunk 不假装是 AI turn），但要动 `Chunk` enum、IPC tag values、`SessionDetail.svelte` 的 `chunk.kind` switch、`displayItemBuilder` 类型分流、`ipc_contract` round-trip、`__fixtures__/multi-project-rich.ts` 测试 fixture——破坏面 5×。**否决**理由：违反 Goals「破坏最小」，也违反既有 spec 主线「teammate 嵌入 AIChunk.teammate_messages」的契约抽象。
- **B. 把 pending_teammates prepend 到 UserChunk**：给 `UserChunk` 加 `teammate_messages_before: Vec<TeammateMessage>` 字段，让 UserChunk 携带"我之前"的 teammate context。**否决**：UserChunk 与 teammate 语义不绑定（teammate 是 AI-side 概念），加字段意味着前端两处渲染 teammate 卡片（AIChunk + UserChunk），与既有显示契约割裂。
- **C. 选定方案：empty-responses AIChunk**：复用既有 chunk 类型 / IPC schema / 前端渲染路径；前端 `aiModel(chunk)` 已有 `responses.length > 0` 守卫（fallback "Claude"），其余 `chunk.responses[...]` 访问均 `{#if chunk.responses.length > 0}` 包裹或 reduce 空安全；新 AIChunk 视觉上是"只含 teammate 卡片的 turn"，符合用户预期。

### D2：flush 触发点不变，只改 `flush_buffer` 内部行为

不引入新 helper `flush_orphan_teammates`、不在 chunk_loop 的 user / Compact / SystemChunk / Slash / Interruption 分支前 inline 检查。**只改 `flush_buffer` 函数本身**——把 buffer 空时的 early return 替换为：

```rust
if buffer.is_empty() {
    if pending_teammates.is_empty() {
        return;
    }
    // 产 responses 为空但 teammate_messages 非空的 AIChunk
    let new_chunk = AIChunk {
        chunk_id: next_chunk_id(&pending_teammates[0].uuid, used_chunk_ids),
        timestamp: pending_teammates[0].timestamp,
        duration_ms: None,
        responses: Vec::new(),
        metrics: ChunkMetrics::zero(),
        semantic_steps: Vec::new(),
        tool_executions: Vec::new(),
        subagents: Vec::new(),
        slash_commands: std::mem::take(pending_slashes),
        teammate_messages: link_pending(...), // 同既有路径
    };
    out.push(Chunk::Ai(new_chunk));
    return;
}
// buffer 非空：原路径不动
```

**Alternatives considered:**

- 显式 helper 让调用点逐处加 `flush_orphan_teammates(...)`：分散 + 易漏一个分支；既有 5 个调用点（`MessageCategory::Compact` / `extract_local_command_stdout` 命中 / `is_meta` 命中 / 普通 user / Interruption）改 5 处，破坏面大于"单点函数行为收敛"。**否决**。
- 选定 D2：单点改 `flush_buffer`，所有调用方语义不变（"flush 把 pending_teammates 处理掉"），符合 Goals「破坏最小」。

### D3：empty-responses AIChunk 的 `chunk_id` base 取 `pending_teammates[0].uuid`

teammate UUID 在 `parse_all_teammate_attrs` 阶段对多 block 已加 `-N` 后缀去重，全局唯一；用作 `chunk_id` base 通过 `next_chunk_id` 的 `<base>:<n>` 形态走 `used_chunk_ids` set 兜底，与既有契约一致（既有 trailing 兜底 `drain_trailing_teammates` 不产新 chunk，复用最后一个 AI base，本 change 是新 chunk 必须独立 base）。

`timestamp` 取 `pending_teammates[0].timestamp` 最自然——chunk timestamp 反映 teammate-message 的实际产生时间，前端按 timestamp 排序的列表（如 ContextPanel）能正确归位。

`metrics` 取 `ChunkMetrics::zero()`：empty AIChunk 没有 token usage / tool count；spec 既有 Requirement「Compute per-chunk metrics」`UserChunk without token usage` Scenario 已确立"无 usage 数据时全零"先例，empty AIChunk 走相同语义。

### D4：`pending_slashes` 在 empty AIChunk 上消费不变

`flush_buffer` 既有路径会 `let slash_commands = std::mem::take(pending_slashes);` 把待挂载的 slash 命令注入新 AIChunk。**保持此行为**——empty-responses AIChunk 的 `slash_commands` 字段同样从 `pending_slashes` 消费。理由：teammate-message 不打断 pending_slashes（与既有 user 消息一样不 clear），如果用户路径上 `slash → teammate → 真实 user`，slash 应当挂在 empty AIChunk 上而不是漂到下一个真实 AIChunk（后者跨过了 user message，违反 spec 的 slash 紧邻 AI turn 约定）。

实际罕见但路径完备性需要它正确。新增单测 `slash_then_teammate_then_user_emits_empty_ai_with_slash_and_teammate` 守住此边界。

### D5：`reply_to_tool_use_id` 链接逻辑复用既有 `link_against_chunks` 不变

empty-responses AIChunk 自身没有 SendMessage `tool_use`（`tool_executions` 为空）；查找链回退到既有 `out` 中最近 `LOOKBACK_LIMIT` 个 AIChunk。多数 orphan-leading 场景下（teammate-message 在 session 首条），链为空，`reply_to_tool_use_id` 落 `None`——与「lead 启动 teammate 时给的 prompt 没有 SendMessage 来源」语义一致。

## Risks / Trade-offs

- **[Risk] 前端将看到一种视觉上不同的 AIChunk**：模型名 fallback `"Claude"`、无 metrics 行、无任何工具调用 / thinking / 输出，只有 teammate 卡片。
  - **Mitigation**：teammate 卡片本身是该 chunk 唯一可视内容，与 sidebar 摘要"你是 kb-shortcuts team..."呼应；模型名 fallback 既有，无新增 UI 工作。
- **[Risk] `__fixtures__/multi-project-rich.ts` 等历史 fixture 没覆盖 empty AIChunk**：spec 已隐式允许 `responses: []`，但前端 `displayItemBuilder` / `SessionDetail.svelte` 老路径未曾命中。
  - **Mitigation**：新增 ipc_contract round-trip + Vitest unit test 覆盖 empty AIChunk 的 displayItem 流（应只产 `teammate_message` items，无 thinking / tool / output），并在 e2e fixture 加一条 multi-tab 子树验证渲染无 console error。
- **[Risk] CompactChunk 边界叠加**：teammate-message → Compact 边界（无 buffer）→ 新行为先 emit empty AIChunk 含 teammate，再 emit CompactChunk。原行为 teammate 被 trailing 兜底丢到 CompactChunk 之后的 last AI（罕见且错位）。
  - **Mitigation**：新增单测 `teammate_then_compact_emits_empty_ai_before_compact` 守住期望顺序。
- **[Trade-off] 引入"empty AIChunk"语义**：将来读 chunk 列表的代码（如 metric aggregation、token 估算、Phase 切分）需要假设 `responses` 可空。
  - **Mitigation**：proposal Impact 段已记录前端访问点全部空安全；后端 `cdt-analyze::context` 等聚合代码使用 iter / reduce 不依赖 `responses[0]`；如未来引入需要非空假设的代码，需显式守卫。

## Migration Plan

无 schema 迁移、无数据迁移。代码改动单点，跟随主 PR 一并发布。

回滚：`EMBED_TEAMMATES = false` 既有开关让整个嵌入路径退回旧行为（teammate 直接 `continue` 丢弃），不需要为本 change 单独引入回滚开关。本 change 在 `EMBED_TEAMMATES = false` 路径下不生效（pending_teammates 永远为空），向前兼容。

## Open Questions

无。所有 D1-D5 决策已在本 design 中冻结。


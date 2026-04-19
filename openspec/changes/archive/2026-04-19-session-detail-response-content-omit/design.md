## Context

Phase 2 (`subagent-messages-lazy-load`) 把 subagent 嵌套 chunks 全文从首屏 IPC 裁掉；Phase 3 (`session-detail-image-asset-cache`) 把内联 image base64 落盘 cache。phase 3 后实测无 image 大 session（46a25772, 1221 msgs）IPC 仍 2799 KB / 427 ms。前端 console 数据校准 IPC 实测吞吐 ≈ 6.5 KB/ms（含 V8 JSON.parse 反序列化）。

升级版 perf bench (`crates/cdt-api/tests/perf_get_session_detail.rs`) breakdown 显示：

| 字段 | 大小 | 占比 |
|------|------|------|
| **`responses[].content`** | **1257 KB** | **41%** |
| `tool_exec` (input + output) | 884 KB | 29% |
| `responses[].meta` (toolCalls + usage + model + timestamps) | 573 KB | 19% |

`responses[].content` 是最大单一字段。审查前端代码 (`grep -rn 'responses' ui/src/`)：

```
ui/src/routes/SessionDetail.svelte:180:    if (c.kind === "ai") return c.responses[0]?.uuid ?? c.timestamp;
ui/src/routes/SessionDetail.svelte:271:      const m = chunk.responses[chunk.responses.length - 1].model;
ui/src/components/SubagentCard.svelte:88:      for (const r of c.responses) {
ui/src/components/SubagentCard.svelte:104:      for (const r of c.responses) {
ui/src/components/SubagentCard.svelte:128:      for (const r of c.responses) {
```

逐行审查后端字段访问：
- L180: 只读 `responses[0].uuid`
- L271: 只读 `responses[last].model`
- SubagentCard L88: 读 `r.model`
- SubagentCard L104: 读 `r.usage`
- SubagentCard L128: 读 `r.toolCalls`

**没有任何前端代码读 `r.content`。** 显示文本走 `semanticSteps`（见 `cdt-analyze::extract_semantic_steps`）：thinking / text 步骤都自带 `text` 字段，`buildDisplayItems` 只用 semanticSteps 不用 responses[].content。

## Goals / Non-Goals

**Goals:**

- 把 `responses[].content` 从 `get_session_detail` 默认 IPC payload 中裁掉（与 phase 2 / phase 3 同模式：derived flag + OMIT 常量回滚开关）。
- 零前端改动——前端本来就不用 content。
- 保持向后兼容：老缓存 / 回滚开关 false 时字段反序列化为完整 `MessageContent`，`content_omitted` 默认 false。

**Non-Goals:**

- 不开新 IPC 懒拉。前端从未读 content，没有"展开时按需拉"的场景。如未来全文搜索 / 复制需求出现，再加 `get_chunk_content(sessionId, chunkUuid, responseIndex)`。
- 不改 `tool_exec` / `responses[].meta`（次大字段）。tool input/output 是 chunk header summary 与 ExecutionTrace 的真实数据源，不能盲裁；留下下轮 follow-up 处理（懒加载或 stream 化）。
- 不改 HTTP path（与 phase 2 / phase 3 同分叉）。

## Decisions

### 决策 1：用 OMIT + 单字段替换，不开新 IPC

**选择**：`responses[].content` 直接替换为空 `MessageContent::Text("")` + `content_omitted = true`。前端零改动。

**替代方案**：开 `get_chunk_content(sessionId, chunkUuid)` 懒拉。
- 缺点：前端代码没用 content → 没有触发懒拉的入口，IPC 永远不会被调；纯白做的接口。
- 风险：未来若加全文搜索 / 复制功能要 content → 那时再加懒拉接口（与 phase 2 同模式），不破坏当前数据流。

**理由**：YAGNI。当前 0 调用方，先把 payload 砍下来。

### 决策 2：用 `MessageContent::Text("")` 替换，不用 `Option<MessageContent>`

**选择**：保留 `content: MessageContent` 字段类型不变，OMIT 时替换为空 `Text("")` + 设 flag。

**替代方案**：把 `content` 改成 `Option<MessageContent>` 或 enum 加 `Omitted` variant。
- 缺点：破坏 `cdt-core` 公共 API，所有构造点都要改；下游 `cdt-analyze` 的 chunk-building 也要调整；改动半径过大。
- 实际收益：`MessageContent::Text("")` 序列化只占 ~20 字节 / response，1221 条 ~24 KB——可忽略。

**理由**：最小改动 + serde 兼容性。`#[serde(default)]` 在 `content_omitted` 上保证老 JSON 反序列化为 false。

### 决策 3：OMIT 顺序：image → response.content → subagent.messages

**选择**：在 `get_session_detail` 序列化前按此顺序应用三层 OMIT。

**理由**：
- `apply_image_omit` 必须最早跑（它递归进 subagent.messages）
- `apply_response_content_omit` 跟进（同样递归 subagent.messages，覆盖回滚 `OMIT_SUBAGENT_MESSAGES=false` 的嵌套层）
- `OMIT_SUBAGENT_MESSAGES` 最后（它直接清空 messages，前两个对 cleared messages 是 no-op）

回滚组合（任一开关设 false）下三层都能正确命中。

### 决策 4：本期不动 `tool_exec` 与 `responses[].meta`

**理由**：
- `tool_exec` (884 KB) 是 chunk header 的真实数据来源——`AIChunk.tool_executions` 决定 `Bash 5 / Read 3` 这类 summary（见 `displayItemBuilder.ts::buildSummary`），UI 还要展示 ExecutionTrace 详情。盲裁会破坏 chunk 标题；正确做法是分字段 OMIT（input/output 分别懒拉），需要新 IPC + 前端 ExecutionTrace 改造，留下轮。
- `responses[].meta` (573 KB) 包含 `toolCalls + usage + model + timestamp + uuid`，前端**全部都用**——SessionDetail 标题用 model、SubagentCard header 用 usage、chunkKey 用 uuid。无可裁字段。

## Risks / Trade-offs

- **[风险] 未来全文搜索功能要用 content**：届时退回 `OMIT_RESPONSE_CONTENT=false` 或加新 IPC `get_chunk_content` 即可。
- **[风险] 老前端 build + 新后端**：前端不用 content → OMIT 后的空 string 不渲染，行为完全等价。零兼容问题。
- **[trade-off] 未碰 `tool_exec`/`responses[].meta`**：剩余 payload ≈ 1.5 MB，按 6.5 KB/ms 仍 230 ms。下轮再深挖。

## Migration Plan

### 部署步骤

1. **Rust 侧无破坏性 schema 变更**：`AssistantResponse.content_omitted` 加 `#[serde(default)]`，老 JSON / 老缓存反序列化为 `false`。
2. **前端零改动**：现有 `chunkKey` / `aiModel` / `SubagentCard` 不读 `content`，OMIT 后空 string 不影响渲染。
3. **回滚**：`OMIT_RESPONSE_CONTENT: bool = false` 即恢复完整 payload。

## Open Questions

- 是否需要 `get_chunk_content(sessionId, chunkUuid)` 懒拉接口？— **不在本期**：当前 0 调用方，加了也没人调。复制功能 / 全文搜索出现时再补。
- `tool_exec` 字段如何瘦身？— **下轮 follow-up**：input/output 分别懒拉（input 占 340 KB / output 占 436 KB），需要 ExecutionTrace 展开时拉的设计；可能也涉及 streaming。
- `responses[].meta` (573 KB) 是否还能压缩？— **不能**：已是高频访问字段。

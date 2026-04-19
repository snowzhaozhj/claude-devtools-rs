## Context

### 问题

`session-detail-lazy-render` 已落地，前端首屏渲染 ≈ 20 ms。但实测 `46a25772`（1221 msgs / 96 chunks / 14 subs）：

```
IPC 556ms (chunks=96, payload=7335KB)
first-paint 599ms
```

IPC 占 first-paint 的 **97%**。Tauri webview IPC 在 macOS 上实测吞吐 ≈ 13 KB/ms。

### Payload Breakdown

`crates/cdt-api/tests/perf_get_session_detail.rs::analyze_payload` (release)：

| Session | total | subagent_messages | response_content | tool_output | tool_input |
|---------|-------|-------------------|------------------|-------------|-----------|
| 4cdfdf06 | 3472 KB | 1712 KB (49%) | 269 KB | 87 KB | 33 KB |
| 7826d1b8 | 5161 KB | 333 KB (6%) | 290 KB | 134 KB | 41 KB |
| 46a25772 | 7700 KB | **4659 KB (60%)** | 1257 KB | 436 KB | 340 KB |

`subagent_messages` 是含大量 subagent 的 session 的最大头。

### SubagentCard header 数据依赖

`ui/src/components/SubagentCard.svelte`（unexpanded 状态）：
- L52-60 `modelName` 从 `process.messages` 找最后一条 AI response.model
- L63-80 `isolatedTokens` 从 `process.messages` 找最后一条 AI usage 累加
- L83-100 `isShutdownOnly`（team-only）依赖 messages 含 1 条 assistant + SendMessage shutdown_response

这 3 个 derived 值在卡片**未展开**也要显示。完全 drop messages 会让 header 失效。

## Goals / Non-Goals

**Goals：**
- 96 chunks / 14 subs session 首屏 IPC < 250 ms（≈ 砍 60% payload）。
- SubagentCard header 视觉零回归（modelName / token count / shutdown-only 显示正常）。
- 用户展开 SubagentCard 时按需拉取 messages，5MB 量级的 trace 单次 IPC 可接受（≈ 100 ms）。
- 嵌套 subagent（depth > 0）递归 lazy load（每层展开时拉自己）。
- 回滚开关 `OMIT_SUBAGENT_MESSAGES: bool = true` 一行切回旧行为。

**Non-Goals：**
- response_content / tool_output / tool_input 的进一步瘦身（占比 < 20%；优先吃 60% 大头）。
- HTTP API 路径（HTTP 无活跃用户，单独留 follow-up）。
- 跨展开状态的 messages 持久化缓存（每次重开 SubagentCard 会再拉一次；可接受）。
- 后端把 `Process.messages` 类型改成 `Option<Vec<Chunk>>`（破坏性大；用空 Vec + `messages_omitted` flag 同等表达力）。

## Decisions

### 1. `Process` 加 4 个 derived header 字段

**选：** 在 `cdt-core::Process` struct 加：
```rust
#[serde(default)]
pub header_model: Option<String>,
#[serde(default)]
pub last_isolated_tokens: u64,
#[serde(default)]
pub is_shutdown_only: bool,
#[serde(default)]
pub messages_omitted: bool,
```

**替代：** (a) 把这些塞进 `metrics` —— `ChunkMetrics` 是跨 chunk 类型共享，不该掺 subagent-only 字段；(b) 新建 `ProcessHeader` sub-struct —— 多一层嵌套不增清晰度。

**理由：** 4 个 `#[serde(default)]` 字段不破坏老 client 反序列化；前端 TS 类型加 optional 字段同样向后兼容（fallback 走 messages 派生）。

### 2. 后端裁剪时机：`get_session_detail` 序列化前

**选：** `LocalDataApi::get_session_detail` 在 `serde_json::to_value(&chunks)` 之前 `clone` chunks，遍历 `Chunk::Ai(ai).subagents`，把每个 `Process.messages = Vec::new()` 并 `messages_omitted = true`。`Process.header_model / last_isolated_tokens / is_shutdown_only` 已在 `candidate_to_process` 阶段填好。

**替代：** 在 `candidate_to_process` 直接构造空 `messages` —— 但 `build_chunks_with_subagents` 链路里的 spawn step 排序等需要 process 字段一致；且若未来某些场景需要完整 messages，单点裁剪比源头永久 drop 更灵活。

**理由：** 单点裁剪 + 显式 flag 让数据流可观测；`OMIT_SUBAGENT_MESSAGES = false` 一秒回滚。

### 3. 派生字段在 resolver 阶段计算

**选：** `crates/cdt-analyze/src/tool_linking/resolver.rs::candidate_to_process` 内调用 `derive_subagent_header(&messages)`：
- `header_model`：找最后一条 AI Chunk 最后一条 response.model，跑 `parse_model_string` 简化（如 `"claude-haiku-4-5-20251001"` → `"haiku4.5"`）
- `last_isolated_tokens`：累加同上 response.usage 的 4 项
- `is_shutdown_only`：count assistants == 1 且 only tool_call 是 SendMessage 且 input.type == "shutdown_response"

**替代：** 前端按 IPC 拉到 messages 后再算 —— 需要把 messages 全传，等于没瘦身。

**理由：** 后端已持有完整 messages，预算几乎零成本。

### 4. 新 IPC：`get_subagent_trace`

**选：**
```rust
async fn get_subagent_trace(
    &self,
    parent_session_id: &str,
    subagent_session_id: &str,
) -> Result<Vec<Chunk>, ApiError>;
```

实现：在 `~/.claude/projects/<project>/<parent_session_id>/subagents/agent-<subagent_session_id>.jsonl`（新结构）或旧结构兼容路径下 `parse_file` + `build_chunks`，返回 `Vec<Chunk>`。**不**做 subagent-of-subagent resolver（嵌套 subagent 走自己的 `get_subagent_trace` lazy 路径）。

**替代：** 单 IPC `get_session_detail` 加可选 `include_subagent_messages: Vec<String>` —— 前端要管理"哪些 subagent 已展开"列表，状态机复杂；专用 IPC 单一职责更清晰。

**理由：** 与既有 `get_session_detail` 同模式（按 ID 拉），无新基础设施。

### 5. 前端 `SubagentCard` 适配

**选：**
```svelte
<script>
  // 优先用预算字段；fallback 兼容旧后端
  const modelName = $derived(process.headerModel ?? deriveFromMessages(process.messages, "model"));
  const isolatedTokens = $derived(process.lastIsolatedTokens ?? deriveFromMessages(process.messages, "tokens"));
  const isShutdownOnly = $derived(process.isShutdownOnly ?? deriveShutdownFromMessages(process.messages));

  // Lazy trace
  let messagesLocal: Chunk[] | null = $state(null);
  async function ensureMessages() {
    if (messagesLocal != null) return;
    if (!process.messagesOmitted) {
      messagesLocal = process.messages;
      return;
    }
    messagesLocal = await getSubagentTrace(parentSessionId, process.sessionId);
  }
  async function toggleExpanded() {
    isExpanded = !isExpanded;
    if (isExpanded) await ensureMessages();
  }
  const traceItems = $derived(
    isExpanded && messagesLocal ? buildDisplayItemsFromChunks(messagesLocal) : [],
  );
</script>
```

**替代：** 直接 mutate `process.messages` —— Svelte 5 props 是只读响应式；写要走 `$state` 或 derived。

**理由：** 局部 `$state` + 显式 ensure 流程清晰；fallback 链让 spec 改动可分阶段灰度。

### 6. parentSessionId 传递

**选：** SubagentCard 新增必传 prop `parentSessionId: string`，由 SessionDetail.svelte 传入（已知 `sessionId`）。嵌套 subagent 渲染时父 SubagentCard 把自己的 `process.sessionId` 作为子的 `parentSessionId`——但**所有** subagent 共享同一 root session 的 disk 路径（`<root>/<root_id>/subagents/agent-<sub_id>.jsonl`），所以实际 `get_subagent_trace` 的 parent 一定是**最外层** session id。

修正：SubagentCard 接收 `rootSessionId`（一路向下传递不变）而非 `parentSessionId`。

### 7. 回滚开关

`crates/cdt-api/src/ipc/local.rs` 顶部：
```rust
const OMIT_SUBAGENT_MESSAGES: bool = true;
```

`get_session_detail` 内 `if OMIT_SUBAGENT_MESSAGES { ... 裁剪逻辑 ... }`。出问题一行切 false。前端 fallback 路径自动生效（消息全在 `process.messages`）。

## Risks / Trade-offs

- **[风险] 嵌套 subagent 的 IPC 串行**：用户连续展开 3 层嵌套 subagent → 串行 3 次 IPC，每次 ~50-100 ms → 可接受（用户主动展开有等待预期）。
- **[风险] disk 路径假设**：`get_subagent_trace` 复用 `find_subagent_jsonl` 逻辑，已支持新旧两种结构；新增 IPC 不引入新路径假设。
- **[风险] 派生字段过期**：file-change 触发 refreshDetail 后，重新跑 resolver → 派生字段重新计算 → 自动同步。
- **[权衡] `Process.messages` 在 IPC 后不再"始终完整"**：调用方需先看 `messagesOmitted` 决定要不要单独拉；老 client（不读 flag）直接用 `messages` 会拿到空数组——前端 fallback 链已规约。
- **[权衡] 多次重开同 SubagentCard 都重拉**：5 MB trace ~50-100 ms IPC，可接受；持久缓存留 follow-up。

## Migration Plan

1. `cdt-core::Process` 加 4 字段（`#[serde(default)]` 兼容老反序列化）。
2. `cdt-analyze::candidate_to_process` 填充 derived 字段；单元测试覆盖。
3. `cdt-api::LocalDataApi`：实现 `get_subagent_trace`；`get_session_detail` 加裁剪逻辑（受 `OMIT_SUBAGENT_MESSAGES` 控制）。
4. `src-tauri/src/lib.rs` 注册新 command。
5. 前端 `Process` TS 类型加 optional 字段；`SubagentCard` 改造（fallback 链 + lazy ensure）；`SessionDetail.svelte` 传 `rootSessionId={sessionId}` 给所有 SubagentCard。
6. `npm run check --prefix ui` + `cargo test --workspace` + `just preflight` 全绿。
7. 自用样本 session 验证：first-paint 数据落到 < 250 ms；展开 subagent 视觉与功能无回归。
8. 稳定 24h 后归档；保留 `OMIT_SUBAGENT_MESSAGES` 常量供应急回滚。

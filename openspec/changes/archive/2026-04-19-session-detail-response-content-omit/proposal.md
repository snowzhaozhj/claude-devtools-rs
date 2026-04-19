## Why

`session-detail-image-asset-cache`（phase 3）落地后，46a25772 case（无 image，1221 msgs / 96 chunks / 14 subs）IPC payload 仍 2799 KB，前端 console 实测 first-paint 455 ms 内 IPC 占 427 ms（94%）。前端 console 数据校准了 Tauri webview IPC 实测吞吐 ≈ **6.5 KB/ms**（之前 13 KB/ms 估算只算了网络字节没算 V8 JSON.parse 开销，实际翻倍）。

`crates/cdt-api/tests/perf_get_session_detail.rs` 升级版 breakdown（commit 0c8a7a6 / 1bfe0ad 后）显示剩余 payload 分布：

| 字段 | 大小 | 占比 |
|------|------|------|
| **`responses[].content`** | **1257 KB** | **41%** |
| `tool_exec` | 884 KB | 29% |
| `responses[].meta` | 573 KB | 19% |
| 其他 (timestamps / uuids) | ~200 KB | 7% |

**`responses[].content`** 是 single-largest field，且 **前端从未使用**——`grep responses` 全 UI 只 6 处用：
- `SessionDetail.chunkKey` 用 `responses[0].uuid`
- `SessionDetail.aiModel` 用最后一条 `model`
- `SubagentCard` fallback 用 `model` / `usage` / `toolCalls`

**没有任何前端代码读 `responses[i].content`**——chunk 内显示文本完全冗余存在 `semanticSteps` 里（thinking / text step 都自带 `text` 字段，`buildDisplayItems` 只用 semanticSteps）。

把 `responses[].content` 默认裁剪为空，复用 phase 2 / phase 3 的 OMIT 模式，最小化改动：单字段 + derived flag + 一行回滚开关。本期 **不需要新 IPC、不需要前端改动**——前端本来就没用 content。如未来全文搜索 / 复制功能要用，再加 `get_chunk_content` 懒拉。

## What Changes

- **MODIFIED**：`cdt-core::AssistantResponse` 加 `#[serde(rename = "contentOmitted", default)] pub content_omitted: bool` 字段（与 `subagent-messages-lazy-load` / `session-detail-image-asset-cache` 同模式）。`content` 字段保留原类型（`MessageContent`）；`content_omitted = true` 表示首屏被裁剪，回滚时 `false`。
- **MODIFIED**：`ipc-data-api` capability 既有 `Expose project and session queries` Requirement —— `get_session_detail` 返回的 `SessionDetail.chunks` 中所有 `AIChunk.responses[i].content` 默认 SHALL 被替换为空 `MessageContent::Text("")`、`contentOmitted=true`。回滚开关 `OMIT_RESPONSE_CONTENT: bool = true` 一行切回。

本期 **不动** `session-display` capability——前端无任何代码读 `responses[].content`，行为不变。

## Capabilities

### New Capabilities

无

### Modified Capabilities

- `ipc-data-api`：`get_session_detail` 返回值修改（response.content 默认裁剪）。

## Impact

- **代码**
  - `crates/cdt-core/src/chunk.rs`：`AssistantResponse` 加 `content_omitted: bool`（`#[serde(default)]` 不破坏反序列化）；roundtrip 测试同步更新。
  - `crates/cdt-api/src/ipc/local.rs::get_session_detail`：序列化前遍历 chunks 内所有 `AIChunk.responses[]`，把 `content` 替换为空 `MessageContent::Text("")` + 设 `content_omitted=true`。顶部 `const OMIT_RESPONSE_CONTENT: bool = true` 回滚开关。覆盖嵌套 `subagent.messages` 内 `AIChunk.responses[]`（与 image OMIT 同模式：先 image OMIT，再 response.content OMIT，最后 subagent OMIT）。
- **依赖**：零新增。
- **前端**：零改动。`grep responses[i].content` 全 UI 无命中——`SessionDetail.chunkKey` / `SessionDetail.aiModel` / `SubagentCard` 都不读 content；显示文本来自 `semanticSteps`。
- **HTTP API**：HTTP path 同样 SHALL NOT 应用 `OMIT_RESPONSE_CONTENT` 裁剪（HTTP 当前无活跃用户，与 phase 2 / phase 3 同处理）。
- **测试**：
  - Rust 单元：`AssistantResponse` roundtrip 含新字段、`apply_response_content_omit` 清 user / AI / 嵌套 subagent.messages 三层。
  - perf bench 重跑确认收益：46a25772 case payload 3070 → ~1800 KB 量级。
- **回滚**：`OMIT_RESPONSE_CONTENT: bool = false` 即恢复完整 payload；前端零改动也不需要 fallback。
- **预期收益**：46a25772 IPC 2799 → ~1540 KB（-45%），按 6.5 KB/ms 算 IPC 427 → ~237 ms，first-paint 455 → ~265 ms。

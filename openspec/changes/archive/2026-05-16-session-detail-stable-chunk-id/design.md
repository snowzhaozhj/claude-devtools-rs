## Context

`SessionDetail.svelte` 当前用 `chunkKey(chunk, index)` 为 `{#each}` 生成 DOM key。PR #113 为避免重复 assistant response uuid 导致 Svelte duplicate-key 崩溃，在前端 key 末尾追加了 `index`。这能止血，但没有给 chunk 提供真正的身份标识：展开状态、滚动保存与搜索定位仍缺少一个后端保证稳定唯一的 chunk 级 id。

已知真实 session `daf7763e-32b8-47b1-8554-df507ccaf85a` 在 compact/replay 后会产生两个 `AIChunk`，且两者 `responses[0].uuid` 都是 `bec5c90e-c7b1-477c-b061-e69c2ee7a995`。因此 assistant response uuid 只能标识 response，不能标识 chunk。

## Goals / Non-Goals

**Goals:**

- 为 `SessionDetail.chunks` 的所有 chunk 输出 `chunkId`，同一 detail 内稳定且唯一。
- 让前端 chunk 级 key、展开状态、滚动保存、搜索定位优先使用 `chunkId`。
- 保持同一 session 文件未变化时多次 `get_session_detail` 的 `chunkId` 稳定。
- 保持 `openOrReplaceTab` 的 per-tab / per-session 状态隔离，不让旧 session 的状态写回污染新 session。

**Non-Goals:**

- 不改变 chunk 的顺序、合并算法或显示内容。
- 不修改 Tauri command 名称或 `get_session_detail` 参数。
- 不为 tool item、semantic step 或 search match 引入新的全局 id；本 change 只定义 chunk 级身份。

## Decisions

### D1: `chunkId` 成为后端 payload 字段

`Chunk` 各 variant 增加 `chunkId` / `chunk_id` 字段，并通过 serde camelCase 暴露为 `chunkId`。`cdt-core` 作为 chunk 类型源头持有该字段，`cdt-analyze` builder 在产 chunk 时填充，`cdt-api` 只做已有 omit/derived 处理，不在 IPC 层临时补 id。

候选方案：

- 在 `cdt-api::get_session_detail` 返回前遍历 chunks 注入 `chunkId`。优点是改动集中；缺点是 HTTP / IPC 外的 chunk 使用方看不到身份字段，也容易出现测试 fixture 与构建器语义脱节。
- 在前端生成 `chunkId`。优点是无需 Rust 类型变更；缺点是无法写 IPC contract，且多个前端调用点会重复实现稳定性规则。
- 在 `cdt-core`/builder 产物中携带 `chunkId`。这是本设计选择：身份字段与 chunk 生命周期绑定，所有调用方共享同一语义。

### D2: 非 AI chunk 使用自身 uuid

`UserChunk`、`SystemChunk`、`CompactChunk` 的 `chunkId` 使用各自已有 `uuid`。这些 chunk 本身就是单条消息或 compact 事件的直接投影，已有 uuid 能表达 chunk 身份。

### D3: `AIChunk` 使用 `firstResponseUuid + occurrenceOrdinal`

`AIChunk` 的 `chunkId` 使用首个 response uuid 加同 uuid 的 occurrence ordinal，例如 `ai:<firstResponseUuid>:<ordinal>`。ordinal 在单次 chunk 构建过程中按输出顺序、按 base id 计数，从 `0` 开始；同一 session 文件未变化时，builder 输入顺序不变，ordinal 也稳定。

候选方案：

- 只用 `responses[0].uuid`：已被 compact/replay 真实数据证明会重复，不能满足唯一性。
- 只用数组 index：能唯一但不表达身份，前面插入/删除 chunk 后会让旧展开状态、滚动锚点与搜索定位绑定到错误 chunk。
- builder 对所有 chunk 分配 `chunk:<index>`：实现统一，但本质仍是 index；除非额外混入消息 uuid，否则稳定性不足。
- `firstResponseUuid + occurrenceOrdinal`：保持与原始消息身份关联，同时只在重复 base id 时用 ordinal 消歧；这是本设计选择。

### D4: 前端以 `chunkId` 为 chunk 级唯一 key，item 级 key 保留局部后缀

`SessionDetail.svelte` 的顶层 `{#each detail.chunks as chunk}` SHALL 使用 `chunk.chunkId`。chunk 内部 tool / display item 的 key 可以继续在 `chunkId` 后追加 tool id、semantic step id 或局部 ordinal，因为这些 item 只需在 chunk 内唯一。

### D5: 状态迁移优先 `chunkId`，tab/session guard 保持不变

`expandedChunks`、chunk 级展开/折叠、滚动保存与搜索定位优先记录 `chunkId`。`onDestroy` 保存 scrollTop 时继续用当前 `tabId` 对应的 `sessionId` guard，避免 `openOrReplaceTab` 复用 tabId 换 session 时，旧实例把旧 session 的 scrollTop 写回新 session 状态。

## Risks / Trade-offs

- [Risk] 历史 fixture / 测试构造 Chunk 时漏填 `chunkId` 导致编译失败 → Mitigation：在同一轮修改所有构造点，优先用 `Option` 以外的必填字段让遗漏暴露在编译期。
- [Risk] `AIChunk` 没有 responses 的异常路径无法生成 `firstResponseUuid` → Mitigation：使用 chunk 内首个可用稳定消息 uuid，若确实不存在则使用 `ai:empty:<ordinal>` 这种按输出顺序消歧的 fallback，并用测试覆盖。
- [Risk] 前端仍有局部 key 使用 response uuid + index → Mitigation：顶层 chunk identity 全部切到 `chunkId`，局部 item key 只作为 chunk 内 key，不再承担 chunk 身份。
- [Risk] 新字段增加 IPC payload → Mitigation：每个 chunk 只增加几十字节字符串，相比已有 SessionDetail payload 可忽略；不触发 payload 瘦身模式。

## Migration Plan

1. OpenSpec delta 先定义 `chunkId` payload 与 UI 使用语义。
2. Rust 端更新 chunk 类型、builder 与 contract/integration test。
3. 前端同步类型、fixtures、SessionDetail key/state/search/scroll 与 Vitest/e2e。
4. 验证 `cargo test -p cdt-api --test ipc_contract`、`npm run check --prefix ui`、`npm run test:unit --prefix ui` 与相关 e2e/mock browser smoke。
5. PR push 后 wait-ci 与 codex 二审；不 merge。

## Open Questions

无。若实现中发现 `AIChunk` 存在无 response 的真实可达路径，按 D3 的 fallback 规则补测试并同步 spec delta。

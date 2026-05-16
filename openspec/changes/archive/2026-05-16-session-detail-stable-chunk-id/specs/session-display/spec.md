## ADDED Requirements

### Requirement: SessionDetail uses chunkId as chunk identity

SessionDetail SHALL 使用后端返回的 `chunk.chunkId` 作为 chunk 级身份标识。顶层 chunk `{#each}` key、chunk 级展开状态、滚动保存相关锚点和 chunk 级 DOM 标记 MUST 优先使用 `chunkId`，MUST NOT 继续依赖不保证全局唯一的 assistant response uuid；数组 index 仅可作为 chunk 内局部 item 的渲染后缀，不得作为 chunk 级长期身份。`openOrReplaceTab` 复用 `tabId` 切换 `sessionId` 时，旧 SessionDetail 实例保存状态 MUST 继续校验当前 `tabId` 仍指向同一 `sessionId`，避免旧 session 的展开或滚动状态写回污染新 session。

#### Scenario: 重复 response uuid 不导致 keyed each 崩溃

- **WHEN** SessionDetail 渲染两个 `AIChunk`，且两个 chunk 的 `responses[0].uuid` 相同但 `chunkId` 不同
- **THEN** 顶层 chunk keyed each SHALL 使用两个不同的 `chunkId`
- **AND** Svelte SHALL NOT 因 duplicate key 抛错或中断渲染

#### Scenario: chunk 级展开状态绑定 chunkId

- **WHEN** 用户展开一个 chunk 级可折叠区域
- **AND** SessionDetail 因 file-change silent refresh 收到相同 session 文件内容对应的新 `chunks` 数组
- **THEN** 展开状态 SHALL 通过 `chunkId` 重新匹配到同一 chunk
- **AND** 展开状态 SHALL NOT 因数组对象重建或重复 response uuid 丢失

#### Scenario: openOrReplaceTab 不污染新 session 状态

- **WHEN** `openOrReplaceTab` 复用同一个 `tabId`，把 active tab 从 session A 替换为 session B
- **AND** session A 的旧 SessionDetail 实例随后 destroy 并尝试保存 `expandedChunks` / `scrollTop`
- **THEN** 保存逻辑 SHALL 检查该 `tabId` 当前仍指向 session A 后才写回
- **AND** session B 的 `expandedChunks` / `scrollTop` SHALL NOT 被 session A 的旧状态覆盖

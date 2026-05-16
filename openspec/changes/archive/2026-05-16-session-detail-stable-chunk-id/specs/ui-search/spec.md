## ADDED Requirements

### Requirement: SessionDetail search preserves chunk identity across refresh

SessionDetail 搜索定位 SHALL 在需要引用 chunk 级位置时使用 `chunkId` 作为稳定身份。搜索栏因 `contentVersion` 变化重跑搜索时，匹配项可按 DOM 顺序重新编号，但任何 chunk 级锚点、滚动目标或测试辅助定位 MUST 使用 `chunkId`，MUST NOT 使用不保证唯一的 assistant response uuid 或纯数组 index 作为长期标识。

#### Scenario: 重复 response uuid 的 chunk 仍可搜索定位

- **WHEN** SessionDetail 中存在两个 `AIChunk`，它们的 `responses[0].uuid` 相同但 `chunkId` 不同
- **AND** 搜索 query 命中第二个 AI chunk 内的文本
- **THEN** 搜索滚动定位 SHALL 定位到第二个 chunk 对应的 DOM 区域
- **AND** 定位逻辑 SHALL NOT 因重复 response uuid 选中第一个 chunk

#### Scenario: silent refresh 后搜索重新匹配当前 chunkId

- **WHEN** SearchBar 处于可见且有 query 状态
- **AND** SessionDetail 因 file-change silent refresh 递增 `contentVersion`
- **THEN** SearchBar SHALL 重跑搜索并按最新 DOM 顺序更新匹配项
- **AND** chunk 级定位 SHALL 继续使用刷新后 DOM 上的 `chunkId`

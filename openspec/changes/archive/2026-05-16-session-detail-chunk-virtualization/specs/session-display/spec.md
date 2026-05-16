## ADDED Requirements

### Requirement: 长会话 Chunk 流虚拟化

SessionDetail SHALL 对长会话的主 `detail.chunks` 对话流进行虚拟化：当虚拟化启用且搜索未激活时，DOM 中只挂载与可视视口和 overscan 相交的 chunk，窗口外内容 SHALL 通过 spacer 元素维持完整滚动高度。虚拟化器 MUST 支持可变高度 chunk：先使用估算高度，再通过 `ResizeObserver` 记录已渲染行的真实高度。

#### Scenario: 长会话仅挂载可见窗口 chunk
- **WHEN** SessionDetail 渲染一个包含 200 个 chunk 且启用虚拟化的会话
- **THEN** `.conversation` DOM SHALL 只包含可见 chunk 与 overscan 行，而不是全部 200 个 chunk 行
- **AND** 滚动容器 SHALL 通过顶部和底部 spacer 保持等价于完整会话的滚动高度

#### Scenario: Chunk 高度测量更新 offset
- **WHEN** 一个虚拟化 chunk 行被挂载，且真实测量高度不同于估算高度
- **THEN** 虚拟化器 SHALL 按该 chunk key 保存真实测量高度
- **AND** 后续可见范围和 spacer 计算 SHALL 使用真实测量高度，而不是估算高度

#### Scenario: 工具展开更新虚拟行高度
- **WHEN** 用户在虚拟化行内展开或收起 AIChunk 工具区域
- **THEN** `ResizeObserver` SHALL 观察该行高度变化并更新虚拟化器测量值
- **AND** 相邻行 SHALL 保持原有顺序，不得重叠或留下持续空白

#### Scenario: Lazy markdown 高度变化被测量
- **WHEN** 虚拟化 chunk 内的 lazy markdown 占位进入视口，并渲染真实 markdown 或 Mermaid 内容
- **THEN** 该行高度变化 SHALL 被测量并反映到 spacer offset 中
- **AND** markdown SHALL 继续通过既有 lazy markdown 管线渲染，保留 XSS 清洗和语法高亮

#### Scenario: 搜索保留全文结果
- **WHEN** SessionDetail 搜索 UI 激活，或正在评估搜索 query
- **THEN** SessionDetail SHALL 渲染完整 chunk 流，或以其它方式保证所有 chunk 都可搜索
- **AND** 既有基于 DOM 的高亮与导航行为 SHALL 仍能找到虚拟窗口之外 chunk 中的匹配项

#### Scenario: 虚拟化下自动刷新保持贴底
- **WHEN** file-change 刷新开始时，用户位于虚拟化对话流底部
- **AND** `getSessionDetail` 返回追加内容
- **THEN** SessionDetail SHALL 在渲染后滚动到虚拟化流末尾，使最新 chunk 保持可见

#### Scenario: 用户查看历史时自动刷新不抢 scroll
- **WHEN** file-change 刷新开始时，用户没有位于虚拟化对话流底部
- **AND** `getSessionDetail` 返回新内容
- **THEN** SessionDetail SHALL NOT 强制滚动到底部
- **AND** 用户 SHALL 仍停留在接近原历史内容的位置，而不是丢失阅读位置

#### Scenario: Per-tab scroll 恢复保持隔离
- **WHEN** 用户切离某个 session tab 后再切回
- **THEN** SessionDetail SHALL 在虚拟化对话流中恢复该 tab 保存的 `scrollTop`
- **AND** 另一个显示同一 session 的 tab 或 pane SHALL NOT 继承这个 scroll 位置

#### Scenario: OpenOrReplace 重置陈旧虚拟状态
- **WHEN** `openOrReplaceTab` 复用既有 tab id 打开不同 `sessionId`
- **THEN** SessionDetail SHALL NOT 复用上一会话的虚拟化测量值、scroll offset 或展开行测量值
- **AND** 新会话 SHALL 根据自己的 chunks 与 per-tab UI 状态渲染

#### Scenario: 虚拟化回滚开关
- **WHEN** SessionDetail chunk 虚拟化回滚常量被关闭
- **THEN** SessionDetail SHALL 使用虚拟化前行为渲染完整 chunk 流
- **AND** 搜索、lazy markdown、工具展开和自动刷新 SHALL 继续工作，调用方无需分支判断

## ADDED Requirements

### Requirement: 多 Pane 并排时 SessionDetail 实例独立

当多个 pane 中各自打开 tab（含不同 pane 打开同一 sessionId 的场景）时，每个 pane 的 SessionDetail 实例 SHALL 独立渲染并维护各自 per-tab UI 状态（expandedChunks、expandedItems、searchVisible、contextPanelVisible、scrollTop）与 session 数据缓存（按 tabId 索引）。一个 pane 内的操作 SHALL NOT 影响另一 pane 的渲染结果。

#### Scenario: 同一 session 在两个 pane 各开一个 tab
- **WHEN** 用户通过 Sidebar "Open in New Pane" 或 tab 拖拽创建了两个 tab 指向同一 sessionId，分别位于 pane 1 与 pane 2
- **THEN** 两个 SessionDetail 实例 SHALL 各自独立渲染，expanded 状态 SHALL 各自独立

#### Scenario: pane A 滚动不影响 pane B
- **WHEN** 用户在 pane A 的 SessionDetail 滚动 conversation 区域
- **THEN** pane B 的 SessionDetail scrollTop SHALL 保持不变

#### Scenario: pane A 展开某 chunk 不影响 pane B
- **WHEN** 用户在 pane A 展开某 chunk 的工具执行详情
- **THEN** pane B 中对应 chunk（若同 tab 打开）的展开状态 SHALL 保持其自身值

#### Scenario: 关闭一个 pane 的 tab 不影响另一 pane
- **WHEN** 用户关闭 pane A 的某 tab（sessionId 同时在 pane B 的 tab 中打开）
- **THEN** pane B 的对应 SessionDetail SHALL 继续渲染，其 UI 状态与缓存 SHALL 不受影响

#### Scenario: 非 focused pane 的 file-change 自动刷新仍生效
- **WHEN** pane A 是 focused pane，pane B 打开了 sessionId=X 的 tab
- **AND** 后端 `file-change` 事件命中 sessionId=X
- **THEN** pane B 的 SessionDetail SHALL 按 `Auto refresh on file change` requirement 所述刷新，不因非 focused 而被跳过

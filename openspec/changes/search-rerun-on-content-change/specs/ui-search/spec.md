## MODIFIED Requirements

### Requirement: Cmd+F 激活会话内搜索

用户在 SessionDetail 视图中按 Cmd+F（或 Ctrl+F）SHALL 显示搜索栏。搜索栏 SHALL 出现在会话内容上方，输入框 SHALL 自动获得焦点。当用户在搜索框输入文本（300 ms debounce 后）触发 `doSearch` 时，系统 MUST 先把 conversation 容器内所有处于 lazy markdown 占位态的 chunk 强制渲染为真实 HTML，再调用 DOM `TreeWalker` 高亮匹配项 — 即匹配总数与全文文本一致，不受 lazy 视口渲染节奏影响。SearchBar 处于可见 + 有 query 状态时，若 conversation 容器内容因 file-change 自动刷新等原因发生变化（调用方通过 `contentVersion` prop 递增信号通知），SearchBar SHALL 自动重跑 `doSearch` 同步匹配索引，使新增 chunk 参与高亮、`totalMatches` 反映最新内容。

#### Scenario: 快捷键激活
- **WHEN** 用户在 SessionDetail 视图中按 Cmd+F 或 Ctrl+F
- **THEN** SearchBar SHALL 变为可见，输入框 SHALL 自动 focus 并 select 已有文本

#### Scenario: 重复按 Cmd+F
- **WHEN** SearchBar 已可见时用户再次按 Cmd+F
- **THEN** 输入框 SHALL 重新获得 focus 并 select 全部文本

#### Scenario: 搜索激活时全量 hydrate lazy markdown
- **WHEN** 用户在 SearchBar 输入查询触发 `doSearch`（无论首次输入或后续修改）
- **THEN** SearchBar SHALL 在调用 `highlightMatches` 之前先调用外部传入的 `onBeforeSearch` 回调
- **AND** SessionDetail 注入的 `onBeforeSearch` 实现 SHALL 调用 lazy markdown 控制器的 `flushAll()` 把所有 pending 占位同步渲染为真实 HTML
- **AND** 后续 `highlightMatches` 走 `TreeWalker` 时 conversation 容器内所有 chunk 的 markdown 文本节点 SHALL 已就绪，匹配数与全文一致

#### Scenario: 视口外 chunk 含唯一关键词时也能命中
- **WHEN** SessionDetail 含 96 条 chunk，唯一关键词 "uniquekeyword" 仅出现在第 80 条（首屏视口外、未渲染状态）
- **WHEN** 用户按 Cmd+F 输入 "uniquekeyword"
- **THEN** SearchBar 显示 `1 / 1`（命中 1 项）
- **AND** 第 80 条 chunk SHALL 已渲染为真实 HTML
- **AND** scrollIntoView SHALL 把当前匹配项滚动至视口中心

#### Scenario: file-change 后自动重搜同步索引
- **WHEN** SearchBar 处于可见状态、用户已输入非空 query 触发过 `doSearch`、有 N 个匹配项
- **WHEN** SessionDetail 因 file-change 触发 `refreshDetail` 拉到新 detail（含 M 个新增 chunk），并把 `contentVersion` 递增传给 SearchBar
- **THEN** SearchBar SHALL 自动重跑 `doSearch`：先调 `onBeforeSearch` 触发 lazy markdown `flushAll`（包含新增 chunk），再走 `clearHighlights` + `highlightMatches`，更新 `totalMatches` 为新内容的匹配总数
- **AND** 用户后续按 Enter / Shift+Enter 走 next / prev 时索引循环 SHALL 基于新总数

#### Scenario: SearchBar 不可见或 query 为空时 contentVersion 变化不触发重搜
- **WHEN** SearchBar 关闭（visible = false）或 query 为空字符串
- **WHEN** `contentVersion` 因 file-change 递增
- **THEN** SearchBar SHALL NOT 触发 `highlightMatches` 等 DOM 操作（避免无效计算）
- **AND** 当用户重新打开 SearchBar 输入 query 时，正常走首次 `doSearch` 流程

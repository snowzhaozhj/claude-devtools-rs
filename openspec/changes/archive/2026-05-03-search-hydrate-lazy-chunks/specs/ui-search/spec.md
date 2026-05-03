## MODIFIED Requirements

### Requirement: Cmd+F 激活会话内搜索

用户在 SessionDetail 视图中按 Cmd+F（或 Ctrl+F）SHALL 显示搜索栏。搜索栏 SHALL 出现在会话内容上方，输入框 SHALL 自动获得焦点。当用户在搜索框输入文本（300 ms debounce 后）触发 `doSearch` 时，系统 MUST 先把 conversation 容器内所有处于 lazy markdown 占位态的 chunk 强制渲染为真实 HTML，再调用 DOM `TreeWalker` 高亮匹配项 — 即匹配总数与全文文本一致，不受 lazy 视口渲染节奏影响。

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

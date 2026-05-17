## MODIFIED Requirements

### Requirement: 打开 session tab

用户从 Sidebar 点击会话时，系统 SHALL 在当前 focused pane 内打开一个 session tab。若该 sessionId 已有打开的 tab（无论在哪个 pane），系统 SHALL 切换焦点到已有 tab 所在 pane 并激活该 tab 而非创建重复 tab。新 tab 的 `label` 字段 SHALL 为 **完整的** session 标题（来自 `SessionSummary.title`，由后端按 `TITLE_MAX_CHARS = 500` 截断；前端 JS SHALL NOT 在此基础上再做任何不可逆截断），`id` SHALL 为唯一标识符。

视觉截断 SHALL 由 TabBar 渲染层的 CSS 实现：`.tab-label` 元素 SHALL 同时设置 `max-width`（合理的桌面 tab 视觉宽度，建议 150-200 px）+ `overflow: hidden` + `text-overflow: ellipsis` + `white-space: nowrap`。

Tab 容器 SHALL 在 `<button>` / `<span>` 等可 hover 的根元素上设置 HTML `title` 属性，值 SHALL 等于 **完整未截断的 tab label**，让浏览器原生 tooltip 在 hover 时显示全文。

`tabStore::shortLabel`（或等价的 JS 截断函数）SHALL 被移除，或改为透传 `(label) => label`；任何 `label.slice(0, N) + "…"` 形式的不可逆截断 SHALL NOT 出现在前端代码中——理由：JS 截断让 hover tooltip 也只能拿到截断版，造成信息丢失，无法通过拉宽 / hover 恢复。

#### Scenario: 首次打开 session
- **WHEN** 用户点击 Sidebar 中一个尚未打开的 session
- **THEN** 系统 SHALL 在 `focusedPaneId` 对应的 pane 中创建新 tab 并设为该 pane 的 activeTabId，对应 PaneView 的 TabBar SHALL 显示该 tab

#### Scenario: 重复点击已打开的 session（同 pane）
- **WHEN** 用户点击 Sidebar 中一个已在 focused pane 内的 session
- **THEN** 系统 SHALL 切换 focused pane 的 activeTabId 到该 tab，不创建新 tab

#### Scenario: 重复点击已打开的 session（其他 pane）
- **WHEN** 用户点击 Sidebar 中一个 tab 位于其他 pane 的 session
- **THEN** 系统 SHALL 把 `focusedPaneId` 切到该 tab 所在 pane，并将该 pane 的 activeTabId 设为该 tab，不创建新 tab

#### Scenario: Tab label 长度由后端控制 不再 JS 截断
- **WHEN** 后端 `SessionSummary.title` 长度为 120 字符
- **THEN** 对应新 tab 的 `label` 字段 SHALL 也是 120 字符（一字不少）
- **AND** TabBar 渲染时 SHALL 通过 CSS `max-width` + `text-overflow: ellipsis` 视觉截断超出部分
- **AND** 用户 hover tab 时浏览器原生 tooltip SHALL 显示完整 120 字符

#### Scenario: Tab tooltip 显示完整 label
- **WHEN** 任意 tab 的 label 含超出 CSS `max-width` 的内容
- **THEN** Tab 容器 HTML `title` 属性 SHALL 等于完整 `tab.label` 字符串
- **AND** 用户 hover 时 SHALL 看到完整字符串的原生 tooltip

#### Scenario: 不允许 JS 不可逆截断
- **WHEN** 在前端代码中搜索 `label.slice(0, ` / `label.substring(0, ` / `tab.label.slice` 等模式
- **THEN** SHALL NOT 出现任何作用于 `tab.label` 的不可逆字符截断（含 "…" 拼接）

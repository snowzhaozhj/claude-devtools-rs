## MODIFIED Requirements

### Requirement: Cmd+F 激活会话内搜索

用户在 SessionDetail 视图中按下 `keyboard-shortcuts` registry 的 `search.in-session` 当前 binding（默认 mac `⌘F` / Win+Linux `Ctrl+F`）SHALL 显示搜索栏。搜索栏 SHALL 出现在会话内容上方，输入框 SHALL 自动获得焦点。当用户在搜索框输入文本（300 ms debounce 后）触发 doSearch 时，系统 MUST 先把 conversation 容器内所有处于 lazy markdown 占位态的 chunk 强制渲染为真实 HTML，再走 DOM 文本节点遍历高亮匹配项 — 即匹配总数与全文文本一致，不受 lazy 视口渲染节奏影响。SearchBar 处于可见 + 有 query 状态时，若 conversation 容器内容因 file-change 自动刷新等原因发生变化，SearchBar SHALL 自动重跑搜索同步匹配索引，使新增 chunk 参与高亮、匹配总数反映最新内容。

`search.in-session` spec 的 `allowInInput` SHALL 为 `true`（当用户已在某 input focus 时按 Cmd+F 仍能调出搜索栏）；spec 的 handler SHALL 调用 `event.preventDefault()` 阻止浏览器默认 find 弹窗。该快捷键 SHALL 由用户在 `Settings → Keyboard Shortcuts` 中自定义。

#### Scenario: 快捷键激活

- **WHEN** 用户在 SessionDetail 视图中按下 `search.in-session` 当前 binding
- **THEN** registry dispatcher SHALL 命中 `search.in-session` spec
- **AND** SearchBar SHALL 变为可见，输入框 SHALL 自动 focus 并 select 已有文本

#### Scenario: 重复按 Cmd+F

- **WHEN** SearchBar 已可见时用户再次按 `search.in-session` 当前 binding
- **THEN** 输入框 SHALL 重新获得 focus 并 select 全部文本

#### Scenario: 搜索激活时全量 hydrate lazy markdown

- **WHEN** 用户在 SearchBar 输入查询触发 doSearch（无论首次输入或后续修改）
- **THEN** SearchBar SHALL 在执行高亮匹配之前先强制 SessionDetail 把所有 pending lazy markdown 占位同步渲染为真实 HTML
- **AND** 后续高亮遍历 conversation 容器时所有 chunk 的 markdown 文本节点 SHALL 已就绪，匹配数与全文一致

#### Scenario: 视口外 chunk 含唯一关键词时也能命中

- **WHEN** SessionDetail 含 96 条 chunk，唯一关键词 "uniquekeyword" 仅出现在第 80 条（首屏视口外、未渲染状态）
- **WHEN** 用户按 `search.in-session` 当前 binding 输入 "uniquekeyword"
- **THEN** SearchBar 显示 `1 / 1`（命中 1 项）
- **AND** 第 80 条 chunk SHALL 已渲染为真实 HTML
- **AND** SHALL 把当前匹配项滚动至视口中心

#### Scenario: file-change 后自动重搜同步索引

- **WHEN** SearchBar 处于可见状态、用户已输入非空 query 触发过搜索、有 N 个匹配项
- **WHEN** SessionDetail 因 file-change 触发刷新拉到新 detail（含 M 个新增 chunk）并通过 `contentVersion` 等价信号通知 SearchBar
- **THEN** SearchBar SHALL 自动重跑搜索：先触发 lazy markdown 全量渲染（包含新增 chunk），再清旧高亮 + 重高亮，更新匹配总数为新内容的命中数
- **AND** 用户后续按 Enter / Shift+Enter 走 next / prev 时索引循环 SHALL 基于新总数

#### Scenario: SearchBar 不可见或 query 为空时 contentVersion 变化不触发重搜

- **WHEN** SearchBar 关闭（visible = false）或 query 为空字符串
- **WHEN** `contentVersion` 因 file-change 递增
- **THEN** SearchBar SHALL NOT 触发高亮 / 任何 DOM 操作（避免无效计算）
- **AND** 当用户重新打开 SearchBar 输入 query 时，正常走首次搜索流程

#### Scenario: 用户自定义 binding 后生效

- **WHEN** 用户在 `Settings → Keyboard Shortcuts` 把 `search.in-session` 改为 `mod+shift+F`
- **THEN** 后续按下 `mod+shift+F` SHALL 显示 SearchBar
- **AND** 按下原默认 `mod+F` SHALL NOT 触发 SearchBar（除非另一 spec 占用了 `mod+F`）

### Requirement: Command Palette 触发

用户 SHALL 可以通过 `keyboard-shortcuts` registry 的 `command-palette.toggle` 当前 binding（默认 mac `⌘K` / Win+Linux `Ctrl+K`）在任意界面打开 Command Palette 模态面板。该 spec 的 `allowInInput` SHALL 为 `true`（input focus 时仍可调出）；handler SHALL `event.preventDefault()`；handler SHALL 实现 toggle 行为（已打开则关闭）。该快捷键 SHALL 由用户在 `Settings → Keyboard Shortcuts` 中自定义。

#### Scenario: Cmd+K 打开

- **WHEN** 用户按下 `command-palette.toggle` 当前 binding
- **THEN** registry dispatcher SHALL 命中 `command-palette.toggle` spec
- **AND** SHALL 弹出模态面板，搜索框自动聚焦

#### Scenario: Esc 关闭

- **WHEN** Command Palette 打开时用户按 Escape 或点击遮罩
- **THEN** 面板 SHALL 关闭，焦点回到之前的内容
- **AND** 该 Escape 处理 SHALL 由 Command Palette 自身 listener 处理（不通过 registry）

#### Scenario: 重复打开

- **WHEN** Command Palette 已打开时再次按 `command-palette.toggle` 当前 binding
- **THEN** SHALL 关闭面板（toggle 行为）

#### Scenario: 用户自定义 binding 后生效

- **WHEN** 用户在 `Settings → Keyboard Shortcuts` 把 `command-palette.toggle` 改为 `mod+P`
- **THEN** 后续按下 `mod+P` SHALL 切换 Command Palette
- **AND** 按下原默认 `mod+K` SHALL NOT 触发 Command Palette

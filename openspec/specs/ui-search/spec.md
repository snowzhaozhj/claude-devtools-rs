# ui-search Specification

## Purpose

定义前端两类搜索模式的行为契约：Cmd+F 会话内文本搜索（在 conversation 容器中以 `<mark>` 高亮、Enter / Shift+Enter 循环导航、跳过 `<pre>` / `<code>` 代码块）与 Cmd+K Command Palette 全局搜索（项目 + 会话组合视图、键盘导航、Tab 系统打开）。两套搜索均独立于后端 `session-search` capability，前者纯 DOM 操作、后者基于已加载的元数据本地过滤。
## Requirements
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

### Requirement: 会话内文本搜索与高亮

输入搜索文本后，系统 SHALL 在 conversation 容器的可见文本节点中查找所有匹配项，并通过 `<mark>` 元素高亮显示。搜索 SHALL 大小写不敏感。

#### Scenario: 输入搜索文本
- **WHEN** 用户在搜索框中输入文本（300ms debounce 后）
- **THEN** conversation 容器中所有匹配的文本片段 SHALL 被 `<mark>` 元素包裹高亮

#### Scenario: 大小写不敏感
- **WHEN** 搜索文本为 "error" 且内容中包含 "Error"、"ERROR"
- **THEN** 所有变体 SHALL 均被高亮

#### Scenario: 跳过代码块
- **WHEN** 搜索执行时
- **THEN** `<pre>`、`<code>`、`<style>`、`<script>` 标签内的文本 SHALL 不参与匹配

#### Scenario: 无匹配结果
- **WHEN** 搜索文本在 conversation 中无匹配
- **THEN** 搜索栏 SHALL 显示 "无结果"

### Requirement: 搜索结果导航

用户 SHALL 能通过 Enter/Shift+Enter 或导航按钮在匹配项间循环移动。当前匹配项 SHALL 有视觉区分，并自动滚动到视口中心。

#### Scenario: Enter 跳到下一个匹配
- **WHEN** 用户按 Enter
- **THEN** 当前索引 SHALL 前进到下一个匹配，该匹配项 SHALL 滚动到视口中心

#### Scenario: Shift+Enter 跳到上一个匹配
- **WHEN** 用户按 Shift+Enter
- **THEN** 当前索引 SHALL 回退到上一个匹配

#### Scenario: 循环导航
- **WHEN** 当前在最后一个匹配按 Enter
- **THEN** SHALL 回到第一个匹配（循环）

#### Scenario: 搜索计数显示
- **WHEN** 存在匹配结果
- **THEN** 搜索栏 SHALL 显示 "当前索引 / 总数" 格式（如 "3 / 12"）

#### Scenario: 当前匹配项高亮
- **WHEN** 导航到某个匹配项
- **THEN** 该 `<mark>` 元素 SHALL 带有 `data-search-current` 属性以区别于其他匹配

### Requirement: 关闭搜索栏

用户按 Esc 或点击关闭按钮 SHALL 关闭搜索栏，清除所有高亮，恢复原始文本。

#### Scenario: Esc 关闭
- **WHEN** 搜索栏可见时用户按 Esc
- **THEN** 搜索栏 SHALL 隐藏，所有 `<mark>` 高亮 SHALL 被移除并恢复原始文本节点，搜索查询 SHALL 被清空

#### Scenario: 点击关闭按钮
- **WHEN** 用户点击搜索栏的关闭按钮
- **THEN** 行为 SHALL 与按 Esc 相同

### Requirement: 搜索状态 per-tab 隔离

搜索可见性 SHALL 作为 per-tab UI 状态的一部分。切换 tab 时当前 tab 的搜索状态 SHALL 保存，切回时 SHALL 恢复。

#### Scenario: 切换 tab 时清理搜索
- **WHEN** tab A 有激活的搜索，用户切换到 tab B
- **THEN** tab A 的 searchVisible 状态 SHALL 保存，tab B SHALL 使用自己的搜索状态

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

### Requirement: Command Palette 搜索模式

Command Palette SHALL 以组合视图展示搜索结果：项目区 + 会话区。项目区为本地过滤。会话区在有查询时 SHALL 跨当前 active context 的**所有项目**做 sessionId 全局定位（不依赖当前是否选中项目），并保留"已选中项目时的组内正文搜索"。会话区展示的数据 SHALL 来自前端已加载的 `list_repository_groups` 快照，**不得**为渲染会话区触发任何会读取 session jsonl 的 metadata 扫描。

会话结果行 SHALL 显示足以跨项目定位的信息：title（已加载时）或 sessionId 前缀（无 title 时兜底）+ 所属项目名。打开某会话 SHALL 使用该结果行**自身**的所属项目，不得使用当前选中项目。

"全局"范围 SHALL 限定为当前 active context 的 group 快照：SSH 远程上下文下 SHALL NOT 包含未连接的其他 host。

#### Scenario: 项目过滤
- **WHEN** 用户输入文本
- **THEN** 项目区 SHALL 显示 displayName 或 path 包含查询文本的项目（大小写不敏感），最多 5 条

#### Scenario: 全局 sessionId 定位（跨所有项目）
- **WHEN** 用户输入长度 ≥ 4 的查询文本
- **THEN** 会话区 SHALL 显示当前 active context 下**所有项目**中 sessionId 包含该查询文本的会话（大小写不敏感），无论是否选中项目
- **AND** 结果 SHALL 按确定性顺序排序后再截断到上限（worktree 最近活动时间倒序，同值按项目名 + sessionId 稳定排序）

#### Scenario: 查询过短时不触发全局 id 定位并给出提示
- **WHEN** 查询文本非空但长度 < 4
- **THEN** 会话区 SHALL NOT 启用全局 sessionId 子串匹配（避免 hex id 海量命中）
- **AND** 会话区 SHALL 维持"已选中项目时组内搜索、未选中项目时为空"的行为
- **AND** 未选中项目时 SHALL 显示可见提示（例如"输入 ≥4 个字符按 Session ID 全局定位"），不留无解释空白

#### Scenario: 保留组内正文搜索且不回归
- **WHEN** 用户已选中项目并输入查询文本
- **THEN** 会话区 SHALL 仍包含该项目（组）内正文匹配的会话
- **AND** 该结果与全局 sessionId 命中合并展示

#### Scenario: 跨 worktree 同会话去重（worktree 级确定性）
- **WHEN** 同一 sessionId 在某 group 的多个 worktree 中出现且被同一查询命中
- **THEN** 会话区 SHALL 仅保留一条，不重复展示
- **AND** 保留版本 SHALL 按确定性规则选择（优先 main / repo-root worktree，否则取遍历顺序首条），不依赖前端不存在的 per-session 时间戳

#### Scenario: title 已加载时显示且不发起补齐 IPC
- **WHEN** 全局命中的会话恰在组件当前已加载的会话数据中（已带 title）
- **THEN** 该结果行 SHALL 显示该 title
- **AND** 系统 SHALL NOT 为会话区渲染调用 `listGroupSessions` / `getSessionSummariesByIds` 等补 title 的接口

#### Scenario: title 未加载时的 best-effort 展示
- **WHEN** 全局命中的会话其 title 不在组件已加载数据中
- **THEN** 该结果行 SHALL 显示 sessionId 前缀 + 所属项目名（及 worktree/branch）作为定位信息
- **AND** 系统 SHALL NOT 为补齐该 title 触发读取 jsonl 的 metadata 扫描

#### Scenario: 命中数超过上限时显式提示
- **WHEN** 会话区命中数超过展示上限
- **THEN** 会话区 SHALL 仅展示上限条数，并显示"仅显示前 N 条"之类的可见提示
- **AND** SHALL NOT 静默丢弃超出部分而不告知

#### Scenario: 按结果行自身项目打开会话
- **WHEN** 用户在会话区选中并打开一条跨项目命中的会话
- **THEN** 系统 SHALL 以该结果行自身所属的项目 / group 打开该会话
- **AND** SHALL NOT 以当前选中项目作为打开 scope

#### Scenario: 双路命中同会话合并后仍按自身归属打开
- **WHEN** 同一 sessionId 同时被全局 id 定位与当前组正文搜索命中并合并为一条
- **THEN** 合并条目 SHALL 保留正文匹配数（hits）
- **AND** 打开该条目 SHALL 使用其自身的 projectId / groupId，不因合并而错置 scope

#### Scenario: 已打开面板随数据刷新同步
- **WHEN** Command Palette 已打开期间发生 file-change 导致项目 / 会话列表刷新
- **THEN** 已打开面板的会话区 SHALL 反映刷新后的数据（新增会话可被定位、已删除会话不再出现）

#### Scenario: 后端正文搜索失败或滞后时不展示陈旧结果
- **WHEN** 用户修改查询后，上一查询的正文搜索结果尚未被新结果替换（搜索进行中或失败）
- **THEN** 会话区 SHALL NOT 把上一查询的正文命中当作当前查询结果展示
- **AND** 全局 sessionId 定位（纯前端）SHALL 不受后端搜索失败影响，仍可用

#### Scenario: 空查询
- **WHEN** 搜索框为空
- **THEN** SHALL 显示全部项目和（当前选中项目的）会话（受数量限制）

### Requirement: Command Palette 键盘导航

Command Palette 结果列表 SHALL 支持完整键盘导航。

#### Scenario: 上下键选择
- **WHEN** 用户按 ↓/↑
- **THEN** 选中高亮 SHALL 在结果列表中移动，跨越项目/会话两个区域

#### Scenario: Enter 选择项目
- **WHEN** 高亮项为项目时用户按 Enter
- **THEN** SHALL 选中该项目（Sidebar 切换）并关闭面板

#### Scenario: Enter 选择会话
- **WHEN** 高亮项为会话时用户按 Enter
- **THEN** SHALL 通过 Tab 系统打开该会话并关闭面板

#### Scenario: 查询变化重置选中
- **WHEN** 搜索文本变化
- **THEN** 选中索引 SHALL 重置为 0

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


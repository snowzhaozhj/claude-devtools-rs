# sidebar-navigation Specification

## Purpose

定义 Sidebar 的导航行为：项目选择、会话列表展示（日期分组 / 排序 / 过滤）、与 Tab 系统的联动、会话 Pin / Hide、右键菜单、宽度拖拽调整。同时覆盖骨架快速渲染、`session-metadata-update` 增量 patch、虚拟滚动等性能机制。多选 / 批量操作留作后续扩展。
## Requirements
### Requirement: 项目选择

Sidebar 顶部 SHALL 提供项目选择器。选择项目后 SHALL 自动加载该项目的会话列表。

#### Scenario: 初始加载
- **WHEN** 应用启动且有可用项目
- **THEN** 系统 SHALL 自动选中第一个项目并加载其会话列表

#### Scenario: 切换项目
- **WHEN** 用户从下拉选择器切换到另一个项目
- **THEN** 会话列表 SHALL 更新为新项目的会话，之前的列表 SHALL 被替换

#### Scenario: 无项目
- **WHEN** 无可用项目
- **THEN** Sidebar SHALL 显示空状态提示

### Requirement: 会话列表日期分组

会话列表 SHALL 按日期分组显示：TODAY、YESTERDAY、PREVIOUS 7 DAYS、OLDER。每个分组 SHALL 显示标签。空分组 SHALL 不显示。

#### Scenario: 今日会话分组
- **WHEN** 会话的 timestamp 在今天范围内
- **THEN** SHALL 归入 "TODAY" 分组

#### Scenario: 昨日会话分组
- **WHEN** 会话的 timestamp 在昨天范围内
- **THEN** SHALL 归入 "YESTERDAY" 分组

#### Scenario: 近 7 天分组
- **WHEN** 会话的 timestamp 在过去 2-7 天范围内
- **THEN** SHALL 归入 "PREVIOUS 7 DAYS" 分组

#### Scenario: 更早分组
- **WHEN** 会话的 timestamp 超过 7 天
- **THEN** SHALL 归入 "OLDER" 分组

#### Scenario: 空分组不显示
- **WHEN** 某个日期分组内无会话
- **THEN** 该分组标签和区域 SHALL 不渲染

### Requirement: 会话项展示

每个会话项 SHALL 显示标题和元数据（消息计数、相对时间、git 分支）。标题 SHALL 优先使用后端提供的 title 字段，无 title 时 fallback 到**完整 sessionId**——CSS 的 `text-overflow: ellipsis` 自然截断超出宽度的部分；同时 SHALL 在该元素上设置 HTML `title` 属性（`title || sessionId` 完整值），让用户 hover 时浏览器原生 tooltip 显示完整字符串。**禁止**在 JS 侧再手动 `slice(0, 8) + "…"`——双重截断让用户看到的是"前 8 字符 + …"既不能复制粘贴定位 session、也丢失了 CSS 自然 ellipsis 提供的 hover 全展能力。

消息计数（`SessionSummary.messageCount`）SHALL 等于该 session 文件中**真实 user-chunk 消息**与配对 assistant 消息的总数——后端 `cdt-api/src/ipc/session_metadata.rs::extract_session_metadata` MUST 用对齐原版 `claude-devtools/src/main/types/messages.ts::isParsedUserChunkMessage` 的过滤函数判定 user 消息：`category != User` 或 `is_meta = true` 或 `MessageContent::Blocks` 不含任何 `Text` / `Image` block（即纯 `tool_result`-only 行）SHALL NOT 计入。配对计数规则保持原状：每个 user-chunk 后，紧接的第一个非 synthetic 非 sidechain 的 assistant 消息计 1（与 `awaitingAIGroup` 状态机一致）。

git 分支（`SessionSummary.gitBranch`）SHALL 在每条会话项第二行 meta 末尾以 `· <GitBranch icon> {branch}` chip 形式渲染；`gitBranch` 为 `null` 时 SHALL NOT 渲染该 chip（不留分隔符 `·`、不留空位）。该 chip MUST 跟随 `session-metadata-update` 事件 patch 的 `gitBranch` 即时更新。

#### Scenario: 有标题的会话
- **WHEN** SessionSummary.title 非空
- **THEN** SHALL 显示 title，文本溢出时由 CSS `text-overflow: ellipsis` 自动截断；HTML `title` 属性 SHALL 等于完整 title 让 hover 显示

#### Scenario: 无标题的会话
- **WHEN** SessionSummary.title 为 null
- **THEN** SHALL 显示**完整 sessionId**（CSS ellipsis 截断超出部分）；HTML `title` 属性 SHALL 等于完整 sessionId 让 hover 显示
- **AND** SHALL NOT 显示 "前 8 字符 + …" 形式的 JS 手动截断结果

#### Scenario: 元数据显示
- **WHEN** 会话项渲染，`gitBranch` 为 null
- **THEN** SHALL 显示消息计数（`<MessageSquare icon> {N}` 格式）和相对时间（"刚刚"/"Nm"/"Nh"/"Nd"/日期），中间用 `·` 分隔

#### Scenario: 元数据含 git 分支
- **WHEN** 会话项渲染，`gitBranch = "feat/x"`
- **THEN** SHALL 在 messageCount + 时间之后追加 `· <GitBranch icon> feat/x`

#### Scenario: 消息计数排除 tool_result-only user 行
- **WHEN** session JSONL 含 1 条真实用户输入（`{role:"user", content:"hi"}`）+ 1 条 assistant tool_use + 1 条 user tool_result（`{role:"user", content: [{type:"tool_result", ...}]}`）+ 1 条 assistant 收尾
- **THEN** `extract_session_metadata` 返回的 `messageCount` SHALL 为 `2`（真实 user + 配对 assistant），**不**计入 tool_result-only 行

#### Scenario: 消息计数包含含 text+tool_result 混合 user 行
- **WHEN** user 消息 `MessageContent::Blocks` 同时含 `Text` block 与 `ToolResult` block
- **THEN** SHALL 计入 messageCount（与原版 `isParsedUserChunkMessage` 行为一致，"Must contain text or image blocks"）

#### Scenario: 消息计数包含 image-only user 行
- **WHEN** user 消息 `MessageContent::Blocks` 只含 `Image` block（用户粘贴截图，无文字）
- **THEN** SHALL 计入 messageCount

#### Scenario: 消息计数排除 is_meta=true 的 user 行
- **WHEN** user 消息 `is_meta = true`
- **THEN** SHALL NOT 计入 messageCount

### Requirement: 会话过滤

Sidebar SHALL 提供搜索输入框用于过滤会话列表。过滤 SHALL 基于 title 或 sessionId 的大小写不敏感子串匹配。

#### Scenario: 输入过滤文本
- **WHEN** 用户在搜索框中输入文本
- **THEN** 会话列表 SHALL 只显示 title 或 sessionId 包含该文本的会话（忽略大小写）

#### Scenario: 清空过滤
- **WHEN** 用户清空搜索框
- **THEN** SHALL 恢复显示所有会话

#### Scenario: 过滤计数
- **WHEN** 过滤激活时
- **THEN** SHALL 显示 "匹配数/总数" 格式的计数

#### Scenario: 无匹配结果
- **WHEN** 过滤文本无匹配
- **THEN** SHALL 显示 "无匹配会话" 提示

### Requirement: 会话选择与 Tab 联动

点击会话项 SHALL 通过 Tab 系统在 focused pane 内打开或切换到对应 session。Sidebar 高亮 SHALL 跟随 focused pane 的 activeTab 的 sessionId。

#### Scenario: 点击会话打开 tab
- **WHEN** 用户点击一个会话项
- **THEN** 系统 SHALL 调用 Tab 系统的 openTab，该 openTab 的作用域 SHALL 为 `focusedPaneId` 对应 pane（具体 tab 生命周期行为由 tab-management spec 定义）

#### Scenario: 高亮跟随 focused pane 的 activeTab
- **WHEN** focused pane 的 activeTabId 变化（无论通过 Sidebar 点击、TabBar 点击、跨 pane focus 切换还是快捷键）
- **THEN** Sidebar 中对应 sessionId 的会话项 SHALL 高亮，之前的高亮 SHALL 移除

#### Scenario: 无 active tab 时无高亮
- **WHEN** focused pane 的 activeTabId 为 null
- **THEN** Sidebar 中 SHALL 无会话项高亮

### Requirement: 加载状态

项目和会话加载过程中 SHALL 显示 loading 状态。加载失败 SHALL 不崩溃且显示错误提示。

#### Scenario: 项目加载中
- **WHEN** 项目列表正在加载
- **THEN** Sidebar SHALL 显示 "加载中..." 提示

#### Scenario: 会话加载中
- **WHEN** 切换项目后会话列表正在加载
- **THEN** 会话区域 SHALL 显示 "加载中..." 提示

#### Scenario: 加载失败
- **WHEN** API 调用失败
- **THEN** 会话列表 SHALL 为空且不显示崩溃界面，错误 SHALL 记录到 console

### Requirement: 会话置顶（Pin）

用户 SHALL 可以将会话置顶。置顶的会话 SHALL 在日期分组之前以独立 "PINNED" 分区显示。Pin 状态为 per-project 内存级。

#### Scenario: 置顶会话
- **WHEN** 用户通过右键菜单选择"置顶会话"
- **THEN** 该会话 SHALL 出现在 PINNED 分区顶部，带蓝色 pin 图标

#### Scenario: 取消置顶
- **WHEN** 用户通过右键菜单选择"取消置顶"
- **THEN** 该会话 SHALL 从 PINNED 分区移除，回到日期分组中对应位置

#### Scenario: Pin 排序
- **WHEN** 多个会话被置顶
- **THEN** 最新置顶的 SHALL 排在最前面（prepend 顺序）

#### Scenario: 无置顶时不显示分区
- **WHEN** 当前项目无置顶会话
- **THEN** PINNED 分区标签和区域 SHALL 不渲染

### Requirement: 会话隐藏（Hide）

用户 SHALL 可以隐藏会话。隐藏的会话默认不显示在列表中。用户 SHALL 可以通过切换按钮临时查看隐藏会话。Hide 状态为 per-project 内存级。

#### Scenario: 隐藏会话
- **WHEN** 用户通过右键菜单选择"隐藏会话"
- **THEN** 该会话 SHALL 从列表中消失（默认模式下）

#### Scenario: 取消隐藏
- **WHEN** 用户在 showHidden 模式下通过右键菜单选择"取消隐藏"
- **THEN** 该会话 SHALL 恢复正常显示

#### Scenario: 显示隐藏会话切换
- **WHEN** hiddenCount > 0 时
- **THEN** filter bar SHALL 显示眼睛图标按钮，点击 SHALL 切换 showHidden 模式

#### Scenario: 隐藏会话视觉
- **WHEN** showHidden 模式开启
- **THEN** 被隐藏的会话 SHALL 以 50% opacity 显示在列表中

### Requirement: 右键菜单

Sidebar 会话项 SHALL 支持右键打开上下文菜单，提供快捷操作。菜单 SHALL 包含 "Open in New Pane"（当 `paneLayout.panes.length < 4` 时可用）操作，用于在当前 focused pane 右侧创建新 pane 打开该 session。

#### Scenario: 打开右键菜单
- **WHEN** 用户在会话项上右键点击
- **THEN** SHALL 在点击位置显示浮层菜单，菜单包含：在新标签页打开、Open in New Pane、置顶/取消置顶、隐藏/取消隐藏、复制 Session ID、复制恢复命令

#### Scenario: Open in New Pane
- **WHEN** 用户在会话项右键菜单选择 "Open in New Pane"
- **AND** `paneLayout.panes.length < 4`
- **THEN** 系统 SHALL 在 focused pane 右侧创建一个新 pane，在该 pane 中打开目标 session 的 tab，新 pane SHALL 成为 focused

#### Scenario: Open in New Pane 达到上限时禁用
- **WHEN** `paneLayout.panes.length === 4`
- **THEN** 右键菜单的 "Open in New Pane" 项 SHALL 显示为禁用状态（灰色 + 不可点击）

#### Scenario: 菜单定位 clamping
- **WHEN** 右键位置靠近视口右边界或下边界
- **THEN** 菜单位置 SHALL clamp 到视口内（8px 内边距）

#### Scenario: 菜单关闭
- **WHEN** 用户点击菜单外区域或按 Escape
- **THEN** 菜单 SHALL 关闭

#### Scenario: 复制操作反馈
- **WHEN** 用户选择复制 Session ID 或复制恢复命令
- **THEN** SHALL 复制到剪贴板，菜单项文本 SHALL 变为 "已复制!" 并在 600ms 后自动关闭菜单

### Requirement: 宽度拖拽调整

Sidebar 右边缘 SHALL 可拖拽调整宽度。

#### Scenario: 拖拽调整宽度
- **WHEN** 用户在 Sidebar 右边缘按下并拖动鼠标
- **THEN** Sidebar 宽度 SHALL 跟随鼠标 X 坐标实时变化

#### Scenario: 宽度范围限制
- **WHEN** 拖拽时鼠标超出范围
- **THEN** 宽度 SHALL 限制在 200px ~ 500px 之间

#### Scenario: 拖拽视觉提示
- **WHEN** 鼠标悬停或拖拽 resize handle
- **THEN** handle SHALL 显示蓝色半透明高亮，鼠标光标 SHALL 变为 col-resize

### Requirement: Auto refresh session list on file change

当后端 `file-change` 事件命中**当前选中的项目**时，Sidebar SHALL 重拉
`listSessions` 刷新会话列表，无论命中事件中的 `sessionId` 是否已存在于现有
列表（覆盖"新会话首次写入"场景）。同一 project 短时间内多次事件 SHALL 合并
为一次 `listSessions` 调用。

#### Scenario: 当前 project 命中时刷新列表
- **WHEN** 用户当前选中 `selectedProjectId = "projectA"`
- **AND** 收到 `file-change` payload `{ projectId: "projectA",
  sessionId: <任意>, deleted: false }`
- **THEN** Sidebar SHALL 调用 `listSessions("projectA")` 并替换 `sessions`
  状态

#### Scenario: 非当前 project 命中时不刷新
- **WHEN** 用户当前选中 `selectedProjectId = "projectA"`
- **AND** 收到 `file-change` payload `{ projectId: "projectB", ... }`
- **THEN** Sidebar SHALL NOT 触发 `listSessions`

#### Scenario: 新 session 写入时出现在列表
- **WHEN** `~/.claude/projects/projectA/` 下首次创建一个新 session 文件
  `<newSid>.jsonl` 并写入第一行
- **AND** 用户当前选中 `selectedProjectId = "projectA"`
- **THEN** 该 `newSid` 对应的 SessionSummary SHALL 出现在 Sidebar 列表中
  （根据 timestamp 落到对应日期分组）

#### Scenario: 同 project 多次 file-change 合并刷新
- **WHEN** 同一 project 在 < 200 ms 内连续收到 3 次 `file-change` 事件
- **THEN** Sidebar SHALL 只发起 1 次 `listSessions` IPC 调用

#### Scenario: 删除事件也触发刷新
- **WHEN** 收到 `file-change` payload `{ projectId: "projectA",
  sessionId: "sessionX", deleted: true }`，且 `selectedProjectId = "projectA"`
- **THEN** Sidebar SHALL 触发 `listSessions("projectA")` 让 `sessionX` 从
  列表中消失

#### Scenario: 切换 project 后旧 project 的事件不再刷新
- **WHEN** 用户已经从 `projectA` 切到 `projectB`
- **AND** 此时延迟到达一条 `projectA` 的 `file-change` 事件
- **THEN** Sidebar SHALL NOT 调用 `listSessions("projectA")`（handler 在
  `selectedProjectId` 变化时已经按新值重新注册）

### Requirement: Ongoing indicator on session item

Sidebar 的每一行 session item SHALL 在 `session.isOngoing === true`
时于标题前渲染一枚绿色脉冲圆点 `<OngoingIndicator size="sm" />`。
圆点 SHALL 出现在 pin 图标（如有）之前；`isOngoing` 为 false /
undefined 时 SHALL NOT 占位。该渲染规则 MUST 同时作用于 PINNED
分区与日期分组（TODAY / YESTERDAY / …）两处 session 列表。

#### Scenario: Ongoing session shows pulsing dot
- **WHEN** `SessionSummary.isOngoing === true` 且该 session 出现在
  日期分组内
- **THEN** sidebar 对应一行 SHALL 在标题文本前渲染一枚绿色圆点，
  圆点 SHALL 带脉冲动画（`animate-ping` 或等价 CSS）

#### Scenario: Finished session shows no dot
- **WHEN** `SessionSummary.isOngoing === false`
- **THEN** sidebar 行 SHALL NOT 渲染圆点，其他视觉元素（pin 图标、
  标题、元数据行）位置 SHALL 与当前表现一致

#### Scenario: Dot appears in PINNED section too
- **WHEN** 一个被 pin 的 session 同时 `isOngoing === true`
- **THEN** PINNED 分区内该条目 SHALL 同时显示绿色圆点与蓝色 pin 图标，
  圆点 SHALL 位于 pin 图标之前

#### Scenario: Indicator updates after auto refresh
- **WHEN** 某个 session 的 `isOngoing` 因 `listSessions` 自动刷新
  从 `true` 变为 `false`
- **THEN** 对应 sidebar 行的绿点 SHALL 在该次刷新的下一帧被移除

### Requirement: 骨架列表快速加载

Sidebar 切换项目或初次加载时 SHALL 在骨架数据（`sessionId` / `projectId` / `timestamp`）到达后立即渲染完整列表骨架；元数据字段（`title` / `messageCount` / `isOngoing`）pending 时 SHALL 使用占位回退（fallback 到 sessionId 前 8 位 + "…" / `C` 空计数 / 无 ongoing 圆点）。元数据 pending 期间 SHALL NOT 显示"加载中..."遮罩，以避免阻挡已可交互的会话项。

#### Scenario: 骨架到达后立即渲染列表

- **WHEN** `listSessions(projectId)` 返回的 `SessionSummary[]` 已填充 `sessionId` / `timestamp` 但 `title` 为 null、`messageCount` 为 0、`isOngoing` 为 false
- **THEN** Sidebar SHALL 立即渲染会话项（按 timestamp 分组），每项标题 SHALL fallback 到 `sessionId` 前 8 位加 "…"，元数据行 SHALL 显示 `C`（无计数）+ 相对时间

#### Scenario: 骨架态不显示加载中遮罩

- **WHEN** 骨架数据已返回但元数据 patch 尚未全部到达
- **THEN** 会话列表区域 SHALL NOT 显示 "加载中..." 文字；列表 SHALL 展示已有骨架

#### Scenario: 仅骨架未返回时才显示加载中

- **WHEN** 切换项目后 `listSessions` 首次调用尚未 resolve（骨架未到）
- **THEN** 会话列表区域 SHALL 显示 "加载中..." 文字，直至骨架返回

### Requirement: 会话元数据增量 patch

Sidebar SHALL 订阅后端 `session-metadata-update` 事件，按 `sessionId` 定位列表中的 `SessionSummary` 并 in-place 替换其 `title` / `messageCount` / `isOngoing` / `gitBranch` 字段。patch 操作 SHALL 保持 `{#each}` 的稳定 key（`sessionId`）不变，避免整行 DOM 重建。非当前 `selectedProjectId` 的 event SHALL 被忽略。

Sidebar SHALL 维护一个 `pendingMetadataUpdates: Map<sessionId, SessionMetadataUpdate>` 缓冲区——listener 每收到一条 update 都 SHALL 写入该 buffer（按 sessionId 覆盖最新值），**无论**当前 `sessions` 数组是否已包含该 sessionId。`sessions` 数组每次写入（非 silent 加载首页 / silent 刷新 / `loadMoreSessions` 翻页扩展）后 SHALL 立即对新数组应用 buffer 中匹配 sessionId 的 update。这是兜底 broadcast 在 IPC return 之前到达时 `sessions.map` 找不到目标的 race——`broadcast::Sender::send` 在前端 listener 已订阅但 sessions 数组还没扩展到新页时，update 会静默丢失（broadcast 不重发），导致 session 永远卡在 sessionId 占位。

切 project / 首次加载（非 silent 路径）SHALL 在调用 `await listSessions(...)` **之前**清空 `pendingMetadataUpdates`，避免旧 project 的 update 残留；同时这一 clear SHALL 早于 await 阻塞窗口，让 listener 在 `await listSessions(...)` 期间收到的新 project update 能被 buffer 保留并在后续 `applyPendingMetadata` 应用上去——后端 `list_sessions` 在 IPC return 之前已 spawn 扫描任务并可能 broadcast emit，clear 若放在 await 后会把这些"早到的"新 project update 一起清掉。silent 刷新与 loadMore SHALL NOT 清空 buffer（buffer 中已有的 update 仍可能匹配 prev sessions 中尚未 patch 的 sessionId）。

`loadSessions(projectId, silent=true)` 路径（file-change 触发或用户点击"有更新"按钮）SHALL 把第一页结果合并到现有 `sessions` 数组而非整体替换：prev 中超出第一页（cursor 之后）的尾部 sessions SHALL 被保留；prev 中与新第一页 sessionId 相同的条目 SHALL 保留已 patch 元数据（与既有"silent 刷新保留已获取元数据"语义一致）。silent 路径 SHALL NOT 重置 `sessionsNextCursor`，保留用户已翻到的分页位置。

非 silent 路径（用户切 project / 首次加载）行为不变：仍然替换式加载第一页，`sessionsNextCursor` 取本次响应的 `nextCursor`。

#### Scenario: 元数据事件按 sessionId 匹配并 patch

- **WHEN** 当前 `selectedProjectId = "projectA"`，前端收到 payload `{ projectId: "projectA", sessionId: "s1", title: "重构 auth", messageCount: 42, isOngoing: false }`
- **THEN** Sidebar SHALL 找到 `sessions[i].sessionId === "s1"` 的条目，将其 `title` 更新为 "重构 auth"、`messageCount` 更新为 42、`isOngoing` 更新为 false；其他条目 SHALL 不变

#### Scenario: 元数据事件不改变列表顺序或重建 DOM

- **WHEN** 一条 `session-metadata-update` patch 到达
- **THEN** 被 patch 的会话项 SHALL 保持在原位置，DOM 节点 SHALL 被复用（Svelte `{#each}` 的 `(session.sessionId)` key 保障），不触发 OngoingIndicator 动画重启或 pin 图标闪烁

#### Scenario: 非当前 project 的事件被忽略

- **WHEN** 当前 `selectedProjectId = "projectA"`，收到 payload `{ projectId: "projectB", sessionId: "sX", ... }`
- **THEN** Sidebar SHALL NOT 修改本地 `sessions` 状态

#### Scenario: 更新到达时 sessions 还未包含 sessionId 时缓冲到 pending buffer

- **WHEN** Sidebar 已加载 page 1（20 条），用户滚动触发 `loadMoreSessions` 启动 page 2 的 `list_sessions` IPC；后端 page 2 的扫描任务先于 IPC return 完成对 `sessionId = "s_new"`（page 2 尾部一条）的 metadata 扫描并 broadcast emit
- **AND** 前端 listener 收到 `s_new` 的 update 时 `sessions` 数组仍为 page 1 的 20 条（不含 `s_new`）
- **THEN** listener SHALL 把 update 写入 `pendingMetadataUpdates`，且对当前 `sessions` 跑一遍 `map`（无效 patch，因为 sessionId 不在）
- **AND** 当 page 2 IPC return 后 `sessions = mergeSessions(prev, result.items, false)` 写入完成，Sidebar SHALL 立即对新数组应用 buffer 中 `s_new` 的 update，使 `s_new` 立即显示真实 title 而非占位

#### Scenario: 切 project 时在 await 之前清空 pending buffer

- **WHEN** 当前 `selectedProjectId = "projectA"`，`pendingMetadataUpdates` 缓冲了若干 projectA 的 update；用户切到 `selectedProjectId = "projectB"`
- **THEN** `loadSessions("projectB", silent=false)` 进入时 SHALL 在调用 `await listSessions(...)` **之前** `pendingMetadataUpdates.clear()`
- **AND** clear 之后 listener 在 `await listSessions("projectB", ...)` 阻塞期间收到的 projectB update SHALL 被 buffer 保留下来；非 silent 路径的 `applyPendingMetadata(fresh, pendingMetadataUpdates)` 会在 IPC return 后立即应用这些"早到的" update，让 projectB 中后端先扫到的 session 不会卡占位
- **AND** clear 放在 `await listSessions(...)` 之**后**是 bug：会把 await 期间到达的 projectB update 一并清掉，等于绕过 race buffer 修复

#### Scenario: file-change silent 刷新保留已获取元数据

- **WHEN** file-change 触发 `loadSessions(projectId, silent=true)` 并返回新骨架（title/messageCount/isOngoing 全部重置为占位）
- **THEN** Sidebar SHALL 按 `sessionId` 将旧 `sessions` 的元数据字段 merge 进新骨架（旧有值的 session 元数据字段不被重置为占位），直到新的 `session-metadata-update` 到达再覆盖

#### Scenario: silent 刷新保留尾部已翻页 sessions

- **WHEN** 用户已通过 `loadMoreSessions` 翻页加载到 `sessions.length === 60`（首页 20 + 第二页 20 + 第三页 20），随后 file-change 触发 `loadSessions(projectId, silent=true)`，silent 请求只返回第一页的 20 条
- **THEN** silent 刷新完成后 `sessions.length` SHALL ≥ 60（含 prev 中超出第一页的所有 sessionId）；前 20 条按合并后 `timestamp` 倒序，prev 中 sessionId 也出现在新第一页的条目 SHALL 保留 prev 已 patch 的元数据
- **AND** `sessions.length` SHALL NOT 在 silent 刷新后瞬间缩水到 20 余条又被 `maybeLoadMoreSessions` 补回——这是"计数来回跳变"反模式

#### Scenario: silent 刷新不重置分页 cursor

- **WHEN** 用户已翻到第三页（`sessionsNextCursor === cursor3`），silent 刷新返回 `result.nextCursor === cursor1`
- **THEN** silent 完成后 `sessionsNextCursor` SHALL 仍为 `cursor3`，下一次 `loadMoreSessions` 用 `cursor3` 请求未看过的第四页，而非用 `cursor1` 重复请求已加载的第二页

#### Scenario: silent 刷新不丢失任何 prev sessionId

- **WHEN** silent 刷新（含 file-change 触发与"有更新"按钮触发两条入口）合并第一页结果到 prev sessions
- **THEN** 合并后 `sessions` SHALL 包含 prev 中所有 `sessionId`（无论该 sessionId 是否出现在新第一页响应里），保证 prev 已渲染会话项的 `{#each (item.key)}` 节点在 DOM 中被复用、`scrollTop` 锚定的会话项仍可定位
- **AND** 滚动位置不变的视觉约束 SHALL 由本 Scenario（合并不丢条目）联合既有 Scenario "file-change 刷新保持滚动位置"（`scrollTop` 不重置）共同保证；Sidebar SHALL NOT 在 silent 刷新完成后自动 `scrollTo({ top: 0 })`

### Requirement: 会话列表虚拟滚动承载

Sidebar 会话列表 SHALL 以 windowing 方式渲染：仅渲染视口内及上下 overscan 区间内的列表项。所有列表项（PINNED 分区头 + pinned sessions + 日期分组头 + 各日期分组内的 sessions）SHALL 摊平为单一固定行高的 flat 列表参与同一 windowing 单元。滚动位置变化时 SHALL 不引发会话项 DOM 整块重建（依赖 `{#each}` 稳定 key + 上下 spacer 占位元素）。

#### Scenario: 超出视口的会话项不渲染

- **WHEN** 当前项目有 200 个可见会话，视口高度可容纳 20 项
- **THEN** DOM 内实际渲染的 `session-item` 节点数 SHALL 为 `20 + 2 * overscan`（overscan 固定 5，即 30 个以内），其余位置 SHALL 由占位（spacer）高度填充

#### Scenario: 滚动不重建复用节点

- **WHEN** 用户滚动列表
- **THEN** 已渲染的会话项 DOM 节点 SHALL 被 Svelte `{#each (item.key)}` 复用（同 sessionId 进入视口时重用原节点），不触发 OngoingIndicator 的 `animate-ping` 动画重启

#### Scenario: 分组头滚动出视口时同步裁剪

- **WHEN** 用户向下滚动到某个分组头已离开视口
- **THEN** 该 `date-group-label` 元素 SHALL 与同步滚出的 session 项一起退出 DOM（参与 windowing 而非 sticky 渲染），spacer 高度 SHALL 正确反映被裁剪的总高度

#### Scenario: 高亮项位于视口外不触发自动滚动

- **WHEN** `activeSessionId` 对应的会话项当前不在视口内
- **THEN** Sidebar SHALL NOT 自动滚动到该项；用户滚动到该位置时 SHALL 正确显示高亮样式

#### Scenario: file-change 刷新保持滚动位置

- **WHEN** file-change 触发 `silent=true` 刷新并替换 sessions
- **THEN** 视口滚动位置 SHALL 保持不变（`scrollTop` 不重置）

### Requirement: 侧栏折叠/展开

Sidebar SHALL 支持折叠（隐藏）与展开两种状态。折叠状态由 `sidebarStore.svelte.ts` 的模块级 runes state 管理（内存级，重启回归默认展开）。

折叠入口 SHALL 提供两条：(1) SidebarHeader 顶部右侧 `PanelLeft` icon 按钮，点击切换；(2) 全局 `Cmd+B`（macOS）/ `Ctrl+B`（其他平台）快捷键 SHALL 切换。展开入口 SHALL 提供 (1) 折叠态下 TabBar 最左侧 `PanelLeft` icon 按钮；(2) 同一快捷键。

折叠时 sidebar SHALL 完全不渲染（不留窄轨道、不留 0 宽度占位 DOM）。展开时 sidebar SHALL 恢复折叠前的宽度（如未拖拽过则为默认宽度）。

#### Scenario: 默认展开

- **WHEN** 应用首次启动
- **THEN** Sidebar SHALL 处于展开状态，宽度为默认值（280px）

#### Scenario: 折叠按钮隐藏 Sidebar

- **WHEN** 用户点击 SidebarHeader 顶部 `PanelLeft` 按钮
- **THEN** Sidebar 整体 DOM SHALL 不再渲染；TabBar 最左侧 SHALL 出现展开按钮

#### Scenario: 展开按钮恢复 Sidebar

- **WHEN** 折叠态下用户点击 TabBar 最左侧 `PanelLeft` 按钮
- **THEN** Sidebar SHALL 重新渲染，宽度恢复为折叠前的值

#### Scenario: 快捷键切换

- **WHEN** 用户按 `Cmd+B`（macOS）或 `Ctrl+B`（其他平台）
- **THEN** 折叠状态 SHALL 切换（展开 ↔ 折叠），等价于点击 PanelLeft 按钮

#### Scenario: 快捷键在折叠态下仍生效

- **WHEN** Sidebar 当前折叠
- **AND** 用户按 `Cmd+B`
- **THEN** Sidebar SHALL 重新展开（快捷键监听挂在 App 顶层，不依赖 Sidebar 自身渲染）

#### Scenario: 重启后回归展开

- **WHEN** 用户折叠 Sidebar 后关闭应用并重新启动
- **THEN** Sidebar SHALL 处于展开状态（折叠状态不持久化，与 sidebar 宽度同维度）

### Requirement: 默认渲染按仓库聚合的 Sidebar

Sidebar SHALL 默认调用 `list_repository_groups()` IPC 拉取按 git 仓库聚合的项目列表，把同一仓库的多个 worktree 折叠为单个可展开行。`RepositoryGroup` 渲染为顶层条目，含仓库名（`group.name`）与 worktree 数量徽章；单成员 group（只含一个 worktree）SHALL 直接平铺渲染为单行，跳过折叠/展开交互（对齐原版 SidebarHeader.tsx 的"无可分组时降级"行为）。

折叠/展开状态 SHALL 在 `sidebarStore.svelte.ts` 内的 `expandedGroupIds: Set<string>` 维护，仅活跃会话生效，**不**做跨会话持久化（与现有 Pin/Hide 状态同模式）。

#### Scenario: 多 worktree group 折叠
- **WHEN** Sidebar 拉到一个 group 含 2 个 worktree（main + 附加）
- **THEN** 默认 SHALL 渲染为一行可展开条目，展开 chevron 朝右
- **AND** 条目右侧 SHALL 显示 worktree 数量徽章（如 `2`）

#### Scenario: 单 worktree group 直接平铺
- **WHEN** Sidebar 拉到一个 group 只含 1 个 worktree（standalone project）
- **THEN** 默认 SHALL 直接渲染为单行，**不**显示 chevron / 数量徽章
- **AND** 点击该行 SHALL 直接选中该 worktree 进入 session 列表

#### Scenario: 展开 group 显示 worktree 子项
- **WHEN** 用户点击多 worktree group 的展开 chevron
- **THEN** SHALL 在该行下方展开 worktree 子列表，每项含 git branch 名（`git_branch`）与最近活动时间
- **AND** main worktree SHALL 排在第一位（已在后端 `WorktreeGrouper` 排序）

### Requirement: 活跃 worktree 选中状态

Sidebar SHALL 维护"当前选中的 worktree"为单一来源真值——按 design.md D7b 决策，不引入新 `activeWorktreeId` state，沿用既有 `App.selectedProjectId`：每个 `Worktree.id` 与底层 `Project.id` 一一对应（见 `cdt-core::Worktree::id`），worktree 子项点击时把 `worktree.id` 作为 project id 注入既有路径。SessionList SHALL 通过 `list_sessions(worktree.id, pagination)` 拉取该 worktree 自身的 sessions（不是 group 级合并）。

后端 `get_worktree_sessions(group_id, pagination)` IPC SHALL 保留作为可选的"group 级合并 sessions"入口，供未来"group 概览页"等场景按需调用；本 Requirement 不要求 sidebar 默认调它。

`selectedProjectId` 不跨会话持久化（仅本次会话状态），刷新后默认选中"最近活动 group 的 main worktree"。

#### Scenario: 切换 worktree 重新拉 sessions
- **WHEN** 用户在展开的 group 内点击非当前 worktree 的子项
- **THEN** `App.selectedProjectId` SHALL 更新为该 worktree 的 id（即对应底层 `Project.id`）
- **AND** SHALL 触发 `list_sessions(worktree.id, pagination)` 拉取该 worktree 自身的 sessions（按 mtime 倒序）
- **AND** SessionList SHALL 渲染该 worktree 的 sessions

#### Scenario: 默认选中最近活动 group 的 main worktree
- **WHEN** 应用启动，`list_repository_groups()` 返回多个 group
- **THEN** `App.selectedProjectId` 初始值 SHALL 为最近活动 group 内 `is_main_worktree=true` 的 worktree id
- **AND** 该 group SHALL 在用户首次打开 SidebarHeader dropdown 时自动展开（若是多 worktree group）

#### Scenario: get_worktree_sessions 仍暴露给未来 group 概览页
- **WHEN** 调用方需要拿到一个 group 内所有 worktree 合并后的 sessions（按 mtime 倒序，跨 worktree）
- **THEN** 后端 `get_worktree_sessions(group_id, pagination)` IPC SHALL 返回 `PaginatedResponse<SessionSummary>`，每条 SessionSummary 含 `worktreeId` / `worktreeName` 字段供调用方按 worktree 过滤
- **AND** 当前 sidebar 默认 SessionList 路径 SHALL NOT 强制走该 IPC（保持单 worktree 视图与既有 list_sessions 调用语义）

### Requirement: 移除 flat 视图 toggle

Sidebar UI SHALL 不暴露 flat / grouped 视图切换控件（对齐 design.md D4 决策——默认且唯一 grouped 视图）。原版 SidebarHeader 的 `viewMode` toggle 在本 port 内**不**实现。

Sidebar SHALL 在 `listRepositoryGroups()` IPC 失败 / 返回空数组时自动 fallback 到 `listProjects()` 平铺渲染（保证后端跑老版本或 grouper 异常时仍可用）。该 fallback SHALL 由 `repositoryGroups.length > 0` 派生条件控制，不引入额外 URL 参数 / config 字段——按 design.md D4b 决策，不引入 dev-only `?mode=flat` URL gate（vite dev / production Tauri 行为统一，简化状态机）。

#### Scenario: 后端 listRepositoryGroups 失败时 fallback 到 listProjects
- **WHEN** `listRepositoryGroups()` 抛错或返回空
- **THEN** Sidebar SHALL 回落到调 `listProjects()` 拿扁平 ProjectInfo 列表
- **AND** SidebarHeader dropdown SHALL 渲染为单层 flat 列表（无折叠 / chevron / worktree 子项）

#### Scenario: 单成员 group 平铺，无 chevron
- **WHEN** 渲染一个 `worktrees.length === 1` 的 RepositoryGroup
- **THEN** SHALL 直接渲染为单行 dropdown-item（无 `.dropdown-group-row`、无 chevron、无 worktree 数量徽章）
- **AND** 点击该行 SHALL 直接选中该 worktree

### Requirement: Worktree 子项展示元信息

每个 Worktree 子项 SHALL 在 Sidebar 内显示：worktree 名（`worktree.name`）、git branch（`worktree.git_branch`，缺失时省略）、最近活动时间（相对时间，对齐 SessionSummary 已有格式化）、session 数量徽章（`worktree.sessions.length`）。

#### Scenario: 子项含 git branch 标签
- **WHEN** worktree.git_branch 存在
- **THEN** 子项右侧 SHALL 显示 branch icon + branch 名（如 `feat/sidebar-click-replace`）

#### Scenario: 子项无 git branch 时省略 branch 标签
- **WHEN** worktree.git_branch 为 None
- **THEN** 子项 SHALL NOT 显示 branch 标签，其它字段保留

### Requirement: 完整加载分页会话历史

Sidebar 默认会话列表 SHALL 使用当前项目的分页结果渐进展示 sessions，而不是为了首屏或普通浏览同步加载完整会话历史。若 `list_sessions` 响应包含 `nextCursor`，Sidebar SHALL 在用户滚动接近列表末尾或显式请求更多时继续分页；Command Palette 需要覆盖完整历史搜索时，MUST 使用 `session-search` 或显式承担逐页加载成本的专用路径，不能要求 Sidebar 首屏预先加载完整历史。

实现 SHALL NOT 使用“扩大 `pageSize` 并从头重拉直到 `nextCursor = null`”作为 Sidebar 首屏策略。实现 SHALL 保证每次分页返回页的 `session-metadata-update` 扫描不会因为后续页加载而错误覆盖或丢失已加载页的 metadata patch。

#### Scenario: Sidebar 首屏不加载默认第一页之后的旧会话

- **WHEN** 当前项目有 51 条会话，且 `list_sessions(projectId, { pageSize: 20, cursor: null })` 返回第一页并带 `nextCursor`
- **THEN** Sidebar SHALL 立即显示第一页 sessions
- **AND** Sidebar SHALL NOT 为了首屏显示第 51 条旧会话而同步加载完整 51 条

#### Scenario: Sidebar 滚动后加载默认第一页之后的旧会话

- **WHEN** 当前项目有 51 条会话，且用户持续滚动到需要更多 sessions
- **THEN** Sidebar SHALL 使用 `nextCursor` 继续请求后续页
- **AND** 第 51 条旧会话 SHALL 在其所在页加载后出现在会话列表中

#### Scenario: Command Palette 全历史搜索不依赖 Sidebar 首屏完整数组

- **WHEN** 当前项目有 51 条会话，且第 51 条旧会话的 title 匹配 Command Palette 查询文本
- **THEN** Command Palette SHALL 通过 `session-search` 或等价显式搜索路径覆盖该旧会话
- **AND** 该能力 SHALL NOT 要求 Sidebar 首屏已经加载第 51 条旧会话

#### Scenario: 会话数量变化时不扩大首屏请求直到完整

- **WHEN** 前端第一次调用 `list_sessions(projectId, { pageSize: 20, cursor: null })` 得到 `nextCursor`
- **AND** 项目在后续分页前新增会话
- **THEN** Sidebar SHALL 继续使用 cursor 分页或刷新当前页
- **AND** Sidebar SHALL NOT 基于 `total` 不断扩大 `pageSize` 从头请求直到完整

### Requirement: Sidebar uses paginated current-project session loading

Sidebar 会话列表 SHALL 对齐原版 `claude-devtools` 的当前项目分页加载语义：首次选中项目或 worktree 时默认只请求当前 `selectedProjectId` 的第一页 sessions，默认 `pageSize` SHALL 为 20；用户滚动接近列表末尾或显式继续加载时，前端 SHALL 使用后端返回的 `nextCursor` 请求下一页。

Sidebar SHALL NOT 为了首屏渲染同步加载当前项目完整会话历史，也 SHALL NOT 在 Dashboard 首页为所有项目触发 sessions 列表加载。首屏与刷新结果 SHALL 按 `timestamp` / mtime 倒序显示；后续分页加载更多时 SHALL 按 cursor 顺序追加新页并按 `sessionId` 去重，避免用户浏览历史时因整表重排导致滚动条跳动。

#### Scenario: 首屏只请求当前项目第一页

- **WHEN** 用户选中 `projectA`
- **THEN** Sidebar SHALL 调用 `list_sessions(projectA, { pageSize: 20, cursor: null })`
- **AND** Sidebar SHALL 在第一页返回后立即渲染已有 sessions
- **AND** Sidebar SHALL NOT 等待 `projectA` 完整历史加载完成

#### Scenario: 滚动触发下一页

- **WHEN** 第一页响应包含 `nextCursor`
- **AND** 用户滚动接近会话列表末尾
- **THEN** Sidebar SHALL 调用 `list_sessions(projectA, { pageSize: 20, cursor: nextCursor })`
- **AND** 新页 SHALL merge 到已加载列表中且按 `sessionId` 去重

#### Scenario: Dashboard 不加载所有项目 sessions

- **WHEN** 应用无 active tab 并显示 Dashboard 项目概览
- **THEN** Dashboard SHALL NOT 为每个项目调用 `list_sessions`
- **AND** sessions 列表加载 SHALL 只在用户选中或展开具体项目时发生

#### Scenario: 浏览历史时延迟实时刷新

- **WHEN** 用户已滚动离开会话列表顶部并正在浏览历史 sessions
- **AND** 当前 project 的 ongoing session 触发 file-change
- **THEN** Sidebar SHALL NOT 立即 silent refresh 整个列表
- **AND** Sidebar SHALL 显示有更新提示，直到用户回到顶部或点击提示后再刷新第一页

#### Scenario: 加载更多时保持已加载顺序

- **WHEN** 用户滚动到底部触发下一页加载
- **THEN** Sidebar SHALL 将新页追加到已加载 sessions 之后并按 `sessionId` 去重
- **AND** Sidebar SHALL NOT 因追加新页而重新排序已加载历史列表

### Requirement: Pinned and hidden sessions reconcile outside the first page

Sidebar SHALL NOT 假设 pinned 或 hidden session 一定位于第一页。当前项目存在 pinned/hidden session ids 且这些 ids 未出现在已加载分页结果中时，Sidebar SHALL 使用按 `sessionId` 补拉的 API 获取对应 light `SessionSummary`，再与分页列表合并。不存在或不属于当前项目的 ids SHALL 被忽略。

Hidden session 的 UI 过滤语义保持既有行为；pinned session 的视觉位置保持既有行为。本 Requirement 只规定数据补齐来源，不重新定义 Pin/Hide 交互。

#### Scenario: pinned session 不在第一页时仍可显示

- **WHEN** `projectA` 的 pinned id `sid-old` 不在第一页 `list_sessions` 响应中
- **AND** `sid-old` 存在于 `projectA`
- **THEN** Sidebar SHALL 通过按 id 补拉获得 `sid-old` 的 `SessionSummary`
- **AND** pinned 区域或列表 SHALL 能显示该 session

#### Scenario: hidden session 不在第一页时仍能过滤

- **WHEN** `projectA` 的 hidden id `sid-hidden` 不在第一页 `list_sessions` 响应中
- **AND** 后续分页或按 id 补拉返回 `sid-hidden`
- **THEN** Sidebar SHALL 按既有 hidden 规则过滤该 session

### Requirement: Sidebar Memory 入口

Sidebar SHALL 在当前选中项目存在 memory layers 时显示 `Memory (N)` 入口，其中 `N` 为可展示 memory layers 数量。点击入口 SHALL 调用 tab 系统打开该项目 Memory tab。若当前项目没有 memory layers，Sidebar SHALL NOT 显示 Memory 入口。

#### Scenario: 当前项目有 memory 时显示入口
- **WHEN** 当前选中项目的 memory discovery 返回 `hasMemory = true` 且 `count = 11`
- **THEN** Sidebar SHALL 在会话列表上方显示 `Memory (11)` 入口

#### Scenario: 点击 Memory 入口打开 tab
- **WHEN** 用户点击 Sidebar 的 `Memory (11)` 入口
- **THEN** 系统 SHALL 调用 tab 系统打开当前项目的 Memory tab

#### Scenario: 当前项目无 memory 时隐藏入口
- **WHEN** 当前选中项目的 memory discovery 返回 `hasMemory = false` 或 `count = 0`
- **THEN** Sidebar SHALL NOT 渲染 Memory 入口

#### Scenario: 切换项目刷新 Memory 入口
- **WHEN** 用户从有 memory 的 project A 切换到无 memory 的 project B
- **THEN** Sidebar SHALL 隐藏 Memory 入口，并继续显示 project B 的会话列表

### Requirement: 会话总数显示口径

Sidebar 顶部 `session-count-num` 元素显示形如 `{visibleSessions.length}/{totalSessions}`。`totalSessions` SHALL 取自 `listSessions` IPC 响应的 `result.total` 字段（项目维度后端骨架阶段 `read_dir` 统计的全部 session 数），而非已加载到本地的 `sessions.length`（后者会随用户翻页累加 20 → 40 → 60 跳变）。

非 silent 路径（首次加载 / 切 project）的 `loadSessions` SHALL 在 IPC 返回后用 `result.total` 覆盖本地 `sessionsTotal`。silent 路径（file-change 触发或"有更新"按钮触发）SHALL 在合并完成后同样用 `result.total` 覆盖（silent 拿到的也是后端最新全量计数）。`loadMoreSessions` 翻页路径 SHALL **不**覆盖 `sessionsTotal`（页内 total 不应改变；首次加载时已有正确值）。

#### Scenario: 首次加载时 totalSessions 取后端 result.total

- **WHEN** Sidebar 首次加载某 project（项目实际 60 个 session）
- **AND** `listSessions(projectId, 20)` 返回 `{ items: [...20 条...], nextCursor: "20", total: 60 }`
- **THEN** `session-count-num` SHALL 显示 `20/60`，**不**显示 `20/20`

#### Scenario: 翻页后 totalSessions 不随 sessions.length 变化

- **WHEN** 用户已加载 page 1（20 条）；调用 `loadMoreSessions` 加载 page 2（再 20 条）
- **THEN** `sessions.length` 从 20 增至 40；`totalSessions` SHALL 保持 60；`session-count-num` 显示 `40/60`，不再出现 `20 → 40 → 60` 跳变

#### Scenario: silent 刷新时 totalSessions 同步刷新

- **WHEN** silent 刷新（file-change 或"有更新"按钮触发）成功，`result.total` 由 60 变为 61（后端检测到新增 session）
- **THEN** Sidebar SHALL 把 `totalSessions` 更新为 61；不破坏既有 silent 刷新对 sessions 数组合并保留尾部的语义


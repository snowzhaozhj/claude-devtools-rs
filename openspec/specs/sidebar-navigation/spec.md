# sidebar-navigation Specification

## Purpose

定义 Sidebar 的导航行为：项目选择、会话列表展示（日期分组 / 排序 / 过滤）、与 Tab 系统的联动、会话 Pin / Hide、右键菜单、宽度拖拽调整。同时覆盖骨架快速渲染、`session-metadata-update` 增量 patch、虚拟滚动等性能机制。多选 / 批量操作留作后续扩展。
## Requirements
### Requirement: 项目选择

应用顶栏（chrome 内 `zone-left-center` 区域）SHALL 提供项目选择下拉作为主导航控件，**项目入口语义 SHALL 为 RepositoryGroup（同一个 git repo 一个条目）**——不再按 worktree 维度暴露多条平铺。选择项目后 SHALL 自动加载该项目（group）的合并 session 列表到 Sidebar。项目选择控件 MUST NOT 渲染在 Sidebar 内部组件内，MUST NOT 随 sidebar 折叠状态消失。

多 worktree group SHALL 在下拉内显示为**单行**（不再 accordion），点击即选中整个 group；单 worktree group 同样渲染为单行，行为一致。worktree 维度的快速切换由 sidebar 顶部的 worktree filter 下拉提供（见 `Worktree filter dropdown for multi-worktree group` Requirement），不在项目切换器内暴露。

#### Scenario: 初始加载
- **WHEN** 应用启动且有可用项目
- **THEN** 系统 SHALL 自动选中第一个项目（按 `mostRecentSession` 倒序的首个 RepositoryGroup）并加载其合并 session 列表
- **AND** chrome 内项目下拉 SHALL 显示当前选中 group 的名称（`group.name`）

#### Scenario: 切换项目
- **WHEN** 用户从 chrome 内项目下拉选择器切换到另一个 group
- **THEN** session 列表 SHALL 更新为新 group 的合并 sessions，之前的列表 SHALL 被替换
- **AND** chrome 内项目下拉 SHALL 显示新选中 group 的名称

#### Scenario: 无项目
- **WHEN** 无可用项目
- **THEN** Sidebar SHALL 显示空状态提示
- **AND** chrome 内项目下拉 SHALL 显示禁用态占位文本（如「无项目」）

#### Scenario: sidebar 折叠不影响项目选择
- **WHEN** 用户点击 chrome 内 sidebar 折叠按钮把 sidebar 收起
- **THEN** chrome 内项目下拉 SHALL 仍可见且可操作
- **AND** 用户 SHALL 可在 sidebar 折叠态切换项目，新项目的会话列表会在重新展开 sidebar 时立即可见

#### Scenario: 多 worktree group 单行展示无 accordion
- **WHEN** 项目切换器渲染一个 worktrees.length === 19 的 group
- **THEN** 下拉内 SHALL 显示**一行**（不展开 19 条子项），点击即选中该 group
- **AND** 行内 SHALL 显示 `group.name` + `group.totalSessions` 计数
- **AND** SHALL NOT 渲染 group accordion 行 / worktree count badge / 展开后的 worktree 子列表

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

每个会话项 SHALL 显示标题和元数据（消息计数、相对时间、git 分支、worktree cwd hint）。标题 SHALL 优先使用后端提供的 title 字段，无 title 时 fallback 到**完整 sessionId**——CSS 的 `text-overflow: ellipsis` 自然截断超出宽度的部分；同时 SHALL 在该元素上设置 HTML `title` 属性（`title || sessionId` 完整值），让用户 hover 时浏览器原生 tooltip 显示完整字符串。**禁止**在 JS 侧再手动 `slice(0, 8) + "…"`——双重截断让用户看到的是"前 8 字符 + …"既不能复制粘贴定位 session、也丢失了 CSS 自然 ellipsis 提供的 hover 全展能力。

消息计数（`SessionSummary.messageCount`）SHALL 等于该 session 文件中**真实 user-chunk 消息**与配对 assistant 消息的总数——后端 session metadata 提取函数 MUST 用对齐原版 user-chunk 消息识别规则的过滤函数判定 user 消息：`category != User` 或 `is_meta = true` 或 `消息内容 blocks` 不含任何 `Text` / `Image` block（即纯 `tool_result`-only 行）SHALL NOT 计入。配对计数规则保持原状：每个 user-chunk 后，紧接的第一个非 synthetic 非 sidechain 的 assistant 消息计 1（与 `awaitingAIGroup` 状态机一致）。

git 分支（`SessionSummary.gitBranch`）SHALL 在每条会话项第二行 meta 末尾以 `· <GitBranch icon> {branch}` chip 形式渲染；`gitBranch` 为 `null` 时 SHALL NOT 渲染该 chip（不留分隔符 `·`、不留空位）。该 chip MUST 跟随 `session-metadata-update` 事件 patch 的 `gitBranch` 即时更新。

worktree cwd hint（`SessionSummary.cwdRelativeToRepoRoot`）SHALL 在 `gitBranch` chip 之后以单独 chip 渲染：`…/<lastTwoSegs>`（最后两段路径）。`cwdRelativeToRepoRoot` 为 `null` / 空字符串时 SHALL NOT 渲染该 chip。

会话项 SHALL NOT 渲染老版本侧栏行尾全路径 cwd label（基于 `cwdTailLabel(session.cwd)` 的 `<span class="session-cwd">` 渲染节点）——该展示路径已被 `Session row branch + cwd chip` Requirement 替代。

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

#### Scenario: 元数据含 cwd hint chip
- **WHEN** 会话项渲染，`cwdRelativeToRepoRoot = "crates"`
- **THEN** SHALL 在 gitBranch chip 之后追加 cwd hint chip 显示 "crates"

#### Scenario: 深层子目录 cwd hint 取最后两段
- **WHEN** 会话项渲染，`cwdRelativeToRepoRoot = ".claude/worktrees/feat-x"`
- **THEN** cwd hint chip SHALL 显示 "worktrees/feat-x"

#### Scenario: repo 根 session 不渲染 cwd hint chip
- **WHEN** `cwdRelativeToRepoRoot` 为 null / 空字符串
- **THEN** SHALL NOT 渲染 cwd hint chip

#### Scenario: 行尾全路径 label 已移除
- **WHEN** 检查 Sidebar 实现的 cwd 渲染逻辑
- **THEN** SHALL NOT 包含老版本基于 `cwdTailLabel` / `<span class="session-cwd">` 的行尾全路径渲染

#### Scenario: 消息计数排除 tool_result-only user 行
- **WHEN** session JSONL 含 1 条真实用户输入（`{role:"user", content:"hi"}`）+ 1 条 assistant tool_use + 1 条 user tool_result（`{role:"user", content: [{type:"tool_result", ...}]}`）+ 1 条 assistant 收尾
- **THEN** session metadata 提取函数返回的 `messageCount` SHALL 为 `2`（真实 user + 配对 assistant），**不**计入 tool_result-only 行

#### Scenario: 消息计数包含含 text+tool_result 混合 user 行
- **WHEN** user 消息 `消息内容 blocks` 同时含 `Text` block 与 `ToolResult` block
- **THEN** SHALL 计入 messageCount（与原版 user-chunk 消息识别规则一致，"Must contain text or image blocks"）

#### Scenario: 消息计数包含 image-only user 行
- **WHEN** user 消息 `消息内容 blocks` 只含 `Image` block（用户粘贴截图，无文字）
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
- **WHEN** 同一 project 在短时间内连续收到 3 次 `file-change` 事件
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

Sidebar SHALL 维护一个按 sessionId 索引的 update 缓冲区（`pendingMetadataUpdates`）——listener 每收到一条 update 都 SHALL 写入该 buffer（按 sessionId 覆盖最新值），**无论**当前 `sessions` 数组是否已包含该 sessionId。`sessions` 数组每次写入（非 silent 加载首页 / silent 刷新 / `loadMoreSessions` 翻页扩展）后 SHALL 立即对新数组应用 buffer 中匹配 sessionId 的 update。这是兜底后端跨进程 push 流在 IPC return 之前到达时 `sessions.map` 找不到目标的 race——前端 listener 已订阅但 sessions 数组还没扩展到新页时，update 会静默丢失（push 流不重发），导致 session 永远卡在 sessionId 占位。

切 project / 首次加载（非 silent 路径）SHALL 在调用 `await listSessions(...)` **之前**清空 `pendingMetadataUpdates`，避免旧 project 的 update 残留；同时这一 clear SHALL 早于 await 阻塞窗口，让 listener 在 `await listSessions(...)` 期间收到的新 project update 能被 buffer 保留并在后续 `applyPendingMetadata` 应用上去——后端 `list_sessions` 在 IPC return 之前已 spawn 扫描任务并可能广播 emit，clear 若放在 await 后会把这些"早到的"新 project update 一起清掉。silent 刷新与 loadMore SHALL NOT 清空 buffer（buffer 中已有的 update 仍可能匹配 prev sessions 中尚未 patch 的 sessionId）。

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

- **WHEN** Sidebar 已加载 page 1（20 条），用户滚动触发 `loadMoreSessions` 启动 page 2 的 `list_sessions` IPC；后端 page 2 的扫描任务先于 IPC return 完成对 `sessionId = "s_new"`（page 2 尾部一条）的 metadata 扫描并广播 emit
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

Sidebar SHALL 支持折叠（隐藏）与展开两种状态。折叠状态由 Sidebar 折叠状态 store 的模块级 runes state 管理（内存级，重启回归默认展开）。

折叠入口 SHALL 提供两条：(1) SidebarHeader 顶部右侧 `PanelLeft` icon 按钮，点击切换；(2) **通过 `keyboard-shortcuts` capability 注册的全局快捷键 `sidebar.toggle`**（默认 binding：mac `⌘B` / Win+Linux `Ctrl+B`）SHALL 切换。展开入口 SHALL 提供 (1) 折叠态下 TabBar 最左侧 `PanelLeft` icon 按钮；(2) 同一 `sidebar.toggle` 快捷键。

折叠时 sidebar SHALL 完全不渲染（不留窄轨道、不留 0 宽度占位 DOM）。展开时 sidebar SHALL 恢复折叠前的宽度（如未拖拽过则为默认宽度）。

`sidebar.toggle` 快捷键 SHALL 由用户在 `Settings → Keyboard Shortcuts` 中自定义（覆盖默认 binding）；自定义后 SHALL 立即生效，重启 SHALL 保留。

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

- **WHEN** 用户按下 `sidebar.toggle` 当前 binding（默认 mac `⌘B` / 其他 `Ctrl+B`）
- **AND** `document.activeElement` 不是 `<input>` / `<textarea>` / `[contenteditable="true"]`
- **THEN** `keyboard-shortcuts` registry dispatcher SHALL 命中 `sidebar.toggle` spec 并调用其 handler
- **AND** Sidebar 折叠状态 SHALL 切换（展开 ↔ 折叠），等价于点击 PanelLeft 按钮
- **AND** `event.preventDefault()` SHALL 被调用

#### Scenario: 快捷键在折叠态下仍生效

- **WHEN** Sidebar 当前折叠
- **AND** 用户按下 `sidebar.toggle` 当前 binding
- **THEN** Sidebar SHALL 重新展开（dispatcher 单一 listener 挂在 `document` 顶层，不依赖 Sidebar 自身渲染）

#### Scenario: 用户自定义 binding 后生效

- **WHEN** 用户在 `Settings → Keyboard Shortcuts` 把 `sidebar.toggle` 改为 `mod+shift+B`
- **AND** 保存生效
- **THEN** 后续按下 `mod+shift+B` SHALL 切换 Sidebar 折叠
- **AND** 按下原默认 `mod+B` SHALL NOT 触发折叠（除非另一 spec 占用了 `mod+B`）

#### Scenario: 重启后回归展开

- **WHEN** 用户折叠 Sidebar 后关闭应用并重新启动
- **THEN** Sidebar SHALL 处于展开状态（折叠状态不持久化，与 sidebar 宽度同维度）

### Requirement: 默认渲染按仓库聚合的 Sidebar

Sidebar SHALL 默认调用 `list_repository_groups()` IPC 拉取按 git 仓库聚合的项目列表，把同一仓库的多个 worktree 合并为单个 RepositoryGroup。项目切换器（chrome 内）SHALL 把每个 `RepositoryGroup` 渲染为单行 dropdown item（无论多 worktree 还是单 worktree group），点击即选中整个 group。

单 worktree group（只含一个 worktree）SHALL 与多 worktree group 走同一渲染分支（无特殊处理），点击行为一致：选中后 sidebar 调 `list_group_sessions(groupId, pageSize, null)` 拉取该 group 的合并 sessions（单 worktree 时合并退化为该 worktree 自身 sessions）。

`expandedGroupIds: Set<string>` state SHALL 移除——项目切换器不再有 accordion 展开/折叠交互。worktree 维度的细化由 sidebar 顶部 worktree filter 下拉提供。

#### Scenario: 多 worktree group 单行渲染
- **WHEN** Sidebar 拉到一个 group 含 19 个 worktree
- **THEN** 项目切换器下拉内 SHALL 渲染**一行** dropdown item（`group.name` + `group.totalSessions`），无 chevron、无 worktree count badge、无展开子列表
- **AND** 点击该行 SHALL 把 `selectedProjectId` 设为 `group.id`，触发 `list_group_sessions` 拉取合并 sessions

#### Scenario: 单 worktree group 同分支渲染
- **WHEN** Sidebar 拉到一个 group 只含 1 个 worktree（standalone project）
- **THEN** 项目切换器下拉内 SHALL 渲染一行 dropdown item（与多 worktree group 同分支）
- **AND** 点击该行 SHALL 把 `selectedProjectId` 设为 `group.id`（标量等于 `worktrees[0].id`）

#### Scenario: 不再渲染 accordion
- **WHEN** 检查项目切换器实现
- **THEN** SHALL NOT 包含 accordion 折叠交互逻辑（无 group accordion 行 / 折叠 chevron / 展开计数 badge / `expandedGroupIds` 状态）

### Requirement: 活跃 worktree 选中状态

Sidebar SHALL 维护"当前选中的 RepositoryGroup"为单一来源真值——`App.selectedProjectId` 字段持 `group.id`（不再持 worktree.id）。SessionList SHALL 通过 `list_group_sessions(group.id, pageSize, cursor)` 拉取该 group 内所有 worktree 合并、按 mtime 全局倒序的 sessions。

worktree 维度的过滤由 sidebar 顶部 worktree filter 下拉控制（见 `Worktree filter dropdown for multi-worktree group` Requirement）；filter state 与 `selectedProjectId` 分离持有（filter state 在 group 级 scope）。

`selectedProjectId` 跨会话持久化由 `Migrate persisted selected_project_id on load` Requirement 负责迁移老 worktree id 格式。

`get_worktree_sessions` IPC 保留为兼容入口供未来"group 概览页 + 多维过滤"等场景按需调用；本 Requirement 不要求 sidebar 默认调它。

#### Scenario: 切换 group 调 list_group_sessions
- **WHEN** 用户在 ProjectSwitcher 切到 group-X
- **THEN** `App.selectedProjectId` SHALL 更新为 `"group-X"`
- **AND** SHALL 触发 `list_group_sessions(groupId: "group-X", pageSize: 50, cursor: null)` 拉取合并 sessions
- **AND** SessionList SHALL 渲染合并后的 sessions

#### Scenario: 默认选中最近活动 group
- **WHEN** 应用启动，`list_repository_groups()` 返回多个 group（按 mostRecentSession 倒序）
- **THEN** `App.selectedProjectId` 初始值 SHALL 为最近活动 group 的 `group.id`
- **AND** worktree filter SHALL 初始为 "全部"

#### Scenario: list_group_sessions 触发 SSE detail 推送
- **WHEN** `list_group_sessions` 返回首页 50 条骨架
- **THEN** 后台 SHALL 对这 50 条 session 触发 `session-metadata-update` SSE 推送
- **AND** 前端 SHALL 按 `(groupId, sessionId)` 匹配 patch metadata

### Requirement: 移除 flat 视图 toggle

Sidebar UI SHALL 不暴露 flat / grouped 视图切换控件，默认且唯一 grouped 视图。原版 SidebarHeader 的 `viewMode` toggle 在本 port 内**不**实现。

Sidebar SHALL 在 `listRepositoryGroups()` IPC 失败 / 返回空数组时自动 fallback 到 `listProjects()` 平铺渲染（保证后端跑老版本或 grouper 异常时仍可用）。该 fallback SHALL 由 `repositoryGroups.length > 0` 派生条件控制，不引入额外 URL 参数 / config 字段，亦不引入 dev-only `?mode=flat` URL gate（vite dev / production Tauri 行为统一，简化状态机）。

#### Scenario: 后端 listRepositoryGroups 失败时 fallback 到 listProjects
- **WHEN** `listRepositoryGroups()` 抛错或返回空
- **THEN** Sidebar SHALL 回落到调 `listProjects()` 拿扁平 ProjectInfo 列表
- **AND** SidebarHeader dropdown SHALL 渲染为单层 flat 列表（无折叠 / chevron / worktree 子项）

#### Scenario: 单成员 group 平铺，无 chevron
- **WHEN** 渲染一个 `worktrees.length === 1` 的 RepositoryGroup
- **THEN** SHALL 直接渲染为单行 dropdown-item（无 `.dropdown-group-row`、无 chevron、无 worktree 数量徽章）
- **AND** 点击该行 SHALL 直接选中该 worktree

### Requirement: 完整加载分页会话历史

Sidebar 默认会话列表 SHALL 使用 `list_group_sessions(groupId, pageSize, cursor)` 的分页结果渐进展示 sessions，而不是为了首屏或普通浏览同步加载完整会话历史。若 `list_group_sessions` 响应包含非空 `nextCursor`，Sidebar SHALL 在用户滚动接近列表末尾或显式请求更多时继续分页；Command Palette 需要覆盖完整历史搜索时，MUST 使用 `session-search` 或显式承担逐页加载成本的专用路径，不能要求 Sidebar 首屏预先加载完整历史。

实现 SHALL NOT 使用"扩大 `pageSize` 并从头重拉直到 `nextCursor = null`"作为 Sidebar 首屏策略。实现 SHALL 保证每次分页返回页的 `session-metadata-update` 扫描不会因为后续页加载而错误覆盖或丢失已加载页的 metadata patch（按 `(groupId, sessionId)` key 匹配 patch，跨页 patch 互不干扰）。

#### Scenario: Sidebar 首屏不加载默认第一页之后的旧会话

- **WHEN** 当前 group 有 51 条合并 sessions，且 `list_group_sessions(groupId, { pageSize: 20, cursor: null })` 返回第一页并带 `nextCursor`
- **THEN** Sidebar SHALL 立即显示第一页 sessions
- **AND** Sidebar SHALL NOT 为了首屏显示第 51 条旧会话而同步加载完整 51 条

#### Scenario: Sidebar 滚动后加载默认第一页之后的旧会话

- **WHEN** 当前 group 有 51 条合并 sessions，且用户持续滚动到需要更多 sessions
- **THEN** Sidebar SHALL 使用 `nextCursor` 继续请求后续页
- **AND** 第 51 条旧会话 SHALL 在其所在页加载后出现在会话列表中

#### Scenario: 切 worktree filter 重置分页 cursor
- **WHEN** 用户在多 worktree group 内切换 filter
- **THEN** Sidebar SHALL 清空已加载页 + 重置 cursor 为 null + 重调 `list_group_sessions`

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

Sidebar SHALL 在当前选中 group 存在 memory layers 时显示 `Memory (N)` 入口，其中 `N` 为可展示 memory layers 数量。点击入口 SHALL 调用 tab 系统打开该 group 的 Memory tab。若当前 group 没有 memory layers，Sidebar SHALL NOT 显示 Memory 入口。

**Anchor 选择**：sidebar 顶部 memory 入口的可见性条件与点击行为 SHALL 用 group 内"repo 根 / main worktree / 第一个 worktree" fallback 链选出的 worktree id 作为 query key（`getProjectMemory` IPC 入参与 `openMemoryTab` projectId 使用同一 anchor），**不**跟随 worktree filter 漂移。理由：memory 文件物理上写在 Claude Code 父进程 cwd 编码出的 project_dir 下（`~/.claude/projects/<encoded-cwd>/memory/`），绝大多数用户只在 repo 根目录跑 Claude Code，每个非 repo 根 worktree 各自的 encoded project_dir 下并不存在 memory 目录；若 anchor 跟随 worktree filter，切到具体 worktree 后查询返回 `count=0` 让入口消失，与用户"memory 是 repo 级别"的心智模型错位。

本约束与 D7（Requirement `selectedGroupId 与 worktree id 分层维护` 表 row 970）不矛盾——后者约束 query id 的形态（仍是一个 worktree id，与 detail API 保持同形），本约束在 query id 形态不变的前提下细化"哪个 worktree id"。

`pin/hide` 等其它 per-project state 仍走"跟随 worktree filter"的 anchor（per-worktree 隔离对置顶/隐藏有真实语义价值），不在本 Requirement scope 内。

#### Scenario: 当前 group 有 memory 时显示入口
- **WHEN** 当前选中 group 内 repo 根 worktree 的 memory discovery 返回 `hasMemory = true` 且 `count = 11`
- **THEN** Sidebar SHALL 在会话列表上方显示 `Memory (11)` 入口

#### Scenario: 点击 Memory 入口打开 group repo 根的 Memory tab
- **WHEN** 用户点击 Sidebar 的 `Memory (N)` 入口
- **THEN** 系统 SHALL 调用 tab 系统打开当前 group repo 根 worktree 的 Memory tab（即 `openMemoryTab(<group repo root worktree id>, "Memory")`）
- **AND** 即使当前 worktree filter 选了非 repo 根 worktree，打开的 tab projectId 仍 SHALL 为 repo 根 worktree id

#### Scenario: 当前 group 无 memory 时隐藏入口
- **WHEN** 当前选中 group 内 repo 根 worktree 的 memory discovery 返回 `hasMemory = false` 或 `count = 0`
- **THEN** Sidebar SHALL NOT 渲染 Memory 入口

#### Scenario: 切到非 repo root worktree 时 memory 入口仍显示 group 维度的 memory
- **WHEN** 用户在含 main + feat-x 双 worktree 的 group 内（main worktree memory `count = 3`、feat-x worktree memory `count = 0`），切 worktree filter 从"全部"到 feat-x
- **THEN** Sidebar 顶部 `Memory (3)` 入口 SHALL 仍可见且数量不变
- **AND** memory IPC query 入参 SHALL 仍是 main worktree 的 id（不是 feat-x 的 id）

#### Scenario: 切回"全部 worktree"后 memory 入口保持
- **WHEN** 用户在多 worktree group 内从 feat-x worktree filter 切回"全部"
- **THEN** Sidebar 顶部 memory 入口 SHALL 持续可见且数量与 repo 根 worktree memory count 一致

#### Scenario: 切换 group 刷新 Memory 入口
- **WHEN** 用户从 group A（repo 根 worktree 有 memory）切换到 group B（repo 根 worktree 无 memory）
- **THEN** Sidebar SHALL 隐藏 Memory 入口，并继续显示 group B 的会话列表

### Requirement: 会话总数显示口径

Sidebar 顶部 `session-count-num` 元素 SHALL 表达"当前 scope 内一共有多少 session"——**用户不感知客户端分页内部状态**，分页加载进度由 sidebar 底部 `▼ 加载更多 · 剩 N 条` 按钮 + `已显示全部 N 条` 端状态承担（PR-A 已落地）；顶部 count 只显总量 + 搜索命中数两态。

**scope 定义**：
- 多 wt group 选中「全部」chip / 单 wt group / flat fallback：scope = group 全集
- 多 wt group 选中具体 worktree chip：scope = 该 worktree 集合

**两态显示**：

- **默认状态（filterQuery 为空）**：显示单数字 `{scopeTotal}`，例如 `127`（filter=「全部」）或 `8`（filter=具体 wt 且该 wt 共 8 个 session）。`scopeTotal` MUST 按 filter scope 派生：
  - filter=ALL_WORKTREES：`scopeTotal = selectedGroup?.totalSessions ?? sessions.length`（fallback 仅在 race window 内 selectedGroup 未就绪时兜底）
  - filter=具体 worktreeId：`scopeTotal = groupWorktrees.find(w => w.id === filter)?.sessions.length ?? sessions.length`（fallback 同上）
- **搜索激活状态（filterQuery 非空）**：显示 `{matchCount} 匹配`，例如 `5 匹配`。`matchCount` MUST 取 `visibleSessions.length`，即客户端已加载范围内 + filterQuery 命中 + 非隐藏的剩余条数。**搜索的 scope 限制 SHALL 通过 search input 的 `aria-describedby` / `title` 属性以 "在已加载范围内搜索" 文本明示用户**——避免用户把 `5 匹配` 误读为"全 scope 命中数"，特别是仍有未加载页的大 group。当 `sessionsNextCursor` 非 null（仍有未加载页）且 filterQuery 非空时，sidebar 可选择性自动 silent loadMore 直到全 scope 加载完，让 matchCount 收敛到全 scope 命中数（非 MUST，但作为优化方向）。

**hover tooltip**：基础显示一层 `总 {scopeTotal}`；当 `hiddenCount > 0` 时 SHALL 追加 ` · {hiddenCount} 已隐藏`。`hiddenCount === 0` 时 SHALL 仅显示一层（避免 ` · 0 已隐藏` 噪音）。tooltip 不暴露分页已加载条数——加载进度由列表底部 `▼ 加载更多 · 剩 N 条` 按钮承载，避免顶部 + 底部双处表达同一概念造成用户认知冗余。

**`scopeTotal` 数据来源链路（统一权威路径）**：

- `list_repository_groups` IPC 后端返回 `RepositoryGroup.totalSessions`（grouper 计算的 group 跨 wt 真值，**唯一权威源**）+ `RepositoryGroup.worktrees[].sessions: string[]`（每 wt 内 session id 列表）
- 前端 `selectedGroup` 由 `repositoryGroups.find(g => g.id === selectedGroupId)` derived；`groupWorktrees = selectedGroup?.worktrees ?? []` derived
- ALL scope 取 `selectedGroup.totalSessions`；具体 wt scope 取 `groupWorktrees.find(...).sessions.length`——两者都直接从 `list_repository_groups` derived 出，**无需第二个本地 state**

`listSessions` / `list_group_sessions` 翻页 IPC 的 `result.total` 字段含义与 `RepositoryGroup.totalSessions` 在 ALL scope 下等同（后端不变量），但前端 SHALL 直接消费 `selectedGroup.totalSessions` derived，不另行存储 `result.total` 到独立 state（避免命名链路冗余）。

**silent 刷新触发 `list_repository_groups` SWR revalidate 的条件**：

silent 刷新（file-change 事件触发或「有更新」按钮触发）SHALL 仅在 file-change payload 满足 `projectListChanged === true || sessionListChanged === true || deleted === true` 任一条件时才 schedule `loadProjects(refresh: true)`（`list_repository_groups` SWR revalidate）。其它情况（普通内容追加：三个标志全 false）SHALL NOT 触发 `loadProjects`，避免活跃 session 持续追加消息时 sidebar 高频 IPC 噪声。字段语义见 `[[push-events::file-change]]`。

`loadMoreSessions` 翻页路径 SHALL **不**修改 `selectedGroup.totalSessions`（页内 total 不应改变）。

#### Scenario: 默认状态 + 全部 worktree filter 显 group total
- **WHEN** Sidebar 首次加载某 group（group 实际 127 个 session 跨多 wt），filter 选「全部」
- **AND** filterQuery 为空
- **THEN** `session-count-num` SHALL 显示单数字 `127`，**不**显示分式（`{已加载}/{总}` 形式）也**不**显示已加载条数后缀

#### Scenario: 默认状态 + 选中具体 worktree 显 wt total
- **WHEN** group 含 worktree `wt-A`（8 个 session）/ `wt-B`（120 个 session），用户切到 `⌗wt-A` chip
- **AND** filterQuery 为空
- **THEN** `session-count-num` SHALL 显示单数字 `8`，**不**显示 `128`（用 group 全集会让用户在该 wt scope 下产生"还有 120 条"误读）

#### Scenario: loadMore 翻页不影响顶部总量
- **WHEN** 用户已加载 page 1（20 条）；调用 `loadMoreSessions` 加载 page 2（再 20 条）
- **THEN** `session-count-num` 显示 `60` 始终不变（顶部 count 不参与分页进度信号）
- **AND** 列表底部 `▼ 加载更多 · 剩 N 条` 按钮 SHALL 同步从 `剩 40 条` 变为 `剩 20 条`（PR-A 已落地的端状态）

#### Scenario: 搜索激活状态显 match 命中数
- **WHEN** 用户在 `scopeTotal=127` 状态下输入 filterQuery 命中（`visibleSessions.length === 5`）
- **THEN** `session-count-num` SHALL 显示 `5 匹配`，**不**再显示 `127`
- **AND** search input SHALL 含 `aria-describedby` / `title` 属性以"在已加载范围内搜索"文本明示 scope 限制（避免用户在仍有未加载页时误读为全 scope 命中数）
- **AND** 用户清空 filterQuery 后 SHALL 回到单数字 `127` 默认显示

#### Scenario: hidden=0 时 tooltip 仅显一层
- **WHEN** 用户 hover `session-count-num`，当前 scopeTotal=127 / hiddenCount=0
- **THEN** native tooltip SHALL 显示 `总 127`，**不**显示 `· 0 已隐藏` 后缀

#### Scenario: hidden>0 时 tooltip 追加 hidden
- **WHEN** 用户 hover `session-count-num`，当前 scopeTotal=127 / hiddenCount=5
- **THEN** native tooltip SHALL 显示 `总 127 · 5 已隐藏`

#### Scenario: silent 刷新 sessionListChanged 时 scopeTotal 同步刷新
- **WHEN** 前端收到 file-change payload 含 `sessionListChanged: true`（字段语义见 `[[push-events::file-change]]`）
- **AND** 前端 Sidebar handler 收到 payload，filter=「全部」
- **THEN** Sidebar SHALL schedule `loadProjects(refresh: true)`（`list_repository_groups` SWR revalidate）
- **AND** revalidate 拉到新 `RepositoryGroup.totalSessions = 128`（含新 session）
- **AND** `selectedGroup.totalSessions` derived 自动更新为 128
- **AND** 默认状态显示 SHALL 立即从 `127` 切到 `128`

#### Scenario: silent 刷新 deleted 时 scopeTotal 同步下降（ALL scope）
- **WHEN** 前端收到 file-change payload 含 `deleted: true, sessionListChanged: true`（字段语义见 `[[push-events::file-change]]`）
- **AND** filter=「全部」
- **THEN** Sidebar SHALL schedule `loadProjects(refresh: true)`，revalidate 拉到新 `RepositoryGroup.totalSessions = 126`
- **AND** `selectedGroup.totalSessions` derived 自动更新为 126
- **AND** 默认状态显示 SHALL 立即从 `127` 切到 `126`
- **AND** 若被删除的 session 在已加载范围内，sidebar SHALL 同步从 `sessions` 数组移除该条；若不在已加载范围（仍在远端未翻到的部分），仅顶部 count 下降，已加载列表不变

#### Scenario: silent 刷新 sessionListChanged 时 scopeTotal 同步下降（具体 worktree scope）
- **WHEN** filter 选中 `⌗wt-A`（原 `wt-A.sessions.length === 8`）
- **AND** 前端收到 file-change payload 含 `sessionListChanged: true`（字段语义见 `[[push-events::file-change]]`，wt-A 内 1 个 session 被删除触发）
- **THEN** Sidebar SHALL schedule `loadProjects(refresh: true)`，revalidate 拉到新 `RepositoryGroup.worktrees[0].sessions.length === 7`
- **AND** `groupWorktrees.find(w => w.id === filter)?.sessions.length` derived 自动更新为 7
- **AND** 默认状态显示 SHALL 立即从 `8` 切到 `7`
- **AND** 若被删除的 session 在已加载范围内，sidebar SHALL 同步从 `sessions` 数组移除该条

#### Scenario: 普通 JSONL append SHALL NOT 触发 loadProjects
- **WHEN** 活跃 session 持续追加消息，后端 watcher enrich 结果 `sessionListChanged: false`（字段语义见 `[[push-events::file-change]]`）
- **AND** 前端收到 file-change payload `{ projectId: "pa", sessionId: "sa", deleted: false, projectListChanged: false, sessionListChanged: false }`
- **AND** 前端 Sidebar handler 收到 payload
- **THEN** Sidebar MUST NOT schedule `loadProjects(refresh: true)`
- **AND** Sidebar 仍 SHALL schedule `loadSessions(currentGroupId, silent: true)`（保持现有 `Auto refresh session list on file change` 契约不变——session 内消息变化仍需刷新当前 group session list）
- **AND** `selectedGroup.totalSessions` 不变（普通 append 不改变 session 集合）

#### Scenario: 旧客户端反序列化缺 sessionListChanged 字段时退化为不触发 loadProjects
- **WHEN** 前端收到的 file-change payload 缺 `sessionListChanged` 字段（向后兼容行为见 `[[push-events::file-change]]`）
- **THEN** 前端 SHALL 把缺字段视为 `false`
- **AND** 当 `projectListChanged === false && deleted === false` 时 Sidebar SHALL NOT 触发 `loadProjects(refresh: true)`

### Requirement: 冷启共享项目数据

Sidebar 与 Dashboard 在应用冷启期间 SHALL 复用同一份前端 project/repositoryGroups 启动数据源，避免为同一批项目发现信息并行发起重复 IPC。实现 MUST 保持 session loading 边界：共享数据只覆盖项目 / repository group 摘要，不预加载所有项目 sessions。

#### Scenario: Dashboard 复用 Sidebar 的 repository groups 请求

- **WHEN** 应用冷启且无 active tab，Sidebar 与 Dashboard 同时渲染
- **THEN** 前端 SHALL 至多发起一次项目发现 IPC 请求用于获取 repository groups / projects
- **AND** Dashboard SHALL 从共享结果派生项目卡片
- **AND** Dashboard SHALL NOT 额外调用 `listProjects` 获取同一批冷启项目数据

#### Scenario: 共享项目数据不触发所有项目 sessions 加载

- **WHEN** Dashboard 从共享 project/repositoryGroups 数据渲染项目概览
- **THEN** Dashboard SHALL NOT 为每个项目调用 `listSessions`
- **AND** Sidebar SHALL 仍只为当前 `selectedProjectId` 请求第一页 sessions

#### Scenario: 共享请求失败时组件独立展示错误

- **WHEN** 冷启项目发现 IPC 失败
- **THEN** Sidebar 和 Dashboard SHALL 复用同一个失败结果
- **AND** 两个组件 MAY 按各自 UI 展示 loading/error 状态
- **AND** 前端 SHALL NOT 因两个组件同时等待而重复发起同一冷启项目发现请求

### Requirement: Sessions store stale-while-revalidate 缓存

Sidebar SHALL 通过新增的模块级单例 store（session 列表 SWR 缓存 store）以 `projectId` 为 key 缓存最近访问过的 `PaginatedResponse<SessionSummary>` 列表（含已 patch 的 metadata）。切 project 时（含初次访问 / 来回切换）Sidebar 触发 `loadSessions` SHALL 先从 store 同步读取缓存：

- **命中**（store 有该 `projectId` 条目）：Sidebar SHALL 立即用缓存数据 hydrate 列表（`sessions` / `sessionsNextCursor` / `sessionsTotal` 三态），**不**经过"加载中..."文本中间态；同时后台 SHALL 触发 SWR refresh（重新调 `listSessions` 拉首页），refresh 返回时 SHALL 通过下文规约的"首页 refresh ghost reconcile"路径 merge 进当前列表（保留尾部、保留分页 cursor），与现有 file-change 兜底刷新路径行为一致。
- **未命中**（store 无该 `projectId` 条目）：Sidebar SHALL 走现有"非 silent 替换式加载"路径——`sessionsLoading=true` + 等 `listSessions` resolve + replace 首页；resolve 后 store SHALL 写入该 `projectId` 条目。

**首页 refresh ghost reconcile**：SWR refresh 是首页（`cursor=null`）请求时，store 与 Sidebar `sessions` 数组的合并 SHALL 满足：
- **新 page 内出现的 sessionId** SHALL 用 refresh 数据覆盖（含 metadata 字段）
- **新 page 的 `pageSize` 范围内但** refresh 数据中**缺失**的 sessionId（即落在 mtime 倒序前 `pageSize` 条但服务端已不返回的）SHALL 从 store 与 Sidebar `sessions` 中**移除**——表示该 session 文件已被删除 / 重命名 / 移出首页范围
- 超出首页 `pageSize` 范围的尾部条目 SHALL 保留（pinned/hidden reconcile 与翻页累加的尾部不受 refresh 影响）

非首页（`cursor !== null`）的 refresh / loadMore SHALL NOT 触发上述删除 reconcile——仅作为追加列表使用，保留既有 `applySilentRefresh` "merge 保留尾部 + 保留 cursor" 行为不变。

Store 容量 SHALL 按 LRU 上限 16 个 `projectId` 淘汰；命中时 SHALL bump 到队首避免冷热混淆。Store **不**持久化到磁盘——进程重启时为空，依赖后端 `MetadataCache` 持久化（详见 `ipc-data-api` spec §"`MetadataCache` 启动 hydrate 与退出 dump"）让冷启时骨架阶段直接命中真值。

Metadata patch 路径（`session-metadata-update` event listener）SHALL 同时写入 store —— in-place mutate 缓存条目内对应 sessionId 的字段，保持 store 与显示列表的一致性，避免下次切回此 project 时缓存返回过期值。

**已知 stale-update race（接受作为最佳努力）**：用户在快速切换路径（A → B → A）+ 期间 A 项目某 session 文件变更时，第一次 A 访问触发的旧扫描可能在 abort 之前已 emit 出旧值的 `SessionMetadataUpdate`，事件在 push 队列上滞后到用户切回 A 时才被 listener 处理，旧 update 会**短暂覆盖**新 metadata 值。该 race 的触发窗口窄（短时间快速切换 + 文件同期变更），file-change watcher 短延迟 debounce 后会触发 silent refresh 拉回真值兜底。本 capability **不**引入额外 IPC schema 字段（如 `scanToken` / `generationId`）来精确丢弃 stale update——接受为已知 race，不规约 listener 侧的 scanToken 校验逻辑。

#### Scenario: 切回曾访问的 project 时立即展示缓存

- **WHEN** 用户先选中 project A 触发 `loadSessions("A")` 完成（store 写入 A 的 `SessionListEntry`），然后选中 project B 触发 `loadSessions("B")`，再次选中 project A
- **THEN** Sidebar SHALL 立即用 store 中 A 的缓存数据 hydrate 列表（`sessions` 数组复用缓存项，DOM 复用稳定 key），**不**显示"加载中..."文本中间态
- **AND** 后台 SHALL 触发对 A 的 SWR refresh（再次调 `listSessions("A", 20)`），返回时通过 `applySilentRefresh` merge

#### Scenario: 首次访问 project 走非 silent 加载

- **WHEN** 用户首次选中某 project，store 中无该 `projectId` 条目
- **THEN** Sidebar SHALL 走非 silent 替换式加载路径（`sessionsLoading=true` + 等 `listSessions` resolve）
- **AND** resolve 后 store SHALL 写入该 `projectId` 的 `SessionListEntry`

#### Scenario: Metadata patch 同步更新 store

- **WHEN** `session-metadata-update` listener 收到 sessionId 为 `S` 的更新
- **THEN** 系统 SHALL 同时对 store 中该 `projectId` 条目内的 `S` session 字段 in-place mutate（`title` / `messageCount` / `isOngoing` / `gitBranch`）
- **AND** 下次切回此 project 走 store cache hit 路径时，SHALL 直接展示已 patch 的真值

#### Scenario: Store LRU 超过 16 个 project 时淘汰

- **WHEN** Store 已含 16 个 `projectId` 条目，用户访问第 17 个 project 触发新条目写入
- **THEN** Store SHALL 淘汰当前最久未访问的条目后再写入新条目，store 大小始终 ≤ 16

#### Scenario: 首页 SWR refresh 删除已不存在的 session

- **WHEN** Store 中 project A 缓存含 sessionId `s1, s2, s3, s4, s5`（pageSize=20，全部在首页范围内）；用户切回 A，后台 SWR refresh 首页（cursor=null）返回 `s1, s2, s4, s5, s6`（`s3` 已被删除、`s6` 是新增）
- **THEN** Store 与 Sidebar `sessions` 数组 SHALL：保留 / 覆盖 `s1, s2, s4, s5`；移除 `s3`；插入 `s6`
- **AND** 显示的 `sessionsTotal` SHALL 用 refresh response 的 `result.total` 覆盖

#### Scenario: 非首页 refresh 不触发删除 reconcile

- **WHEN** Store 中 project A 已加载 page 1+2（cursor 已推进），随后 file-change 触发 silent refresh，但前端按"首页 only"策略仅 refresh `cursor=null`
- **THEN** refresh 返回的首页数据 SHALL 用 ghost reconcile 路径合并；page 2 尾部 sessionId 在 refresh 数据外的 SHALL **保留**（不被误删，因为它们超出 refresh 的 pageSize 范围）

### Requirement: Store `loadFirstPage` / `loadMore` 内部 generation token 取消机制

`sessionListStore` 的 `loadFirstPage(projectId, ...)` / `loadMore(projectId)` API SHALL 用 **generation token** 机制取消已过时的 in-flight 请求，让 store 自身的并发 SWR refresh / 翻页路径在快速调用时不会让旧 response 错误地覆盖更新的 entry 状态。

实现 SHALL 满足：

- store 在每个 `SessionListEntry` 上维护 `generation: number` 字段，每次 `loadFirstPage` / `loadMore` 启动时 `++entry.generation` 并记录 `my = entry.generation`
- 调用 `listSessions(...)` resolve 时 SHALL 检查 `entry.generation === my`，不等则丢弃 response（不写入 store）
- **浏览器 runtime** SHALL 额外创建 `AbortController` 挂到 fetch 路径；新 generation 启动时 SHALL `previousController.abort()` 让网络层立即释放连接
- **Tauri runtime** 由于 `invoke()` 不支持 abort，generation token 是唯一手段；后端 `LocalDataApi::list_sessions` 既有 `active_scans` per-`(projectId, cursor)` abort 机制 SHALL 自然处理后台扫描去重，前端无需主动通知后端

**Sidebar 集成边界**：Sidebar 当前**未**强制通过 store API 调 `loadFirstPage` / `loadMore`——继续走自己的 `listSessions` 直调 + `selectedProjectId` / `sessionsNextCursor` 校验路径，并通过 `sessionsLoadingMore` flag 防同 cursor 重复加载。store API 的 cancel 机制保留作为 SWR refresh 调用 + 未来 sidebar 完全使用 store 重构时的契约。

#### Scenario: store 内部并发 loadFirstPage 仅保留最新 response

- **WHEN** 调用方对同一 projectId 在第一次 `loadFirstPage` IPC 未 resolve 时再次调用 `loadFirstPage`
- **THEN** store SHALL `++entry.generation` 让第一次 response resolve 时被 `generation` 校验丢弃
- **AND** 浏览器 runtime SHALL `controller_first.abort()` 让网络层立即释放连接
- **AND** 仅最后一次 response SHALL 写入 `entry.sessions` / `entry.nextCursor` / `entry.total`

#### Scenario: store loadMore 同 cursor 不重复 fetch

- **WHEN** store `loadMore("A")` 启动 cursor=`C1` 的请求；请求未 resolve 时再次调 `loadMore("A")`（cursor 未推进，仍是 `C1`）
- **THEN** 第二次调用 SHALL 因 inflight short-circuit 直接 return，不产生新 IPC

### Requirement: Sidebar SHALL 订阅 sse-recovered / sse-lagged 触发 silent refresh

为兜底 SSE / IPC 异常路径——SSE OPEN 超时让 patch 永久丢失、backend broadcast 容量打满让 patch 静默丢弃、以及 file-change broadcast Lagged 让 enriched event 错过——Sidebar SHALL 在 `onMount` 阶段订阅 `sse-recovered` 与 `sse-lagged` 两个 transport 层 pseudo-event。

实现 SHALL 满足：

- 两个 event 共用同一恢复 handler：当前 `selectedProjectId` 非空时调 `listSessions(projectId, Math.max(sessions.length, SESSION_PAGE_SIZE))` 触发后端按**已加载范围**重新扫描
- handler SHALL 同时 schedule `loadProjects(refresh: true)`（`list_repository_groups` SWR revalidate）—— lag 期间可能错过 file-change event 的 structural 信号（字段语义见 `[[push-events::file-change]]`：`projectListChanged` / `sessionListChanged` / `deleted`），保守 SWR revalidate 让 `selectedGroup.totalSessions` 与最新 group 集合对齐
- handler SHALL **消费 response** 通过 `mergeRecoveryResponse(sessions, result.items)` 写回 sessions + store。recovery 路径**不**叠加 `applyPendingMetadata`——buffer 可能保留了 lag 之前的旧 SSE patch（buffer 跨 SSE 异常周期持久），叠加会让 buffer 旧值覆盖刚刚 mergeRecoveryResponse 写入的 response 新真值，stale 自愈失败。`mergeRecoveryResponse` 是 SSE 恢复路径**专用**合并：
  - **cache hit 真值仅在 response 里**：后端 fast-path inline 返完整 metadata，**不**入后台扫描 spawn、**不** emit SSE patch；前端 SHALL 让 response 真值覆盖 prev
  - **cache miss 真值**仍走 SSE patch 路径——后端 spawn 后台扫描后广播 session-metadata-update（payload 形态见 `[[push-events::session-metadata-update]]`），UI listener 写回；response 里 cache miss 项是骨架，`mergeRecoveryResponse` 在 next 是骨架时保留 prev
  - prev 中不在 next 内的尾部条目 SHALL 保留（防 next.length < prev.length 漏项）
- race guard：异步完成时 `projectId !== selectedProjectId` SHALL 跳过写回
- handler SHALL 在 `onDestroy` 阶段清理 unsubscribe

**Tauri runtime 兼容性**：`sse-recovered` 由 BrowserTransport 内部 synthesize（仅 server-mode 浏览器 client 触发）；`sse-lagged` SHALL 由两路 emit 共同承担：

- **server-mode 浏览器路径**：BrowserTransport 在 SSE broadcast Lagged 路径 synthesize（既有行为不变）
- **Tauri runtime 路径**：Tauri host 在 file-change broadcast bridge 收到 Lagged 时 emit `sse-lagged`（payload 形态见 `[[push-events::sse-lagged]]`）；TauriTransport SHALL 显式 listen 后通过 dispatch 路径 fanout 给所有 handler

前端 Sidebar 的 sse-lagged / sse-recovered 订阅 SHALL **不再**包在 `if (!isTauriRuntime())` 门禁内——两 runtime 下 handler 都注册：

- Tauri 下 `sse-lagged` 通过 TauriTransport listen 路径触发 handler；`sse-recovered` 在 Tauri 下不会被 emit（IPC channel 不会"恢复"），但订阅 noop 无副作用
- server-mode 下 `sse-recovered` / `sse-lagged` 通过 BrowserTransport synthesize 路径触发 handler

#### Scenario: sse-recovered 触发当前 project 的 silent refresh

- **WHEN** Sidebar 已 mount + `selectedProjectId === "A"`
- **AND** transport 层 SSE 恢复后 emit 一次 `sse-recovered` event
- **THEN** Sidebar SHALL 调 `loadSessions("A", true)` 触发 silent refresh
- **AND** Sidebar SHALL 同时 schedule `loadProjects(refresh: true)` 让 `selectedGroup.totalSessions` 与最新真值对齐
- **AND** silent merge SHALL 保留之前已 patch 的 metadata 真值不被骨架值覆盖

#### Scenario: sse-lagged 同样触发 silent refresh（server-mode 浏览器）

- **WHEN** SSE handler 因 broadcast Lagged 推送 sse-lagged event 给浏览器 client（payload 形态见 `[[push-events::sse-lagged]]`）
- **THEN** transport 层 SHALL 转 `sse-lagged` event name 派发给 Sidebar handler
- **AND** Sidebar SHALL 调 `loadSessions(selectedProjectId, true)` 触发 silent refresh
- **AND** Sidebar SHALL 同时 schedule `loadProjects(refresh: true)` 兜底 lag 期间错过的 structural 信号
- **AND** 后续后端重新扫描 emit 的 session-metadata-update（payload 形态见 `[[push-events::session-metadata-update]]`）SHALL 通过 SSE patch 路径正常写回

#### Scenario: Tauri runtime 下 file_tx Lagged 触发 sse-lagged

- **WHEN** Tauri host 的 file-change bridge broadcast receiver 返回 Lagged(n)（broadcast capacity 满 + slow renderer）
- **THEN** Tauri host bridge SHALL emit `sse-lagged`（payload 形态见 `[[push-events::sse-lagged]]`）让 webview 收到
- **AND** TauriTransport SHALL 通过 listen 桥接到 dispatch 路径让所有 handler 收到
- **AND** Sidebar handler SHALL 触发，调 `loadSessions(selectedProjectId, true)` + `loadProjects(refresh: true)`
- **AND** bridge SHALL NOT 退出 loop，继续处理后续 event

#### Scenario: Sidebar sse 订阅在 Tauri runtime 下也注册

- **WHEN** Sidebar `onMount` 在 Tauri runtime 下执行
- **THEN** sse-lagged / sse-recovered 订阅注册 SHALL **不**被 `isTauriRuntime()` 门禁包裹，handler 注册路径在两 runtime 下统一
- **AND** Tauri runtime 下 `sse-recovered` 不会被触发（订阅 noop 无副作用）；`sse-lagged` 在 Tauri host bridge 检测到 Lagged 时通过 emit 触发

#### Scenario: 已翻到 page 2+ 时 SSE 异常仍补齐尾部 metadata

- **WHEN** 用户已翻到 page 3（sessions.length = 60），SSE 期间 lag + 恢复
- **THEN** handler SHALL 调 `listSessions(selectedProjectId, 60)` 让后端按 60 条范围重新扫描
- **AND** mergeRecoveryResponse 保留 prev 中不在 next 内的尾部条目

### Requirement: Metadata 占位字段视觉渐显

骨架行 SHALL 用条件 CSS class `.metadata-pending` 标识占位状态，class 上 SHALL 应用**静态** opacity 占位样式（不含 `infinite` 动画 / `background-position` 等 paint-only 周期重绘）；元数据 patch 到达后 SHALL 移除 class，触发 CSS `transition: opacity 150ms ease-out` 让真值 fade-in。

为避免 metadata 字段（`title` / `messageCount` / `isOngoing` / `gitBranch`）从骨架占位（`null` / `0` / `false`）到真值的瞬变带来视觉断层，骨架态用静态 opacity（如 `0.55`）+ 静态背景（如 `linear-gradient` 占位渐变）让"未加载"在视觉上与真值有层次差，但**不**通过周期动画提示"加载中"——遵循 `PRODUCT.md::Design Principle 5`「实时但不闪烁，避免 loading 中间态打断阅读」与 `DESIGN.md::The One Live Signal Rule` 边界条款「Skeleton placeholder 必须**静态** opacity 占位，**禁用** shimmer」。

实现 SHALL 满足：

- 每条 session 渲染时 SHALL 通过 `class:metadata-pending={!session.title && session.messageCount === 0 && !session.isOngoing}` 判定（与既有占位回退路径同条件）
- `.metadata-pending` 的 CSS SHALL **不**含 `animation` / `@keyframes` 任何 `infinite` 或周期性 `background-position` / `background-color` / `opacity` 抖动；`transform` / `opacity` 的一次性短动画（≤ 250 ms）允许，但不在本 Requirement 范围内
- `transition` SHALL 用 CSS 而**非** Svelte `transition:fade`——metadata patch 是字段 mutate 不重建 DOM 节点，Svelte transition 指令绑定 mount/unmount 不触发
- 渐显时长 SHALL 在 `100 ms ≤ X ≤ 200 ms` 区间（取 `150 ms` 作为默认值）；过短等同瞬变无渐显感，过长让用户感到"卡顿等待"
- 骨架占位视觉 SHALL NOT 依赖 metadata 请求等待时长（"已请求 N ms"）—— 占位视觉由占位条件本身（骨架字段 = `null` / `0` / `false`）决定，与到达时间阈值无关；具体实现选型（是否需要 `requestedAt` 跟踪用于非视觉用途如 telemetry）不在本视觉契约范围内

#### Scenario: 骨架行渲染时显示静态占位

- **WHEN** Sidebar 渲染一条骨架 session（`title=null`，`messageCount=0`，`isOngoing=false`）
- **THEN** 该行 SHALL 携带 `.metadata-pending` class 应用静态 opacity 占位样式
- **AND** 该行的 `.session-title-text` / `.session-meta` 元素 `getComputedStyle().animation` SHALL 为 `none` 或等价空值
- **AND** title 区显示既有占位回退（**完整 sessionId**，由 CSS `text-overflow: ellipsis` 自然截断；与 `Requirement: 会话项展示::Scenario: 无标题的会话` 一致，**禁止**在 JS 侧再手动 `slice(0, 8) + "…"`）

#### Scenario: Metadata patch 到达后字段渐显

- **WHEN** `session-metadata-update` listener 收到 sessionId 为 `S` 的更新，更新该 session 的 `title` 为 `"My Session"`
- **THEN** 该行 SHALL 在 patch 同帧移除 `.metadata-pending` class
- **AND** title 文本 SHALL 通过 CSS `transition: opacity 150ms ease-out` 从骨架占位的 `opacity: 0.55` 渐升到正常的 `opacity: 1`（不是 `0 → 1`——骨架态本身就用 `0.55` 半透明而非完全透明，避免内容彻底消失再重绘的视觉断层）
- **AND** 整个过程中 SHALL 不出现 shimmer / 周期重绘 / `background-position` 平移等动画

#### Scenario: Metadata 长时间未到达仍保持静态

- **WHEN** 某条骨架 session 的 metadata 在 `> 1500 ms` 后仍未通过 `session-metadata-update` 推送到达
- **THEN** 该行 SHALL 仍保持与 `< 1500 ms` 时**完全一致**的静态 opacity 占位，**不**升级为任何形式的 shimmer / 周期动画 / "加载更慢了" 视觉提示
- **AND** `.metadata-pending` class 的 CSS 样式 SHALL 不引用任何与等待时长相关的 CSS 自定义属性 / `:hover` 之外的状态选择器

### Requirement: Session row branch + cwd chip 替代行尾 cwd 全路径

Sidebar 会话项渲染 SHALL 在每条 session 行右侧 chip group 显示 worktree 归属信息：
1. **分支 chip**：当 `session.gitBranch` 非空时渲染 git icon + branch 名（沿用既有项目切换器 dropdown item 分支 chip 样式）
2. **cwd hint chip**：当 `session.cwdRelativeToRepoRoot` 非空且非空字符串时渲染 `…/<lastTwoSegs>`（路径取最后两段：例如多段相对路径取末两段、`.claude/worktrees/feat-x` → `worktrees/feat-x`）

Sidebar 会话项 SHALL NOT 渲染老版本基于 `cwdTailLabel(session.cwd)` 的 `<span class="session-cwd">` 行尾全路径 label——该 label 渲染逻辑 SHALL 从 Sidebar 实现中移除。

Session 详情视图顶部 cwd badge SHALL 保持不变（详情页需要完整 cwd path）。

#### Scenario: session 行渲染分支 chip
- **WHEN** session.gitBranch = "feat/x"
- **THEN** session 行右侧 SHALL 渲染 git icon + "feat/x" chip

#### Scenario: 子目录 cwd session 行渲染 cwd hint chip
- **WHEN** session.cwdRelativeToRepoRoot = "crates"
- **THEN** session 行右侧 SHALL 渲染 "crates" chip（短路径直接全显）

#### Scenario: 深层子目录截取最后两段
- **WHEN** session.cwdRelativeToRepoRoot = ".claude/worktrees/feat-x"
- **THEN** session 行右侧 cwd hint chip SHALL 显示 "worktrees/feat-x"

#### Scenario: repo 根 session 不渲染 cwd hint
- **WHEN** session.cwdRelativeToRepoRoot 为 None 或 空字符串
- **THEN** session 行 SHALL NOT 渲染 cwd hint chip（仅渲染分支 chip）

#### Scenario: 行尾 cwd 全路径 label 已移除
- **WHEN** 检查 Sidebar 实现的 cwd 渲染逻辑
- **THEN** SHALL NOT 包含 `<span class="session-cwd">` / `cwdTailLabel` 老版本行尾全路径 label 渲染

### Requirement: selectedGroupId 与 worktree id 分层维护

Sidebar 当前选中的项目入口 SHALL 用 `selectedGroupId` 字段持 `RepositoryGroup.id`，用于顶层导航 / 列表分页 / push event 过滤 / 用户配置持久化。Session tab identity 与详情 API 入参归 `[[tab-management]]` owner；Sidebar 仅消费会话列表项携带的 worktree / group / session 三元信息来发起打开行为，不在本 Requirement 重复定义 tab 字段。

Sidebar 通过 `SessionSummary.worktreeId` + `SessionSummary.groupId` + `SessionSummary.sessionId` 三元组把 group 级列表项与具体 worktree session 关联：sidebar 顶层 `selectedGroupId` 保持 group id；涉及 tab 创建、tab 高亮归属与详情加载 identity 的行为由 `[[tab-management]]` 对应 Requirement 守护。

收敛点行为契约：

- 顶层导航状态 SHALL 持 group id（不再是 worktree id）
- session 列表分页 SHALL 调 `listGroupSessions(groupId, pageSize, cursor)` 拉合并 sessions
- session 列表缓存 cache key SHALL 用 `(groupId, filterWorktreeId | null)` 复合 key 区分 filter 维度，否则切 filter 串台
- push event `session-metadata-update` SHALL 按 `[[push-events]]` 定义的 group id 语义过滤：前端 filter 按 event 所属 group 等于 `selectedGroupId` 匹配；worktree id 字段仅供 `[[tab-management]]` / detail 路径消费
- 后台任务 `active_scans` per-key cancel 分两类 key：detail 拉取 = `(project_id /*worktree id*/, session_id)`（不变）；group 分页拉取 = `(group_id, page_cursor_hash)`（新加）
- session 打开后的 tab identity 与详情 API 入参 SHALL 由 `[[tab-management]]` 守护；Sidebar 不重复定义 tab 字段
- Command Palette 全局搜索 SHALL 改调 `listGroupSessions(selectedGroupId, pageSize, null)` 拿合并候选；候选项 onclick 时 SHALL 按 `candidate.worktreeId` 交给 `[[tab-management]]` 创建 tab
- 用户配置 `selected_project_id` 改 `selected_group_id`；启动时若读到老 worktree id，按 grouper 反查 group id 后改写一次（迁移）
- 项目 memory / prefs（如有 per-project state）SHALL **不变**（维持 per-worktree，与 detail API 一致）

单 worktree group 时 group id 与 worktree id 字符串相同（grouper 在 standalone project 场景下 `group.id = project.id`），单 worktree 项目用户无感知 ID 变化。

#### Scenario: push event 按 groupId filter
- **WHEN** 用户当前 `selectedGroupId = "group-X"`，push 流推送 `session-metadata-update` event 含 `groupId: "group-Y"` + `projectId: "wt-Z"`
- **THEN** 前端 SHALL 丢弃该 event（不 patch 到当前列表）

#### Scenario: push event 同 group 命中
- **WHEN** 用户当前 `selectedGroupId = "group-X"`，event 含 `groupId: "group-X"` + `projectId: "wt-X1"` + sessionId
- **THEN** 前端 SHALL 在列表中找到对应 session 并 patch metadata

#### Scenario: CommandPalette 全局搜索走 group 维度
- **WHEN** 用户在 Command Palette 输入查询词，当前 `selectedGroupId = "group-X"`
- **THEN** Command Palette SHALL 调 `listGroupSessions("group-X", pageSize: 200, cursor: null)` 拿合并候选（覆盖所有 worktree 的 session）
- **AND** 候选项 onclick 时 SHALL 按 `candidate.worktreeId` 创建 tab，tab 内 detail 路径走 worktree id

#### Scenario: 列表缓存 cache key 含 worktree filter
- **WHEN** 用户在 group-X 内切 filter 从 "全部" → "wt-X1" → "全部"
- **THEN** session 列表缓存 SHALL 用 `(groupId, filterWorktreeId | null)` 作为 cache key 区分三个状态的缓存
- **AND** 切回"全部"时 SHALL 命中第一次"全部"的缓存，不重发 IPC

#### Scenario: 单 worktree group group id 等于 worktree id
- **WHEN** standalone project 转化的 RepositoryGroup
- **THEN** `group.id` SHALL 等于 `group.worktrees[0].id`（即 encoded project dir 名）
- **AND** 单 worktree 项目用户配置 `selected_project_id` 在迁移前后字符串相同，无需写迁移

### Requirement: Worktree filter chip cluster for multi-worktree group

Sidebar SHALL 在顶部（与 Memory entry 同 region、Session search bar 之上、独占一行）渲染 worktree filter chip cluster，**仅当**当前选中的 RepositoryGroup `worktrees.length > 1` 时可见；单 worktree group 或退化的 flat fallback 模式下 SHALL 隐藏该控件。

chip cluster 实现 SHALL 用独立子组件（专用 Worktree filter chip cluster 组件，横向 flex + `overflow-x: auto` + `scrollbar-width: none`），**不**复用通用 Dropdown 组件（dropdown 形态在多 wt group 下迫使用户「打开 → 选 → 关闭」两步交互且看不到全 wt 名总览）。

chip 数据顺序 SHALL 与原 dropdown 一致：
1. 「全部」chip（默认 selected，无 `⌗` 前缀，纯文字 label）
2. group 内 `isRepoRoot=true` 的 worktree（repo 根，label `⌗{group.name}`）
3. 其它 worktree 按 `is_main_worktree` 优先 + `most_recent_session` 倒序（label `⌗{worktree.name}`）

每个 chip SHALL 显示：
- 「全部」chip：`全部` 文字 + 可选 group 内 session 总数徽章
- worktree chip：`⌗{worktree-display-name}`（与 PR-A meta 行 `.session-wt-label` 同字体 mono + 同色族 muted-secondary，让顶部 chip 与行内 label 是同一信号语言的两个尺度）

chip 视觉规范 SHALL 沿用 `DESIGN.md::The Persistent Selection Is Quiet Rule`（持久选中不用 Focus Blue）+ `DESIGN.md::The Border Before Shadow Rule`（用 border-emphasis 不用 shadow）+ `DESIGN.md::The Status Owns the Color Rule`（不新增彩色装饰）+ `DESIGN.md::The Machine Information Rule`（worktree id 是机器信息用 mono）：
- default：transparent 背景 + `--color-text-secondary` + transparent border
- hover：`--tool-item-hover-bg` 背景
- active（持久选中）：`--color-surface-overlay` 背景 + `--color-text` + `--color-border-emphasis` 1px border
- focus-visible（键盘焦点 / 瞬时焦点允许 blue）：`--color-accent-blue` 2px outline + 1px offset

chip cluster SHALL 实现以下键盘 / ARIA 行为（`PRODUCT.md` 「桌面优先的可键盘操作产品 UI」+ DESIGN.md 「交互控件应具备明确 focus-visible 状态、ARIA 语义和可达标签」硬约束）：
- 容器：`role="radiogroup"` + `aria-label="按 worktree 过滤会话"`
- 每个 chip：`role="radio"` + `aria-checked` 反映选中态 + 文字本身作为可达标签
- 选中 chip 的 `tabindex="0"`，未选中 `tabindex="-1"`（roving tabindex 模式）
- 焦点在某 chip 时按 `ArrowRight` / `ArrowLeft` SHALL 切到下 / 上一个 chip 并触发选中（与 dropdown 一致的「即选即触发」语义）
- 在某 chip 上按 `Enter` 或 `Space` SHALL 切到该 chip（鼠标点击的键盘等价）
- 边界（不绕回）：在**最末** chip 上按 `ArrowRight` SHALL 停在最末（不回到首位）；在**最首** chip 上按 `ArrowLeft` SHALL 停在首位（不跳到末位）。两端均不绕回，遵循 WAI-ARIA radiogroup 模式
- focus-visible 状态 SHALL 用 `--color-accent-blue` 2px outline 表达瞬时键盘焦点

chip 单选语义 SHALL 与原 dropdown 完全一致——切 chip 触发 `worktreeFilter` state 变更，复用既有 `$effect` → `loadSessions(filter)` 链；filter 切换 SHALL 重置当前 group 的 session 列表分页状态（清空已加载页 + cursor 重置 + session-list 容器 `scrollTop` 重置为 0）；server-side filter 通过 cursor `Exhausted` 表达的逻辑（详 ipc-data-api spec `Expose group session listing via k-way merge pagination`）保持不变。

filter state SHALL session-scoped（仅本次会话状态），切 group 时重置为「全部」，不跨会话持久化。

**自动补页保护**：若 server-side filter 后某页 sessions 数仍 < `pageSize`（理论上仅在该 worktree 接近耗尽时发生），sidebar SHALL 自动 loadMore 直到填满一屏或 cursor 全部 `Exhausted`，避免视觉空白。

**chip overflow 处理**：chip 数过多导致总宽超 sidebar 宽度时，cluster 容器 SHALL 横向滚动；scrollbar 隐藏（`scrollbar-width: none` + WebKit `::-webkit-scrollbar { display: none }`）；`flex-wrap: nowrap` 保持单行 32px 高度不变（与 Memory entry / Session search bar 同行高族）；容器右侧 SHALL 渲染 fade mask（`mask-image: linear-gradient(to right, black calc(100% - 16px), transparent)` 或等价 `::after` 渐变叠层），让用户感知"右侧还有更多 chip"——隐藏 scrollbar + 缺乏 overflow indicator 会让 5+ chip 场景下后段 chip 不可发现，违背 PRODUCT.md「快速定位」原则。

#### Scenario: 多 worktree group 默认显示 chip cluster
- **WHEN** 用户切到 worktrees.length === 2 的 group
- **THEN** sidebar 顶部 SHALL 渲染 worktree filter chip cluster 组件，含「全部」+ 2 个 worktree chip 共 3 个 chip
- **AND** 「全部」chip SHALL 默认 selected（active 视觉态）

#### Scenario: 单 worktree group 隐藏 chip cluster
- **WHEN** 用户切到 worktrees.length === 1 的 group（standalone project）
- **THEN** sidebar 顶部 SHALL NOT 渲染 worktree filter chip cluster 组件

#### Scenario: 切 chip 构造 server-side filter cursor
- **WHEN** 用户在多 worktree group（含 worktree `wt-A` / `wt-B` / `wt-C`）点击 `⌗wt-B` chip
- **THEN** session 列表 SHALL 立即清空
- **AND** session-list 容器 `scrollTop` SHALL 重置为 0（避免旧滚动位置残留导致新列表初始停在中段或空白边界）
- **AND** 前端 SHALL 构造 cursor `{ "wt-A": Exhausted, "wt-B": NotStarted, "wt-C": Exhausted }` (base64 JSON) 调 `list_group_sessions(groupId, pageSize, cursor)`
- **AND** server 返回 sessions SHALL 仅含 `wt-B` 的 sessions
- **AND** `⌗wt-B` chip SHALL 切到 active 视觉态，原「全部」chip 切回 default 态

#### Scenario: 从深滚动位置切 filter 时列表回到顶部
- **WHEN** 用户在多 wt group 已选「全部」并把 session-list 滚到下半部（`scrollTop=400`），点击 `⌗wt-B` chip
- **THEN** session-list 容器 `scrollTop` SHALL 重置为 0，新列表从最顶部开始展示

#### Scenario: 切回「全部」清空 cursor
- **WHEN** 用户在已选 `⌗wt-B` 状态点击「全部」chip
- **THEN** 前端 SHALL 调 `list_group_sessions(groupId, pageSize, null)`（cursor 重置 null）
- **AND** server 返回 sessions SHALL 含全 group 的合并条目
- **AND** 「全部」chip SHALL 切到 active 视觉态

#### Scenario: 切 group 重置 chip 选中
- **WHEN** 用户从 group A（选中 `⌗wt-B`）切到 group B
- **THEN** 「全部」chip SHALL 自动重置为 active（无论 group A 上次选中哪个 chip）

#### Scenario: 切 group 时 session-list 滚动位置重置
- **WHEN** 用户在 group A 已把 session-list 滚到下半部（`scrollTop=400`），点击项目切换器切到 group B
- **THEN** session-list 容器 `scrollTop` SHALL 重置为 0（与 chip 切换的 scroll reset 语义对齐——任何使 sessions 集合整体替换的操作都 SHALL 滚回顶部）
- **AND** group B 的新列表 SHALL 从最顶部开始展示

#### Scenario: chip cluster 横向滚动 overflow
- **WHEN** group 含 7 个 worktree（chip 数 8 个含「全部」），sidebar 宽 280px
- **THEN** chip cluster 容器 SHALL 横向滚动；scrollbar 不可见但 wheel / touch / 拖拽可滚
- **AND** chip cluster 高度 SHALL 保持 32px（与 Memory entry / Session search 同行高族），不换行
- **AND** 容器右边缘 SHALL 渲染 fade mask（线性渐变到透明）让用户感知"右侧还有更多 chip"

#### Scenario: 键盘方向键切换 chip
- **WHEN** 用户键盘 Tab 到 chip cluster（焦点落在当前 active chip 上），按 `ArrowRight`
- **THEN** 焦点 SHALL 移到下一个 chip 并触发选中（`worktreeFilter` 状态更新 + session 列表重拉）
- **AND** 新焦点 chip 的 focus-visible outline SHALL 用 `--color-accent-blue` 2px 表达
- **AND** 在最末 chip 上按 `ArrowRight` 不绕回头部（停止在末尾，遵循 WAI-ARIA radiogroup 模式）
- **AND** 在最首 chip（即「全部」chip）上按 `ArrowLeft` SHALL 停在首位（不跳到末位）

#### Scenario: 自动补页防止首屏视觉空白
- **WHEN** server-side filter 返回某页 sessions 数 < pageSize 但 cursor 还有非 Exhausted worktree
- **THEN** sidebar SHALL 自动续调 loadMore 直到填满一屏或 cursor 全部 Exhausted

### Requirement: Worktree chip 右键菜单

worktree filter chip cluster 渲染的每个 worktree chip 元素 SHALL 通过 `use:contextMenu` action 挂载右键菜单，让用户对 worktree 路径执行"复制路径 / 在编辑器打开 / 在终端打开 / 在 Finder/Explorer 中显示"等核心操作。菜单 items 由 `buildWorktreeChipItems` factory 构造；factory 入参含 `{ path: string; name: string }`；`open_in_terminal` / `open_in_editor` 走对应 IPC，路径长时通过 `pathLabel: { short, full }` 截断显示。

#### Scenario: 右键 worktree chip

- **WHEN** 用户在 sidebar 任一 worktree chip 上右键
- **THEN** SHALL 弹出含 "复制路径"、"在编辑器打开"、"在终端打开"、"在 Finder/Explorer 中显示" items 的菜单
- **AND** 触发位置遵循 `frontend-context-menu` viewport 边界 clamp 规则
- **AND** chip 上 `use:contextMenu` action 的 `stopPropagation` 阻止事件 bubble 到 sidebar 会话项的右键 handler

#### Scenario: 在终端打开 worktree 目录

- **WHEN** 用户点击 "在终端打开"
- **THEN** SHALL 调 `open_in_terminal(worktree.path)` IPC
- **AND** 后端按 Settings `terminalApp` 分流；macOS 走 `open -a <App> <path>`、Windows 走三级 fallback（`wt.exe` → PowerShell → cmd）、Linux 走 `x-terminal-emulator --working-directory=<path>` 或 DE-specific 命令
- **AND** 终端 app SHALL 仅 cd 到 worktree 目录，**不**执行任何 shell 命令

#### Scenario: 在编辑器打开 worktree

- **WHEN** 用户点击 "在编辑器打开"
- **THEN** SHALL 调 `open_in_editor(worktree.path, None, None)` IPC（不带行号参数）
- **AND** 后端按 Settings `externalEditor` 分流；以目录形式打开（VS Code/Cursor 直接接受目录参数；Zed/Sublime 同；System fallback 走 `open <path>` 等价行为）

### Requirement: 项目卡右键菜单

Sidebar 渲染的每个项目卡（含项目名称 + worktree chip cluster 的容器）SHALL 通过 `use:contextMenu` action 挂载右键菜单，items 由 `buildProjectCardItems` factory 构造，包含"复制项目路径 / 复制项目名 / 在编辑器打开项目 / 在终端打开项目根目录"。项目卡级菜单与 worktree chip 级菜单 SHALL 通过事件 `stopPropagation` 互不穿透——chip 级 action 拦截后，事件不冒泡到 project card；project card 级 action 仅在用户点中卡片本体（非 chip）时触发。

#### Scenario: 右键项目卡

- **WHEN** 用户在项目卡的非 chip 区域（项目名称 / 卡片背景）上右键
- **THEN** SHALL 弹出含 "复制项目路径"、"复制项目名"、"在编辑器打开"、"在终端打开" items 的菜单

#### Scenario: 项目卡 vs chip 菜单互不穿透

- **WHEN** 用户在项目卡内部的 worktree chip 上右键
- **THEN** chip 的 `use:contextMenu` action SHALL 优先触发并 `stopPropagation`
- **AND** 项目卡级菜单 SHALL **不**触发
- **AND** 用户感知：右键 chip 弹 chip 菜单；右键卡片其它区域弹项目卡菜单

#### Scenario: 在编辑器打开项目根目录

- **WHEN** 用户点击 "在编辑器打开"
- **THEN** SHALL 调 `open_in_editor(project.path, None, None)` IPC
- **AND** project.path 是已发现项目的根目录绝对路径

### Requirement: Sidebar 既有"右键菜单" Requirement 保持不变

Sidebar 会话项的右键菜单（已有 Requirement "右键菜单"）SHALL 不被 Phase 2 改动——会话项菜单 items（"在新标签页打开 / Open in New Pane / 置顶 / 隐藏 / 复制 Session ID / 复制恢复命令"）保持原行为；Phase 2 仅在会话项之外的 worktree chip 与项目卡上**新增**菜单挂载点，不影响会话项行为。

#### Scenario: 会话项右键菜单回归测试

- **WHEN** 用户在 sidebar 会话项上右键（既不是 worktree chip 也不是项目卡）
- **THEN** SHALL 弹出 Phase 1 既有会话项菜单（"在新标签页打开 / Open in New Pane / 置顶/取消置顶 / 隐藏/取消隐藏 / 复制 Session ID / 复制恢复命令"）
- **AND** 菜单内容、顺序、文案 SHALL 与 Phase 1 完全一致

### Requirement: Store `loadMore` 实现 leading + trailing 限频

`sessionListStore.loadMore(projectId)` API 自身 SHALL 实现 **leading + trailing 组合** debounce，让调用方在高频触发场景下（如未来 sidebar 把 `maybeLoadMoreSessions` 直接转发到 store）无需自己实现限频。具体 debounce 阈值（短窗口长度，约 100 ms 量级）为实现 tuning，不在 spec 层固定。

API 内部行为 SHALL 满足：

1. **Inflight short-circuit（最先判断）**：相同 `currentCursor` 的请求在飞时直接 return（已有相同 cursor 的请求 inflight）
2. **Leading**：不在 cooldown 窗口内时立即 fire fetch，并记录本次触发时间
3. **Trailing**：处于 cooldown 窗口内时合并到单一 trailing timer（已 pending 时不重复 schedule），timer 触发时**再次走 inflight short-circuit 判定**，仍未 inflight 才发 fetch

**Sidebar 集成边界**：当前 Sidebar 的 `loadMoreSessions` **不**直接调 `store.loadMore`——继续走原 `listSessions(projectId, pageSize, cursor)` IPC 直调路径，并通过 `sessionsLoadingMore` flag 提供 leading-fire + inflight short-circuit 等效保护。Sidebar 现有 `maybeLoadMoreSessions` 由 scroll 事件触发，scroll 事件在用户停下后会自然停止，trailing-fire 的边际收益（仅在用户停顿在 debounce 阈值附近的边角场景）相对引入 store-sidebar reactive 同步复杂度（subscribe / unsubscribe / pendingMetadataUpdates buffer 与 store entry 的双写）不划算。store.loadMore 的 leading+trailing 实现保留作为可选 API + 未来重构契约。

#### Scenario: store loadMore leading 立即触发 + inflight short-circuit

- **WHEN** 调用方在短 cooldown 窗口内连续多次调 `store.loadMore("A")`
- **AND** 第一次 fetch 仍在飞（未 resolve）
- **THEN** store SHALL 在第一次调用立即 fire 1 次 IPC（leading）；后续调用 SHALL 因 inflight short-circuit 全部丢弃

#### Scenario: store loadMore cooldown 内多次调用合并为一次 trailing fire

- **WHEN** 第一次 `store.loadMore("A")` leading fire 后 fetch 已 resolve（不再 inflight）；接下来 cooldown 窗口内调用方再调多次 `loadMore("A")`
- **THEN** 多次 cooldown 内调用 SHALL 合并为单一 trailing timer；trailing 触发时若仍未 inflight，SHALL 再 fire 1 次 fetch
- **AND** 总 fetch 数 SHALL ≤ 2（leading 1 + trailing 1）

#### Scenario: store loadMore 单次调用后停顿不重复 fire

- **WHEN** 调用方调 `store.loadMore("A")` 一次（leading fire），fetch resolve 后调用方停止调用
- **THEN** store SHALL NOT 在 cooldown 结束时再次触发 fetch（无 pending trailing timer）


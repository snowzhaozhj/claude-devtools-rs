## MODIFIED Requirements

### Requirement: Worktree filter chip cluster for multi-worktree group

Sidebar SHALL 在顶部（与 Memory entry 同 region、Session search bar 之上、独占一行）渲染 worktree filter chip cluster，**仅当**当前选中的 RepositoryGroup `worktrees.length > 1` 时可见；单 worktree group 或退化的 flat fallback 模式下 SHALL 隐藏该控件。

chip cluster 实现 SHALL 用独立子组件 `ui/src/lib/components/WorktreeChipCluster.svelte`（横向 flex + `overflow-x: auto` + `scrollbar-width: none`），**不**复用 `Dropdown.svelte`（dropdown 形态在多 wt group 下迫使用户「打开 → 选 → 关闭」两步交互且看不到全 wt 名总览，已在本 change design.md D1 论证）。

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
- **THEN** sidebar 顶部 SHALL 渲染 `WorktreeChipCluster` 组件，含「全部」+ 2 个 worktree chip 共 3 个 chip
- **AND** 「全部」chip SHALL 默认 selected（active 视觉态）

#### Scenario: 单 worktree group 隐藏 chip cluster
- **WHEN** 用户切到 worktrees.length === 1 的 group（standalone project）
- **THEN** sidebar 顶部 SHALL NOT 渲染 `WorktreeChipCluster` 组件

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
- **WHEN** 用户在 group A 已把 session-list 滚到下半部（`scrollTop=400`），点击 ProjectSwitcher 切到 group B
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

`listSessions` / `list_group_sessions` 翻页 IPC 的 `result.total` 字段含义与 `RepositoryGroup.totalSessions` 在 ALL scope 下等同（后端不变量），但前端 SHALL 直接消费 `selectedGroup.totalSessions` derived，不另行存储 `result.total` 到独立 state（避免命名链路冗余）。silent 刷新（file-change 或「有更新」按钮触发）SHALL 通过 `list_repository_groups` SWR revalidate 自动更新 `selectedGroup.totalSessions`；`loadMoreSessions` 翻页路径 SHALL **不**修改 `selectedGroup.totalSessions`（页内 total 不应改变）。

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

#### Scenario: silent 刷新时 scopeTotal 同步刷新
- **WHEN** silent 刷新（file-change 或「有更新」按钮触发）通过 `list_repository_groups` SWR revalidate 拉到新 `RepositoryGroup.totalSessions = 128`（后端检测到新增 session），filter=「全部」
- **THEN** `selectedGroup.totalSessions` derived 自动更新为 128；不破坏既有 silent 刷新对 sessions 数组合并保留尾部的语义
- **AND** 默认状态显示 SHALL 立即从 `127` 切到 `128`

#### Scenario: silent 刷新删除 session 时 scopeTotal 同步下降（ALL scope）
- **WHEN** silent 刷新通过 `list_repository_groups` SWR revalidate 拉到新 `RepositoryGroup.totalSessions = 126`（后端检测到 1 个 session 被删除，例如用户清理了 jsonl 文件），filter=「全部」
- **THEN** `selectedGroup.totalSessions` derived 自动更新为 126
- **AND** 默认状态显示 SHALL 立即从 `127` 切到 `126`
- **AND** 若被删除的 session 在已加载范围内，sidebar SHALL 同步从 `sessions` 数组移除该条；若不在已加载范围（仍在远端未翻到的部分），仅顶部 count 下降，已加载列表不变

#### Scenario: silent 刷新删除 session 时 scopeTotal 同步下降（具体 worktree scope）
- **WHEN** filter 选中 `⌗wt-A`（原 `wt-A.sessions.length === 8`），silent 刷新拉到新 `RepositoryGroup.worktrees[0].sessions.length === 7`（wt-A 内 1 个 session 被删除）
- **THEN** `groupWorktrees.find(w => w.id === filter)?.sessions.length` derived 自动更新为 7
- **AND** 默认状态显示 SHALL 立即从 `8` 切到 `7`
- **AND** 若被删除的 session 在已加载范围内，sidebar SHALL 同步从 `sessions` 数组移除该条；若不在已加载范围（仍在远端未翻到的部分），仅顶部 count 下降，已加载列表不变

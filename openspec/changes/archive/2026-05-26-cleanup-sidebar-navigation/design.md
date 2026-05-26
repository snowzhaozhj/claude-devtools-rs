## Context

issue #303 9-PR 计划批次 B 之后的 PR 5 —— 单 cap 反例清理，目标 cap 是 `sidebar-navigation`：

- 当前 baseline `spec/sidebar-navigation 44`
- 命中分布：p1 内部 mod path 1 / p2 src path 21 / p3 PR 引用 3 / p4 metric 18 / p6 lib 1
- 21 处 src path 集中在「项目选择」「会话项展示」「侧栏折叠」「Sessions store SWR 缓存」「`selectedGroupId 与 worktree id 分层维护` 收敛点 checklist 表」「Worktree filter chip cluster」「Worktree chip / 项目卡右键菜单」7 个 Requirement
- 18 处 metric 集中在 store 内部 leading+trailing debounce 100 ms 描述（11 处）+ race window 200-500 ms（1 处）+ 用户感知渐显 100/150/200/1500 ms（4 处）+ Scenario 测试常数 20 ms / 5 次 / 200 ms after resolve（2 处）
- 3 处 PR 引用全是 `PR #183` 同一上下文（行尾 cwd label 移除背景）

工艺直接复用：

- `2026-05-25-spec-cleanup-frontend-test-pyramid`（PR #309）—— 单 cap 反例清理首例
- `2026-05-25-ssh-remote-context-cleanup`（PR #312, D-1b 数字三分）—— 14 Requirement 重写
- `2026-05-26-cleanup-config-and-context-menu`（PR #319, D-2b 数字三分）—— 双 cap 合并 1 PR

本 change 是这套工艺第四次实战，相比 PR #312 / #319 改动量类似（14 Requirement），但反例集中度更高（src path + 实现 tuning 数字两类同时密集）。

## Goals / Non-Goals

**Goals:**

- `sidebar-navigation` 44 hits 降至 7（保留 7 处用户感知阈值数字作 NFR 契约——按 D-2c 三分；分布：1 处 600ms toast + 6 处 150/200/250/1500 ms 渐显（含 100-200 ms 区间））
- 14 Requirement body 重写后行为契约语义 100% 等价
- 移除的实现 tuning 与 src 路径作为「参考实现指引」段落留 design.md，方便维护者溯源

**Non-Goals:**

- 不改代码 / 测试 / 配置
- 不改 Requirement / Scenario 数量
- 不改其它 capability spec
- 不动 IPC 字段名 `groupId` / `projectId` / `worktreeId` / `sessionId` / `cwdRelativeToRepoRoot` / `gitBranch` 等数据契约
- 不动 Purpose 段（已经简洁）
- 不重命名 Requirement 标题中的 `Store loadMore` / `loadFirstPage` 这类内部 store API 名（标题级 RENAMED 工艺成本相对收益不划算，留作后续 spec 重组 PR 一起处理；本 PR 已对 `Store loadMore 实现 leading + trailing debounce 100 ms` 中 100 ms tuning 数字部分做 RENAMED → `... leading + trailing 限频`，详 D-6）

## Decisions

### D-1：行为契约 100% 不变

**问题**：14 Requirement 涉及 44 hits，部分句子表面像反例（如「`broadcast::Sender::send` 在 IPC return 之前到达」「`pendingMetadataUpdates: Map<sessionId, SessionMetadataUpdate>` 缓冲区」）实则承载用户可观察的契约（broadcast 路径在 race window 内可能丢失更新；前端 buffer race 兜底机制）。

**决策**：所有 SHALL / MUST 句的**语义**完全对等迁移：

- Rust / TS 内部类型签名（`Map<sessionId, SessionMetadataUpdate>` / `PaginatedResponse<SessionSummary>` / `SessionListEntry` 等）→ 概念描述（"按 sessionId 索引的 update 缓冲区" / "session 列表分页响应"）
- 内部模块路径（`broadcast::Sender::send` / `cdt-api/src/ipc/session_metadata.rs::extract_session_metadata` / `claude-devtools/src/main/types/messages.ts::isParsedUserChunkMessage`）→ 行为描述（"后端跨进程 push 流" / "session metadata 提取函数" / "对齐原版的 user-chunk 消息识别规则"）
- 具名前端文件 / 组件路径（`Sidebar.svelte` / `ProjectSwitcher.svelte` / `WorktreeChipCluster.svelte` / `Dropdown.svelte` / `SessionDetail.svelte` / `sidebarStore.svelte.ts` / `sessionListStore.svelte.ts` / `tabStore.svelte.ts` / `projectDataStore.svelte.ts` / `CommandPalette.svelte`）→ 角色描述（"Sidebar 组件" / "项目切换器" / "Worktree filter chip cluster" / "通用 Dropdown 组件" / "Session 详情视图" / "Sidebar 折叠状态 store" / "session 列表缓存 store" / "tab 状态 store" / "项目数据 store" / "Command Palette"）
- 协议契约（IPC 方法名 `listSessions` / `listGroupSessions` / `getSessionDetail` / `loadMoreSessions` / `loadProjects` / `list_repository_groups`、event 名 `session-metadata-update` / `file-change` / `sse-recovered` / `sse-lagged`、配置 key `selected_project_id` / `selected_group_id`、字段名 `groupId` / `projectId` / `worktreeId`）→ **保留**，外部协议契约
- RFC2119 关键词（SHALL / MUST / SHOULD / MAY / MUST NOT / SHALL NOT）保留英文

### D-2：sidebar-navigation 反例分类处理

按 44 hits 各类分别给出处理规则，apply 阶段照表批改：

| 类 | 数 | 处理方式 |
|---|---|---|
| **p1 内部模块/类/函数名** | 1 | `broadcast::Sender::send` → "后端跨进程 push 流"（同时去掉 p6 命中） |
| **p2 源文件路径** | 21 | 所有 `*.svelte` / `*.ts` / `*.rs` / `ui/src/...` / `crates/...` / `cdt-api/src/...` 全部改为角色 / 行为描述（详 D-1 分流表）；Scenario 内"组件 grep `<标识>`" 类断言改为"对外可观察契约"（如 `Sidebar.svelte grep 'cwdTailLabel'` → "Sidebar 实现 SHALL NOT 包含老版本 cwdTailLabel 渲染逻辑"——保留 contract assertion 但去掉 src 路径）；`crates/cdt-discover` 这种作为路径 example 出现的也抽象（→ "目录路径示例"） |
| **p3 commit/PR/issue** | 3 | 所有 `PR #183` 引用全删 —— 删除"该 label 由 PR #183 引入"的历史背景。Requirement body 改写为不带历史溯源的行为契约（`SHALL NOT 渲染 ... <span class="session-cwd"> ...`），Scenario 标题去掉 `PR #183` 前缀 |
| **p6 库框架名** | 1 | 同 p1（合并处理 `broadcast::Sender::send`） |

### D-2c：sidebar-navigation 数字三分（用户感知 vs 实现 tuning vs 测试场景常数）

**触发**：18 个 ms 数字命中按 SPEC_GUIDE 反例 4 + ssh-remote-context-cleanup D-1b + cleanup-config-and-context-menu D-2b 工艺审查后，分三类：

| 数字 | 出现位置 | 类别 | 处置 |
|---|---|---|---|
| 600ms（"已复制!"反馈关闭）| Scenario `复制操作反馈` THEN | **用户感知**（toast 时长契约）| **保留** |
| 200ms（同 project 短时间多次 file-change 合并）| Requirement body + Scenario | **实现 tuning**（debounce 触发窗口的诊断数字）| body 改"短时间内连续多次"；Scenario WHEN 改"短时间内连续 3 次"，**保留 contract**（多次→单次 IPC）但抽象掉具体 200 ms 窗口 |
| 200-500 ms（A→B→A 切换 race window）| Requirement body 描述 | **实测/诊断**（race 触发窗口诊断）| body 抽象为"短时间快速切换路径"；具体诊断窗口移 design.md 参考实现指引段 |
| 100 ms（store loadMore leading+trailing debounce 阈值）| Requirement 标题 + body + Scenario | **实现 tuning**（codex round 2 + spec-guide round 1 共同 push back）| **抽象掉** —— 第一轮 D-2c 把 100 ms 归"用户感知"理由是"人类滚动停顿阈值"，但 codex round 2 反驳：当前 sidebar 不直接调 `store.loadMore`（Requirement body 自述），用户实际感知不到这个 timer 边界；99 / 101 ms 边界用户主观无差异。修法：(1) RENAMED Requirement 标题去 100 ms（`leading + trailing debounce 100 ms` → `leading + trailing 限频`）；(2) body 抽象 "短窗口长度见 design.md::D-4 参考实现指引"；(3) Scenario 抽象 "短 cooldown 窗口内"；具体 100 ms 数值移 design.md::D-4 |
| 20 ms / 5 次 / 200 ms after resolve | Scenario WHEN 测试场景常数 | **测试构造常数**（不是契约阈值，只是描述测试场景）| 改抽象 —— "连续多次调用，每次间隔短于 debounce 窗口"；保留可断言契约（leading 立即 fire + 后续被 inflight short-circuit 丢） |
| 99 ms（"在 threshold 边缘 99 ms 后又继续滚"）| body 边角案例描述 | 实现 tuning 边角描述 | 整段移 design.md 参考实现指引段，body 简化为"在 debounce 阈值附近的边角场景" |
| 150 ms（CSS transition 时长）| Requirement body + Scenario THEN | **用户感知**（渐显时长用户能看见）| **保留** |
| 100 ms ≤ X ≤ 200 ms（渐显时长区间）| Requirement body | **用户感知**（区间范围用户能看见）| **保留** |
| 250 ms（一次性短动画允许时长）| Requirement body 边界条款 | **用户感知**（动画时长上限）| **保留** |
| 1500 ms（metadata 长时间未到达）| Requirement body + Scenario WHEN/THEN | **用户感知**（"长时间等待"代表）| **保留** —— 这个数字承载 contract「不论等多久，骨架视觉永远静态，不升级 shimmer」；Scenario `> 1500 ms` 是用户视角的"长时间"代表 |
| 1 次（IPC 调用）/ 16 个 LRU 上限 / 50 / 20 等 page size | 列表 page size / IPC 频次 / cache cap | **协议契约**（page size 是 IPC 入参契约 / IPC 频次是用户感知去重契约 / cache LRU 上限是用户切 project 命中率边界）| **保留** |

**结果**：18 hits → 7 hits（保留的 7 处都是 Scenario WHEN/THEN 内或 body 中可断言用户感知契约：600ms toast / 150 ms transition / 100-200 ms 渐显区间 / 250 ms 动画上限 / 1500 ms long-wait 代表）。

**理由**：SPEC_GUIDE 反例 4 三分（用户感知 / 实现 tuning / 实测 baseline）已在两个姊妹 cap 验证。本 cap 复用规则不重新发明：用户能"看见 / 感知 / 区分"的阈值（toast 闪现时长、渐显感知区间、长时间等待视觉不升级）保留具体值；用户感知不到的 store API 内部 timer（100 ms 滚动停顿阈值——sidebar 不直接调用，用户感知不到）+ race 触发窗口（200-500 ms）+ 测试场景常数移 design.md。

**round 2 反转记录（不删原条款，保留决策审计）**：第一轮 D-2c 把 100 ms 归"用户感知"，论证依据是 body 自述「100 ms 是人类感知滚动停顿阈值」。codex round 2 + spec-guide round 1 都质疑：「滚动停顿 cognitive 阈值」是学术依据，但当前 Requirement body 自己说 sidebar 不直接调 `store.loadMore` —— 这意味着 100 ms 实际只约束 store API 内部 timer 行为（未来调用方契约），用户当前路径感知不到，应归 implementation tuning。round 2 接受反驳：标题 RENAMED 去 100 ms / body & Scenario 抽象。该 100 ms 数值移 D-4 参考实现指引。

### D-3：`selectedGroupId 与 worktree id 分层维护` 收敛点 checklist 表删除

原 Requirement body 含 12 行表格列出"位置 / 现状 / 改动 / 备注"——这是该 change apply 阶段的实施 checklist（每行映射到一处具体 src 文件 + symbol），属于 *implementation guide* 不是 *behavior contract*。该 change 早已 archive，主 spec 不需要保留实施 checklist。

**决策**：表格整段删除。Requirement body 保留行为契约部分：

- `selectedGroupId` 持 RepositoryGroup id 用于顶层导航 / 分页 / SSE filter / 持久化
- detail API（`get_session_detail` 等）继续按 worktree id 入参
- 三元组 `(groupId, worktreeId, sessionId)` 桥接两侧
- SSE event payload 含 `groupId` 字段供前端按 group 过滤
- 单 worktree group 时 group id 与 worktree id 字符串相同

实施 checklist 历史已 archive 在 `2025-XX-XX-...`（具体 archive 路径不抄进主 spec），需要 implementer 视角的可查 `git log` + archive 内 design.md。

### D-4：参考实现指引（apply 后留资源）

apply 阶段从 spec body 移除的实现细节归集在本段落，做主 spec → 当前实现的索引：

**Sidebar 组件路径锚点**：

- 主 sidebar 组件 → `ui/src/components/Sidebar.svelte`
- 项目切换器 → `ui/src/components/ProjectSwitcher.svelte`
- Sidebar 顶部容器 → `ui/src/components/SidebarHeader.svelte`
- Worktree filter chip cluster → `ui/src/lib/components/WorktreeChipCluster.svelte`
- 通用 Dropdown 组件 → `ui/src/lib/components/Dropdown.svelte`
- Session 详情视图 → `ui/src/routes/SessionDetail.svelte`
- Command Palette → `ui/src/components/CommandPalette.svelte`

**Sidebar 状态 store 路径锚点**：

- Sidebar 折叠状态 store → `ui/src/lib/sidebarStore.svelte.ts`
- 项目数据 store → `ui/src/lib/projectDataStore.svelte.ts`
- session 列表 SWR 缓存 store → `ui/src/lib/sessionListStore.svelte.ts`
- tab 状态 store → `ui/src/lib/tabStore.svelte.ts`

**后端实现锚点**：

- session metadata 提取函数 → `cdt-api/src/ipc/session_metadata.rs::extract_session_metadata`
- TS 原版 user-chunk 消息识别 → `claude-devtools/src/main/types/messages.ts::isParsedUserChunkMessage`
- 后端 push 流（IPC return 之前到达 race）→ tokio `broadcast::Sender::send`
- file-change broadcast bridge → 见 `push-events` capability

**实现 tuning 数字锚点**：

- A → B → A race 触发窗口诊断 → 200–500 ms 切换间隔 + 文件同期变更
- file-change watcher debounce → 100 ms 兜底 silent refresh
- **store loadMore debounce 阈值** → 100 ms（leading + trailing 组合 debounce 的 cooldown 窗口长度；初版 D-2c 把它归"用户感知"误判，round 2 反转作 implementation tuning 移到此处）
- store loadMore debounce edge case → "在 100 ms threshold 边缘 99 ms 后又继续滚" 的边角场景
- store loadMore Scenario 测试构造常数 → "20 ms 间隔连续 5 次 / 100 ms cooldown 内 3 次 / 200 ms after resolve"

**收敛点 checklist 历史**：

- `selectedGroupId 与 worktree id 分层维护` Requirement 原 body 含 12 行表格映射到具体 src 文件 + symbol（`sidebarStore.svelte.ts::selectedProjectId` rename / `sessionListStore.svelte.ts` cache key / `tabStore.svelte.ts::tab.projectId` 等）；该 checklist 历史已 archive，需要查 git log + archive 内对应 change design.md

**`selectedGroupId` 与 worktree id 分层迁移**的执行映射（spec 不抄）：

- `sidebarStore.selectedProjectId` rename → `selectedGroupId`，持 group.id
- `projectDataStore.fetchProjectData` 默认从 `group.id` 推导
- Sidebar `loadSessions` 入参从 worktree id 改 group id（调 `listGroupSessions(groupId, ...)`)
- session 列表缓存 store cache key 用 `(groupId, filterWorktreeId | null)` 复合
- tab 状态保留 worktree id（`tab.projectId`），新增 `tab.groupId` 用于高亮
- `getSessionDetail(projectId, sid)` 不变（继续传 worktree id）
- Command Palette 改调 `listGroupSessions` 拿合并候选；onclick 时按 candidate worktree id 创建 tab
- 用户配置 `selected_project_id` 改 `selected_group_id`，启动时迁移老 worktree id

### D-5：Scenario 命名扫一遍

按 SPEC_GUIDE「Scenario 标题用用户视角短语，避免内部 symbol」要求扫所有 Scenario 标题：

| Scenario 标题 | 视角 | 处置 |
|---|---|---|
| `行尾全路径 label 已移除` | 用户视角（删除某条视觉元素）| 保留 |
| `PR #183 行尾 cwd 全路径 label 已移除` | PR 内部视角 | 改为 `行尾 cwd 全路径 label 已移除`（去 PR # 引用） |
| `SSE patch 按 groupId filter` | 内部 transport 视角（SSE 是 server-mode 路径名，Tauri runtime 走 IPC 而非 SSE）| 改为 `push event 按 groupId filter`（统一两 runtime 视角）|
| `SSE patch 同 group 命中` | 同上 | 改为 `push event 同 group 命中` |
| `sessionListStore cache key 含 worktree filter` | 内部 store 名视角 | 改为 `列表缓存 cache key 含 worktree filter` |
| `不再渲染 accordion` | 用户视角 | 保留 |
| `store 内部并发 loadFirstPage 仅保留最新 response` | 内部 store API 视角，但 generation token 是协议级行为 | 保留（标题中 `loadFirstPage` 是 store API 名，姊妹工艺 D-2b 同款判定：内部 store API 标题命名留 follow-up，本 PR 不动）|
| `store loadMore 同 cursor 不重复 fetch` | 同上 | 保留 |
| `store loadMore leading 立即触发 + inflight short-circuit` | 同上 | 保留 |
| `store loadMore cooldown 内多次调用合并为一次 trailing fire` | 同上 | 保留 |
| `store loadMore 单次调用后停顿不重复 fire` | 同上 | 保留 |

共 4 处 Scenario 标题改写：1 处去 `PR #183` 前缀 + 3 处去 SSE / 内部 store 名（与 body 抽象保持一致）。

### D-6：实现 tuning 标题命名（部分落地 + 部分 follow-up）

两个 Requirement 标题嵌入了内部 store API 名 + tuning 数字，按严格 SPEC_GUIDE 命名应为「sidebar 加载更多 session 的高频限频」「sidebar 分页加载并发取消」之类领域语义。

| Requirement 标题 | 处置 | 理由 |
|---|---|---|
| `Store loadMore 实现 leading + trailing debounce 100 ms` | **本 PR RENAMED** → `Store loadMore 实现 leading + trailing 限频`（仅去 100 ms tuning 数字）| codex round 2 + spec-guide 都 push back 100 ms 不该作"用户感知"，必须从标题里去掉以保 D-1 自洽；但 store / loadMore 内部 API 名留 follow-up（更彻底领域语义重命名走 spec 重组 PR）|
| `Store loadFirstPage / loadMore 内部 generation token 取消机制` | **本 PR 不动** | 标题不含 tuning 数字，仅 store API 名命中——RENAMED 工艺成本相对收益不划算（PR #319 D-2b 同款判定）|

**round 2 反转记录**：第一轮 D-6 决策"本 PR 不动两个标题"，因为标题 RENAMED 工艺成本视为不划算。codex round 2 + spec-guide round 1 共同 push back：100 ms 标题嵌入与 D-2c body 抽象不一致——若 body 抽象但标题保留 100 ms，purity hit 仍命中且 D-1 内部矛盾。round 2 接受反驳：仅对含 tuning 数字的标题做 RENAMED；store API 名命名仍留 follow-up。

### D-7：known-residual（spec-guide warn 2 follow-up 列表）

spec-guide round 1 warn 2 指出 spec body / Scenario 内仍有内部 fn / state field 名残留。本 PR 不在 cleanup 范围（清理重点在 src 路径 + PR 引用 + tuning 数字三类），但显式列出已知 residual 让下一轮 reviewer 不重复发现：

- 前端状态机名：`awaitingAIGroup`
- 前端 buffer / state field：`pendingMetadataUpdates` / `sessionsLoadingMore` / `sessionsNextCursor` / `sessionsTotal`
- 前端 fn 名：`mergeSessions` / `applyPendingMetadata` / `applySilentRefresh` / `mergeRecoveryResponse` / `loadMoreSessions` / `maybeLoadMoreSessions` / `loadFirstPage` / `loadMore`
- 内部 jargon Scenario 标题（D-5 漏判）：`Metadata patch 同步更新 store` / `Store LRU 超过 16 个 project 时淘汰` / `首页 SWR refresh 删除已不存在的 session` / `非首页 refresh 不触发删除 reconcile`

这批 residual 归"内部 fn / state field 名 → design"档（SPEC_GUIDE 反例 3）。与 D-6 中 store / loadMore API 名同源，下一次 spec 重组 PR 一并处理。本 PR 不动以保 PR scope 集中（src 路径 + PR 引用 + 数字三分 + RENAMED 100 ms），避免一次 PR 改动面过大让 reviewer review burden 失控。

## Risks / Trade-offs

- **风险 1**：抽象掉 src 路径锚点后，未来 reviewer 看主 spec 不知道行为契约对应哪个组件实现。**Mitigation**：D-4 段保留路径锚点索引；维护者按"行为 → 组件"反查
- **风险 2**：D-3 删除收敛点 checklist 表后，未来想做类似的 group-id / worktree-id 重构 reference 可能不易回溯。**Mitigation**：archive 内对应 change design.md 永久保留；git log 检索关键词 `selectedGroupId` / `selectedProjectId rename` 仍可找到
- **Trade-off**：D-2c 的 100 ms 是否算"用户感知阈值"边界争议。第一轮选"保留"，依据 body 自述「100 ms 是人类感知滚动停顿阈值」；codex round 2 + spec-guide round 1 共同 push back（当前 sidebar 不调用 `store.loadMore`，用户实际感知不到），round 2 决定**抽象掉**——RENAMED 标题 + body / Scenario 抽象 + 100 ms 移 D-4 参考实现指引段。该决策反转记录见 D-2c / D-6 的"round 2 反转记录"小节
- **Trade-off**：D-7 known-residual（fn / state field 名 + 4 处内部 jargon Scenario 标题）不在本 PR 处理。本 PR scope 已含"src 路径 + PR 引用 + 数字三分 + RENAMED 100 ms"4 类清理 + 14 MOD Requirement，再加一类清理会让 PR 改动面过大。已显式列入 D-7 让下一轮 reviewer 知道这批 residual 是已知 follow-up 不是漏判

## Migration Plan

无 —— 纯 spec 文档清理，apply 后跑 `openspec archive cleanup-sidebar-navigation -y` sync 主 spec 即收口。

## Open Questions

无。

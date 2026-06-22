## Context

Cmd+K 命令面板（`ui/src/components/CommandPalette.svelte`）会话区当前：onMount 调 `loadProjectData()` 把 `data.projects` 复制进**局部** state；`filteredSessions` 在 `!selectedProjectId` 时返回空；有 query 时调后端 `searchGroupSessions(selectedProjectId, q)` 做**组内正文全文搜索**。

关键现状（均已查证带行号）：
- `RepositoryGroup.worktrees[].sessions: Vec<String>` 是**全量 sessionId**，`list_repository_groups` 直接返回、无瘦身（`cdt-core/src/project.rs:99`、`project_scanner.rs:332`）→ 全局按 id 匹配的数据**已在前端内存**。
- title **不在内存**：靠 `MetadataCache`（LRU `METADATA_CACHE_CAPACITY = 2000`）+ 仅在 `build_group_session_page` 分页时触发的 `scan_metadata_for_page`，**无任何启动/空闲全量预热**（`session_metadata.rs:691`、`local.rs:1253-1337`）。
- `get_session_summaries_by_ids` 写死 `title: None`（`local.rs:3831`，`ipc_contract.rs:1713` 固化）→ 没有"只读缓存、miss 不扫"的现成 title 接口。
- 跨 worktree 去重语义 `dedup_sessions_across_worktrees`（同 sessionId 留 mtime 最大）只在后端分页路径调用（`local.rs:1103`、`2660`），前端裸遍历不继承。
- `list_repository_groups` 按 active context 选 resolver（`local.rs:5019`）→ "全局"实为当前 context 快照，SSH 未连接 host 不在列表。

本 change 是 codex 二审（verdict: NEEDS_CHANGES）后收敛出的 **X 阶段（Global Session ID Locate）**：只交付零文件 I/O 的全局 sessionId 定位，规避全局 title 搜索的同步 I/O 成本。

## Goals / Non-Goals

**Goals:**
- 用户在 Cmd+K 输入一个（部分）sessionId，**跨所有项目**定位到该会话并能直接打开，无需先知道/选中它所属项目。
- 零新后端 IPC、零文件 I/O、零 metadata scan——纯前端遍历已加载的 `list_repository_groups` 快照。
- 保留现有"选中项目时组内正文搜索"，不回归。
- 修复改全局后必然暴露的既有缺陷（stale 快照 / 去重 / 归属 / 截断 / debounce）。

**Non-Goals（显式留给后续 change）:**
- 全局按 **title** 匹配（需全量 metadata 在内存）。
- 全局按 **正文** 匹配（需读全部 jsonl）。
- "启动/空闲全量后台预热 metadata" 基础设施 + "只读缓存 miss 不扫" title 接口——这是把 X 升级成 Y 的前提，单独立项。
- 跨多个 SSH host 的真·全局（受 active-context 单快照限制）。

## Decisions

### D1：全局 sessionId 匹配走前端遍历内存快照，不新增后端接口
有 query 时，遍历 `projectDataStore` 的 `repositoryGroups` → `worktrees[]` → `sessions[]`，对 `sessionId.toLowerCase().includes(q)` 匹配，跨全部项目。数据已在内存，零 IPC、零 I/O。
- **备选（拒）**：新增后端 `locate_session(idPrefix)` IPC。X 阶段数据已全量在前端，新接口纯属多余 IPC 往返。

### D2：title 仅来自组件已加载的会话；跨项目命中显示 id 前缀 + 项目名，绝不触发扫描
**关键数据事实**（codex 二审坐实）：`Worktree.sessions` 是纯 `string[]`，`projectDataStore` **不缓存任何 session summary**（`projectDataStore.svelte.ts:8-11`），所以前端唯一带 title 的来源是组件**当前已加载的 `sessions` 本地数组**（即当前选中组经 `listGroupSessions` 拉到、本就含 title 的那批）。因此 title 规则收紧为：
- 命中会话**恰在**该已加载数组中（= 当前选中组、已翻到的会话）→ 显示其 title；
- 其余跨项目命中 → 显示 `sessionId.slice(0,8)` + 所属项目名 + worktree/branch 作为定位信息；
- **任何情况都不为补 title 发 IPC / 触发 metadata 扫描**。
- **备选（拒）**：命中后 lazy 调 `listGroupSessions` 补 title。codex R1（CRITICAL）：该路径是"分页+miss-scan+SSE"模型，冷缓存下 N 条命中触发 N 次 jsonl 扫描，违反 idle CPU<2% / 冷路径预算；面板关闭后 SSE patch 无订阅者（Sidebar 专门做了 buffer race fix，CommandPalette 没有）。
- **备选（拒）**：`get_session_summaries_by_ids` —— 契约写死 `title: None`，给不了 title。
- **诚实取舍**：X 阶段绝大多数跨项目命中只有 id 前缀 + 项目名，无 title——这是规避全局 title I/O 的自觉代价；补齐 title 是 Y 阶段（全量预热）的事。文案不暗示"会话无标题"。

### D3：CommandPalette 改为响应式读 `projectDataStore` 快照，消除 stale/ghost
不再 onMount 一次性把 `data.projects` 复制进局部 state；改为 `$derived` 读 `projectDataStore` 的响应式 `repositoryGroups`，使全局 file-change 刷新 store 后已打开面板自动同步（codex R2）。
- **备选（拒）**：面板内自起 `$effect` 轮询/重拉。重复 store 已有的刷新机制，徒增 IPC。

### D4：归一化候选 + 单一 identity = sessionId 去重（worktree 级近似，不伪称对齐后端 mtime）
**关键数据事实**：前端**无 per-session mtime**，只有 worktree 级 `mostRecentSession`（`api.ts` Worktree）。后端 `dedup_sessions_across_worktrees`（`local.rs:1103`）按 per-session mtime 保最大，前端**无法对齐**。因此：
- 全局遍历产出统一 **normalized row**：`{ sessionId, projectId, groupId, projectName, worktreeName, gitBranch, worktreeMostRecent, title?, hits? }`（A 路全部来自遍历的 `group.id` / `worktree.id` / `worktree.name` / `worktree.gitBranch` / `worktree.mostRecentSession`，均内存现成、零 I/O；title? 按 D2 规则）。无 title 时用 `projectName` + `worktreeName`/`gitBranch` 作定位补充。
- 去重 **identity = `sessionId`**（单一 key，消除 D4/D7 的 key 冲突）；同 sessionId 跨 worktree 多次出现时，tie-break 取**确定性**优先级：优先 `isMainWorktree`/`isRepoRoot` 的 worktree，否则取遍历顺序（group→worktree）首条。
- design 与 spec **明示这是 worktree-level 近似**，不宣称"per-session mtime 最大"（codex R4a）。

### D5：最小 query 长度门槛 + 短查询空态提示
sessionId 多为 hex，输 "a"/"3" 会命中海量。全局 id 匹配仅当 `query.length >= 4` 时启用（阈值为单一可调常量）；长度 1–3 时**不**启用全局匹配，会话区维持现有"组内"行为，并给可见空态/提示（"输入 ≥4 个字符按 Session ID 全局定位"），避免用户粘 3 字符片段后无结果又无解释（codex R3 / N2）。

### D6：确定性排序 + 显式截断，不静默丢结果（不按 per-session recency）
**关键数据事实**：无 per-session mtime，无法按单条会话时间排序。改为**确定性排序**：先按 `worktreeMostRecent` 倒序（worktree 级近似，让最近活跃项目靠前），同值再按 `projectName` + `sessionId` 字典序兜底稳定。排序**后**截断到 `MAX_SESSIONS`；超限给可见提示（"仅显示前 N 条，输入更多字符缩小范围"），不静默丢（perf.md「无静默 cap」）。因 D5 门槛后命中通常极少，排序仅作稳定性保障。

### D7：A（全局 id）+ B（组内正文）合并——单 identity 合并，B 优先补 hits/归属
两路都先转成 D4 的 normalized row，再按 **identity = `sessionId`** 合并（与 D4 同一把 key，无冲突）：
- 同一 sessionId 被 A、B 同时命中时，**保留 B 版本的 `projectId/groupId/hits`**（B 来自当前选中组，归属权威且带正文 match count），并合并 A 的 title?/projectName 兜底。
- UI 行显示**项目名**消解"为啥有的项目出会话有的不出"（codex R5）。
- `openSession` SHALL 用合并后 row **自身的 `projectId` / `groupId`** 调 `openTab(sessionId, projectId, label, groupId)`（`tabStore` 已支持 `groupId?`）；A 路的 groupId 来自遍历的 `group.id`，B 路来自 `selectedGroupId`。杜绝跨项目归属错误（codex R3a）。

### D8：query 驱动的整段重算统一 debounce
现有 `$effect` 每次按键即发 `searchGroupSessions`（无 debounce）。统一对 **query 驱动的整段重算**（含全局遍历/过滤/排序 + 后端 `searchGroupSessions`）加 debounce（~300ms，对齐 SearchBar），不仅是后端调用——否则全局 `$derived` 仍每键遍历排序（codex N1）。静态 candidate 列表（来自 `repositoryGroups` 快照）可在快照变化时构建一次、query 变化时只做轻量 filter。

### D9：明确"全局"= 当前 active context 快照
spec 与文案明确：全局覆盖当前 active context 的所有 group；SSH 远程上下文下不含未连接的其他 host（codex R6）。不夸大为"所有 host 全局"。

### D-V1（Visual）：会话结果行展示与状态覆盖
会话区结果行新增/保证以下视觉信息（改的是既有 `CommandPalette` 单组件，非新建组件）：
- **正常**：title（有则显示）/ `sessionId.slice(0,8)`（无 title 兜底）+ 项目名（跨项目定位必需）。
- **空结果**：query ≥ 4 但无 id 命中 → 会话区显示空态文案，不报错。
- **截断**：命中 > `MAX_SESSIONS` → 行尾/区脚提示"仅显示前 N 条"。
- 引用 `DESIGN.md::The Status Owns the Color Rule`（来源/状态标记不自创色）；其余沿用现有 palette 行样式，不引入新组件、不进 DESIGN.md delta。

## Risks / Trade-offs

- **改响应式快照影响面板渲染** → 用只读 `$derived` 在快照变化时构建一次 candidate 列表（30 项目×538 会话≈16k id）；query 变化只做轻量 filter，且整段重算经 D8 debounce；不在渲染热路径做 I/O。
- **跨项目命中绝大多数只有 id 前缀（无 title）** → 项目名 + worktree/branch 兜底保证可定位（D2）；这是规避全局 title I/O 的自觉取舍，Y 阶段（全量预热）才补 title。文案不暗示"会话无标题"。
- **排序仅 worktree 级近似（无 per-session mtime）** → D6 确定性排序保证稳定可测；D5 门槛后命中极少，排序非关键路径；spec 不宣称"按会话最近时间精确排序"。
- **去重丢正文 match count** → D7 单 identity 合并显式保留 B 版本含 `hits`。
- **"全局"名实不符（SSH）** → D9 文案明确边界，spec Scenario 固化"仅当前 context"。
- **最小长度 4 误伤** → 选 4 是 hex 爆量与可用性的折中，单一常量；短查询给空态提示（D5），便于按真实反馈调整。

## Open Questions

- 全局命中行是否需要显式"来源徽标"（id-match vs 正文-match）区分，还是仅靠项目名 + 是否带 preview 片段隐式区分？倾向后者（更轻），apply 时按实现复杂度定，必要时回填 D7。

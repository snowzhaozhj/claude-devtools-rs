## Context

PR #183（feat(project-discovery)：合并 composite project）已经把"同一个 encoded 目录下不同 cwd 的 session 拆分为多个虚拟 subproject"语义干掉，sidebar 上同 base_dir 恒呈一个 project，session.cwd 字段下发让 UI 在行尾展示 cwd 全路径。但用户实测发现：

1. **worktree 仍然显示为 N 个独立条目** —— `~/.claude/projects/` 下，每个 worktree 自身的 cwd 被编码成独立目录（`-Users-...-rs--claude-worktrees-<name>`），主仓子目录里跑 claude（如 `crates/`）也产生独立 encoded 目录。后端 `WorktreeGrouper`（`crates/cdt-discover/src/worktree_grouper.rs`）已经按 `.git` common-dir walk-up 合并成一个 `RepositoryGroup`，但 UI `ProjectSwitcher.svelte` 把多 worktree group accordion 展开为 N 条平铺。
2. **撞名 "main"** —— `locate_git_dirs`（`worktree_grouper.rs:154-179`）只看 `.git` 是否目录决定 `is_main_worktree=true`；主仓 cwd 与主仓 `crates/` 子目录 cwd 都 walk-up 到同一 `.git` 目录，二者都被标 `is_main_worktree=true`；UI `ProjectSwitcher.svelte:166-168` hardcoded `wt.isMainWorktree ? "main" : wt.name`，多个条目都叫 "main" 用户无法区分。
3. **session 行尾 cwd 冗余** —— PR #183 在 `ui/src/components/Sidebar.svelte:836-839` 加了 `cwdTailLabel(session.cwd)` 展示完整 cwd path，用户觉得视觉噪音。

codex pre-propose 二审（agentId `a0c0f3d9f03eb6e8d`）已对方案 B 给出 4 个 design 风险：D3 group sessions IPC 不能复用全量实现、D4 read_semaphore 必须共享、D7 `is_main_worktree` 必须在本 change 修语义、D10 测试层必须覆盖 group 入口状态机。本 design.md 把这 4 点内化为决策 D1-D8。

性能基线（`.claude/rules/perf.md`）：`list_repository_groups` 95ms / user-real=0.13 / RSS 59MB；`get_session_detail` 60-74ms（PR #183 后）。回归 > 阈值即拒。

## Goals / Non-Goals

**Goals:**
- 把"项目"心智模型从 worktree / cwd 维度统一到 git repo 维度：同一个 git repo 在 sidebar `ProjectSwitcher` 占一行
- 选中 group 后 sidebar session 列表合并该 group 下所有 worktree 的 sessions，按 mtime 全局降序，分页加载
- session 行尾用极简 chip（分支名 + cwd 相对 repo 根的最后两段）替代 PR #183 的完整 cwd 全路径
- 修复 `is_main_worktree` 语义：区分"主 working tree 根"与"主 working tree 子目录"
- 提供 worktree filter 下拉作为"需要专注某 worktree 时"的兜底入口
- 全程不突破 `.claude/rules/perf.md` 的 wall/CPU/RSS 预算

**Non-Goals:**
- 不删除 PR #183 引入的 `Session.cwd` 字段（SessionDetail badge / worktree filter / 行尾 chip 都消费）
- 不改 `pinned_sessions` / `hidden_sessions` 的持久化 key（仍是 session id）
- 不实现 server-side iterator state（k-way merge cursor 走 server 无状态）
- 不引入新的跨进程依赖 / git 子进程 / 持久化 cache
- 不改 `cdt-ssh` 远端实现 —— SSH project 当前不走 worktree grouping，本 change 维持现状

## Decisions

### D1：`is_main_worktree` 语义修正为"working tree 根"

**问题**：现 `locate_git_dirs` 从 `start` 向上 walk，遇到 `.git` 即返回 `(git_dir, common_dir, is_main_worktree=true if .git is dir)`。`crates/` 子目录 walk 到 `<repo>/.git` 时同样返 `is_main=true`，但语义上它**不是**主 working tree 根，只是位于主 working tree 内部的子目录。

**修法**：在 `RepoLookup` 引入 `is_repo_root: bool`，仅当 `start == <repo>` 且 `<repo>/.git` 是目录时为 `true`。原 `is_main_worktree` 字段保留语义为"common-dir 是主 .git 而非 linked worktree gitdir"——只用于排序时让"属于主 .git common-dir"的 worktree 排前；UI 不再依赖此字段判定展示名。

**为何不直接把 `is_main_worktree` 改名/复用**：保持向后兼容性使 grouper 排序逻辑（main worktree 排前）不变；新增字段比修原字段语义更清晰。

**替代方案**：UI 侧 dedupe（按 path 字符串去重）。**否决理由**：codex D7 明确指出"UI-only dedupe 会掩盖错误语义"，且排序也受 `is_main_worktree` 影响。

### D2：`cwd_relative_to_repo_root` 仅放在 Worktree 层，IPC SessionSummary 通过 join 填（scheme c）

**问题**：UI 需要在 session 行 chip / worktree filter 下拉里展示 "这是 repo 的哪个子目录 cwd"——`crates`、`src-tauri`、`.claude/worktrees/feat-x` 等。直接传完整 cwd 让前端做相对路径计算重复且易错。

**初版方案（已废）**：在 `cdt-core::Session` / `Project` 两层都加 `cwd_relative_to_repo_root` 字段，scanner 算一次 + grouper 算一次。**codex post-propose 二审驳回**——field 放置矛盾：grouper 是 scanner 下游，scanner 阶段 Project 还没经过 grouper，给 Session 加字段会导致 scanner 内重走 repo 解析（双 walk-up + 违反 D2 原本"复用 grouper 解析结果"承诺）。

**最终方案（scheme c）**：
- `cwd_relative_to_repo_root` **仅** 放在 `cdt-core::Worktree` 层（grouper 输出的类型），由 `WorktreeGrouper::group_by_repository` 计算一次
- `cdt-core::Session` 不加该字段（保持 Session 是"原始扫描数据"语义，不混入 group join 派生信息）
- IPC 序列化 `SessionSummary` 时，在 IPC handler（`list_sessions` / `list_group_sessions` / `get_worktree_sessions`）做一次 group→worktree join：从 `LocalDataApi` 持有的 grouper 缓存或最近一次 `list_repository_groups` 输出找到 session 所属 worktree，把 `worktree.cwd_relative_to_repo_root` 填到 `SessionSummary.cwdRelativeToRepoRoot`

**Grouper 内部计算逻辑**（纯字符串运算 0 syscall 0 spawn）：
- `repo_root = canonical(identity.id).strip_suffix("/.git")`（identity.id 已经是 canonical `<repo>/.git`，本就是 grouper 解析 identity 时拿到的副产物）
- `worktree.cwd_relative_to_repo_root = project.path.strip_prefix(repo_root)`
- repo 根本身或 strip_prefix 失败时为 `None`

**计算时机**：在 grouper 已有 `lookup.identity + project.path` 配对的循环内（`worktree_grouper.rs::296-318`）直接计算。Grouper 输出的 `Worktree` 增加 `cwd_relative_to_repo_root: Option<String>` 字段。

**IPC handler join 实现**：`LocalDataApi` 内部维护轻量 `worktree_id → cwd_relative_to_repo_root` 映射缓存（key on `worktree.id` 即 encoded project dir 名），随 `list_repository_groups` 调用刷新；`list_sessions` / `list_group_sessions` 序列化 SessionSummary 时直接查映射，0 额外 fs 调用。grouper 未跑过（首屏 `list_sessions` 早于 `list_repository_groups`）时该映射为空，`SessionSummary.cwdRelativeToRepoRoot` 为 None；UI 在收到 `list_repository_groups` 响应后才有完整数据——但实测 `list_repository_groups` 是 UI 启动首个 IPC，不会有该缺口（项目数据 store `fetchProjectData` 已经走 list_repository_groups 在前）。

**性能影响**：0 额外 fs 调用 / 0 额外 git 子进程。`identity` 解析本就跑过的 walk-up + canonicalize 是本 change **复用**而非新增的工作。`is_repo_root` 也只是把现有 walk-up 终点的 path 跟 `start` path 做字符串比较，0 syscall 增量。join 查表是 HashMap O(1)。

**性能预算确认**：本 change 在 discovery 层引入的所有新计算（`is_repo_root` 判定 + `cwd_relative_to_repo_root` 计算）SHALL 不让 `list_repository_groups` baseline 95ms 退化超过 5%（≤ 100ms）。validate 路径：`cargo test -p cdt-api --release --test perf_cold_scan -- --ignored --nocapture` apply 前 / 后各跑 3 次取 min。

### D3：新增 `list_group_sessions` IPC，k-way merge 流式分页

**问题**（codex D3）：现有 `traits.rs:329-342` `get_worktree_sessions` 用 `page_size=usize::MAX` 全量收集再排序分页。直接复用做 group 列表入口会击穿 RSS + wall time 预算。

**修法**：新增 `LocalDataApi::list_group_sessions(group_id, page_size, cursor) -> GroupSessionPage`：

```rust
pub struct GroupSessionPage {
    sessions: Vec<SessionSummary>,
    next_cursor: Option<String>,  // base64 JSON
}

#[derive(Serialize, Deserialize)]
struct GroupCursor {
    per_worktree: BTreeMap<String /*worktree_id*/, WorktreeOffset>,
}

enum WorktreeOffset {
    NotStarted,
    AfterMtime { mtime_ms: i64, sid: String },
    Exhausted,
}
```

算法：
1. 拿到 group 内 N 个 worktree id
2. 并发跑 `ProjectScanner::scan_project_dir`（共享 `Arc<Semaphore>` 限流，见 D4）拿每个 worktree 的骨架（已按 mtime 降序）
3. parse cursor → 二分定位每个 worktree 的指针起点
4. `BinaryHeap<HeapEntry>`（max-heap on `(mtime_ms, sid)`）k-way merge 取 `page_size` 条
5. 编码 `next_cursor`（包含每个 worktree 当前指针 / `Exhausted`）
6. 同时 fire-and-forget 触发 SSE detail 推送（仅这页的 session id），key on `(group_id, session_id)` 走 active_scans per-key cancel

**性能预期**：单页 N readdir（~19ms）+ k-way merge 几十 µs + 内存峰值 ~200 KB。

**为何 server 无状态**：claude-devtools 单用户桌面应用，server iterator state 增加 GC 复杂度且重启不可恢复；cursor 自描述 ~几百字节 base64，每页代价微秒级，server-stateful 收益不值。

**替代方案 1**：前端 N 并发调 `list_sessions`。**否决**（codex D3）：19 个 IPC 同时跑会击穿共享 read semaphore（见 D4），无法保证全局 mtime 顺序，前端聚合排序代价 O(全量 sessions) 而非 O(page_size)。

**替代方案 2**：后端一次性 `list_group_sessions_all(group_id)` 返回全量。**否决**：RSS 击穿，且首屏不需要全部数据，违反"骨架 + SSE 渐进展示"模式。

### D4：`ProjectScanner` 的 `read_semaphore` 改为共享 `Arc<Semaphore>` 注入

**问题**（codex D4）：`ProjectScanner::new` 每次创建独立 `read_semaphore`（`project_scanner.rs:52-59`），容量 64。`list_sessions_skeleton` 每次 IPC 新建 scanner（`local.rs:764-768`），前端对 19 个 worktree 并发拉 sessions 时 head-read 实际并发上限是 19×64 = 1216，而不是预期的全局 64，违反 `.claude/rules/perf.md::CPU 反模式`"CPU-bound 路径串行改并发不限流"。

**修法**：
1. `ProjectScanner` 构造改为 `fn new_with_semaphore(projects_dir, fs, semaphore: Arc<Semaphore>)`；原 `new` 保留为 `cfg(test)` 便利构造器
2. `LocalDataApi` 持有一个 `shared_read_semaphore: Arc<Semaphore>`（容量 64，与原 `SHARED_READ_CONCURRENCY` 一致）；所有 `ProjectScanner::new_with_semaphore` 调用复用此实例
3. `list_group_sessions` 内部并发 `scan_project_dir` 自然走共享 semaphore

**为何不在 `list_group_sessions` 入口加一层 Semaphore**：会与 head-read 的限流目的混淆（外层 Semaphore 控 worktree 并发，内层控 file read 并发），逻辑层级清晰。共享 semaphore 已经把 file read 并发限到 64，外层不再设限。

### D5：UI ProjectSwitcher 简化为单层 group list

**问题**：现 `ProjectSwitcher.svelte` 对多 worktree group 渲染 accordion（chevron + worktree count + 展开后的 worktree rows），切 worktree 即切 `selectedProjectId`。方案 B 要求"同 git repo 一行"，accordion 形态不再需要。

**修法**：
1. 删除 `ProjectSwitcher.svelte` line 136-188（多 worktree group accordion 分支）+ `dropdown-group-row` / `dropdown-group-chevron` / `dropdown-group-badge` CSS
2. 单 worktree group 与多 worktree group 走同一渲染分支（line 117-135）；多 worktree group 的 `dropdown-item-name` 显示 `group.name`，`dropdown-item-count` 显示 `group.totalSessions`
3. `onclick={() => selectWorktree(group.worktrees[0])}` 改为 `onclick={() => selectGroup(group)}`，传 group 而非 worktree

**worktree 高级入口**：sidebar 顶部加 `worktree filter` 下拉（D6），用户需要专注某 worktree 时使用——而不是回退到 ProjectSwitcher 的 accordion。

**为何不保留 accordion 作为"高级选项"**：双入口（顶层 group + accordion worktree）会让用户产生"我现在选的是 group 还是某 worktree"的认知混乱，违背简化目的。worktree filter 只是"在当前 group 视图内筛选"，不是"切到 worktree 视图"，语义清晰分离。

### D6：sidebar worktree filter 下拉（server-side filter via cursor Exhausted）

**位置**：sidebar 顶部，会话搜索框旁边；默认 visible（多 worktree group 时）；单 worktree group 时隐藏。

**Options**：
- "全部"（默认，selected）
- 该 group 下每个 distinct cwd（按 `cwd_relative_to_repo_root` 去重 + 排序：repo 根优先 → `.claude/worktrees/<name>` 按 branch 字典序 → 其他子目录字典序）
- 每个 option 显示：分支名（git icon）+ cwd 相对路径 chip + session count

**filter 语义**：单选，切换时清空 sidebar session 列表 + 重新调 `list_group_sessions(groupId, pageSize, cursor)`。

**初版方案（已废）**：纯前端展示过滤。**codex 二审驳回**——低占比 worktree（< 5%）时一页 50 条只命中 2-3 条，且首页 50 条 fetch 不足以触发滚动 loadMore（用户看到几乎空的列表卡住）。

**最终方案：server-side filter via cursor `Exhausted`**：

- filter state 不是单独的 IPC 参数，而是编码到 cursor 里：`list_group_sessions` 入参 `cursor` 自描述哪些 worktree `Exhausted`
- 切 filter 为某 worktree `wt-X` 时，前端构造**初始 cursor**：
  ```
  GroupCursor {
      per_worktree: {
          "wt-X": WorktreeOffset::NotStarted,
          // 所有非 X 的 worktree
          "wt-other-1": WorktreeOffset::Exhausted,
          "wt-other-2": WorktreeOffset::Exhausted,
          ...
      }
  }
  ```
- k-way merge 跳过 `Exhausted` 的 worktree，自然只产出 wt-X 的 sessions，一页 50 条全部命中
- 切 filter 回 "全部" 时清空 cursor（`null`），后端按全 worktree 拉

**前端构造 cursor 的 helper**：
```ts
// ui/src/lib/groupCursor.ts
function buildFilterCursor(groupWorktrees: Worktree[], selectedWorktreeId: string): string {
  const per_worktree = Object.fromEntries(
    groupWorktrees.map(w => [w.id, w.id === selectedWorktreeId ? "NotStarted" : "Exhausted"])
  );
  return base64(JSON.stringify({ per_worktree }));
}
```

**后端 cursor 解析**：现有 `parse_cursor` 已经认 `Exhausted` 变体，无需额外改动；filter 是纯前端构造 cursor 的语义复用，后端 0 改动。

**filter state 持久化**：切 group 时重置为"全部"；同 group 内切回不持久化（session-scoped）。filter 当前选择只在前端 store 里持有用于渲染下拉选中态，不持久化到后端。

**自动补页保护**：若 server-side filter 仍出现一页全空（理论上不会，因为 cursor 已经过滤掉非选 worktree），sidebar SHALL 自动 loadMore 直到填满一屏或所有 cursor `Exhausted`——作为兜底防护。

### D7：id 边界——sidebar/SSE 层用 group id，session detail 链路保留 worktree id

**问题**（codex 风险 2 + 二审 #3）：`selectedProjectId` 现在指向某个 worktree id（即 encoded project dir 名）。如果一刀切改为 group id，会破坏 `get_session_detail(projectId, sessionId)` 链路——detail API 是按 project（即 worktree）定位 session 文件的，多 worktree group 下 group id 找不到具体 session 文件。

**最终方案**：分层维护两个 id，明确各自适用场景：

| 层 | 字段语义 | 适用 IPC / 用法 |
|---|---|---|
| sidebar 当前选中（顶层导航） | `selectedGroupId: string` | `list_group_sessions(group_id, ...)` / `list_repository_groups` / SSE event filter / 用户配置持久化 |
| 当前打开的 session 详情（tab 内） | `tab.projectId: string`（实际是 worktree id）+ `tab.sessionId` | `get_session_detail(projectId, sessionId)` / `get_tool_output` / `get_image_asset` / `get_subagent_trace`（detail-link API 都按 project 定位文件） |
| 持久化层 | `pinned_sessions` / `hidden_sessions` 仍 key on session id | 不变 |

**Sidebar 打开 session 流程**：
1. 用户点击 session 行 → 拿 `session.worktreeId`（IPC 返回的 SessionSummary 含此字段，IPC handler 在 join 时填）
2. 创建 / 切换 tab，写入 `tab.projectId = session.worktreeId` + `tab.sessionId = session.id`
3. `SessionDetail` 调 `getSessionDetail(tab.projectId, tab.sessionId)` 走原有路径，无变更

**收敛 checklist（apply 时按表 verify）**：

| 位置 | 现状 | 改动 | 说明 |
|---|---|---|---|
| `sidebarStore.svelte.ts::selectedProjectId` | worktree id | rename → `selectedGroupId`，持 group id | 顶层导航 |
| `projectDataStore.svelte.ts::fetchProjectData` 推导默认 selected | 取 `group.worktrees[0].id` | 取 `group.id` | 顶层导航 |
| `Sidebar.svelte::loadSessions` 入参 | worktree id → `list_sessions` | group id → `list_group_sessions` | 列表加载 |
| `Sidebar.svelte::sessionListStore` cache key | project id | group id + worktree filter id（cache key 含 filter 维度） | session 列表缓存 |
| SSE `session-metadata-update` event payload | 含 `projectId`（worktree id） | 新增 `groupId` 字段；前端 filter 按 `groupId === selectedGroupId` 匹配（保留 `projectId` 字段供 detail 路径用） | event 过滤 |
| `active_scans` per-key cancel | key = project_id | 分两类 key：detail 拉取 = `(project_id /*worktree id*/, session_id)`（不变）；group 分页拉取 = `(group_id, page_cursor_hash)`（新） | 后台任务取消 |
| `tabStore.svelte.ts::tab.projectId` | worktree id | **不变**（保留 worktree id）；新增 `tab.groupId` 字段供 sidebar 高亮"该 tab 属于哪个 group" | tab 状态 |
| `SessionDetail.svelte` `getSessionDetail(projectId, sid)` 调用 | 用 `tab.projectId` | **不变**（仍用 worktree id） | detail API |
| `CommandPalette.svelte` 调 `listSessions(selectedProjectId, ...)` | worktree id | 改为调 `listGroupSessions(selectedGroupId, ...)` 拿合并 sessions 做搜索 fuzzy 候选；或加一个 toggle"当前 group / 当前 worktree" | 全局搜索 |
| 用户配置 `selected_project_id` 持久化 | worktree id | 改为 `selected_group_id`；启动时若读到老 worktree id，按 grouper 反查 group id 后改写一次（迁移） | persistence |
| 项目 memory / prefs（如有 per-project state） | worktree id | 维持 per-worktree（session detail / agent configs 走 worktree id 路径，per-project state 自然 key on worktree） | per-project state |

**迁移**：`ConfigManager::load` 检查 `selected_project_id` 是否是某 group 的 worktree id；是 → 改写为 group id + 备份到 `<config>.pre-group-id.bak`（与 PR #183 的 migrate-composite-ids 同模式）；如果对应 group 不存在（worktree 被 prune 且 fallback 也失败）则清空字段让首屏走"最近活动 group"默认。

### D7b（apply 阶段反转，2026-05-21）：实测 cdt-config 无 `selected_project_id` 持久化字段，迁移取消

apply 期间 grep 实测 `crates/cdt-config/src` 与 `ui/src/lib` 均无 `selected_project_id` / `selectedProjectId` 的持久化读写（前者无该字段，后者 store 是 session-scoped in-memory）。`migrate_composite_ids`（PR #183 引入）只 fold `pinned_sessions` / `hidden_sessions` 里的 composite key（含 `::`），不涉及 selected_project_id。

**结论**：D7 设计中的 `ConfigManager::load 迁移 selected_project_id` + `<config>.pre-group-id.bak` 备份**不需要**实现。`selectedProjectId` 改名 `selectedGroupId` 后语义切换是纯 UI 层 store 改动，启动时 fallback 到"最近活动 group"默认（已是现有行为），无需任何后端迁移。

**spec / tasks 同步**：
- `spec sidebar-navigation::Migrate persisted selected_project_id on load` Requirement 整体移除（apply 反转）
- `tasks.md::5. 配置迁移` 整组移除

**关键不变量**：detail API（`get_session_detail` / `get_tool_output` / `get_subagent_trace` / `get_image_asset`）的入参 project id **不变**——本 change 不动这些 IPC。所有顶层导航 / 列表分页 / SSE event filter 层改 group id；所有 session 详情 / 工具输出 / per-project state 层保留 worktree id。两者通过 `SessionSummary.worktreeId` + `SessionSummary.groupId`（IPC join 填）桥接。

### D8：session 行尾 chip 替代 cwd 全路径

**删除**：`Sidebar.svelte:836-839` 的 `{#if session.cwd}<span class="session-cwd">{cwdTailLabel(session.cwd)}</span>{/if}` 整块。

**新增**：session 行右侧 chip group：
- 分支 chip（git icon + branch name），样式参考 `ProjectSwitcher.svelte` 的 `dropdown-item-branch` —— 当 session 所在 worktree 有 `git_branch` 时渲染
- cwd 相对路径 chip：当 `session.cwd_relative_to_repo_root` 非 None 且非空时渲染 `…/<lastTwoSegs>`（如 `.claude/worktrees/feat-x` → `worktrees/feat-x`；`crates/cdt-discover` → `crates/cdt-discover`）

**SessionDetail.svelte 顶部 cwd badge 保持不变**（PR #183 line 584-590）——详情页需要完整 cwd path。

**信息密度合理性**：codex 风险问"chip 是不是仍算冗余"。chip 与全路径的区别：（a）chip 是结构化展示，分支与 cwd 分两个视觉单元，扫读快；（b）cwd hint 只有 last two segs，文字量 < 50% 原 cwd label；（c）大多数 session 来自 repo 根 cwd（`cwd_relative_to_repo_root=None`），chip 只显示分支不显示 cwd hint，行更简洁。

## Risks / Trade-offs

- **`is_main_worktree` 字段语义变化破坏依赖该字段做"是不是主仓"的下游代码** → 全 workspace grep `is_main_worktree` 找所有消费方，每处确认是排序意图还是"判定 working tree 根"意图；后者全部迁移到新 `is_repo_root`
- **k-way merge cursor 含 sid 二级排序，多 session 相同 mtime 时 cursor 仍稳定** → 单测覆盖"两 session mtime 相同 + 不同 worktree" 的 page boundary case；cursor 反序列化失败时整体 fallback 为 first-page
- **前端展示 filter 在 worktree session 占比极低时翻很多页** → 后续可升级 server-side filter；本 change 仅展示过滤 + 监控用户使用模式
- **selectedProjectId 迁移失败导致用户启动后 sidebar 空白** → ConfigManager migrate 失败时 fallback 到第一个 group（首屏不阻塞），错误写 tracing::warn 不 panic
- **共享 semaphore 在 SSH 远端 / mock fs 路径上的兼容** → 老 `ProjectScanner::new` 在 `cfg(test)` 下保留 + ssh impl 走自己的 semaphore，本 change 不动 ssh 路径
- **`active_scans` per-key cancel 的 key 形态变化** → 旧 key=project_id 的取消请求在迁移期可能错杀新 group 的拉取，但 grouper 启动时清空 `active_scans`，无跨重启状态污染
- **PR #183 的 perf 优化（消除 get_session_detail 全扫）依赖什么？** → 依赖 `get_session_detail` 的 single-file metadata 路径，本 change 不动 get_session_detail，不会回退

## Migration Plan

1. apply 阶段先落 D1 / D2（discovery 层）+ D4（共享 semaphore），不动 IPC 形态——验证 grouper 输出含 `is_repo_root` + `cwd_relative_to_repo_root`、共享 semaphore 不破坏现有路径
2. 加 D3 IPC `list_group_sessions` + IPC contract test，UI 暂不切换
3. UI 一次性切换 D5 / D6 / D7 / D8 + D7 的迁移代码——保证一个 PR 内完成 selectedProjectId 语义切换的所有位点
4. ConfigManager migrate 写迁移代码 + 备份；启动 idempotent
5. Rollback：revert PR 即可；migrate 的 `.pre-group-id.bak` 文件保留供用户手动恢复

## Open Questions

- 无（codex 已审；如 apply 阶段发现新决策点，回填到本 design）

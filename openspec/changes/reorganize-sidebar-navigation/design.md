## Context

issue #333 是 issue #303 9-PR plan 阶段 3 的第三个 PR，前序 PR 7 已把 session-display 拆出 markdown / tool-viewer-routing / edit-diff-view。`sidebar-navigation` 当前仍有 36 个 Requirement / 168 个 Scenario，按历史实现增量堆叠，混合了项目导航、会话列表、会话操作、列表性能、Worktree 多 group 切换、Sidebar 形态，以及少量 Tab 生命周期 / Tab identity 行为。

本 change 是纯 OpenSpec 重组：不改代码、不改测试、不改 IPC / HTTP / SSE / Tauri command。核心目标是让主 spec 的 owner 与用户行为一致，并将明确属于 `tab-management` 的 Scenario 迁到唯一 owner cap。

## Goals / Non-Goals

**Goals:**

- `sidebar-navigation` 主 spec 改为用户行为视角组织，reviewer 可按「用户如何导航 / 查找 / 操作 / 切换 worktree / 调整 sidebar」定位契约。
- 每个保留或迁移的 Scenario 的 WHEN / THEN / AND / OR / NOT 子句字符级保持；行为契约 100% 不变。
- 将 4 个 Tab owner 候选 Scenario 从 `sidebar-navigation` 交给 `tab-management` 唯一 owner：3 个新增到 active delta，1 个由 `tab-management` 既有 Scenario 覆盖。
- `sidebar-navigation` Purpose 改写为产品 / 用户价值视角，不引入新的 SHALL/MUST 行为契约。
- 同步 spec-purity baseline 中受重组影响的 capability 计数。

**Non-Goals:**

- 不改 Rust / Svelte / Tauri 代码、测试、IPC 字段、HTTP route、SSE event 或配置。
- 不新增 Requirement / Scenario；只做 owner、分组、标题与 Purpose 元描述调整。
- 不批量清理未被本次 MODIFIED 覆盖到的历史 spec-purity 污染。
- 不拆出新 capability（如 `sidebar-worktree-grouping`）；本 PR 只完成 `sidebar-navigation` 内部重组 + `tab-management` 迁入。
- 不 merge PR；merge 属 destructive shared state，留给主 session / 用户拍板。

## Decisions

### D-1：Scenario 行为契约字符级保持

**问题**：本 change 涉及 36 个 Requirement 重组与 4 个 Tab owner 候选 Scenario 跨 cap 裁定，任何顺手改写 WHEN / THEN / AND 子句都会把“文档重组”变成行为变更。

**决策**：所有保留或迁移 Scenario 的 WHEN / THEN / AND / OR / NOT 子句字符级保持。允许变化仅限：cap owner、Requirement 标题、Scenario 标题、Requirement body 的用户视角组织语言、Purpose 元描述。

**校验**：D-3 中 4 个候选逐条裁定；人工抽样比对迁入表中的 3 个新增 Scenario；codex design 二审重点检查是否漏迁或改写子句。

### D-2：用户视角分组工艺

**问题**：现有 36 个 Requirement 按实现演进顺序排列，列表分页 / SWR / SSE recovery / worktree chip / context menu 分散在全文件，难以从用户行为入口检索。

**决策**：active change delta 只表达 OpenSpec 能安全验证的 owner 迁移；archive 后在同一个 archive commit 内直接编辑 `openspec/specs/sidebar-navigation/spec.md` 的 Requirement 顺序，把剩余 35 个 Requirement 按以下用户行为分组排序。Requirement 可重命名为用户视角标题，但 Scenario 子句不改。

| 分组 | Requirement |
|---|---|
| 项目导航 | 项目选择；默认渲染按仓库聚合的 Sidebar；活跃 worktree 选中状态；移除 flat 视图 toggle；冷启共享项目数据 |
| 会话列表 | 会话列表日期分组；会话项展示；会话过滤；加载状态；Auto refresh session list on file change；Ongoing indicator on session item；骨架列表快速加载；会话元数据增量 patch；Sidebar Memory 入口；会话总数显示口径；Metadata 占位字段视觉渐显；Session row branch + cwd chip 替代行尾 cwd 全路径 |
| 会话操作 | 会话置顶（Pin）；会话隐藏（Hide）；右键菜单；Pinned and hidden sessions reconcile outside the first page；Sidebar 既有"右键菜单" Requirement 保持不变 |
| 列表性能 | 会话列表虚拟滚动承载；完整加载分页会话历史；Sidebar uses paginated current-project session loading；Sessions store stale-while-revalidate 缓存；Store `loadFirstPage` / `loadMore` 内部 generation token 取消机制；Sidebar SHALL 订阅 sse-recovered / sse-lagged 触发 silent refresh；Store `loadMore` 实现 leading + trailing 限频 |
| Worktree 多 group 切换 | selectedGroupId 与 worktree id 分层维护；Worktree filter chip cluster for multi-worktree group；Worktree chip 右键菜单；项目卡右键菜单 |
| Sidebar 形态 | 宽度拖拽调整；侧栏折叠/展开 |

**替代方案**：拆出 `sidebar-worktree-grouping` 新 capability。暂不采用，因为 Worktree filter chip、group 选中、项目卡菜单仍共享 Sidebar 用户入口与列表 scope；本 PR 若同时新增 cap 会扩大 archive 顺序风险。

### D-3：跨 cap 迁移映射表

| 来源 Requirement | Scenario | 目标 cap / Requirement | 决策 | 理由 |
|---|---|---|---|---|
| `会话选择与 Tab 联动` | `点击会话打开 tab` | `tab-management::打开 session tab` | 迁移 | openTab 生命周期和 focused pane 作用域由 Tab 系统维护，Sidebar 只提供点击入口 |
| `会话选择与 Tab 联动` | `高亮跟随 focused pane 的 activeTab` | `tab-management::Sidebar 与 Tab 联动` | 迁移 | tab-management 已有 focused pane / activeTab 高亮 owner，迁入后避免重复 |
| `会话选择与 Tab 联动` | `无 active tab 时无高亮` | `tab-management::Sidebar 与 Tab 联动` | 不新增 | tab-management 已有 `无 active tab 时 Sidebar 无高亮` Scenario 覆盖该语义，active delta 不复制近重复条目 |
| `selectedGroupId 与 worktree id 分层维护` | `打开 session detail 用 worktree id` | `tab-management::打开 session tab` 或新增同 Requirement 内小节 | 迁移 | `tab.projectId` / `tab.groupId` / detail API 入参属于 session tab identity；Sidebar 保留列表与 group filter owner |

**不迁移裁定**：

| 来源 | 不迁移内容 | owner |
|---|---|---|
| `活跃 worktree 选中状态` | 切 group 调 `list_group_sessions`、默认最近活动 group、SSE detail 推送 | `sidebar-navigation` |
| `selectedGroupId 与 worktree id 分层维护` | push event 按 groupId filter、Command Palette group 搜索、列表缓存复合 key、单 worktree group id 等于 worktree id | `sidebar-navigation` |
| `Worktree filter chip cluster for multi-worktree group` | 全部 chip 可见性、ARIA、keyboard、scroll reset、cursor 构造、overflow、自动补页 | `sidebar-navigation` |

### D-4：Worktree group 灰区 owner 裁定

**问题**：worktree group 切换既影响 Sidebar 列表 scope，也影响 tab 打开时的 detail identity，容易在 `sidebar-navigation` 与 `tab-management` 间形成双 owner。

**决策**：按“用户入口 + 状态真相源”裁定：

- group 选择、worktree filter、列表分页、列表缓存、push patch 过滤、项目 / worktree context menu 归 `sidebar-navigation`。
- tab 创建、activeTab / focusedPane、高亮跟随、tab 内 detail identity 归 `tab-management`。
- 跨 cap 描述只引用 capability，不复制对方的完整协议字段约束。

**结果**：4 个候选中 3 个在 active delta 迁入 `tab-management`，`无 active tab 时无高亮` 由 tab-management 既有 Scenario 覆盖而不复制。issue 指定的 L516 与 L1048 均保留 Sidebar owner；L999 只迁移其中 `打开 session detail 用 worktree id`，其余 Scenario 保留 Sidebar owner。

### D-5：spec delta、重组排序与 Purpose 工艺

`openspec` delta 不支持在同一 capability 中对同名 Requirement 同时 `REMOVED` + `ADDED`，因此不能用 active delta 表达“同 cap 重排 35 个 Requirement”。本 change 采用两段式：

1. active change delta 中只用 `REMOVED` / `MODIFIED` / `ADDED` 表达可由 OpenSpec 验证的跨 cap owner 迁移：`sidebar-navigation` 移除 `会话选择与 Tab 联动`，并从 `selectedGroupId 与 worktree id 分层维护` 删除 `打开 session detail 用 worktree id`；`tab-management` 接收 3 个新增 Scenario，另 1 个候选由既有 Scenario 覆盖。
2. archive 后在同一个 archive commit 内直接编辑主 spec 的 `## Purpose` 段与 Requirement 顺序，完成 D-2 的用户行为分组排序；该元编辑仅改 Purpose 与纯排序，不改 Requirement body / Scenario 子句。

该工艺沿用 change `split-session-display` 的 D-6 先例对 Purpose 元描述的处理，并额外记录 OpenSpec 对同 cap 重排的 schema 限制。

## Risks / Trade-offs

| 风险 | 等级 | 缓解 |
|---|---|---|
| Purpose 改写引入新约束 | 中 | Purpose 只写用户价值和边界，不写新的 SHALL/MUST；codex 检查 |
| 重组遗漏 Scenario | 高 | D-3 显式列 4 个候选并区分 3 个迁入 / 1 个既有覆盖；spec-guide-reviewer + codex 二审 |
| tab-management 迁入与既有 Requirement 重叠 | 中 | 迁入到既有 `打开 session tab` / `Sidebar 与 Tab 联动`，不新增双 owner Requirement |
| Worktree group 灰区裁定争议 | 中 | D-4 以“入口 + 真相源”裁定；只迁 detail identity 这一条 |
| spec-purity baseline 数字不准 | 中 | apply 后运行 `bash scripts/check-spec-purity.sh`，同步 baseline 同 commit 落地 |
| archive 顺序覆盖 | 中 | 本 PR 期间不并发修改 `sidebar-navigation` / `tab-management`；archive commit 作为最后一个 commit |

## Migration Plan

1. 写 proposal / design / spec delta / tasks，并运行 `openspec validate reorganize-sidebar-navigation --strict`。
2. 设计阶段启动 codex 二审，重点检查 D-3 迁移映射、tab-management owner 重叠、Purpose 是否引入新 SHALL/MUST。
3. apply 阶段完成 active delta 中 4 个候选的 owner 裁定（3 个新增迁入，1 个既有覆盖）；同 cap sidebar 主 spec 排序留到 archive commit 的元编辑步骤。
4. 运行 `just preflight`、`bash scripts/check-spec-purity.sh`、D-3 候选覆盖校验。
5. commit + push + PR；并行启动 wait-ci、codex PR 二审、spec-guide-reviewer。
6. 三路通过后 `openspec archive reorganize-sidebar-navigation -y`，同 archive commit 更新主 spec Purpose 与 spec-purity baseline。
7. archive commit push 后再次 wait-ci 全绿；不 merge。

## Open Questions

无。迁移边界按 D-3 / D-4 执行。

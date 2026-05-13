## Context

主 session 与其衍生 subagent / 用户开的 worktree 可分布在多个 `~/.claude/projects/<encoded-cwd>/` 目录下。Claude Code 按**当条消息发生时的 cwd** 编码 `project_dir`，但 `sessionId` 字段保持 root session 一致——因此 sub-message 与 main-message 即使物理上在不同 `project_dir`，逻辑上仍属同一 root session。

实测复现（worktree 内）：

```
~/.claude/projects/
├── -Users-...-claude-devtools-rs/
│   └── 83886886-....jsonl                 (1274 行，主 session 全量；无 subagents/ 子目录)
└── -Users-...-claude-devtools-rs--claude-worktrees-sidebar-click-replace/
    ├── 83886886-....jsonl                 (1 行，仅 ai-title 元数据)
    └── 83886886-.../subagents/
        ├── agent-afc4056265e762485.jsonl  (cwd=worktree path, sessionId=主 session)
        └── agent-a7506c4d294737035.jsonl
```

当前 `crates/cdt-api/src/ipc/local.rs::scan_subagent_candidates(&project_dir, ...)` 只在主 `project_dir` 下扫 `{session_id}/subagents/`，导致跨目录的 subagent 不被识别；同时 `LocalDataApi::list_projects` 拍平返回 `Vec<ProjectInfo>`，sidebar 把主 + 各 worktree project_dir 视为独立项目。

`crates/cdt-discover/src/worktree_grouper.rs::WorktreeGrouper` 已就绪：通过 `LocalGitIdentityResolver` 调 `git rev-parse --git-common-dir`，把同 repo 下的多个 worktree 合并到同一 `RepositoryGroup`；现有 `LocalDataApi` 未注入。

原版（TS）`../claude-devtools/src/renderer/components/layout/SidebarHeader.tsx` 通过 `groupWorktreesBySource` 按 source 分类、按 `mostRecentSession` 排序、`mainWorktree` 优先；store `contextSlice` 维护 `repositoryGroups` 与 `viewMode` 状态机。Rust port 需 Svelte 5 化移植。

## Goals / Non-Goals

**Goals:**

- 修 subagent 跨 `project_dir` 装载 bug，已 archive 的 subagent-related changes（`subagent-messages-lazy-load` / `align-subagent-ui-with-original` 等）不需要改 spec 主行为，仅修订 path 解析约定
- 把 `WorktreeGrouper` 接入 IPC 层，新增 `list_repository_groups` 与 `get_worktree_sessions`
- 前端 grouped sidebar 默认开启（对齐原版），同 repo 多 worktree 折叠为可展开行
- 性能：跨 `projects_dir` 扫描在 50 个 project 量级下后端总耗时 < 50 ms
- 测试金字塔覆盖：Rust IPC contract + 跨目录集成测试 + Vitest mockIPC + Playwright e2e

**Non-Goals:**

- **不**清理 `.claude/worktrees/` 下的 4 个残留 subagent worktree —— 用户自决，超出本 change 范围
- **不**新增反向索引或后台扫描任务 —— 用 fan-out 一次性 stat 即可满足性能预算
- **不**改变 `list_projects` 拍平语义 —— 老 IPC 路径保持，flat 视图 fallback 仍由 UI 决定何时使用（如 e2e fixture / Dashboard）
- **不**支持 `.claude/worktrees/<slug>` 这种 nested worktree 自身作为 sidebar 顶层独立项（被 group 后自然吸纳）
- **不**改 SSH 远端 subagent 装载路径 —— 远端 path 解析有独立 `cdt-ssh` capability，本 change 局限本地实现
- **不**修 `tauri-plugin-updater` 配置链 / release workflow

## Decisions

### D1: 跨 `project_dir` 扫描策略 —— Fan-out vs 反向索引

**选 fan-out**：每次 `get_session_detail(project_id, session_id)` 调用时，遍历 `projects_dir` 下所有 project_dir，对每个 `dir` 探测 `{dir}/{session_id}/subagents/` 是否存在，存在则 `read_dir` 收集 `agent-*.jsonl` 文件。

候选方案：

| 方案 | 优点 | 缺点 | 是否选择 |
| --- | --- | --- | --- |
| **Fan-out**（每次调用扫所有 project_dir） | 实现简单、无状态、零延迟首次响应、跟现有 path_decoder 解耦 | 重复 stat（但 metadata cache 命中后毫秒级） | ✅ 选 |
| 反向索引（启动时扫全部 projects_dir 建 parent_uuid → path 索引） | 多次查询零 fs IO | 启动慢、需 invalidation、要后台任务 | ❌ |
| 限定到 `WorktreeGrouper` 同 repo 兄弟 project_dir | 扫描范围最小 | 需先跑 git rev-parse / common-dir，前置成本不低；漏检 cwd 切到非 worktree 子路径的极端场景 | ❌ |

**性能预算计算**（50 个 project_dir）：每个 dir 一次 `tokio::fs::metadata` ≈ 100 µs，50 × 100 µs = 5 ms；命中后 `read_dir` 解 N 个文件 ≈ 5 ms × 数量；解析每个 `agent-*.jsonl` 首 10 行 ≈ 1-2 ms。总预算 < 50 ms 给三阶段总和（fan-out + read_dir + parse）。`tracing::info!(target: "cdt_api::perf", projects_scanned, dirs_with_subagents, candidates_found, total_ms, ...)` 探针落地后跟踪真实数据。

**回滚开关**：顶层 `const CROSS_PROJECT_SUBAGENT_SCAN: bool = true;`，false 时回退到原 `scan_subagent_candidates(&project_dir, ...)`。

### D2: 旧结构（flat `{project_dir}/agent-*.jsonl`）如何处理

**旧结构保持只扫主 `project_dir`**：

- 旧结构的 `agent-*.jsonl` 通过首行 `parentUuid` / `sessionId` 关联到父，跨 `project_dir` 扫描时**无法靠目录名锁定父子**，必须逐个 parse 才能判断归属。50 个 project × N 个旧结构 jsonl 的代价不可控。
- 调研：用户机器实测旧结构文件**全部位于主 project_dir**（Claude Code 老版本 cwd 切换不普及），跨目录扫旧结构属于过度防御。
- 决策：`scan_subagent_candidates_cross_project` 只覆盖新结构；旧结构走原 `scan_subagent_candidates(&主_project_dir, ...)` 补一遍合并到候选列表。

### D3: `WorktreeGrouper` 注入策略

**`LocalDataApi` 内部 lazy 缓存 `WorktreeGrouper<LocalGitIdentityResolver>`**：

- 沿用 CLAUDE.md "`LocalDataApi` 构造器扩展"约定：新增 `new_with_worktree_grouper(...)` 构造器，但**不**改 `new()` 签名（已被 `crates/cdt-api/tests/*.rs` 大量依赖）。
- 默认 `new()` 内部 `WorktreeGrouper::new(LocalGitIdentityResolver::new())` 自动初始化。
- 测试可通过 `new_with_worktree_grouper(scanner, ..., FakeGitIdentityResolver)` 注入假 git resolver，避免子进程 git 调用。

### D4: 前端 sidebar 默认视图 —— grouped vs flat

**默认 grouped，永久 grouped（不暴露切换 UI）**：

- 原版有 `viewMode` toggle，但实测用户使用习惯 grouped；保留 flat 模式增加 UI 复杂度（dropdown 切换 + per-mode active state 跟踪）。
- 对齐 CLAUDE.md "优先与原版对齐" 但允许简化：保留原版分组算法、grouped dropdown 渲染；删 viewMode toggle，让 grouped 成为唯一视图。
- 如果未来需要 flat fallback（例如 e2e fixture 测试），靠 `?fixture=...&mode=flat` URL 参数支持，不进 production UI。

候选方案对比：

| 方案 | 优点 | 缺点 | 是否选择 |
| --- | --- | --- | --- |
| **默认 grouped，永久 grouped** | UI 简单、对齐原版 90%、维护成本低 | 用户无 flat 选项 | ✅ 选 |
| 默认 grouped，header toggle 切 flat | 完全对齐原版 | 状态机复杂、per-mode active state 双轨 | ❌ |
| 默认 flat，header toggle 切 grouped | 兼容老用户习惯 | 偏离原版默认行为 | ❌ |

### D5: spec delta 拆分粒度

**单个 change 内承载 4 个 capability 的 delta**：

- `tool-execution-linking`：MODIFIED `Resolve Task subagents with three-phase fallback matching`（加 Scenario）
- `ipc-data-api`：MODIFIED `Expose project and session queries` + `Lazy load subagent trace` + ADDED `list_repository_groups` + ADDED `get_worktree_sessions`
- `project-discovery`：MODIFIED `Group projects by git worktree`（补 Scenario 完整度）
- `sidebar-navigation`：新建（ADDED 全部 Requirement）

CLAUDE.md "spec delta 写法" 硬约束：`ADDED/MODIFIED Requirement` 首段必须含 `SHALL` 或 `MUST`，否则 `openspec validate --strict` 报错。中文背景描述放规约句之后。

### D6: 集成测试 fixture 设计

构造 `tempfile::tempdir` 模拟两个 project_dir：

```
<tmpdir>/projects/
├── -ws-my-proj/
│   └── <root_uuid>.jsonl                  (主 session, type=user/assistant 真实形态)
└── -ws-my-proj-wt-feat-x/
    └── <root_uuid>/subagents/
        └── agent-<sub_uuid>.jsonl         (subagent jsonl, sessionId=主 session, agentId=sub)
```

避禁用字符（CLAUDE.md "Windows NTFS 目录名禁用字符"条）：fixture 编码名只用字母数字 + `-`，**不**写真实 `encode_path` 输出（含 `:` 在 Windows fail）。`ProjectScanner` 接受任何形态的 encoded 名，依赖 JSONL `cwd` 字段恢复真实路径。

测试断言：`get_session_detail(主_project_id, root_session_id)` 返回的 `chunks[i].subagents` 含 `Process { session_id: <sub_uuid>, ... }`，证明跨 `project_dir` 装载成功。

### D7: 前端组件设计 —— grouped sidebar 数据流

```
backend list_repository_groups()        → Vec<RepositoryGroup>
        ↓
Sidebar.svelte loadProjects()           → repositoryGroups: $state<RepositoryGroup[]>
        ↓
{#each repositoryGroups as group (group.id)}
        ↓
  <RepositoryGroupItem group={group}>
        ↓ click expand
    {#each group.worktrees as worktree (worktree.id)}
      <WorktreeItem worktree={worktree} onSelect={...}>
```

- 顶层渲染 `RepositoryGroup`（含 `name` / `worktrees[]` / `mostRecentSession` / `totalSessions`）
- 每个 group 默认折叠；点击展开后渲染 worktree 子列表
- 单 worktree group（standalone project）直接平铺渲染为单行，跳过折叠交互（对齐原版"无可分组时降级为 flat 项"）
- worktree 排序：`is_main_worktree` 优先，再按 `most_recent_session` 倒序（已在 `WorktreeGrouper::group_by_repository` line 144-152 实现）
- active worktree 选中态：`sidebarStore.activeWorktreeId` 取代原 `activeProjectId`（store 改动尽量小）

## Risks / Trade-offs

- **Risk 1**：fan-out 跨目录扫描在用户机器 project_dir 数量极大时（>200）首屏延迟可能超预算 —— **Mitigation**：tracing 探针记录真实分布，若实测 P95 > 100 ms 则 fallback 到反向索引（D1 备选方案）。回滚开关 `CROSS_PROJECT_SUBAGENT_SCAN: bool` 一键切回原行为。
- **Risk 2**：`git rev-parse` 在大量 worktree 下并发跑会 fork 多个 git 进程 —— **Mitigation**：`WorktreeGrouper` 内部对每个 project 串行调用 git，可控；测试用 `FakeGitIdentityResolver` 注入避免真子进程。
- **Risk 3**：grouped sidebar 移除 flat toggle 与原版差异 —— **Mitigation**：D4 已论述；如未来用户反馈需要 flat，按 D4 末尾的 URL 参数 fallback 临时支持。
- **Risk 4**：spec delta 跨 4 个 capability，archive 顺序若被打乱（按 CLAUDE.md "archive 顺序坑"），后续 change 误覆盖 —— **Mitigation**：本 change 是同一 PR 内的 4 个 delta 一次 archive，不存在跨 archive 合并；后续修订需注意。
- **Risk 5**：跨 `project_dir` 装载会引入 subagent 候选数量增加，三阶段 resolver 匹配可能出现新 collision —— **Mitigation**：resolver Phase 1 用 `result-based`（`toolUseResult.agentId` 精确匹配），跨目录后多出的 candidate 不会污染 Phase 1；Phase 2/3 description / positional 退化场景与原同 project_dir 已有的 collision 行为等价。
- **Trade-off**：默认 grouped 永久化的简化，牺牲了部分原版功能完整度，换取本 change 实现成本与状态机维护成本下降；用户感知收益高于功能损失。

## Migration Plan

不涉及数据迁移、不破坏现有 JSONL / 配置文件格式。前端 `tabStore` 内 `activeProjectId` 升级为 `activeWorktreeId`（命名变更，单次 PR 内全替换；persistence 字段同步迁移）。回滚路径：

- 后端跨目录扫描：`CROSS_PROJECT_SUBAGENT_SCAN: bool` 顶层开关
- 前端 grouped 视图：`Sidebar.svelte` 内 grouped 渲染分支，保留 `?fixture=flat` URL 参数走老 flat 渲染（如有兼容需要）

## Open Questions

- 是否需要在 sidebar 给 nested worktree（`<repo>/.claude/worktrees/<slug>/`）打特殊标签（"sub-agent worktree" vs "user worktree"）？暂不做——原版没有此区分，等用户反馈再加。
- `get_worktree_sessions(group_id, pagination)` 的 `pagination` 是按整 group 合并后分页，还是按 worktree 分别分页再合并？倾向**整 group 合并后按 mtime 排序再分页**，对齐 `list_sessions(project_id, pagination)` 语义；tasks 阶段确认。

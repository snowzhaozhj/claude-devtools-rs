## MODIFIED Requirements

### Requirement: Resolve Task subagents with three-phase fallback matching

系统 SHALL 用三阶段 fallback 策略把 `Task` 工具调用解析到对应 subagent session，按以下顺序，作为消费外部传入候选集的纯同步函数：

1. **Result-based**：若 Task 对应的 `ToolExecution.output` 是结构化 JSON 且包含 `teammate_spawned` 或 `session_id` 字段，直接从 `candidates` 中按 session id 取出 `Process`。
2. **Description-based**：用 Task 的 `description` 与 `candidate.description_hint` 做匹配，要求 `|task_ts − candidate.spawn_ts|` 落在 60 秒窗口内；若某 Task 唯一匹配到一个 candidate 则 link。
3. **Positional**：若 phase 2 结束后仍有未分配 Task 且"未分配 Task 数 == 未分配 candidate 数"，则按 spawn order 一一配对。

未解析的 Task 调用 SHALL 保留为 `Resolution::Orphan`。候选集合的装载不属本 capability——它由下游能力（例如 `project-discovery` 与 `team-coordination-metadata`）负责预过滤后传入。

**候选集合的装载范围**：当 caller 是 IPC 层（`ipc-data-api`），candidate 列表 SHALL 由 **跨 `project_dir`** 扫描得到——即同一 `projects_dir`（`~/.claude/projects/` 或 SSH 远端等价路径）下**所有** project 目录的 `{rootSessionId}/subagents/agent-*.jsonl`（新结构），合并去重后传入本 resolver。Resolver 自身行为不变（纯同步函数，对 candidate 来源无感知）。

#### Scenario: teammate_spawned result links directly
- **WHEN** 一个 `Task` 调用对应的 `ToolExecution` 的结构化 output 含 `teammate_spawned` hint 与 subagent session id，且该 session id 在 `candidates` 中存在
- **THEN** 函数 SHALL 直接返回 `Resolution::ResultBased(Process)`，不再评估后续阶段

#### Scenario: No result-based link, description matches one subagent
- **WHEN** 一个 `Task` 调用没有可用的 `teammate_spawned` hint，但其 description 在 60 秒 spawn 窗口内唯一匹配一个 candidate
- **THEN** 函数 SHALL 返回 `Resolution::DescriptionBased(Process)`

#### Scenario: Description ambiguous, positional fallback applies
- **WHEN** description 阶段无任何唯一匹配，但未解析 Task 数等于未解析 candidate 数
- **THEN** 函数 SHALL 按 spawn order 为每对返回 `Resolution::Positional(Process)`

#### Scenario: Task call matches no subagent
- **WHEN** 三阶段对某 Task 调用均无匹配
- **THEN** 函数 SHALL 对该 task 返回 `Resolution::Orphan`，对应 `ToolExecution` SHALL 原样保留

#### Scenario: Unrelated candidate does not trigger positional match
- **WHEN** Task 调用无 description 匹配，candidate 池含归属其它父 session 的 subagent，使等量 check 失败
- **THEN** 函数 SHALL NOT 走 positional 链接，SHALL 返回 `Resolution::Orphan`

#### Scenario: Subagent JSONL located in a different project directory
- **WHEN** 主 session（rootSessionId = `S`）在 `project_dir = A`，且该 session 的某个 subagent JSONL 物理位于 `project_dir = B`（路径为 `B/S/subagents/agent-<subUuid>.jsonl`，例如 subagent 通过 EnterWorktree 把 cwd 切到 `<repo>/.claude/worktrees/<slug>/` 时 Claude Code 写入 B）
- **AND** caller 在装载 candidate 列表时跨 `projects_dir` 扫描所有 project 目录的 `{S}/subagents/agent-*.jsonl`
- **THEN** B 下的 subagent JSONL SHALL 被装载为 `SubagentCandidate` 并进入 resolver candidates 列表
- **AND** 三阶段匹配 SHALL 正常对该 candidate 评估（不因物理目录差异退化）

# background-jobs

## Purpose

提供 `claude --bg` 后台任务的 GUI 面板——对齐 CLI `claude agents` 的分组/状态/PR 语义，额外提供 session 跳转和顶栏 badge。

## ADDED Requirements

### Requirement: Parse state.json into BackgroundJob model

系统 SHALL 从 `~/.claude/jobs/<job_id>/state.json` 解析后台任务状态。解析失败的 job SHALL 被跳过（不 crash）。

#### Scenario: Valid state.json parsed

- **WHEN** state.json 包含合法 JSON 且含 `state` / `name` 字段
- **THEN** 解析为 `BackgroundJob` 含 jobId / state / name / detail / intent / children / sessionId / linkScanPath / cwd / tempo / inFlight / createdAt / updatedAt

#### Scenario: Malformed state.json skipped

- **WHEN** state.json 为空或非法 JSON
- **THEN** 该 job 被跳过，不影响其它 job 的展示

#### Scenario: Unknown state value treated as idle

- **WHEN** state.json 的 `state` 字段值不在已知 6 态中
- **THEN** 系统 SHALL 将其视为 idle 状态

### Requirement: Group jobs by priority

系统 SHALL 将 jobs 分为四组，按以下优先级排列：

1. Ready for review：`children[]` 含 `kind: "pr"`
2. Needs input：`state === "blocked"`
3. Working：`state === "working"` 或 `state === "idle"`
4. Completed：`state in ["done", "failed", "stopped"]` 且无 open PR

#### Scenario: Job with PR child grouped as Ready for review

- **WHEN** job 的 `children` 数组含 `{ kind: "pr", href: "..." }`
- **AND** 无论其 `state` 值是什么
- **THEN** 该 job 归入 "Ready for review" 组

#### Scenario: Blocked job without PR grouped as Needs input

- **WHEN** job 的 `state === "blocked"` 且无 PR child
- **THEN** 该 job 归入 "Needs input" 组

#### Scenario: Working or idle job grouped as Working

- **WHEN** job 的 `state` 为 "working" 或 "idle" 且无 PR child
- **THEN** 该 job 归入 "Working" 组

#### Scenario: Terminal state job grouped as Completed

- **WHEN** job 的 `state` 为 "done" / "failed" / "stopped" 且无 PR child
- **THEN** 该 job 归入 "Completed" 组

### Requirement: Compute badge priority

系统 SHALL 按以下优先级计算 TitleBar badge：

- 红底：有 failed job
- 黄底：有 blocked job（无 failed）
- 绿底：有 ready-for-review job（无 failed 且无 blocked）
- 无 badge：working / 空列表 / 全 completed

#### Scenario: Failed job shows red badge

- **WHEN** 至少一个 job 的 `state === "failed"`
- **THEN** badge 显示红底 + 对应数量

#### Scenario: Only blocked shows amber badge

- **WHEN** 无 failed 但有 blocked job
- **THEN** badge 显示黄底 + blocked 数量

#### Scenario: Only ready-for-review shows green badge

- **WHEN** 无 failed 且无 blocked 但有 PR child 的 job
- **THEN** badge 显示绿底 + ready-for-review 数量

#### Scenario: All working or empty shows no badge

- **WHEN** 所有 job 为 working/idle/done/stopped 且无 PR child
- **OR** job 列表为空
- **THEN** 不显示 badge

### Requirement: Extract projectId from linkScanPath

系统 SHALL 从 `linkScanPath` 截取 `projects/` 后第一段作为 projectId。fallback 到 `encode_path(cwd)`。

#### Scenario: linkScanPath contains projects segment

- **WHEN** `linkScanPath` 格式为 `.../.claude/projects/<encoded_id>/...`
- **THEN** projectId = `<encoded_id>`

#### Scenario: linkScanPath empty falls back to encode_path(cwd)

- **WHEN** `linkScanPath` 为空或不含 `projects/` 段
- **AND** `cwd` 有值
- **THEN** projectId = `encode_path(cwd)`

#### Scenario: Both empty disables session jump

- **WHEN** `linkScanPath` 和 `cwd` 均为空
- **THEN** session 跳转按钮 SHALL 被禁用

### Requirement: Degrade gracefully when jobs directory absent

系统 SHALL 在 `~/.claude/jobs/` 不存在时隐藏所有 jobs UI 入口。

#### Scenario: Jobs directory does not exist at startup

- **WHEN** 应用启动时 `~/.claude/jobs/` 不存在
- **THEN** TitleBar jobs icon 不渲染
- **AND** 系统不建目录、不 watch

#### Scenario: Jobs directory appears after startup

- **WHEN** 用户点击 Jobs tab 或切回 Jobs tab 时
- **AND** `~/.claude/jobs/` 此时存在
- **THEN** 系统 SHALL 开始 watch 并展示数据

#### Scenario: SSH mode hides jobs entry

- **WHEN** 应用处于 SSH remote 模式
- **THEN** TitleBar jobs icon 不渲染

### Requirement: Display job list with status indicators

每个 job 行 SHALL 展示：状态 indicator (16px) / name (13px) / detail (12px muted) / PR chip (可选) / age (mono) / chevron。

#### Scenario: Working job shows animated spinner

- **WHEN** job 的 `state === "working"`
- **THEN** indicator 为 10px blue spinner (1.2s linear infinite)

#### Scenario: Terminal job shows static dot with state color

- **WHEN** job 的 `state` 为 done/failed/stopped/blocked/idle
- **THEN** indicator 为对应颜色的静态圆点

#### Scenario: Job with PR child shows PR chip

- **WHEN** job 的 `children[]` 含 `kind: "pr"` 项
- **THEN** 行内显示 PR chip（可点击跳浏览器）

### Requirement: Expand job to show details

点击 chevron 或行 SHALL 展开详情区域。

#### Scenario: Expand shows intent and metadata

- **WHEN** 用户展开一个 job
- **THEN** 显示 intent 原文（italic + 左竖线）/ detail 完整文本 / metadata chips（worktree branch / project / age / tempo）/ 操作按钮

#### Scenario: Ready-for-review shows Review PR as primary action

- **WHEN** 展开的 job 属于 "Ready for review" 组
- **THEN** 主操作为 "Review PR →"（跳浏览器）/ 次操作为 "打开 session →"

#### Scenario: Needs-input shows session jump as primary action

- **WHEN** 展开的 job 属于 "Needs input" 组
- **THEN** 主操作为 "回到 session 答复 →"

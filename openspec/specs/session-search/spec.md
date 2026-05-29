# session-search Specification

## Purpose

提供针对单个 session、单个项目、全量项目的文本搜索能力，支持 SSH 上下文下的分阶段限速搜索与基于文件 mtime 的搜索文本缓存，过滤掉 hard-noise / tool_result 内部 payload / sidechain 等不可见内容，使前端搜索体验与 UI 上的会话视觉对齐。
## Requirements
### Requirement: Search within a single session

系统 SHALL 在一个 session 的文本内容中搜索给定 query，返回有序命中列表，每条命中携带消息 uuid、命中在 content 中的偏移、以及简短上下文预览。

#### Scenario: Query matches text in multiple messages
- **WHEN** query 在该 session 的 3 条消息中各有命中
- **THEN** 结果 SHALL 包含 3 条命中，按消息时间戳排序，每条带预览片段

#### Scenario: Query matches nothing
- **WHEN** query 不在任何消息中出现
- **THEN** 结果 SHALL 为空命中列表，不抛错

#### Scenario: Case-insensitive match
- **WHEN** query 为小写而消息内容含同词的混合大小写形式
- **THEN** 命中 SHALL 仍然成立

### Requirement: Search across all sessions of a project

系统 SHALL 在指定 project 的所有 sessions 中搜索 query，按命中 session 聚合：每个匹配 session 一个结果条目，附命中数与若干预览片段。

#### Scenario: Project with 100 sessions and query matching 5
- **WHEN** query 命中 100 个 sessions 中的 5 个
- **THEN** 结果 SHALL 含 5 个 session 条目，按最近修改时间倒序

### Requirement: Search across all projects

系统 SHALL 支持跨所有项目的全局搜索，返回按 project 分组的结果，每组列出该项目下命中的 sessions、命中数、预览片段。

#### Scenario: Global search with query appearing in two projects
- **WHEN** query 命中两个不同项目下的 sessions
- **THEN** 结果 SHALL 含两个 project 分组，分别列出各自命中 sessions

### Requirement: Exclude filtered content from search index

系统 SHALL 在搜索匹配阶段排除 hard-noise 消息、`tool_result` 内部 payload、sidechain 消息，使搜索结果只反映用户在 UI 上可见的会话文本。

#### Scenario: Search term appears only inside a hard-noise system-reminder
- **WHEN** 唯一命中位于一条被分类为 hard noise 的消息内
- **THEN** 结果 SHALL NOT 包含该命中

### Requirement: Support staged-limit search over SSH contexts

系统 SHALL 在 SSH 上下文下的搜索按阶段施加结果数上限，避免过长的网络往返延迟；当当前阶段已收集到足够结果时 SHALL 提前返回。

#### Scenario: Global search over SSH with many matches
- **WHEN** 当前上下文是 SSH 且全局搜索 query 命中大量 sessions
- **THEN** 当达到配置的 SSH fast-search 阶段上限时，搜索 SHALL 返回部分但有序的结果集，且结果 SHALL 标注是否仍有更多结果可继续搜索

### Requirement: Cache extracted search text

系统 SHALL 缓存每个 session 的可搜索文本，使重复搜索在文件未变更时不重复解析整份 JSONL。

#### Scenario: Second search on same session after first
- **WHEN** 对同一 session 发起第二次搜索且该 session 在两次搜索之间未被修改
- **THEN** 系统 SHALL 复用缓存的搜索文本而非重新解析 JSONL

### Requirement: Search uses current Claude root

系统 SHALL 在执行 project 搜索与全局搜索时使用当前 Claude root 下的 `projects` 目录定位 session 文件；当前 Claude root 来自 `general.claudeRootPath`，为空时使用默认 home 下 `.claude`。

#### Scenario: Project search uses custom Claude root
- **WHEN** 当前 Claude root 配置为 `/data/claude-alt`
- **AND** 指定 project 的 sessions 位于 `/data/claude-alt/projects/<project_id>/`
- **THEN** 搜索 SHALL 从该目录读取 sessions
- **AND** SHALL NOT 从默认 `~/.claude/projects/<project_id>/` 读取 sessions

#### Scenario: Global search uses custom Claude root
- **WHEN** 当前 Claude root 配置为 `/data/claude-alt`
- **AND** `/data/claude-alt/projects/` 与默认 `~/.claude/projects/` 各自包含不同 project
- **THEN** 全局搜索 SHALL 只扫描 `/data/claude-alt/projects/`
- **AND** 默认 root 中仅存在的命中 SHALL NOT 出现在结果中

#### Scenario: Global search follows root change without restart
- **WHEN** 全局搜索已在默认 Claude root 下执行过
- **AND** 用户把 Claude root 更新为 `/data/claude-alt`
- **THEN** 后续全局搜索 SHALL 使用 `/data/claude-alt/projects/`

### Requirement: Search across all worktrees of a repository group

系统 SHALL 支持按 repository group 维度搜索：给定一组 worktree project ids，收集所有 worktree 对应目录下的 session 文件，全局按 mtime 降序排列后逐文件执行全文搜索。缺失的 worktree 目录 SHALL warn 并跳过，不中断搜索。SSH stage-limit 与 time_budget SHALL 按合并后的总文件数生效。

#### Scenario: Group search merges results from multiple worktrees in mtime order
- **WHEN** group 包含 worktree A（含 session X mtime=100）和 worktree B（含 session Y mtime=200）
- **AND** query 同时命中 session X 和 session Y
- **THEN** 结果 SHALL 含 2 个条目，session Y 在前（mtime 200 > 100）

#### Scenario: Group search skips missing worktree directory
- **WHEN** group 包含 worktree C 但其对应目录不存在
- **AND** worktree A 和 B 正常且命中结果
- **THEN** 结果 SHALL 只含 A 和 B 的命中，不报错

#### Scenario: Group search respects SSH stage-limit across worktrees
- **WHEN** 当前 context 是 SSH 且 group 包含 3 个 worktree 共 200 个 session 文件
- **AND** 搜索在处理第 40 个文件时已命中 min_results 条结果
- **THEN** 搜索 SHALL 在第一阶段上限处提前返回，result.is_partial = true

#### Scenario: Single worktree group degenerates to existing search
- **WHEN** group 仅含 1 个 worktree
- **THEN** 行为 SHALL 等价于对该 worktree 单独调用 `search_sessions`


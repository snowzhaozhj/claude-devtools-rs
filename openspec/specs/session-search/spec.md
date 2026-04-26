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

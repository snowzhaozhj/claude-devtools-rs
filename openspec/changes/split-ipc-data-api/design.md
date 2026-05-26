## Context

issue #303 9-PR plan 最后一步（PR 9），前序 PR 1-8 已全部 merge。`ipc-data-api` 当前 56 Requirement / 290 Scenario / 2518 行，远超 issue #296 规划的"收敛到 ~12 Requirement 仅描述 IPC 边界"目标。

按 issue #296 迁移表 + 当前实际 spec 内容，本 change 将 ~30 个 Requirement 按 domain owner 迁出，留 ~26 个协议层 Requirement。比 #296 原规划多留 ~14 个——这些是 #296 写成后新增的协议层基础设施（ProjectScanCache / Unified invalidator / typed struct / contract test / mtime overlay 等），属于 IPC 层正当 owner。

工艺直接复用 change `split-session-display`（PR #331）：source cap `REMOVED Requirements` + target cap `ADDED Requirements`，字符级搬运。

## Goals / Non-Goals

**Goals:**

- `ipc-data-api` 从 56 Requirement / 2518 行降到 ~26 Requirement / ~1200 行
- 30 个 Requirement 迁到 7 个 domain cap，每个 Requirement 有唯一 owner
- 行为契约 100% 不变（字符级对等，不允许修订 SHALL/WHEN/THEN 子句）
- 顺手修正 `Expose auxiliary read operations` 内 3 个本属 `project-discovery` 的 Scenario（Get worktree sessions / batch get-sessions-by-ids 内仓库分组引用）—— 连带迁出

**Non-Goals:**

- 不改代码 / 测试 / 配置 / IPC 字段名 / Tauri command 名（纯 spec 文档拆分）
- 不修改 ipc-data-api 留下的 ~26 个 Requirement 的 body（历史污染留后续 cleanup）
- 不重命名迁出的 Requirement 标题（标题字符级搬运，避免 spec-fidelity-reviewer 断裂）
- 不引入 BREAKING change

## Decisions

### D-1：行为契约 100% 不变（字符级对等）

**问题**：30 个 Requirement 跨 cap 移动，若修订了 SHALL/WHEN/THEN 子句会破坏行为契约。

**决策**：每个 ADDED Requirement 的 body 与所有 Scenario 字符级等于原 ipc-data-api spec.md 对应段落。校验手段：archive 前对比行数（迁出 + 留下 = 原始 2518 ± Purpose 段差异）。

### D-2：spec delta 工艺 — REMOVED + ADDED

**问题**：30 个 Requirement 离开 ipc-data-api，选两种 delta 写法。

**决策**：ipc-data-api delta 内 `## REMOVED Requirements` 段列 30 个 Requirement 标题；7 个 target cap 在各自 delta `## ADDED Requirements` 段写完整 body。理由同 change `split-session-display` D-2（语义清晰 + delta 体量最小化）。

### D-3：迁移分类表（30 Requirements → 7 targets）

| # | Requirement 标题 | 迁移目标 | 理由 |
|---|---|---|---|
| 1 | Expose SSH and context operations | `ssh-remote-context` | SSH domain 逻辑，已有 14 Req 在该 cap |
| 2 | Emit push events for file changes and notifications | `push-events` | push event 收发机制，PR 2 已建该 cap |
| 3 | Stream detected errors to subscribers | `push-events` | 错误事件推送，broadcast 机制属 push |
| 4 | Emit session metadata updates | `push-events` | metadata update push 机制 |
| 5 | Expose teammate messages on AIChunk | `team-coordination-metadata` | teammate 业务逻辑 |
| 6 | Expose teammate spawn metadata on ToolExecution | `team-coordination-metadata` | teammate 业务逻辑 |
| 7 | Strip teammate-message tags from session title | `team-coordination-metadata` | teammate 业务逻辑 |
| 8 | Expose subagent messages total count | `team-coordination-metadata` | teammate/subagent 派生数据 |
| 9 | Resolve project id from session id alone | `project-discovery` | 项目发现逻辑 |
| 10 | Expose git branch on session summary and metadata updates | `project-discovery` | git 元数据属项目发现 |
| 11 | Expose CompactChunk derived metadata in SessionDetail | `chunk-building` | chunk 语义层派生 |
| 12 | Expose repository group queries | `project-discovery` | 仓库分组查询 |
| 13 | Expose worktree sessions query | `project-discovery` | worktree 会话查询 |
| 14 | Tauri commands for repository groups and worktree sessions | `project-discovery` | 仓库/worktree Tauri 注册 |
| 15 | `extract_session_metadata` 按 `FileSignature` 缓存 | `session-parsing` | metadata 缓存策略属解析层 |
| 16 | metadata 缓存 ownership 由 `LocalDataApi` 持有 | `session-parsing` | 缓存所有权属解析层 |
| 17 | Expose memory read operations | `session-parsing` | memory 读取属会话解析上下文 |
| 18 | `extract_session_metadata` 流式判定 isOngoing | `session-parsing` | 流式解析策略 |
| 19 | `get_tool_output` 与 `get_image_asset` 走 parsed-message LRU 缓存 | `session-parsing` | 解析层 LRU 缓存 |
| 20 | parsed-message 缓存按 file-change 广播主动失效 | `session-parsing` | 缓存失效策略属解析层 |
| 21 | parsed-message 缓存 ownership 由 `LocalDataApi` 持有 | `session-parsing` | 缓存所有权属解析层 |
| 22 | Stable chunk identifiers in SessionDetail | `chunk-building` | chunk ID 稳定性属 chunk 语义 |
| 23 | Title length is bounded by TITLE_MAX_CHARS constant | `session-parsing` | title 截断属解析层 |
| 24 | Title algorithm changes do not invalidate MetadataCache | `session-parsing` | title 与 cache 交互属解析层 |
| 25 | IPC SHALL expose http_server_start / _stop / _status commands | `server-mode` | HTTP server 生命周期管理 |
| 26 | Expose group session listing via k-way merge pagination | `project-discovery` | 分组列表查询 |
| 27 | Tauri command for list_group_sessions | `project-discovery` | 分组 Tauri 注册 |
| 28 | SessionSummary 增加 worktree 元信息字段 | `project-discovery` | worktree 元数据属项目发现 |
| 29 | Expose telemetry snapshot pull endpoint | `application-telemetry` | 遥测数据暴露 |
| 30 | Expose telemetry correctness event batch endpoint | `application-telemetry` | 遥测事件批量 |
| 31 | SessionDetail 暴露与 SessionSummary 同源派生的 title | `session-parsing` | title 派生逻辑属解析层 |

### D-4：留在 ipc-data-api 的 ~26 个 Requirement（正当 owner）

协议层核心：
- Expose project and session queries（IPC 表面定义 + OMIT 系列开关 + isOngoing 双路判定 + HTTP 骨架）
- Expose search queries / config+notification / file+path validation / auxiliary read / Expose search via Tauri IPC command
- Validate inputs and return structured errors
- Lazy load subagent trace / inline image asset / tool output（payload 瘦身策略）
- Bulk and per-item notification operations
- Session list pagination avoids duplicate full scans / List sessions uses project-scoped light pagination
- Fetch session summaries by id
- Dispatch project/session reads by active context
- Session 列表序列化暴露 cwd 字段
- get_session_detail 本地路径以单文件 stat 取元数据
- Contract test asserts get_session_detail does not cross project boundary
- ProjectScanner shared read semaphore injection
- ProjectScanCache 按事件语义分级失效
- SessionDetail 与高频 DataApi 方法 SHALL 用 typed Rust struct
- SessionDetailMetrics 与 SessionDetailMetadata 字段定义
- ipc_contract 测试 SHALL 覆盖 typed 字段命名 round-trip
- Unified invalidator 作为 LocalDataApi.file_tx 唯一生产者
- ProjectScanCache 维护 per-project mtime overlay

### D-5：边界灰区裁定

| Requirement | 候选 | 裁定 | 理由 |
|---|---|---|---|
| Expose memory read operations | session-parsing / 新 cap `memory-viewer` | **session-parsing** | memory 读取依赖 session 上下文解析；memory-viewer cap 侧重 UI 展示而非数据获取 |
| Emit session metadata updates | push-events / session-parsing | **push-events** | metadata 产生属 session-parsing，但 broadcast 推送机制属 push-events domain；split 后 push-events 引用 session-parsing 的 cache 结果 |
| Expose subagent messages total count | team-coordination-metadata / chunk-building | **team-coordination-metadata** | subagent 是 team 概念的子集，count 派生属 team metadata |

### D-6：执行顺序 — 先大后小

按 Requirement 数量降序写 delta：session-parsing(9) → project-discovery(8) → team-coordination-metadata(4) → push-events(3) → chunk-building(2) → application-telemetry(2) → ssh-remote-context(1) → server-mode(1)。大 cap 先写能尽早暴露边界问题。

## Risks / Trade-offs

1. **体量风险**：30 个 Requirement 字符级搬运 ~1300 行 spec text，手误概率随体量线性增长。缓解：archive 前行数校验。
2. **引用断裂**：其他 spec 通过 `[[ipc-data-api::Requirement 标题]]` 引用被迁 Requirement 会断裂。缓解：grep 所有 `[[ipc-data-api::` 引用并更新指向新 cap。
3. **archive 原子性**：本 change 同时修改 9 个 cap 的 spec，archive 是原子操作（一步 mv），不会出现 #296 担心的"跨 PR 顺序覆盖"问题。

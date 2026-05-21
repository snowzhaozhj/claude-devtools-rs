## ADDED Requirements

### Requirement: Expose group session listing via k-way merge pagination

系统 SHALL 实现 `list_group_sessions(group_id, page_size, cursor)` IPC：定位 `group_id` 对应 `RepositoryGroup`，对 group 内 N 个 worktree 各自的 sessions（已在 `WorktreeGrouper` / `ProjectScanner` 层按 `mtime` 倒序）做 **k-way merge 流式分页**，返回 `GroupSessionPage { sessions: Vec<SessionSummary>, next_cursor: Option<String> }`。

实现 MUST 满足：
- **Server 无状态**：cursor 自描述每个 worktree 当前指针位置（`BTreeMap<worktree_id, WorktreeOffset>`，`WorktreeOffset` 枚举为 `NotStarted` / `AfterMtime { mtime_ms, sid }` / `Exhausted`），序列化为 base64(JSON)，重启服务后仍可继续分页
- **全序定义**：全局排序方向为 `(mtime_ms desc, sid asc)`——`mtime_ms` 大的排前，同 `mtime_ms` 时 `sid` 字典序小的排前
- **k-way merge**：内部用 `BinaryHeap<HeapEntry { mtime_ms, sid, worktree_id, idx }>`，`Ord` 实现按全序"排前者优先 pop"（max-heap 视角：`mtime_ms` 大 / 同 mtime 时 `sid` 小为"大"），取 `page_size` 条；每次 pop 后把对应 worktree 的下一条 push 回堆
- **续页定位**：cursor `AfterMtime { mtime_ms: last_mtime, sid: last_sid }` 表示"已消费到 `(last_mtime, last_sid)` 这条"；续页时对每个 worktree 二分定位 SHALL 找到第一条**严格在 `(last_mtime, last_sid)` 之后**的 session，即满足 `(s.mtime_ms < last_mtime) || (s.mtime_ms == last_mtime && s.sid > last_sid)` 的最早条目；MUST NOT 重复返回 `(last_mtime, last_sid)` 自身，MUST NOT 漏掉同 mtime 但 sid 更大的条目
- **不全量收集**：MUST NOT 在产出当前页前把 group 所有 sessions 全部 collect 到 `Vec`（避免 RSS 击穿）；MUST NOT 对每个 worktree 调 `list_sessions_sync(page_size = usize::MAX)` 复用全量路径
- **共享并发限流**：内部并发跑 `ProjectScanner::scan_project_dir` SHALL 使用 `LocalDataApi` 持有的共享 `Arc<Semaphore>`（见 `ProjectScanner shared read semaphore injection`），不得为每个 worktree 新建独立 semaphore
- **页面 SSE detail 触发**：返回页骨架后，SHALL fire-and-forget 触发 `session-metadata-update` 后台拉取，**仅**对当前页 sessions（key on `(project_id /*worktree id*/, session_id)`，复用现有 detail 拉取 active_scans 键空间），借 `active_scans` per-key cancel 在切页 / 切 group / 切 worktree filter 时取消旧拉取
- **worktree filter 通过 cursor 表达**：前端切 worktree filter 为某 worktree `wt-X` 时 SHALL 构造初始 cursor，让所有非 X 的 worktree `WorktreeOffset = Exhausted`，k-way merge 自然只产出 X 的 sessions（server 不感知 filter，纯 cursor 语义复用）

错误形态：
- `page_size == 0` SHALL 立刻返 `ApiError::validation`，message 含 `pageSize must be > 0`
- `group_id` 不存在 SHALL 返 `ApiError::not_found`
- cursor 反序列化失败 SHALL 视为首页请求（fallback 为 `cursor = null`），并在 tracing 写 `warn` 留痕

序列化 SHALL 使用 camelCase（`pageSize` / `nextCursor` / `worktreeId` / `worktreeName` / `cwdRelativeToRepoRoot`）。

#### Scenario: 首页请求返回 page_size 条按全局 mtime 倒序
- **WHEN** caller 调用 `invoke("list_group_sessions", { groupId: "g1", pageSize: 50, cursor: null })`，g1 含 2 个 worktree 各 30 个 session（mtime 交错）
- **THEN** 响应 `sessions` SHALL 含 50 条，按 `timestamp` 严格倒序
- **AND** 响应 `nextCursor` SHALL 非空，每个 worktree 的 offset 反映已消费到的最后一条

#### Scenario: 续页请求按 cursor 续位
- **WHEN** caller 接上一页 `nextCursor` 再调 `invoke("list_group_sessions", { groupId, pageSize: 50, cursor })`
- **THEN** 响应 SHALL 返回剩余 sessions，不重复返回上一页内容；保持全局 mtime 倒序

#### Scenario: 所有 worktree 流耗尽时 next_cursor 为 null
- **WHEN** caller 续到最后一页，所有 worktree offset SHALL 为 `Exhausted`
- **THEN** 响应 `nextCursor` SHALL 为 `null`

#### Scenario: 同 mtime session 按 sid 字典序稳定排序
- **WHEN** 两个 worktree 各含一条 `mtime_ms = 1000` 但 `sid` 不同的 session（`sidA` < `sidB`）
- **THEN** 全局排序 SHALL 把 `sidA` 排在 `sidB` 之前
- **AND** cursor 记录的 `AfterMtime { mtime_ms: 1000, sid: "sidA" }` SHALL 在续页时跳过 sidA 自身但保留 sidB

#### Scenario: 续页定位边界
- **WHEN** worktree W1 的 sessions 按全序为 `[(2000,"a"), (1000,"b"), (1000,"d"), (500,"c")]`，cursor `AfterMtime { mtime_ms: 1000, sid: "b" }`
- **THEN** 续页 SHALL 跳过 `(2000,"a")` 与 `(1000,"b")`，从 `(1000,"d")` 开始返回
- **AND** SHALL NOT 重复返回 `(1000,"b")`（cursor 自身已消费）
- **AND** SHALL NOT 漏掉 `(1000,"d")`（同 mtime 但 sid > "b"）

#### Scenario: worktree filter via cursor Exhausted
- **WHEN** caller 构造 cursor `{ "wt-X": NotStarted, "wt-other-1": Exhausted, "wt-other-2": Exhausted }` 调 `list_group_sessions`
- **THEN** 响应 sessions SHALL 仅含 `wt-X` 的 sessions（按 mtime 倒序）
- **AND** 续页 cursor 中 `wt-other-1` / `wt-other-2` SHALL 仍为 `Exhausted`

#### Scenario: 不全量收集
- **WHEN** group 含 10 个 worktree 各 100 个 session（共 1000 条），caller 请求 `pageSize: 20`
- **THEN** 实现内部 MUST NOT 把 1000 条 session 全部加载到内存再排序分页
- **AND** 单次请求 RSS 增量 SHALL 在 200 KB 量级（骨架字段 × 1000 条）

#### Scenario: pageSize 为 0 时拒绝
- **WHEN** caller 调用 `invoke("list_group_sessions", { groupId: "g1", pageSize: 0 })`
- **THEN** SHALL 立即返 `ApiError::validation`，message 含 `pageSize must be > 0`

#### Scenario: 损坏 cursor fallback 为首页
- **WHEN** caller 调用 `invoke("list_group_sessions", { groupId: "g1", pageSize: 50, cursor: "invalid-base64" })`
- **THEN** 实现 SHALL fallback 为首页请求（等价 `cursor = null`），返回首页内容
- **AND** SHALL 在 tracing 写 `warn` 留痕

### Requirement: Tauri command for list_group_sessions

系统 SHALL 通过 Tauri `invoke_handler!` 注册 `list_group_sessions` IPC command，入参顶层 `groupId: string` + `pageSize?: number` + `cursor?: string`（**不**嵌套 `pagination` 对象，与既有 `list_sessions` 保持一致），返回 `GroupSessionPage`。command 名 SHALL 同步出现在 `crates/cdt-api/tests/ipc_contract.rs::EXPECTED_TAURI_COMMANDS` 与 `ui/src/lib/tauriMock.ts::KNOWN_TAURI_COMMANDS` 两处常量列表中。

HTTP 路径 SHALL 走 `GET /api/repository-groups/{groupId}/sessions?pageSize=...&cursor=...` query string。

#### Scenario: invoke list_group_sessions 返回 GroupSessionPage
- **WHEN** 前端调用 `invoke("list_group_sessions", { groupId: "g1", pageSize: 20, cursor: null })`
- **THEN** 响应 SHALL 为 `{ sessions: SessionSummary[], nextCursor: string | null }` 形态（camelCase）

#### Scenario: command 注册在 invoke_handler 与 mock 列表
- **WHEN** ipc_contract 测试遍历 `EXPECTED_TAURI_COMMANDS`
- **THEN** `list_group_sessions` SHALL 在列表内
- **AND** `ui/src/lib/tauriMock.ts::KNOWN_TAURI_COMMANDS` SHALL 含 `list_group_sessions`

### Requirement: SessionSummary 增加 worktree 元信息字段

系统 SHALL 在 `SessionSummary`（IPC 序列化形态）中增加 `worktreeId: String` / `worktreeName: String` / `groupId: String` / `cwdRelativeToRepoRoot: Option<String>` 四个字段：
- `worktreeId` = 该 session 所属 worktree 的 id（等同底层 `Project.id`，encoded project dir 名）
- `worktreeName` = 该 session 所属 worktree 的展示名
- `groupId` = 该 session 所属 `RepositoryGroup.id`（让前端按 group 维度过滤 SSE event / cache key）
- `cwdRelativeToRepoRoot` = 该 session 所属 `Worktree.cwd_relative_to_repo_root`（`None` 时通过 `#[serde(skip_serializing_if = "Option::is_none")]` 省略）

这四个字段 SHALL 同时出现在 `list_sessions` / `list_group_sessions` / `get_worktree_sessions` 三个 IPC 返回的 `SessionSummary` 中，保证 UI 在任一调用路径下都能拿到 worktree / group 归属信息。

**填值来源（scheme c join）**：IPC handler 在序列化 `SessionSummary` 时，从 `LocalDataApi` 持有的轻量 `worktree_id → (worktree_name, group_id, cwd_relative_to_repo_root)` 映射缓存（随 `list_repository_groups` 调用刷新）查表填入。`cdt-core::Session` SHALL NOT 持有这些字段，避免 scanner 阶段重走 repo 解析。

映射缓存 MUST 在 `list_repository_groups` 调用后立即填充；后续 IPC（含 `list_sessions` / `list_group_sessions` / `get_worktree_sessions`）SHALL 复用同一映射；缓存失效 SHALL 在 grouper 重跑（filesystem 变化触发 refresh）时整体替换。

序列化 SHALL 使用 camelCase。

#### Scenario: 映射缓存随 list_repository_groups 刷新
- **WHEN** caller 调 `invoke("list_repository_groups")` 后再调 `invoke("list_group_sessions", { groupId })`
- **THEN** 后者返回的每条 `SessionSummary` SHALL 含 `worktreeId` / `worktreeName` / `groupId` / `cwdRelativeToRepoRoot`（非 None 时）字段
- **AND** 这些字段 SHALL 与 `list_repository_groups` 返回的 group 内对应 worktree 信息一致

#### Scenario: 缓存未填充时 SessionSummary 缺 worktree 字段
- **WHEN** caller 在首次 `list_repository_groups` 之前调用 `list_sessions(projectId, ...)`（理论上不发生，UI 启动顺序保证 list_repository_groups 在前）
- **THEN** 返回的 SessionSummary `worktreeId` SHALL 等于 `projectId`（fallback：worktree id 就是 project id），`groupId` SHALL 等于 `projectId`（fallback：单 worktree group），`cwdRelativeToRepoRoot` SHALL 为 None

#### Scenario: list_sessions 返回 SessionSummary 含 worktree 字段
- **WHEN** caller 调用 `invoke("list_sessions", { projectId, pageSize: 10 })`
- **THEN** 响应 `items[i]` SHALL 含 `worktreeId` / `worktreeName` / `groupId` 字段（对应该 session 所在 Project / Worktree / Group）

#### Scenario: repo 根 session 省略 cwdRelativeToRepoRoot
- **WHEN** session 所属 worktree `is_repo_root = true`
- **THEN** SessionSummary 序列化 SHALL 省略 `cwdRelativeToRepoRoot` 键
- **AND** SHALL 仍含 `worktreeId` / `worktreeName` / `groupId` 字段

#### Scenario: 子目录 session 含 cwdRelativeToRepoRoot
- **WHEN** session 所属 worktree `is_repo_root = false` 且 `cwd_relative_to_repo_root = Some("crates")`
- **THEN** SessionSummary 序列化 SHALL 含 `"cwdRelativeToRepoRoot": "crates"`

### Requirement: ProjectScanner shared read semaphore injection

`ProjectScanner` SHALL 接受外部注入的 `Arc<tokio::sync::Semaphore>` 控制 head-read 并发，所有 `LocalDataApi` 内部调用 SHALL 复用同一 `Arc<Semaphore>` 实例（容量默认 `SHARED_READ_CONCURRENCY = 64`）；MUST NOT 在每次 IPC（含 `list_sessions` / `list_group_sessions` / `list_repository_groups`）新建独立 semaphore，否则多 IPC 并发时实际并发上限会变为 `IPC 数 × 64`，违反 `.claude/rules/perf.md::CPU 反模式`。

`ProjectScanner::new(projects_dir, fs)` 旧构造器 SHALL 保留为 `#[cfg(test)]` 便利构造（内部仍新建 semaphore），生产代码 SHALL 调 `ProjectScanner::new_with_semaphore(projects_dir, fs, semaphore)`。

`LocalDataApi` SHALL 在构造时创建 / 接受 `shared_read_semaphore: Arc<Semaphore>` 字段，所有内部 `ProjectScanner` 构造点 SHALL 传入该字段。

#### Scenario: 19 worktree 并发拉骨架共享 semaphore
- **WHEN** `list_group_sessions` 内部并发跑 19 个 `scan_project_dir`，每个 worktree 含 100 个 session
- **THEN** 同时 in-flight 的 `read_lines_head` 调用数 SHALL 不超过 64（共享 semaphore 容量）
- **AND** SHALL NOT 出现 19 × 64 = 1216 并发的击穿

#### Scenario: 测试代码可用 new 便利构造
- **WHEN** `#[cfg(test)]` 单测调 `ProjectScanner::new(projects_dir, fs)`
- **THEN** 测试代码无需手动创建 semaphore；编译通过

#### Scenario: 生产代码强制走 new_with_semaphore
- **WHEN** 生产代码 grep `ProjectScanner::new\(` （非 cfg(test) 块）
- **THEN** SHALL 仅出现 `new_with_semaphore` 调用，老 `new` 调用 SHALL 仅在 `#[cfg(test)]` 块内

## MODIFIED Requirements

### Requirement: Expose repository group queries

系统 SHALL 暴露 `list_repository_groups()` IPC：把 `ProjectScanner::scan()` 结果通过 `WorktreeGrouper::group_by_repository` 聚合为 `Vec<RepositoryGroup>`，每个 group 含 `id` / `identity` / `name` / `worktrees[]` / `mostRecentSession` / `totalSessions` 字段。Worktree 排序 SHALL 按 `is_repo_root` 优先（repo 根排前）、再按 `is_main_worktree` 优先、再按 `most_recent_session` 倒序（已在 `WorktreeGrouper` 内部实现）。Group 排序 SHALL 按 `mostRecentSession` 倒序。

每个 `Worktree` 序列化形态 SHALL 含 `id` / `path` / `name` / `gitBranch` / `isMainWorktree` / `isRepoRoot` / `cwdRelativeToRepoRoot`（`None` 时省略）/ `sessions` / `createdAt` / `mostRecentSession` 字段。

序列化 SHALL 使用 camelCase（`isMainWorktree` / `isRepoRoot` / `cwdRelativeToRepoRoot` / `gitBranch` / `mostRecentSession` / `totalSessions` / `createdAt`）。

#### Scenario: 列出多 worktree 仓库分组
- **WHEN** 同一 git 仓库下存在主 worktree 与一个用户开的附加 worktree，且两者都有 sessions
- **THEN** `list_repository_groups()` SHALL 返回一个 group，`worktrees` 数组含两项，`worktrees[0].isMainWorktree=true` 且 `worktrees[0].isRepoRoot=true`、`worktrees[1].isMainWorktree=false` 且 `worktrees[1].isRepoRoot=false`

#### Scenario: 独立项目作为单成员分组
- **WHEN** 一个 project 路径无 git 元数据（不属任何 worktree）
- **THEN** `list_repository_groups()` SHALL 返回一个 group，`worktrees` 数组含该项目一项，`identity` 为 `null`

#### Scenario: 序列化 camelCase
- **WHEN** `list_repository_groups()` 返回结果被序列化为 JSON
- **THEN** 字段名 SHALL 为 `isMainWorktree` / `isRepoRoot` / `cwdRelativeToRepoRoot`（仅在非 None 时出现）/ `gitBranch` / `mostRecentSession` / `totalSessions` / `createdAt`（不是 snake_case）

#### Scenario: 主仓子目录 cwd 作为同 group 内的非 repo-root worktree
- **WHEN** 主 repo `/repo` 含 `.git` 目录；另存在 project `/repo/crates`（独立 encoded 目录）
- **THEN** `list_repository_groups()` SHALL 返回一个 group，两 worktree 均在该 group 内
- **AND** `/repo` 对应 worktree `isRepoRoot=true` 排第一位
- **AND** `/repo/crates` 对应 worktree `isRepoRoot=false`，`cwdRelativeToRepoRoot="crates"` 排在后面

### Requirement: Expose worktree sessions query

系统 SHALL 保留 `get_worktree_sessions(group_id, pagination)` IPC 作为兼容入口：定位 `group_id` 对应 `RepositoryGroup`，把该 group 下所有 worktree 的 sessions 合并为单一列表，按 `timestamp` 倒序后再应用 `PaginatedRequest`（`pageSize` + `cursor`）。返回 `PaginatedResponse<SessionSummary>`，每个条目 SHALL 含 `worktreeId` / `worktreeName` / `cwdRelativeToRepoRoot`（仅非 None 时）字段。

**注意**：sidebar 默认 session 列表加载路径 SHALL 走 `list_group_sessions`（k-way merge 流式分页），**不**走本 IPC（避免 `page_size = usize::MAX` 全量扫描风险）。`get_worktree_sessions` 仅供"需要拿一整个 group 的合并 sessions"等特殊场景使用，且实现内部 SHALL NOT 用 `usize::MAX` 全量收集。

`pageSize == 0` 时 SHALL 立即拒绝（`ApiError::validation`），`pageSize` 不再被静默 clamp 为 1，避免隐藏调用方错误参数。

未命中 `group_id` 时 SHALL 拒绝（`ApiError::not_found`）。

错误形态遵循既有项目约定：trait / HTTP 层产 `ApiError { code, message }` 结构化错误；Tauri command wrapper 沿用 `Result<_, String>` —— 把 `ApiError` 通过 `to_string()` 序列化为含错误前缀的人类可读字符串（与 `list_sessions` / `get_session_detail` 等既有 command 一致），结构化 `code` 字段仅在 HTTP `axum::IntoResponse` 路径暴露。

Tauri command 入参 SHALL 与既有 `list_sessions` 风格一致——顶层 `groupId: string` + `pageSize?: number` + `cursor?: string`，**不**嵌套 `pagination` 对象。HTTP 路径走 `GET /api/worktrees/{groupId}/sessions?pageSize=...&cursor=...` query string。

#### Scenario: 合并多 worktree sessions 按时间排序
- **WHEN** caller 调用 `invoke("get_worktree_sessions", { groupId: "repo-1", pageSize: 10 })`，repo-1 含两个 worktree 各 5 个 session
- **THEN** 响应 `items` SHALL 含 10 项，按 `timestamp` 倒序排列
- **AND** 每项 SHALL 含 `worktreeId` / `worktreeName` 字段

#### Scenario: 分页继续
- **WHEN** caller 接上一页 `nextCursor` 再调 `invoke("get_worktree_sessions", { groupId, pageSize, cursor: nextCursor })`
- **THEN** 响应 SHALL 返回剩余 sessions，不重复返回上一页内容

#### Scenario: pageSize 为 0 时拒绝
- **WHEN** caller 调用 `invoke("get_worktree_sessions", { groupId: "g1", pageSize: 0 })`
- **THEN** trait 层 SHALL 立刻返 `ApiError::validation(...)`，message 含 `pageSize must be > 0`
- **AND** Tauri command wrapper 把 ApiError 字符串化后让 `invoke` Promise reject 含该 message；HTTP 层走 `IntoResponse` 返 400 + `{code: "validation_error", message}` 结构化 JSON
- **AND** SHALL NOT 静默 clamp 为 1 也 SHALL NOT 返回部分结果

#### Scenario: group_id 不存在
- **WHEN** caller 调用 `invoke("get_worktree_sessions", { groupId: "nonexistent-group", pageSize: 10 })`
- **THEN** trait 层 SHALL 返 `ApiError::not_found(...)`，message 含 group id 标识符
- **AND** Tauri command wrapper 把 ApiError 字符串化后让 `invoke` Promise reject；HTTP 层走 `IntoResponse` 返 404 + `{code: "not_found", message}` 结构化 JSON

#### Scenario: 实现不允许全量扫描
- **WHEN** caller 调用 `invoke("get_worktree_sessions", { groupId, pageSize: 10 })`
- **THEN** 实现 SHALL NOT 在内部对各 worktree 调 `list_sessions_sync(pageSize = usize::MAX)`
- **AND** SHALL 走与 `list_group_sessions` 等价的 k-way merge 路径（或共享同一实现）

## MODIFIED Requirements

### Requirement: Strip teammate-message tags from session title

`extract_session_metadata` 提取的 `SessionSummary.title` MUST 在做长度截断之前剥除任何 `<teammate-message ...>...</teammate-message>` 包裹片段，避免 sidebar 标题吐出原始 XML。

实现 SHALL 在 `cdt-api::session_metadata` 标题提取路径中完成两步——先调 `extract_teammate_summary_title` 跑 fast-path；未命中（非 teammate 主导，或主导但 summary + body 都空）再走 `sanitize_for_title` 跑 fallback 整段剥标签。两个 helper 是独立函数，调用顺序由 `extract_session_metadata_with_ongoing` / `extract_session_metadata_from_parsed` 统一编排：

1. **Fast-path（teammate 主导消息）**：若 trim 后 text 以 `<teammate-message` 开头，SHALL 按以下优先级提取标题候选：
   - **优先 `summary` 属性**：regex 抽 `summary="..."` 属性内容，非空时 SHALL 直接返回作为标题候选（截断长度由常量 `TITLE_MAX_CHARS` 控制）
   - **fallback 到 body 文本**（2026-05-21 修订）：`summary` 属性缺失或值为空时，SHALL 提取开标签 `>` 与闭标签 `</teammate-message>` 之间的 body 文本（无闭标签时取剩余全部），trim 后非空则作为标题候选。**只有** body 也为空时才退回到下一步。
   理由：用户实测发现"teammate-message 主导消息但作者忘写 `summary` 属性"是常见场景（如 `<teammate-message teammate_id="team-lead">用户在 Claude Code auto mode 下调用 codex:codex-rescue 被拦截...</teammate-message>`），body 才是真实对话内容；初版 spec 直接剥除整段会让 title 永久 null，UI 列表项 fallback 到 sessionId 前缀让用户无法识别 session。

2. **Fallback（剥标签）**：若 fast-path 完全未命中（非 teammate 主导，或主导但 summary + body 都空），SHALL 在既有标签剥除循环中追加 `teammate-message` 标签——把整段 `<teammate-message ...>body</teammate-message>` 从文本中删除（含 attributes 与 inner body）。剥除后若文本为空，SHALL 回退到 `command_fallback` 或 `None`，按既有路径处理。

   注意：此 fallback 仅在 **非 teammate 主导消息**（混合内容中嵌入 teammate 块）时触发；teammate 主导消息已在 fast-path 由 summary 或 body 兜底，不会落到这一步剥除 body。

`sanitize_for_title` MUST 不再在标题里输出任何 `<teammate-message` / `</teammate-message>` 字面量。

#### Scenario: Title takes summary attribute when message is wrapped solely by teammate-message
- **WHEN** session 第一条 user 消息 content 为 `<teammate-message teammate_id="alice" summary="Set up project">body</teammate-message>`
- **THEN** `extract_session_metadata.title` SHALL 为 `Some("Set up project")`

#### Scenario: Title falls back to body when teammate-message has no summary
- **WHEN** session 第一条 user 消息 content 为 `<teammate-message teammate_id="alice">用户在 auto mode 下调用 codex 被拦截</teammate-message>`（无 summary 属性，body 非空）
- **THEN** `extract_session_metadata.title` SHALL 为 `Some("用户在 auto mode 下调用 codex 被拦截")`
- **AND** title SHALL NOT 含 `<teammate-message` / `</teammate-message>` 字面量

#### Scenario: Title returns None when teammate-message has no summary and empty body
- **WHEN** session 第一条 user 消息 content 为 `<teammate-message teammate_id="alice"></teammate-message>`（无 summary 属性，body 也空）
- **THEN** `extract_session_metadata.title` SHALL 为 `None` 或退回到 `command_fallback`

#### Scenario: Mixed content strips teammate-message tag
- **WHEN** 第一条 user 消息 content 为 `Hello team. <teammate-message teammate_id="alice">body</teammate-message> please continue.`
- **THEN** title SHALL 不含 `<teammate-message`，剥除后 SHALL 仅保留 `Hello team.  please continue.`（trim 后），整体走既有截断路径
- **AND** 此场景下 teammate 块按 fallback 路径整段剥除（含 body），与 teammate 主导消息的 body fallback 行为不冲突

## Context

Rust 端口当前已经具备 `PaginatedRequest` / `PaginatedResponse` 与 `session-metadata-update` 机制，但 Sidebar 仍有“加载完整会话历史”的 spec 契约，后端列表路径也容易退化为先发现/排序完整集合再分页。原版 `claude-devtools` 的实际列表优化不是持久索引或流式近似发现，而是 project-scoped cursor pagination：首屏 `limit = 20`，`includeTotalCount = false`，`prefilterAll = false`，`metadataLevel = 'light'`，滚动继续分页，pinned sessions 通过按 id 补拉保障可见。

## Goals / Non-Goals

**Goals:**

- 对齐原版会话列表语义：Sidebar 首屏只请求当前项目/当前 worktree 的第一页，默认 `pageSize = 20`。
- 让 `list_sessions` 成为 project-scoped cursor pagination 入口，不再承诺 Sidebar 首屏完整加载项目历史。
- 保持列表同步响应轻量，深度 metadata 仅对当前页或可见窗口触发后台扫描。
- 让 pinned/hidden session 的可见性依赖按 id 补拉/合并，而不是依赖第一页命中。
- 避免 Dashboard 项目概览触发所有项目 sessions 列表加载。

**Non-Goals:**

- 不引入持久索引、SQLite、跨启动缓存或 schema migration。
- 不设计“近似首屏 + 后台全局收敛”的新流式协议。
- 不在本 change 中实现全局最近会话列表；如需全局 feed，另开 change 明确定义行为。
- 不改变 session detail 的 deep 解析语义。

## Decisions

### D1: `list_sessions` 保持 project-scoped，不升级为全局 feed

`list_sessions(projectId, pageSize, cursor)` SHALL 只返回指定 project/worktree 的 sessions，按 mtime 倒序、cursor 分页。Sidebar 选中 project/worktree 后只消费该 project 的第一页和后续页。

候选方案：
- 全局最近会话 feed：能统一跨项目排序，但需要重新定义 Dashboard/Sidebar 语义，并且容易滑向流式近似设计。
- Project-scoped pagination：与原版一致，改动范围小，能直接减少首屏 work。

选择 project-scoped pagination，因为本 change 目标是对齐原版，而不是发明新导航模型。

### D2: 首屏默认 `pageSize = 20`，`total` 不作为精确契约

前端 Sidebar 首次加载使用 `pageSize = 20`，后续滚动用 `nextCursor` 拉更多。`PaginatedResponse.total` 在列表首屏路径不再作为精确总数依赖；如果底层类型暂时仍要求 number，可保留兼容值，但 UI SHALL 只依赖 `nextCursor` / `hasMore` 判断是否继续加载。

候选方案：
- 保留精确 `total`：调用方体验简单，但会迫使后端统计完整集合，抵消分页收益。
- 弱化 `total`：对齐原版 `includeTotalCount=false`，减少首屏阻塞。

选择弱化 `total` 依赖；如需精确数量，由后续专用统计能力提供。

### D3: 列表同步响应采用 light metadata，deep metadata 按当前页/可见窗口扫描

同步 `list_sessions` 返回的 `SessionSummary` SHALL 包含 `sessionId` / `projectId` / `timestamp` 等轻量字段，`title` / `messageCount` / `isOngoing` / `gitBranch` 可为占位。后台扫描触发范围限定为本次返回页；前端如需要对新可见窗口补 metadata，可通过继续分页或后续专用能力触发，不应一次扫完整项目。

候选方案：
- 首包 deep metadata：列表更完整，但会 parse 多个 JSONL 文件，首屏慢。
- light metadata + event patch：沿用当前 Rust 端口机制，也对齐原版 `metadataLevel='light'` 的优化目的。

选择 light metadata + event patch。

### D4: Pinned/hidden session 用按 id 补拉合并

Pinned/hidden 状态不能假设 session 一定位于第一页。前端 SHALL 在拿到 pinned/hidden id 集合后调用按 session ids 查询的能力补齐缺失 `SessionSummary`，再与分页结果合并。hidden 仍可在 UI 过滤，pinned 可固定展示或参与排序，具体视觉保持既有行为。

候选方案：
- 扩大第一页直到包含所有 pinned：会退化为完整加载。
- 按 ids 补拉：与原版 `get-sessions-by-ids` 对齐，成本与 pinned 数量成正比。

选择按 ids 补拉。

### D5: Dashboard 不触发 sessions 列表加载

Dashboard 项目卡片只消费 project discovery / project summary 能力，不为了展示项目概览调用每个 project 的 `list_sessions`。需要显示最近会话时，仅对用户打开/展开的项目按分页加载。

候选方案：
- Dashboard 预取所有项目 sessions：能让后续点击更快，但会把成本前置到首页。
- Dashboard 只展示项目级信息：首屏稳定，符合“列表按需加载”。

选择 Dashboard 不预取 sessions。

### D6: Command Palette 不再依赖 Sidebar 本地完整会话数组

旧契约要求 Command Palette 搜索覆盖默认第一页后的旧会话，因此迫使 Sidebar 完整加载。新契约下，Command Palette 可以先搜索已加载 sessions；全历史搜索 SHALL 调用 `session-search` 或显式逐页加载，不得把完整历史加载作为 Sidebar 首屏副作用。

候选方案：
- 保持完整本地数组：搜索简单，但与首屏性能目标冲突。
- 搜索入口显式承担全历史成本：行为清晰，性能成本只在用户搜索时发生。

选择显式搜索能力。

### D7: 历史浏览时稳定性优先于实时整表刷新

用户滚动离开列表顶部浏览历史时，Sidebar SHALL 暂停 file-change 触发的 silent full refresh，只保留已加载 session 的 metadata patch；待用户回到顶部或点击“有更新”提示时再刷新第一页。分页加载更多时保持已加载顺序，只追加新页，不因 metadata 或补拉结果对整表重新排序。自动补页只用于首屏/resize 后填满容器，不在每次加载更多后连续追页。

候选方案：
- 锚点保持：实时性更强，但需要扩展虚拟列表 scrollTo/anchor 能力，时序风险更高。
- 暂停刷新 + 追加稳定：符合浏览历史时“不要打断我”的体验，改动范围小。

选择暂停刷新 + 追加稳定；顶部实时严格排序让位于历史浏览的滚动稳定性。

## Risks / Trade-offs

- [Risk] 首屏不再包含旧页 session，用户滚动前看不到历史深处条目。→ Mitigation：保留 infinite scroll / load more，`hasMore` 明确驱动继续加载。
- [Risk] Command Palette 旧的本地搜索测试会失败。→ Mitigation：把全历史搜索迁移到 `session-search` 或显式分页加载测试，不让 Sidebar 首屏承担。
- [Risk] `total` 兼容字段与新语义冲突。→ Mitigation：spec 明确 UI 不依赖精确 `total`；实现期若类型允许，改为 `Option<usize>`，否则填当前已知下界并同步 contract test。
- [Risk] metadata scan 只扫当前页后，旧页 metadata 直到加载该页才补齐。→ Mitigation：这是预期行为；测试覆盖“每次分页只推送该页 metadata”。
- [Risk] 按 ids 补拉需要新增 IPC/trait 方法。→ Mitigation：方法语义窄，只按 session ids 返回 light summaries，不做全局搜索。
- [Risk] 浏览历史时顶部 ongoing 更新延迟可见。→ Mitigation：显示“有更新”提示，用户点击或滚回顶部后刷新。
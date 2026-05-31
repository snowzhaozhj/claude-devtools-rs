## ADDED Requirements

### Requirement: Grouper 并发限流

`WorktreeGrouper::group_by_repository` SHALL 限制同时进行的 `spawn_blocking` 任务数量不超过 `GROUPER_CONCURRENCY_LIMIT`（默认 8），避免冷启动时瞬间打满 blocking 线程池。结果顺序 SHALL 与输入 projects 顺序一致。

#### Scenario: Grouper 并发度不超过上限

- **GIVEN** 一组 30 个 project
- **WHEN** `group_by_repository` 执行
- **THEN** 同时 active 的 spawn_blocking 任务 SHALL ≤ GROUPER_CONCURRENCY_LIMIT
- **AND** 最终结果与无限流时一致（顺序保持）

### Requirement: Groups 结果缓存

`list_repository_groups_inner` SHALL 缓存 grouper 计算结果，key 为 `(root_generation, context_generation, scan_invalidation_generation)` 三元组，附加 TTL 兜底（≤ 10 秒）。缓存命中时 SHALL 跳过 grouper 执行直接返回。

#### Scenario: Groups cache 命中时跳过 grouper

- **GIVEN** groups cache 未过期且三元组 generation 未变
- **WHEN** 调用 `list_repository_groups_inner`
- **THEN** SHALL 直接返回缓存结果，不执行 `group_by_repository`

#### Scenario: Generation 变化时 cache 失效

- **GIVEN** groups cache 存在
- **WHEN** `scan_invalidation_generation` / `context_generation` / `root_generation` 任一递增
- **THEN** 下次调用 SHALL 重跑 grouper 并更新 cache

#### Scenario: TTL 过期时 cache 失效

- **GIVEN** groups cache 存在但创建时间超过 TTL（10 秒）
- **WHEN** 调用 `list_repository_groups_inner`
- **THEN** SHALL 重跑 grouper 并更新 cache

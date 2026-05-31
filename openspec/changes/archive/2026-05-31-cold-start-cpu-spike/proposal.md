# Proposal: cold-start-cpu-spike

## Problem

冷启动时 WorktreeGrouper 通过无界 `join_all` 一次性 dispatch ~54 个 `spawn_blocking` 任务（27 project × 2 dunce::canonicalize），瞬间打满 `max_blocking_threads=64` 的线程池，导致 78 线程 / 32% CPU 峰值。

此外，每次 `list_group_sessions` IPC 调用都会重新执行完整 grouper（包括所有 spawn_blocking），尽管 project scan 已有 cache，grouper 结果本身无缓存。

## Scope

- **根因 1**：`cdt-discover::WorktreeGrouper::group_by_repository` 的无界 `join_all`
- **根因 3**：`cdt-api::LocalDataApi::list_repository_groups_inner` 每次重跑 grouper 无缓存

根因 2（FSEvents 首扫 cache invalidation 竞态）可能被 1+3 间接缓解，本 change 不直接修。

## Success Criteria

- 启动峰值线程：≤ 35（当前 78）
- 启动 CPU 峰值：< 15%（当前可达 32%）
- `list_repository_groups` wall time：不回归（≤ 150ms，当前基线 95ms）
- 无 stale data 用户可感知场景

## Capabilities Affected

- `project-discovery`（cdt-discover）
- `ipc-data-api`（cdt-api）

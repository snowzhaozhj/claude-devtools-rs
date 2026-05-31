# Tasks: cold-start-cpu-spike

## 1. Grouper 并发限流

- [x] 1.1 在 `worktree_grouper.rs` 添加 `GROUPER_CONCURRENCY_LIMIT` 常量（= 8）
- [x] 1.2 在 `group_by_repository` 的 `join_all` 闭包内加 Semaphore acquire
- [x] 1.3 添加单元测试验证限流生效（mock git resolver + 计数并发度）

## 2. Groups 缓存

- [x] 2.1 在 `project_scan_cache.rs` 暴露 `invalidation_generation()` public getter
- [x] 2.2 在 `local.rs` 新增 `GroupsCacheEntry` struct 和 `groups_cache` 字段
- [x] 2.3 `list_repository_groups_inner` 开头加 cache hit 逻辑
- [x] 2.4 在所有 invalidation 路径清 groups_cache（generation 比较自动失效，无需主动清除）
- [x] 2.5 cache 逻辑由 perf_cold_scan bench + 现有 list_group_sessions 测试间接覆盖

## 3. 验证

- [x] 3.1 跑 `perf_cold_scan` bench 确认 wall time 不回归（112ms ≤ 150ms ✅）
- [ ] 3.2 手动测 `just dev` 启动观察 Activity Monitor 线程数 + CPU%

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过
- [ ] N.4 archive change

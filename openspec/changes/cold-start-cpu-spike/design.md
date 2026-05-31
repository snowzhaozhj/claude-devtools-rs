# Design: cold-start-cpu-spike

## Decisions

### D1: Grouper 并发限流用 Semaphore 而非 buffer_unordered

**选择**：在现有 `join_all` 内部加 `Arc<Semaphore>` permit guard。

**理由**：
- `join_all` 保证返回顺序与输入一致，后续 `projects.into_iter().zip(resolved)` 依赖顺序对应
- `buffer_unordered` 按完成顺序产出，需额外 index + 重排，改动面更大
- Semaphore 不改现有结构，只在每个 async 闭包开头 acquire permit

**cap = 8**：2× `worker_threads(4)`，对 I/O-bound stat 调用提供足够并行度，同时将瞬时 blocking 线程从 ~54 压到 ≤ 8。

### D2: Groups 缓存用 generation 组合 key + 短 TTL 双保险

**选择**：`(root_generation, context_generation, scan_invalidation_generation)` 三元组作为 cache key，命中时直接返回 `Arc<Vec<RepositoryGroup>>`。额外加 10 秒 TTL 兜底。

**理由**：
- generation 变化覆盖：项目目录文件事件（watcher invalidate → scan gen bump）、SSH 切换（context gen bump）、Claude root 切换（root gen bump）
- **不覆盖**：用户在外部做 git branch switch 不触发 Claude watcher → generation 不变但 git_branch 可能过时
- 10 秒 TTL 兜底：最多 10 秒后 grouper 重跑刷新 branch 元数据，可接受（原版 TS 也无实时 branch 更新）
- TTL 过期不阻塞——过期时异步刷新，当次返回旧值（stale-while-revalidate 不适用此场景因 grouper 很快，直接同步刷新即可）

### D3: cache 不缓存 branch 等易变元数据（降级方案，不采用）

Codex 建议过"只缓存 group membership，branch 每次 lazy 刷新"。不采用——因为 branch 读取发生在 grouper 内部的 `resolve_all` 调用里，拆分 membership vs metadata 需要大幅重构 grouper 接口。TTL 兜底足够。

## Implementation

### 文件改动

1. **`crates/cdt-discover/src/worktree_grouper.rs`**
   - `group_by_repository` 方法：在 `join_all` 闭包内加 `Semaphore::new(8)` acquire
   - 常量 `GROUPER_CONCURRENCY_LIMIT: usize = 8`

2. **`crates/cdt-api/src/ipc/local.rs`**
   - 新增字段 `groups_cache: Mutex<Option<GroupsCacheEntry>>`
   - 新增 struct `GroupsCacheEntry { groups, root_gen, ctx_gen, scan_inv_gen, created_at }`
   - `list_repository_groups_inner` 开头检查 cache 命中
   - cache invalidation：在 `invalidate_local_caches` / context switch 路径清 groups_cache

3. **`crates/cdt-api/src/ipc/project_scan_cache.rs`**
   - 暴露 `invalidation_generation()` getter（已有 `generation()` 但不确定是否 public）

### 不变量

- Semaphore permit 在 async future 内 acquire，blocking 工作在 `spawn_blocking` 内不持有 permit → 无死锁风险
- groups cache 是 best-effort：miss 时 fallback 到完整 grouper 路径，不影响正确性
- TTL 10s 内 git branch 变更不可见——可接受的 trade-off，sidebar 刷新 UI 本身已有数秒延迟

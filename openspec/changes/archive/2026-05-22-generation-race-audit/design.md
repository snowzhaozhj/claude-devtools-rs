## Context

`LocalDataApi` 的 context 切换路径（`switch_context` / `ssh_connect` / `ssh_disconnect` / `reconfigure_claude_root` / `shutdown_ssh_all`）依赖 `context_generation: AtomicU64` 作"in-flight scan 失效信号"——所有 5 处 callsite **bump-first**：在 ssh_mgr / projects_dir mutate **之前** `fetch_add(1)`，让任何 in-flight `list_sessions_skeleton` / `build_group_session_page` 在 spawn 时记录的 `expected_context_generation` 立刻失效（broadcast 前 check `current != expected` → silent drop）。`ssh_watcher_ops: Mutex<()>` 锁序列化 5 处 callsite 自身的 cancel-then-mutate 操作。

这套模式在以下 callsite 已经稳定运行：
- `list_sessions_skeleton`（`local.rs:1728`）：先 `expected_context_generation.load()`，再 `active_fs_and_context_strict().await` 拿 fs/ctx；spawn 的 scan task 复用早 load 的 expected。
- `scan_metadata_for_page`（`local.rs:2089` / `2106`）：每次 broadcast 前双 check `context_generation` 不变。
- `list_repository_groups`（`local.rs:3591`）：scan + grouper 前后做 pre/post snapshot；mismatch 时 skip refresh_worktree_meta_cache。

**残留 race（PR #198 codex 第三轮 verify）**：

### Race 1：`list_repository_groups::refresh_worktree_meta_cache` 的 generation pre/post 不充分

`switch_context` (3309) 流程：
```
let _ops = ssh_watcher_ops.lock().await;
context_generation.fetch_add(1);                 // ① PRE-BUMP
abort_scans_for_ssh_context_id(prev).await;
ssh_mgr.switch_context(target).await;            // ② STATE CHANGE（可能 await 数百 ms 网络 RTT）
attach_remote_watcher(next).await;
```

`list_repository_groups`（3591-3622）当前实现：
```
let pre  = context_generation.load();             // ③ 可能落在 ① 之后 ② 之前
let (fs, projects_dir, ctx, _, resolvers) = self.active_fs_and_policy().await?;  // ④ ssh_mgr.active_context_id() 返回 OLD（② 未完）→ ctx = OLD
let projects = scan_projects_cached_with(&fs, &projects_dir, &ctx).await?;
let groups = grouper.group_by_repository(...).await;
let post = context_generation.load();             // ⑤ 落在 ② 完成前 → 仍 = pre
if pre != post { skip refresh; return groups; }
self.refresh_worktree_meta_cache(&groups);        // ⑥ BUG：用 OLD ctx 数据 clear-and-rebuild 全局 cache
```

`refresh_worktree_meta_cache` 是 `worktree_meta_cache.write().clear()` 后插入 group 内每个 worktree 的 meta（line 513-530）——**flat key 是 `worktree_id` (= `project_id`)，无 ctx namespace**。OLD ctx 数据写入后会被 NEW ctx 的 `apply_worktree_meta`（line 537-547）误读：worktree_id 巧合重合时返错乱 meta，不重合时返 None fallback。

**修法候选与决策**（详 D1）：
- (a) 改 `switch_context` 顺序为"先 await ssh_mgr 后 bump"——破坏 bump-first 对 in-flight scan 的早 fail 契约（line 3303-3308 的设计动机）；需对 `list_sessions_skeleton` / `build_group_session_page` 等 5+ 个 spawn 路径加替代 invalidation 机制；audit + 测试成本高。
- (b) `active_fs_and_policy` 同时返 `(ctx, generation)` caller 双重比较——atomic snapshot 表面看似解决，但 bump-first 让 generation 已 = N+1 而 fs/ctx 仍 OLD：`(ctx=OLD, gen=N+1)` 是 self-consistent 的"快照"，pre/post 都 N+1，仍误判过校验。**根本不修 race**。
- (c) **bump-twice**（pre + post）：`switch_context` 在 ssh_mgr.switch_context 完成后再 bump 一次。pre/post 校验在大多数情况能识别。但 caller's pre/post window 若整段落在 ① 之后、② 完成前（switch_context 内 ssh_mgr.switch_context 网络 RTT 可能 100+ ms，scan + grouper 在 cache hit 路径 60-100ms，**完全可能内嵌**），pre = post = N+1，仍漏掉。
- (d) **本 change 选**：refresh 前显式 ctx-equality 校验 + 单一 captured snapshot。`list_repository_groups` 拆 inner（同时返 `ctx`）；wrapper 在调 `refresh_worktree_meta_cache` **之前** 短暂 `ssh_watcher_ops.lock()` + `ssh_mgr.active_context_id().await` 校验 captured ctx 仍是当前——若不匹配 skip refresh。锁内 ssh_mgr.active_context_id() 与 switch_context 的 mutate 互斥（switch_context 也持同锁）→ 当 caller 拿到 ssh_watcher_ops 锁时 ssh_mgr 状态稳定。这是结构性 fix（不依赖 timing），且锁持有时间极短（一次 active_context_id 调用），不影响吞吐。

### Race 2：`build_group_session_page` 跨两次 active 抽样的 (groups, fs, ctx) 拼接

`build_group_session_page`（574-584）当前：
```
let expected_context_generation = self.context_generation.load();
let expected_root_generation = self.root_generation.load();
let groups = self.list_repository_groups().await?;          // ⓐ 内含 active_fs_and_policy().await 拿 fs/ctx；可能 safe-degrade 返 OLD groups
let group = groups.into_iter().find(|g| g.id == group_id)...;
let cursor_state = parse_group_cursor(cursor);
let (fs, projects_dir, ctx) = self.active_fs_and_context_strict().await?;  // ⓑ 第二次独立抽样；可能拿 NEW
```

scenario：
1. ⓐ 期间 ssh_switch 未发生 → groups = OLD。ⓐ 完成。
2. ⓑ 之前 ssh_switch 完成 → ctx = NEW，fs = NEW。
3. 后续 `scanner.list_sessions(wt_id, ...)` 用 OLD groups 内的 worktree_id 调 NEW fs scanner → 该 wt_id 在 NEW fs 不存在 → 返空 sessions list → 整页 sessions 空。

**修法**：让 (groups, fs, projects_dir, ctx) 来自单一同源 snapshot。抽 `list_repository_groups_inner` 返回四元组，`build_group_session_page` 直接用 inner，不再独立 await `active_fs_and_context_strict`。

## Goals / Non-Goals

**Goals**：
- 关闭 Race 1 sub-window：`refresh_worktree_meta_cache` 不被并发 ssh switch 期间的旧 ctx 数据污染
- 关闭 Race 2 sub-window：`build_group_session_page` 用 (groups, fs, ctx) 同源快照 scan worktrees
- 保留 bump-first 对 in-flight scan task 的早 fail 契约（不改 5 处 bump 顺序）
- 不引入新的 `active_fs_and_*` API 变体（重用现有 `active_fs_and_policy` / `active_fs_and_context_strict`）
- 加 invariant 测试覆盖两个 race 路径
- spec 把 race-free 行为契约写明（worktree_meta_cache 刷新 SHALL 用 ssh_watcher_ops + ctx-equality 二次校验；build_group_session_page SHALL 用 single-snapshot）

**Non-Goals**：
- **不**改 `switch_context` / `ssh_connect` / `ssh_disconnect` / `reconfigure_claude_root` / `shutdown_ssh_all` 的 bump 顺序（保留 bump-first 契约）
- **不**让 `worktree_meta_cache` 改 per-context namespace（更大改动；当前 fix 用锁 + ctx-equality 已结构性闭合）
- **不**让 `active_fs_and_*` 返 generation（D1 (b) 不修根因）
- **不**改 IPC 字段 / 前端契约
- **不**改 `metadata_cache` / `parsed_message_cache`（这两个已是 `(ContextId, PathBuf)` key namespace 隔离，无 race）
- **不**audit 其它 spawn 路径（`list_sessions_skeleton` 等已用 expected_context_generation late-load 模式正确，本 change 不动）

## Decisions

### D1: Race 1 fix —— inner 抽样四元组 + wrapper 锁内 (ctx + generation) 双重校验

**问题**：`switch_context` bump-first 让 `context_generation` 在 `ssh_mgr.switch_context` 网络 RTT 期间已经领先于实际 ssh_mgr 状态。`list_repository_groups` 的 pre/post snapshot 校验只能识别"window 完全跨过 bump"的 caller，不能识别"window 完全嵌入 bump 之后到 ssh_mgr.switch_context 完成之前"的 caller。

**决策**：选 D1-(d-revised) —— `list_repository_groups` 拆 inner 返五元组（含 captured_generation），wrapper 在 refresh 之前显式校验 (captured ctx == current ctx) **AND** (captured generation == current generation) 双重一致：

```rust
async fn list_repository_groups_inner(
    &self,
) -> Result<
    (Vec<cdt_core::RepositoryGroup>, Arc<dyn FileSystemProvider>, PathBuf, cdt_fs::ContextId, /* captured_generation */ u64),
    ApiError,
> {
    let pre_root = self.root_generation.load(Ordering::SeqCst);
    let pre_ctx_gen = self.context_generation.load(Ordering::SeqCst);
    let (fs, projects_dir, ctx, _policy, resolvers) = self.active_fs_and_policy().await?;
    // captured_generation 在 active_fs_and_policy 完成**之后**立即 load——与 (fs, ctx) 同 snapshot；
    // inner 后续不再 bump，wrapper 锁内 current_generation 与之比较即可识别 inner 期间 / inner 完成
    // 到 wrapper 拿锁之间任何 bump（含同 host 快速 disconnect+reconnect 引发的双 bump）。
    let captured_generation = self.context_generation.load(Ordering::SeqCst);
    let projects = self.scan_projects_cached_with(&fs, &projects_dir, &ctx).await?;
    let grouper = cdt_discover::WorktreeGrouper::new_dyn(resolvers.git_identity_resolver.clone());
    let groups = grouper.group_by_repository((*projects).clone()).await;
    // inner 内保留 root_generation / context_generation pre/post 校验作 fast-path——
    // mismatch 时 wrapper 自然也会 mismatch；保留 inner 内提早返回避免 wrapper 路径多走一次锁。
    let post_root = self.root_generation.load(Ordering::SeqCst);
    let post_ctx_gen = self.context_generation.load(Ordering::SeqCst);
    if pre_root != post_root || pre_ctx_gen != post_ctx_gen {
        // 这里 captured_generation 仍是 inner 内 active_fs_and_policy 之后的值，wrapper 校验会 fail
        // 因为 current_generation = post_ctx_gen ≠ captured_generation —— 与本快路径行为一致。
        return Ok((groups, fs, projects_dir, ctx, captured_generation));
    }
    Ok((groups, fs, projects_dir, ctx, captured_generation))
}

async fn list_repository_groups(&self) -> Result<Vec<cdt_core::RepositoryGroup>, ApiError> {
    let (groups, _fs, _projects_dir, captured_ctx, captured_generation) =
        self.list_repository_groups_inner().await?;
    // 锁内校验：ssh_mgr 当前 active 重新构造的 ContextId 与 captured 全等 + captured_generation
    // 与 current_generation 全等。任一 mismatch 就 skip refresh（safe degrade）。
    {
        let _ops = self.ssh_watcher_ops.lock().await;
        let current_ctx = self.current_active_context_id_under_lock().await;
        let current_generation = self.context_generation.load(Ordering::SeqCst);
        if current_ctx != captured_ctx || current_generation != captured_generation {
            tracing::debug!(
                target: "cdt_api::perf",
                captured = ?captured_ctx,
                current = ?current_ctx,
                captured_gen = captured_generation,
                current_gen = current_generation,
                "list_repository_groups: state changed mid-scan (ctx or generation), skip refresh_worktree_meta_cache"
            );
            return Ok(groups);
        }
        self.refresh_worktree_meta_cache(&groups);
    }
    Ok(groups)
}

// helper：在 ssh_watcher_ops 锁保护下读 ssh_mgr.active 状态并构造与 active_fs_and_policy 同形的 ContextId。
// SHALL 仅在已持 ssh_watcher_ops 锁的 callsite 调用，函数内部 self.projects_dir.lock().await 是普通 mutex；
// switch_context / ssh_connect / ssh_disconnect / shutdown_ssh_all 都持 ssh_watcher_ops 不能并发跑。
async fn current_active_context_id_under_lock(&self) -> cdt_fs::ContextId {
    if let Some(id) = self.ssh_mgr.active_context_id().await {
        if let Some((_provider, ctx)) = self.ssh_mgr.provider_and_context_id(&id).await {
            return ctx;
        }
        // active=Some 但 provider lookup miss（ssh_disconnect 中间态）→ fall through Local
    }
    let projects_dir = self.projects_dir.lock().await.clone();
    cdt_fs::ContextId::local(projects_dir)
}
```

**关键不变量**：
1. `ssh_watcher_ops` 锁内 ssh_mgr.active_context_id() / `provider_and_context_id()` 与 `switch_context` / `ssh_connect` / `ssh_disconnect` / `shutdown_ssh_all` 的 mutate 互斥（5 处都持同锁）。锁内读到的 ssh_mgr 状态 = 当前真实状态。
2. `captured_ctx` 是 `active_fs_and_policy` 在 inner 内部一次原子 `provider_and_context_id` 调用拿到的——与当时 ssh_mgr 状态一致；`captured_generation` 在 active_fs_and_policy 完成之后立刻 load，inner 后续不再修改 generation。
3. **(ctx + generation) 双重校验**关键性：单 ctx-equality 不能识别"同 host disconnect+reconnect 间隔同 ContextId 但 generation bumped 两次"场景；单 generation-equality 不能识别"reconfigure_claude_root 改 projects_dir 同时 bump root + context generation"场景。两者合一：
   - 同 host 快速 disconnect("a") → connect("a") 期间：每次 disconnect/connect 都 `context_generation.fetch_add(1)` → 共增 2；captured_generation 在 inner 第一步抽样时为 N，wrapper 锁内 current_generation ≥ N+2 → mismatch → skip refresh ✓
   - reconfigure_claude_root 改 Local projects_dir：`ContextId::Local { projects_dir }` 包含 projects_dir 字段 → `captured == ContextId::Local(old_dir)` 与 `current == ContextId::Local(new_dir)` 不等 → mismatch → skip refresh ✓
   - 普通 ssh_a → Local 切：context_generation bump（ssh_disconnect 或 switch_context 任一） → mismatch → skip refresh ✓
4. mismatch 时 caller 仍拿到 groups（safe degrade）；只是 worktree_meta_cache 不刷新——下次 IPC 自然重做。
5. inner 内保留的 pre/post 校验是 fast-path（如果 inner 期间已 mismatch 则 inner 直接返带 captured_generation 的旧 groups；wrapper 仍会做锁内最终校验确认）；实施时 inner 不需要改返路径，因为 wrapper 校验是权威——保留 pre/post 仅作 tracing 记录。

**ctx 表达**：使用 `cdt_fs::ContextId` 直接 `==` 比较（`ContextId::Local { projects_dir }` 与 `ContextId::Ssh { host, remote_home }` 派生 `Eq`）；reconfigure 改 projects_dir 在 Local 比较时也算上 → 同一表达式覆盖 ssh switch + reconfigure 两类 race。

**为什么不直接 `_ops` 锁住整段 inner**：inner 内含 scan + grouper（cache miss 时数百 ms），锁住会让 switch_context 等同时间——把瞬时锁变成"ScanLock"，破坏 switch 响应性。锁仅在 refresh 之前短暂获取（active_context_id + provider_and_context_id 调用 + projects_dir.lock + cache write 耗时 < 1ms），不影响吞吐。

**对 reconfigure_claude_root / shutdown_ssh_all 的覆盖**：reconfigure 改 Local projects_dir 时 `ContextId::Local` 字段变 → ctx-equality 判 mismatch；shutdown 把 ssh_mgr 全清后 active = None + context_generation bump → ctx + generation 双 mismatch。两条路径都被本 D1 双重校验覆盖。

**codex commit-stage Bug 1 + Bug 2 修订（2026-05-22）**：仅有"ContextId 比较"还不够，需要 reconfigure / shutdown 自身也持 `ssh_watcher_ops` 锁与 refresh 路径互斥。原实现：(a) `reconfigure_claude_root` 完全不持锁，仅依赖 `_scanner.lock + _projects_dir.lock` 间接序列化——但这两个 mutex 与 refresh 路径用的 `ssh_watcher_ops` **不是同一个**。reconfigure 已 bump generation 但未写新 projects_dir 的 window 内，refresh wrapper 的 `current_active_context_id_under_lock` 内部 `projects_dir.lock().await` 拿到的还是旧 projects_dir → 与 captured 等同 → 通过 → 旧 groups 写入 cache 污染随后切到新 projects_dir 的查询。修法：`reconfigure_claude_root` 函数开头 `let _ops = self.ssh_watcher_ops.lock().await;` 持锁覆盖整段 mutate（bump → abort → swap scanner / projects_dir → respawn watcher）。(b) `shutdown_ssh_all` 用 `try_lock()`，失败即绕过锁直接调 `ssh_mgr.shutdown_all` —— 同破坏前提。改 `lock().await` 排队等同锁持有者；shutdown 自身已是 app exit 路径，等几毫秒可接受。两个 fix 把"5 处 mutate 入口与 refresh 路径同锁互斥"从 spec 文档约束变成代码事实。

**取消原 follow-up #5.1**——reconfigure_claude_root 已加锁后 ContextId.Local 字段比较 + (ctx + generation) 双重校验完整闭合，不再列入 followups。

### D2: Race 2 fix —— `build_group_session_page` 用 inner 单一 snapshot + spawn 前锁内二次校验

**修改**：
```rust
async fn build_group_session_page(
    &self,
    group_id: &str,
    page_size: usize,
    cursor: Option<&str>,
) -> Result<GroupSessionPage, ApiError> {
    // 单一 snapshot：groups / fs / projects_dir / ctx / captured_generation 同源。
    // 删掉旧实现 line 584 的第二次 active_fs_and_context_strict().await。
    let (groups, fs, projects_dir, ctx, captured_generation) =
        self.list_repository_groups_inner().await?;
    // root_generation 仍单独抽样——本函数不直接监控 reconfigure，但下游 spawn task 沿用既有
    // expected_root_generation/expected_context_generation 双轴校验语义（与 list_sessions_skeleton
    // 同形）；后台 broadcast 时 root_generation mismatch 也会 silent drop。
    let expected_root_generation = self.root_generation.load(Ordering::SeqCst);
    let expected_context_generation = captured_generation;

    let group = groups
        .into_iter()
        .find(|g| g.id == group_id)
        .ok_or_else(|| ApiError::not_found(format!("repository group {group_id}")))?;
    // ... cursor + scanner.list_sessions 并发跑（scan 用 inner 内的 fs，wt_id 同源 OK）...

    // 用户可见 page 内容（k-way merge + apply_worktree_meta）已组装完毕；下面 spawn 后台
    // metadata scan task 之前 SHALL 持 ssh_watcher_ops 锁 + 二次校验。
    {
        let _ops = self.ssh_watcher_ops.lock().await;
        let current_ctx = self.current_active_context_id_under_lock().await;
        let current_generation = self.context_generation.load(Ordering::SeqCst);
        if current_ctx != ctx || current_generation != captured_generation {
            tracing::debug!(
                target: "cdt_api::perf",
                captured = ?ctx,
                current = ?current_ctx,
                captured_gen = captured_generation,
                current_gen = current_generation,
                "build_group_session_page: state changed mid-page-build, skip metadata scan task spawn"
            );
            // 返页面骨架（用户已渲染），不 spawn 后台 metadata scan——避免向新 ctx UI broadcast 旧 ctx update。
            return Ok(GroupSessionPage { sessions: page_sessions, next_cursor });
        }
        // match：在锁内 spawn 所有 page_jobs 的 scan_metadata_for_page task；spawn 期间 switch
        // 不可能跑（持同锁）。spawn 完释放锁。task 自身仍 carry expected_*_generation，broadcast
        // 前再校验 generation 不变 → 锁释放后 switch 进来 bump 仍能 silent drop。
        for (wt_id, (dir, jobs)) in page_jobs_by_wt {
            // existing spawn loop with expected_root_generation / expected_context_generation
        }
    }

    Ok(GroupSessionPage { sessions: page_sessions, next_cursor })
}
```

**关键不变量**：
- `(groups, fs, projects_dir, ctx, captured_generation)` 来自 `active_fs_and_policy` 一次原子调用；inner 内 scan + grouper 在同一 fs/ctx 上跑 → groups 内的 worktree_id 对应同 fs/ctx 的 projects → scanner.list_sessions 用 wt_id 在 fs 上查得到。
- inner 仍可能跨过 ssh switch（ssh_switch 不持 inner 内锁）：例如 inner 在 active_fs_and_policy 之后 + grouper 之前被 switch 跨过——此时 (groups, fs, ctx) 全 OLD 仍 self-consistent，**page 骨架内容正确**。
- spawn 前锁内 (ctx + generation) 二次校验：识别 inner 完成 → 锁拿到之间任何 mutate（包括 switch / connect / disconnect / reconfigure / 同 host 来回切）。mismatch 时 SHALL NOT spawn 后台 metadata scan——因为 spawn 后 task 持的 expected_context_generation = captured_generation 已与 current 不一致，但锁释放后到 broadcast 之间 generation 可能再 bump 让 task 自身的 mismatch 校验可能因为巧合 generation 又回到 captured 而误判（理论低概率，但本 spawn-前校验直接消除该 case）。返页面骨架 + 不 spawn = 用户看到正确 page 内容，没有后台 broadcast 串扰新 ctx UI。
- match 时在锁内 spawn task：spawn 期间 switch 不能跑（持同锁），所有 task 一致拿到 captured_generation == 当前 generation。锁释放后 switch 进来 bump，task 现有的 broadcast-time 校验 (`current != expected`) 仍能 silent drop。
- 移除 line 584 的第二次抽样消除 (groups OLD, fs NEW) 的混合态。

**对其它 list_repository_groups callsite 的影响**：grep `list_repository_groups` 的用法显示**只**有 IPC handler 自身（line 3593 公开方法）和 `build_group_session_page` (line 574)。前者保留为 wrapper（外部 IPC 入口），后者直接用 inner。

**锁持有期与开销**：spawn 前锁段含 `current_active_context_id_under_lock` 调用 + 比较 + spawn 循环（per-worktree 一次 `active_scans.lock + remove + insert + tokio::spawn`）。spawn 本身是同步操作（不 await），整段 < 5ms（5-10 个 worktree 量级）。switch_context 被排队的最坏延迟 = 该 spawn 段时长，可接受。

### D3: invariant 测试设计

**目标**：覆盖两个 race 真实触发路径的"修复后断言"，**不依赖伪造 captured 参数绕过结构性 race**（codex 二审 Bug 1 的反思：mock counter 必须验证 prod 路径真触发 if 分支，不是孤立单测 if 内逻辑）。

**Test 1 (Race 1 真触发)**: `list_repository_groups_skips_refresh_on_concurrent_ssh_switch`
- 设置：`tokio::test(start_paused = false)` + 真 `LocalDataApi`，注入 `FakeSshManager` 暴露可控 delay 的 `switch_context` 钩子（`switch_delay_tx` 在 ssh_mgr 内部 mutate 之前 await 一个 oneshot；test 持 sender）
- 流程：
  1. 注入 ctx-a、ctx-b 两个 fake context；先 ssh_connect 到 ctx-a 让 active = ctx-a。`worktree_meta_cache` 填 ctx-a 的 mapping
  2. spawn task A：`api.switch_context("local")`——在 fake ssh_mgr.switch_context 内部 await delay sender 卡住
  3. 主测：`api.list_repository_groups().await` —— 此时 ssh_mgr.active_context_id() 仍返 ctx-a（task A 卡在 ssh_mgr.switch_context 内 await），inner 拿 captured_ctx = ctx-a，captured_generation = N+1（task A 已 bump-first）
  4. 主测之前给 task A 释放 delay sender 让它完成 switch；同时主测 inner 内 scan + grouper 完成
  5. 主测调 `list_repository_groups` 内的 wrapper 路径拿 ssh_watcher_ops 锁——因 task A 持锁，主测被排队；task A 释放锁前已 ssh_mgr 完成切到 Local + post-switch 的 attach 完成 + 锁释放
  6. 主测拿到锁：current_ctx = Local，captured_ctx = ctx-a → mismatch；same time captured_generation < current_generation（task A 完成后 generation 已 ≥ N+2 因为 ssh_disconnect 隐含 bump 或 switch_context 自身 bump）→ skip refresh
- 断言：`worktree_meta_cache` 内容 = task A 切走前 ctx-a 的旧 mapping（被 skip 不清空保留旧值）；exposed counter `refresh_worktree_meta_cache_call_count` 与 setup 后初值相比 +0

实施细节：
- 加 `cfg(test)` feature 给 `LocalDataApi` 暴露 `pub(crate) async fn list_repository_groups_for_test()` 拿 inner + wrapper 路径的细粒度 hook
- 加 `cfg(test)` 计数器 `refresh_worktree_meta_cache_call_count: AtomicUsize`，prod 路径在真正写入 cache 时 +1
- Fake SshSessionManager trait：本仓 `crates/cdt-ssh/` 已有 `SshSessionManager` trait + 测试用 fake；本测试在 fake 上加 `switch_delay_rx: Mutex<Option<oneshot::Receiver<()>>>` 字段

**Test 2 (Race 1 同 host disconnect+reconnect)**: `same_host_reconnect_during_inner_skips_refresh`
- 设置：注入 ctx-a；ssh_connect 到 ctx-a；调一次 list_repository_groups 让 cache 填满
- 流程：
  1. spawn task A：`ssh_disconnect("ctx-a")`，fake ssh_mgr.disconnect 内部 await delay sender 卡住
  2. 主测：`list_repository_groups()` —— inner 拿 captured_ctx = ctx-a (task A 还没 disconnect 完)，captured_generation = N+1
  3. 给 task A 释放 → ssh_disconnect 完成（context_generation.fetch_add → N+2 已在 bump-first 阶段；这里实际 N+1 了）。立刻 spawn task B：`ssh_connect("ctx-a")` 同 host 重连，fake mgr 也用 delay sender 卡住
  4. 释放 task B → ssh_connect 完成（generation → N+3 通过 ssh_connect 的 bump-first），ssh_mgr.active 重回 ctx-a
  5. 主测的 inner 内 scan 完成，wrapper 拿锁
- 断言：current_ctx == captured_ctx == ctx-a（同 host 重连后 ContextId 相等）但 current_generation == N+3 ≠ captured_generation == N+1 → mismatch → skip refresh；counter +0
- 这是 codex 报的 Bug 1 真实场景的回归屏障

**Test 3 (Race 2 真触发)**: `build_group_session_page_skips_metadata_spawn_on_concurrent_switch`
- 设置：与 Test 1 类似 fake ssh_mgr + delay sender。先填好 ctx-a 的一个 group + 几个 worktrees fixture
- 流程：
  1. spawn task A：switch_context("local") 卡在 ssh_mgr.switch_context delay 处（已 bump-first）
  2. 主测：`build_group_session_page("g1", 50, None)` —— inner 拿 (groups for ctx-a, fs ctx-a, captured_generation = N+1)，page 骨架组装完成
  3. 释放 task A → switch 完成，ctx 切到 Local，generation → N+2 (post-bump 假设我们 follow design 加 post-bump，但本 change 不加 post-bump；所以 generation 仍是 N+1) **重要：本 change 不引入 post-bump，所以 captured = current = N+1**——但 current_ctx (Local) ≠ captured_ctx (ctx-a) → ctx mismatch → skip spawn
  4. 主测 wrapper 拿锁做二次校验
- 断言：返回 GroupSessionPage 含 page 骨架 sessions（不为空，因为 inner 已用 ctx-a 的 fs scan 完）；exposed counter `metadata_spawn_count` = 0；session_metadata SSE 不发任何 update（broadcast tx send 计数 = 0）

**Test 4 (Race 2 单 snapshot 验证)**: `build_group_session_page_calls_active_fs_only_once`
- 设置：`LocalDataApi` 加 `cfg(test)` 计数器 `active_fs_and_policy_call_count`
- 流程：调一次 `build_group_session_page("g1", 50, None)`
- 断言：counter == 1（只 inner 内调，不再 line 584 第二次抽样）

**Test 5 (regression 双重保险)**: `concurrent_list_groups_and_switch_context_no_panic_no_pollution`
- 设置：tokio test，spawn 两 task：A 反复 100 次 `list_repository_groups`；B 反复 100 次 `switch_context("ssh-a")` ↔ `switch_context("local")` 来回切
- 断言：两 task 都不 panic；最终 worktree_meta_cache 内容 = 最后一次 switch 完成后 ctx 的 mapping（确认无混合脏数据）
- 实施风险：真并发难精确触发 sub-window 但保证 panic-free + 最终一致；结构性闭合证据靠 Test 1/2/3。

## Risks / Trade-offs

- **锁延迟引入新的 contention**：`list_repository_groups` 每次 refresh 时短暂持 `ssh_watcher_ops`。switch_context 也持同锁。最坏并发 `list_repository_groups` × N 与 `switch_context` × M 时 switch 路径需排队 N 个 list 完成。锁持有时间是"`ssh_mgr.active_context_id().await + cache.write().clear() + insert`"——active_context_id 对 SshSessionManager 是 RwLock read（瞬时），cache 写是 std RwLock write（瞬时）。整体 < 1ms，不影响用户感知。
- **D1-(d) 不解决 reconfigure_claude_root 引发的次级 race**：见 D1 末段。本 change scope 限定 ssh ctx switch 路径；reconfigure 触发频率极低（用户主动改 claude_root 设置）+ root_generation 的 pre/post 仍可作兜底。如果 follow-up 需要彻底闭合，把 inner 内增加 `pre_root` 与 captured snapshot 后的 `post_root` 校验对齐 ctx 校验（跟 D1-(d) 同形）。
- **不改 switch_context bump 顺序的 trade-off**：保留 bump-first 对 in-flight scan 的早 fail 契约。否则需要给 `list_sessions_skeleton::active_scans.insert` 路径加 `ssh_watcher_ops` 锁——这是 hot path（用户每次切 group / 切 worktree 都触发），会让 list 本身被 switch 阻塞。当前选择把"refresh 路径"短锁，"insert 路径"无锁（已有 generation 双 check 保护）。

## Migration Plan

无 migration 需要。本 change 是内部实现修复 + spec 文档化。新加 invariant 测试随 change 落地；现有测试不需改。

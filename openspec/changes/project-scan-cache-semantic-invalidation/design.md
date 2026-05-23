## Context

`ProjectScanCache`（`crates/cdt-api/src/ipc/project_scan_cache.rs`）由 PR #198 引入，目标是让 `list_repository_groups` / `list_projects` 跨 IPC 复用 `ProjectScanner::scan` 结果（一次扫描产出 `Arc<Vec<Project>>`）。当时为了简化实现，在 `LocalDataApi::spawn_watcher_runtime`（`crates/cdt-api/src/ipc/local.rs:2271-2282`）把 invalidator 写成「订阅 `FileWatcher::subscribe_files()` 任意事件 → 调 `invalidate_local()` 清光所有 Local entry」，注释承认 "partial invalidation 不划算"。

实测（2026-05-23 用户机器，30 project × 538 session corpus，活跃 claude-code 会话持续追加写）：

- `ps` lifetime CPU 65.7% / 活动监视器瞬时 52.3% / 7,713 csw/s / 211 idle blocking-pool worker
- 60s `sample` 显示 hot path：`File::open` 1268 命中 / `read_dir` 1232 / `canonicalize` 166 / `extract_session_cwd` 88+ / `LocalGitIdentityResolver::resolve_all` 62+
- `top` 瞬时 CPU 在 0%-75% 间 burst 跳动，与活跃 session JSONL 写入强相关

codex 二审（三轮）确认主诊断链。第二轮指出 `cdt-core::FileChangeEvent` 仅含 `project_id` / `session_id` / `deleted` / `project_list_changed` 4 个字段，**无原始 path**。第三轮（B 路线）发现 watcher `mark_project_seen` 在 `FileWatcher::new` 时**预填**当前已存在的 project 目录到 `known_projects` HashSet（`crates/cdt-watch/src/watcher.rs:30-41,79`），所以「已知 project 下新增 session」时该 set 早已含 pid，**`mark_project_seen` 不返回 true**——watcher 输出 `plc=false, deleted=false`，与"已知 session JSONL 追加"事件**外观完全相同**。

watcher path 形态压缩到事件字段的实际语义表（B 路线 invalidator 必须基于此判定，不能假设 plc 包含"新 session 首次出现"）：

| watcher 看到的真实 path 形态 | 输出 FileChangeEvent |
|---|---|
| `<projects_dir>/<pid>` 顶层 dir-create（pid 是新出现的目录） | `pid, sid="", deleted=false, plc=true` |
| `<projects_dir>/<pid>/<sid>.jsonl` 主 session 改（首次见 pid，启动后从未广播过此 pid） | `pid, sid, deleted, plc=true` |
| `<projects_dir>/<pid>/<sid>.jsonl` 主 session 改（已知 pid，含已知 session **追加** 与 已知 project **新 session 首次出现**） | `pid, sid, deleted=false, plc=false` |
| `<projects_dir>/<pid>/<sid>/subagents/agent-*.jsonl`（折叠到父） | `pid, sid=父, deleted=false, plc=false` |
| `<projects_dir>/<pid>/<sid>.jsonl` 主 session 删除 | `pid, sid=自, deleted=true, plc=false（典型情况）` |
| `<projects_dir>/<pid>/<sid>/subagents/agent-*.jsonl` subagent 删除（折叠到父） | `pid, sid=父, deleted=true, plc=false` |
| `~/.claude/todos/...` | 不进入 `file_tx` channel（走独立 `todo_tx`） |

**核心洞察修订**：仅 `project_list_changed` + `deleted` 两个 bool **不足以**区分「已知 session 追加」vs「已知 project 下新 session 首次出现」——必须额外查询 cache snapshot 是否含此 `(project_id, session_id)`。这是本 change 与第二轮 design 的差异（codex 第三轮 BLOCK 1）。

相邻缓存：
- `MetadataCache`（session metadata）—— 不在本 change 改动范围，命中率 97.5%
- `ParsedMessageCache`（JSONL 解析结果）—— 不在本 change 改动范围
- 三个 cache 共享 `FileWatcher::subscribe_files()` 广播但走各自独立 invalidator closure

## Goals / Non-Goals

**Goals:**

- 「已知 session JSONL 追加」事件（`plc=false, deleted=false, sid 在 cache snapshot 内`）SHALL NOT 触发 `ProjectScanCache` 失效
- 「project / session 拓扑变化」事件（`plc=true` OR `deleted=true` OR `sid` 非空且不在 cache snapshot 内）SHALL 触发 `invalidate_local()`
- 失效粒度判定**仅**在 `LocalDataApi::spawn_watcher_runtime` 内现有的 project-scan invalidator closure 内进行；`cdt-core::FileChangeEvent` 类型 / `cdt-watch::FileWatcher::route_event` / `parse_project_event` 不动
- 长时间使用桌面应用稳态 CPU 显著下降（设计目标：idle < 2%；含活跃 session 写入的稳态 < 10%）
- 加测试覆盖：JSONL append 命中 cache、新 session 首次出现失效、删 session 失效、新 project 目录失效、subagent 折叠后不失效、broadcast lag 保守失效、telemetry counter 分布、`extract_session_cwd` 仅读首行的不变量

**Non-Goals:**

- 不扩 `cdt-core::FileChangeEvent` 字段（codex 第二轮已确认 path-vs-ids 三元组不一致是新 confusing 设计；当前 4 字段对本 change 充分）
- 不动 `cdt-watch::FileWatcher::parse_project_event` 内 path → 字段的解析逻辑
- 不动前端 `Sidebar.svelte::file-change` handler 路径或 250ms throttle 窗口（后续独立 PR）
- 不动 `MetadataCache` / `ParsedMessageCache` 失效逻辑或新增 throttle
- 不动 `ParsedMessageCache` invalidator 内现有的 `(project_id, session_id) → path` 推算代码（codex 指出此处推算重复存在但属于 cleanup 范畴，不在本 change scope）
- 不动双 `tokio::Runtime` 共存（`src-tauri/src/lib.rs:923`）或 `max_blocking_threads` 上限（后续独立 PR）
- 不动 SSH `ProjectScanCache` entry 失效逻辑（SSH 路径靠 TTL 自然过期，本 change 仅改 Local）
- 不引入 per-project 失效粒度（codex 指出 `ProjectScanCache.entries: HashMap<ContextId, Arc<Vec<Project>>>` 没有 per-project entry 概念；引入需重写数据结构，超本 change scope）

## Decisions

### D1：失效语义判定放 `LocalDataApi` invalidator closure，不动 `cdt-watch`

**选择**：在 `crates/cdt-api/src/ipc/local.rs::spawn_watcher_runtime` 现有 project-scan invalidator spawn task 内，对每条 `FileChangeEvent` 按 D2 三档判定（`project_list_changed` / `deleted` / `contains_session_id` 反查）决定调 `invalidate_local()`、还是直接放行。

**备选 A**（已驳回）：在 `cdt-core::FileChangeEvent` 加 `path: PathBuf` 字段，由消费者按 path 形态做语义判定。

驳回理由（codex 第二轮 + 用户决策对话）：
- watcher 当前对 subagent JSONL 折叠 `(pid, sid)` 到父 session，但**真实 fs path** 在 `<sid>/subagents/agent-*.jsonl`。加 path 字段后 `(pid, sid, path)` 三元组在 subagent 场景下指向不同实体（ids 是父 session 的逻辑视图、path 是 subagent 真实位置），是新引入的 confusing 设计。
- 本 change 范围内 path 信息**不必要**：watcher 已经把所有"会改 ProjectScanCache 应展示内容"的语义压缩到 `project_list_changed`/`deleted`。
- 跨 crate 字段扩展（cdt-core / cdt-watch / 所有序列化路径）blast radius 大。
- "未来 subagent 级失效"是 YAGNI，触发时再加字段。

**为何**：保持事件类型稳定 + 复用 watcher 已经做的语义压缩 + 单一信号源（消费者只看 ids 视图，无歧义）。

**备选 B**（已驳回）：在 `cdt-watch::FileWatcher::parse_project_event` 内增加新 broadcast channel 把"structural 事件"和"content append"分流。

驳回理由：codex 第一轮原句"不要在 `cdt-watch` 全局吞事件，否则 Sidebar 当前列表刷新仍需要 `file-change`"——`Sidebar.svelte:643-670` 仍需要看到所有 file 改动来 reload 当前选中 group 的 sessions。**`cdt-watch` 是 source 层中立的事件总线**；任何语义判定都属于消费者侧。

### D2：失效逻辑（三档判定，含 cache snapshot lookup）

伪代码：

```rust
match recv_result {
    Ok(event) => {
        let structural = {
            let mut cache = cache.lock().unwrap_or_else(|p| p.into_inner());
            // has_entry 守护：cache 空时不把"普通 append"误判为 unknown_session
            // → 防 lag 后续事件 storm（D2b 修订；codex PR 二审 WARN 1）
            let unknown_session = !event.session_id.is_empty()
                && cache.has_entry(&local_ctx)
                && !cache.contains_session_id(&local_ctx, &event.project_id, &event.session_id);
            let s = event.project_list_changed || event.deleted || unknown_session;
            if s { cache.invalidate_local(); }
            s
        };
        let counter_name = if structural {
            "project_scan_cache.invalidate.structural"
        } else {
            "project_scan_cache.invalidate.content_append_skipped"
        };
        counter(counter_name).inc();
    }
    Err(RecvError::Lagged(_)) => {
        cache.lock().unwrap_or_else(|p| p.into_inner()).invalidate_local();
        counter("project_scan_cache.invalidate.lag_conservative").inc();
    }
    Err(RecvError::Closed) => break,
}
```

### D2b：`has_entry` 守护（apply 阶段反转 / codex PR 二审 WARN 1 修订）

原 D2 的规则 2 在 lag 路径触发后会引发死锁：lag 调 `invalidate_local()` 清空 Local entry → 后续普通 append 事件 `contains_session_id` 一律返 false → unknown_session 命中 → 又调 `invalidate_local()` + bump `invalidation_generation` → 在重扫期间 `try_insert` 因 generation mismatch 一直丢弃 snapshot → cache 长期无法 repopulate（持续重扫风暴）。

**修订**：规则 2 加 `cache.has_entry(local_ctx)` 守护——cache 空时 unknown_session 不成立，走规则 3 等待业务路径下次 `list_repository_groups` 触发重扫填回。`ProjectScanCache` 同步加 `pub fn has_entry(&self, ctx: &ContextId) -> bool` API。spec delta 同步说明四档判定与 `has_entry` 守护契约（详 spec `ProjectScanCache 按事件语义分级失效` Requirement）。

**为何三档不可压缩到二档**：codex 第三轮 BLOCK 1 实证 `mark_project_seen` 在构造时预填 known_projects（`watcher.rs:30-41,79`）。已知 project 下新建 session 时 watcher 输出 `plc=false, deleted=false`——与"已知 session 追加"在事件字段上**外观一致**。仅靠 (plc, deleted) 两 bool 判定会让新 session 最长 `LOCAL_CACHE_TTL = 300s` 不可见，dealbreaker。第三档 cache lookup 是**最低代价**的精确化方案（无须改 watcher / 不引入 path 字段）。

**为何此判定正确（按事件语义详尽枚举）**：

| 实际 fs 事件 | watcher 输出 | cache 含 sid? | 第三档判定 | 我们的处理 | 正确性论证 |
|---|---|---|---|---|---|
| 新 project 目录创建（`<root>/<pid>` dir-create） | `plc=true, sid=""` | n/a | n/a | invalidate（plc 命中） | ✓ 拓扑变 |
| 启动后首次见某 pid 的事件（`mark_project_seen` race-cold） | `plc=true` | n/a | n/a | invalidate（plc 命中） | ✓ 即使 cache 已有该 sid 也无害（启动初 cache 多半为空） |
| 已知 project 已知 session 追加（普通 hot path） | `plc=false, deleted=false` | **是** | append | no-op | ✓ Project.sessions 列表不变；session.cwd 由 D4 不变量保证不漂移 |
| **已知 project 下新 session 首次出现** | `plc=false, deleted=false` | **否** | structural | invalidate | ✓ cache 漏此 sid → 重扫纳入；BLOCK 1 修复关键 |
| subagent JSONL 修改（watcher 折叠 sid=父） | `plc=false, deleted=false` | **是**（父 sid 在 cache） | append | no-op | ✓ subagent 修改不改顶层 list |
| subagent JSONL 删除（watcher 折叠 sid=父） | `plc=false, deleted=true` | n/a | n/a | invalidate（deleted 命中）— **false-positive** | ⚠ R6（事件无法区分 subagent vs 主 session 删除；接受多扫一次的成本，详 R6） |
| 主 session JSONL 删除 | `plc=false, deleted=true` | n/a | n/a | invalidate（deleted 命中） | ✓ Project.sessions 列表减少 |
| 空 sid 事件（如顶层 dir-create 已在 case 1） | `plc=*, sid=""` | n/a | append（第三档 `!sid.is_empty()` 守护） | 由 plc/deleted 主导 | ✓ 不会因为"sid 不在 snapshot"误判（sid 为空时跳过 lookup） |

**未列举的极端场景**：
- watcher fs 错误重启 / inotify subscription 漂移 → 由 `LOCAL_CACHE_TTL = 300s` 兜底（`project_scan_cache.rs:11`）

### D3：`ProjectScanCache::contains_session_id` 反向查询 API

`ProjectScanCache` 加：

```rust
/// 查询指定 ctx 的 entry snapshot 是否含 (project_id, session_id) 这一 session。
/// 用于 invalidator 第三档判定"已知 vs 未知 session"。
///
/// 复杂度：O(N project × N session_per_project)。corpus 30 project × 538 session
/// 单次 ~10µs（Vec<String> 直比），相比 fs scan 16k+ syscall 微不足道。
#[must_use]
pub fn contains_session_id(
    &self,
    ctx: &ContextId,
    project_id: &str,
    session_id: &str,
) -> bool {
    let entry = match self.entries.get(ctx) {
        Some(e) => e,
        None => return false,
    };
    entry.snapshot.iter()
        .find(|p| p.id == project_id)
        .map(|p| p.sessions.iter().any(|s| s == session_id))
        .unwrap_or(false)
}
```

**为何不维护反向索引**：每 ctx 单 lookup ~10µs，活跃场景下每秒 ≤ 几次 fsevents，CPU 成本 < 50µs/s = 0.005% CPU——远低于反向索引维护成本（写入路径加 HashMap insert + invalidate 时清理，且 codex 已警告"invalidate_path 重建 snapshot 超 scope"）。直接遍历足够。

**为何接受 cache miss 时返回 false**：cache 没有 entry → `contains_session_id` 返 false → 第三档判 structural → invalidate（no-op，因为 cache 本来就没东西）。无副作用。

### D4：cwd hidden risk 处理（`extract_session_cwd` 仅读首行的不变量）

codex 二审指出：`extract_session_cwd`（`crates/cdt-discover/src/project_scanner.rs:328-345,390-405`）实现是「读首 20 行 + 失败兜底 read_to_string 整文件」。**理论上**JSONL 后续追加（第 21+ 行）可能影响 cwd 抽取结果。

**实证检验**：claude-code 写 JSONL 的格式是每行一条 message，第一条总是 user message 含 `cwd` 字段。前 20 行内必然找到 cwd，**绝不会**走 `read_to_string` 兜底分支。

**测试断言**（用 `cdt-fs::with_fs_counter` 返回的 `FsOpCounts`，**不**用私有 `snapshot()`——codex BLOCK 修订）：

- 在 `crates/cdt-discover/src/project_scanner.rs::tests` 加 `extract_session_cwd_uses_first_line_only`：
    1. 构造 1000 行 JSONL（首行含 user message + `cwd` 字段，其余 999 行 assistant message 不含）
    2. 构造 scanner 时用 `cdt_fs::InstrumentedFs::new(cdt_fs::local_handle())` 包装 fs（这是 `FsOpCounter` 计数生效的硬要求——未包 wrapper 的 provider 调 trait 方法不计数，详 `crates/cdt-fs/src/instrumentation.rs:153-160`）
    3. 调用 `let (cwd, counts) = cdt_fs::with_fs_counter(|| async { scanner.extract_session_cwd(jsonl_path).await }).await;`
    4. 断言 `cwd == Some("/path/to/proj".to_string())`（首行字面量）
    5. 断言 `counts.read_to_string == 0`（兜底分支未触发）
- 加 `jsonl_append_after_first_line_does_not_change_cwd`：
    1. 写首行含 cwd，调一次 `extract_session_cwd`，记录结果 R1
    2. append 100 行 assistant message
    3. 再调一次 `extract_session_cwd`，结果 R2
    4. 断言 R1 == R2 且两次 `with_fs_counter` 返回的 `read_to_string == 0`

这两个测试把"cwd 在首行"升级为 `project-discovery` capability 的不变量契约（详 spec delta）。

**备选**（已驳回）：在 invalidator 的 ContentAppend 路径再加一次轻量 cwd revalidate（`fs::stat` 拿 mtime + 比较 cache 中 session.cwd）。
驳回理由：codex 指出 `parsed-message 缓存按 file-change 广播主动失效` 的 invalidator 已经每事件 stat 一次（`local.rs:2294-2356`），再加一次会让 metadata stat base load 更高（用户 ProjectScanCache 86k+ ops 类似的现状）。**信任 first-line invariant 并由测试守护**是更省成本的安全策略。

### D5：SSH backend 不动

SSH `ContextId` 下的 `ProjectScanCache` entry 走独立路径：无 watcher 主动 invalidate，仅靠 `SSH_CACHE_TTL = 10s` 自然过期（`project_scan_cache.rs:16`）。

`watcher` 是 Tauri 本地 fs 的硬不变量（与 `parsed-message 缓存按 file-change 广播主动失效` 同源约束）；invalidator 推算 `ContextId::local(projects_dir)` 决定失效作用域，SHALL NOT 触碰 `FsKind::SSH` entry。`invalidate_local()` 自身实现已经 `retain(|_, e| !matches!(e.fs_kind, FsKind::Local))`（`project_scan_cache.rs:181-185`），SSH entry 自然不受影响。

`contains_session_id` 在被 invalidator 调用时也 SHALL 仅查 `ContextId::local(projects_dir)` 的 entry，与同函数对其它 ctx 的 entry 隔离。

### D6：观测埋点（telemetry counter）

新增 3 个 counter 注册到 `crates/cdt-telemetry/src/registry.rs::COUNTER_NAMES`：

- `project_scan_cache.invalidate.structural` — `project_list_changed=true OR deleted=true` 触发的失效
- `project_scan_cache.invalidate.content_append_skipped` — 普通 JSONL append（含 watcher 折叠的 subagent 事件）放行的事件数
- `project_scan_cache.invalidate.lag_conservative` — `broadcast::Receiver::recv` 返回 `Lagged(_)` 走的保守全失效（与 structural 区分以诊断广播背压）

**为何 3 个而非 4 个**：D2 没有"未识别形态走兜底"分支（B 路线只有两档），不需要 `fallback_unknown` counter。

**为何 `lag_conservative` 单独一档而非合并到 structural**：lag 期间 watcher 可能广播过结构性事件被丢，invalidator 必须保守全失效——但**也可能 lag 期间只有 content append**（被吞掉无副作用）。两种 lag 触发的全失效行为相同但**信号源不同**，单独 counter 让运维侧能判断「lag 频繁吗 / lag 时是否真有结构性事件被吃掉」。每事件 inc 成本可忽略（`AtomicU64::fetch_add`）。

### D7：与现有 `ParsedMessageCache invalidator` 行为差异

`parsed-message 缓存按 file-change 广播主动失效` Requirement 的 lag 处理（`local.rs:2353-2356`）：

> broadcast lag（`broadcast::Receiver::recv` 返回 `Err(RecvError::Lagged)`）时 SHALL 静默继续 loop——lag 仅代表事件激增，下次 lookup 由被动 `FileSignature` mismatch 兜底，不影响正确性。

本 change 的 ProjectScanCache invalidator lag 处理是**保守全失效**，行为不一致。

**理由**：ProjectScanCache 没有 path-level 被动校验机制（不像 ParsedMessageCache 在 lookup 时 stat 比对 signature），lag 期间错过的结构性事件**没有兜底兑现机会**——只能保守清空让下次 IPC 重扫。该差异已在 spec delta 显式标注。

## Risks / Trade-offs

- **[R1] cwd lazy 写入**：极小概率 case（claude-code 未来引入"先建空 jsonl 再补 cwd"的格式）→ 首行无 cwd → 实际兜底走 `read_to_string` → cache 不失效但 cwd 字段错。
  → **Mitigation**：D3 的 `extract_session_cwd_uses_first_line_only` 测试断言 `FsOpCounter.read_to_string == 0`；若 claude-code 真改格式，先在该测试挂掉。
  → **检测**：D5 telemetry 不直接捕获该漂移；可加 `parsed-message` 既有 stat 逻辑作为间接信号（mtime 变 = 行为漂移可能性）。

- **[R2] watcher fs 错误 / inotify 漂移导致事件丢失**：watcher 出错重启或 macOS fsevents 漂移期，结构性事件可能完全没广播。
  → **Mitigation**：`LOCAL_CACHE_TTL = 300s`（已存）兜底；用户 5 分钟内不切 sidebar 时延迟可接受。
  → **未做**：watcher 自检上报机制（OQ）。

- **[R3] perf 收益不达预期**：本 change 解决 ProjectScanCache 风暴，但 sample 同时显示 `MetadataCache` lifetime 86k+ ops + `ParsedMessageCache invalidator` 每事件 stat。即修本 change 后 spike 强度可能下降但不归零。
  → **Mitigation**：tasks 加 perf bench 60s 长 sample 前后对比，设硬阈值 `File::open + read_dir` 命中数下降 ≥ 80%；不达标则回到 design 重审。后续 PR 才处理 MetadataCache stat 风暴。

- **[R4] watcher 标错 `project_list_changed`**：`mark_project_seen` 用 `HashSet` 去重，重启后 set 为空，启动后第一条对每个已存 project 的事件都会标 `plc=true` → invalidate 一次。
  → **接受**：启动时 cache 本来也是空的，第一次 IPC 必走全扫。重启后第一波事件 invalidate 是无副作用。

- **[R5] subagent 折叠的语义假设**：本 change 假设 watcher 折叠 subagent 到父 sid 是稳定行为。若未来 watcher 改成不折叠（subagent 事件单独广播），plc 字段含义变化。
  → **接受 + spec lock**：spec delta 把 watcher 当前的折叠行为隐式锁定为契约（已有 `file-watching::Route nested subagent JSONL changes to parent session` Requirement，不动）。

- **[R6] subagent 删除触发 false-positive invalidate**：watcher 折叠 subagent 到父 sid 时**保留** `deleted` 字段（`watcher.rs:213-223`），所以 subagent 文件被删时事件 `(pid, sid=父, deleted=true, plc=false)` 与"主 session 删除"在事件字段上**外观完全一致**——没有 path 信息可区分。本 change 走"deleted=true → invalidate"统一处理，subagent 删除时多扫一次 ProjectScanner。
  → **接受**：subagent 删除是低频事件（用户极少手动删除 subagent JSONL），即使触发 invalidate 也仅产生重扫成本（数百 ms），无正确性问题。spec delta 显式标注此 false-positive 行为契约。
  → **检测**：D6 telemetry `structural` counter 的"deleted 触发"无法直接区分两种成因；如未来需要更精确的运维诊断，可在 invalidator 内部加细粒度 sub-counter。

- **[R7] 与 ParsedMessageCache invalidator lag 处理的不一致状态窗口**：本 change 在 broadcast lag 时保守 `invalidate_local()`，而 `parsed-message 缓存按 file-change 广播主动失效` 在 lag 时静默继续（依赖 `FileSignature` 被动兜底，详 D7）。lag 之后短窗口内：用户调 `list_repository_groups` 走 cache miss 重扫拿到最新 project list，但同时调 `get_session_detail` / `get_image_asset` / `get_tool_output` 仍可能命中过期 parsed-message cache 拿到老数据。
  → **接受**：parsed-message 在 lookup 时 stat 比对 signature，过期数据**最终**会被发现并重 parse；用户感知就是"sidebar 列表更新但详情页内容更新有 0~2s 延迟"——可接受。该差异在 spec delta 与本 design 显式标注。
  → **未做**：让 parsed-message lag 也保守失效——会引入额外 parse 风暴成本，与本 change 减少全扫的目标矛盾。

## Migration Plan

无 breaking change，纯后端内部缓存策略调整：

1. **rollout**：单 PR 合并即生效，无 feature flag（cache 失效是内部决策，前端无感）。
2. **rollback**：若发现行为漂移，单 commit revert 把 invalidator closure 改回 `cache.lock().invalidate_local()` 一行（恢复"任何事件全失效"原状）。
3. **观测**：合并后 1 周内通过 D5 telemetry counter 看 `content_append_skipped / structural` 比例。预期活跃使用场景下 `content_append_skipped >> structural`（活跃 session 高频写入 → 大量 plc=false 事件）。

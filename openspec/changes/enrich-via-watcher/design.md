## Context

archived change `enrich-file-change-with-session-list-changed`（PR #291）把 `FileChangeEvent.session_list_changed` 字段语义定为"group 内 session 集合是否变化"，并把 enrich 决策放在 `cdt-api::project_scan_cache::apply_file_event_to_project_scan_cache` 三档判定中。判定规则 2 依赖 `cache.has_entry(local_ctx) || cache.has_in_flight_scan()` 守护，本意是防"lag 后 cache 被清空 + 普通 append 反复 bump generation 导致重扫风暴"。

但守护把"通知前端 revalidate"和"清 scan cache"两件事耦合到同一个 `structural` bool 上：cache 空 + 无 in-flight scan 时，即使收到真实的"已知 project 下新 session"事件，`unknown_session=false` → `session_list_changed=false` → 前端三档守护命中"无任何 flag" → Sidebar / DashboardView / ProjectSwitcher 均**不**调用 `loadProjects(true)` revalidate `list_repository_groups`。

实测复现路径：用户启动桌面端 → Sidebar mount 调一次 `list_repository_groups` 填 cache → 长时间 idle 让 entry TTL（`LOCAL_CACHE_TTL` = 300s）过期 → 用户在 IDE 用 Claude Code 在已知项目下新建会话 → watcher emit `plc=false, deleted=false, sid=new` → invalidator 看到 cache lookup 已返 None 但 `entries` 仍持有过期 entry（lookup TTL 检查不删 entry）→ `has_entry=true` 应触发规则 2 → **但**用户报告未刷新。需要两条独立诊断路径排查：(a) 实际生产路径的 cache 状态机是否在某种 race 下让 `has_entry=false`（如 `reconfigure_claude_root` 调 `invalidate_all`、SSH context 切换让 Local entry 被驱逐）；(b) 即使语义按 PR #291 设想正确，"是否通知前端"绑在 cache 状态上本身是脆弱设计，未来加 surface / 改 cache 行为容易再次踩坑。

同时 `spawn_unified_cache_invalidator` 的 `RecvError::Lagged` 分支当前只调 `apply_lag_to_project_scan_cache` 不向 `channels.files` emit 任何事件。下游 src-tauri 的 `sse-lagged` event 监听的是 `channels.files`（即 `subscribe_file_changes` receiver），而非上游 `watcher.subscribe_files()`——`watcher` 这一档的 lag 不会让 `channels.files` 跟着 lag（unified invalidator 静默吞了），前端连兜底 silent refresh 都收不到。这是与 cold-cache enrich bug 同源的"emit/cache 决策耦合"问题。

codex 二审独立确认两条 bug + 推荐"watcher 视角 enrich + cache 层只管 invalidate + lag 路径补 synthetic event"方向，并指出 5 个修正点（删除事件无条件 emit / lazy known_sessions 接受 false positive / SSH polling 对称改造 / cache 层 OR 不覆盖 / lag synthetic event）。

## Goals / Non-Goals

**Goals：**
- 修复"已知 project 下新建会话后 Sidebar / DashboardView / ProjectSwitcher 不刷新"的用户可见 bug，覆盖 cold-cache / TTL 过期 / context 切换 / `reconfigure_claude_root` 等所有 cache 状态
- 修复 `RecvError::Lagged` 路径前端兜底信号丢失
- 把 `session_list_changed` 字段填写权从 `ProjectScanCache` 层挪到 watcher 层，建立单一权威源——未来 cache 层重构 / 新 surface 加入不会再因状态漂移踩坑
- 保留 PR #291 的 IPC 减噪声收益（>95% append 事件不触发前端 revalidate）

**Non-Goals：**
- **不**改 `FileChangeEvent` 字段集合 / 序列化形态（`projectListChanged` / `sessionListChanged` / `deleted` 字段名 + camelCase 不变；前端 api.ts interface 不动）
- **不**降低前端守护粒度（Sidebar / DashboardView 守护逻辑保持不变）
- **不**做"增量 patch event"架构（让 `list_repository_groups` 走 push snapshot 模式以彻底删除前端 file-change 守护层）——属于后续 perf 优化，单独 change 走
- **不**预填 watcher `known_sessions`（接受 lazy false positive 而非用启动期 stat 538 文件 / 跨 crate 注入接口换取消除 0.7% 噪声）

## Decisions

### D1：把 `session_list_changed` enrich 从 cache 层挪到 watcher 层

**决策**：`cdt-watch::FileWatcher` 维护 `known_sessions: Mutex<HashSet<(PathBuf, String)>>`，在 `parse_project_event` 内填 `session_list_changed` 字段；`cdt-api::project_scan_cache::apply_file_event_to_project_scan_cache` 不再 enrich 该字段，只决定是否 `invalidate_local()`。

**理由**：判定来源单一化。watcher 是 fs 事件的天然观察者，"这个 (project, session) 是不是首次见"在 watcher 层用 HashSet O(1) 即可判定，不依赖任何下游 cache 的状态。cache 视角的 `contains_session_id` 在稳态下与 watcher 视角等价，但任何让 cache 状态变化的边角（cold start / TTL / context switch / `reconfigure_claude_root`）都会让 cache 视角判错。

**替代方案 A（cache 视角加更多守护）**：保留 PR #291 架构，给规则 2 加更多 fallback 路径（如 cache 空且无 scan 时仍 emit 但不 invalidate）。否决理由：每加一种 cache 状态守护就要测试矩阵 ×2，"通知前端"和"清 cache"语义仍耦合，未来再加 surface / 改 cache 行为继续踩坑。

**替代方案 B（增量 patch event）**：新加 `session-list-delta { kind: added/removed }` push event，前端 store reducer 内部 patch 不调 `list_repository_groups`。否决理由：grouper 拓扑（git identity / `isRepoRoot` / `cwdRelativeToRepoRoot`）依赖后端字段，前端无法可靠推导；100ms debounce 内 created+deleted race 让 delta 顺序不可信；lag / context switch / reconfigure 仍要 structural fallback 把复杂度拉回来。属于 perf 优化层，独立 change 评估。

### D2：`known_sessions` 启动时不预填（lazy）

**决策**：watcher 启动时 `known_sessions` 初始化为空集合，第一次见到任何 (project, session) 时插入。**不**通过 `std::fs::read_dir` 预扫所有现有 jsonl 文件路径预填。

**理由**：lazy 引入的 false positive（启动后旧 session 第一次写多触发一次 revalidate）实测约 5-10 次/天（活跃 session 数 × 启动频率），占 PR #291 优化收益（1328 次/天）的 < 1%。预填方案需要 538 文件 stat（~50ms 启动期阻塞）+ 跨 crate 注入接口（让 `cdt-discover::ProjectScanner` 第一次 scan 完后回填 watcher 状态），代价远大于收益。

**替代方案 A（启动期预扫预填）**：watcher 构造时 `read_dir` + `entries.filter_map` 把所有 `<projects>/*/<*>.jsonl` 路径填入 `known_sessions`。否决理由：538 文件 stat 启动延迟 / 与 ProjectScanner 重复扫 / 跨 OS 路径行为复杂度。

**替代方案 B（ProjectScanner 完成后注入）**：watcher 启动 lazy，`cdt-discover::ProjectScanner::scan` 第一次完成后调 `watcher.bulk_mark_sessions_seen(snapshot)` 注入。否决理由：跨 crate 接口（cdt-watch 与 cdt-discover 无现有依赖）+ 与 ProjectScanner 多次重 scan 路径协调复杂；为 < 1% 噪声不值得。

### D3：删除事件 `session_list_changed` 无条件 `true`

**决策**：watcher 处理 `deleted=true` 事件时 SHALL **无条件**填 `session_list_changed=true`，同时调 `unmark_session` 把 (project, session) 从 `known_sessions` 移除。**不**用 `unmark_session` 返回的 "是否真删除" 来决定 emit。

**理由**：lazy 模式下 `known_sessions` 不预填，删除一个 watcher 没见过的旧 session（启动后从未 append 过的、用户直接删的）很常见。如果按 `unmark.returns_true` 判定 emit，这类删除事件会被错误吞掉，前端拿到 `deleted=true` + `sessionListChanged=false` 也仍触发刷新（因为前端守护是 `plc || sLC || deleted`），但 cache 层 D4 OR 兜底无法补——破坏字段语义一致性。

**替代方案（按 unmark.returns_true 判 emit）**：让 `session_list_changed` 字段语义更严格地反映"watcher 视角下首次见的反向操作"。否决理由：和 `deleted` 字段产生语义重叠且更弱（任何删除都改变列表），加测试矩阵复杂度无收益。

### D4：cache 层 emit 决策与 invalidate 决策独立 + OR 兜底

**决策**：`apply_file_event_to_project_scan_cache` 返回 `EnrichDecision { invalidated: bool, emit_session_list_changed_hint: bool }`。原三档判定逻辑保留**仅用于 invalidate 决策**（保 `has_entry || has_in_flight_scan` 守护防风暴）。`spawn_unified_cache_invalidator` emit 时取 `event.session_list_changed || decision.emit_session_list_changed_hint`——watcher 已知（first-seen）+ cache 已知（snapshot 视角下也算 unknown）取并集，最大兜底。

**理由**：
- "是否清 scan cache"和"是否通知前端"是两件独立的事——cache 空时清 cache 是 no-op（生产路径不调 `entries.remove(local_ctx)` 也无害），但通知前端不能漏；
- watcher 视角虽是单一权威，但 cache 视角在 cache 已 fill 场景下也是有效信号（特别是 watcher 重启 / `reconfigure_claude_root` 让 watcher `known_sessions` 清空但 cache 还在的窗口期），OR 兜底比单源更鲁棒；
- 字段名不变（仍是 `session_list_changed`），前端 api.ts 不需要改，IPC contract test 字段名校验自然过。

**替代方案（cache 层完全不 enrich）**：watcher 单源决定字段值，cache 层只 invalidate 不参与 emit。否决理由：watcher 重启 / `reconfigure_claude_root` 期间 watcher `known_sessions` 重置但 cache 仍持有旧 entry 的窗口，"watcher 视角误判 first-seen → emit true → 实际是冗余 revalidate"虽不漏但偶尔误报；OR 兜底让 cache 视角作为第二判定源（cache 视角下"contains_session_id 返 true"代表"此前业务路径见过"，是更长记忆），减少误报。

### D5：SSH polling watcher 复用 `baseline` 实现对称 first-seen 判定

**决策**：`cdt-ssh::polling_watcher::build_change_event` 当前直接构造 `session_list_changed=false`。改为基于 `RemotePollingWatcher` 已有的 `baseline: BTreeMap<PathBuf, FileFingerprint>`：
- baseline 不含 path（新增 path）→ `session_list_changed=true`
- baseline 含 path 但当前 readdir 不返（删除）→ `session_list_changed=true`
- baseline 含 path 且仍存在（size/mtime 变化）→ `session_list_changed=false`

第一次 poll 静默建 baseline（spec 已规定，无 emit），baseline 内的 session 自然算"已见"。

**理由**：与 D1 watcher 视角对称。SSH context 切回 local 时，本地 watcher `known_sessions` 仍 lazy 不预填，但 SSH 侧的 baseline 已建过——两路径都用各自的"已见集合"判定 first-seen 即可，不依赖 cache 状态。

**SSH 断连重连 baseline diff 边角**（codex round 1 GAP #2 修订）：SSH 断连期间用户在远端新建 / 删除 session，重连后 watcher 第一轮 poll 会"静默建 baseline"——按 spec 现行规则，新增 path 默认进 baseline 不 emit，会让前端漏一次首见信号。修法：`RemotePollingWatcher::spawn` 接收上次断连时的 baseline 快照（如有），重连首轮 poll 把"新 readdir 结果"与"上次 baseline 快照"做 diff，对断连期间新增 path emit `session_list_changed=true`，对断连期间删除 path emit `session_list_changed=true + deleted=true`。首次启动（无上次 baseline）行为不变（静默建 baseline）。spec MODIFIED 明确这条 Scenario，避免实现侧在"复用旧 baseline / 重建新 baseline"之间漂移。

**替代方案（SSH 路径继续填 false 由 cache 层 enrich）**：保留 SSH polling 现状，让 cache 层三档判定 enrich SSH 事件的 `session_list_changed`。否决理由：D1 已明确把 enrich 挪出 cache 层，SSH 不对称会让 codex 找到的"context switch 自动正确"论点不成立。

### D6：`RecvError::Lagged` 分支显式 emit synthetic structural event

**决策**：`spawn_unified_cache_invalidator` 收到 `RecvError::Lagged(n)` 时除调 `apply_lag_to_project_scan_cache` 外，SHALL 显式 send 一条 synthetic `FileChangeEvent { project_id: "", session_id: "", deleted: false, project_list_changed: true, session_list_changed: true }` 到 `channels.files`。

**理由**：当前 lag 路径丢信号是因为 src-tauri 的 `sse-lagged` event emit 监听的是 `channels.files`（=`LocalDataApi.subscribe_file_changes` receiver），而 watcher → unified invalidator 这一档的 lag 在 `watcher.subscribe_files()` receiver 上，后续不会让 `channels.files` 跟着 lag——unified invalidator 静默吞了。修法是 invalidator 自己主动 emit 一条触发前端三档守护的 synthetic event，让 src-tauri 桥正常 forward 到前端 `file-change` listener，前端 Sidebar / DashboardView 收到后自动走兜底全量 revalidate 路径（已有 `if (!currentGroupId || !payload.sessionId) return` 守护防止误尝试 `loadSessions("")`）。

**替代方案 A（专用 sse-lagged 信号通道）**：让 invalidator 调 `app_handle.emit("sse-lagged", { source: "watcher" })`。否决理由：`cdt-api` crate 不该直接持有 `tauri::AppHandle`（破坏 IPC vs HTTP 抽象边界），需要新跨层 channel；synthetic FileChangeEvent 复用既有 broadcast 路径，零新协议。

**替代方案 B（保持现状，依赖被动 generation race 兜底）**：lag 后用户下次主动操作触发 `list_repository_groups` 时 cache 已被 `apply_lag_to_project_scan_cache` 清空，自然 miss + 重 scan。否决理由：用户感知滞后到下次操作，期间项目列表 / 总数显示与磁盘真实状态不一致，违反 ipc-data-api spec §"会话总数显示口径"silent refresh 契约。

## Risks / Trade-offs

- **lazy `known_sessions` false positive**：watcher 启动 / `reconfigure_claude_root` 后旧 session 第一次写多触发一次全量 revalidate。**Mitigation**：实测约占 PR #291 优化收益的 < 1%（5-10 次/天 vs 1328 次/天 append 噪声归零），可接受。如未来发现影响，按 D2 替代方案 B（ProjectScanner 完成后注入）单独优化。

- **synthetic lag event 字段为空字符串**：前端三个 surface 必须能正确处理 `payload.projectId === "" && payload.sessionId === ""` 的兜底信号。**Mitigation**：Sidebar 已有 `if (!currentGroupId || !payload.sessionId) return` 守护跳过 `loadSessions("")` 调用；DashboardView 守护是"任一 flag true 就 loadData"不依赖 id；新加 unit test 覆盖 synthetic payload 行为，反转 fix 验证测试抓得到回归。

- **D4 OR 兜底带来的语义双源**：watcher 视角 + cache 视角并集决定 emit，理论上有"两源都判 false 但实际是 unknown_session"的极端 race——`reconfigure_claude_root` 同时让 watcher `known_sessions` 重置 + `ProjectScanCache::invalidate_all` 清空 entries + 此刻 `has_in_flight_scan == false` 同时三件事撞在一起的瞬时窗口内，正巧到达一个本应 emit 的 first-seen 事件。此 race 下两源都判 false，无人 emit，前端漏一次 revalidate。**该 race 不是由 broadcast lag 触发，D6 synthetic event 不会兜底**。**Mitigation**：(a) `reconfigure_claude_root` 路径已是阻塞 sync 操作（用户主动切 Claude root 触发），实际上 watcher 重置 + cache 清空发生在同一关键路径，对外可见的事件流自然按"reconfigure 完成 → 业务 list_repository_groups 触发 begin_scan → in_flight_scan=1 → 此后 watcher 首见事件能命中规则 2 OR hint=true"顺序——race 窗口仅存在于 reconfigure 完成与首次业务调用之间的极短时段；(b) 显式接受此 race 下漏刷新一次，下次任意 file event（含同 session 的后续 append，watcher 视角已是 first-seen → emit true）会自动触发兜底 revalidate；(c) tasks 加测试限制：spec Scenario 仅覆盖"cache 空 + watcher 首见 → emit true"主路径，不强制覆盖 reconfigure race 漏 emit 行为；测试矩阵中显式标 race 为 `#[ignore = "documented as accepted edge case"]` 留 trace。

- **D6 synthetic event 在 HTTP SSE bridge 路径的传播守护**：synthetic event `{ project_id: "", session_id: "", project_list_changed: true, session_list_changed: true }` 经 `LocalDataApi.file_tx` broadcast → `cdt-api::http::bridge::spawn_file_bridge` 转 `PushEvent::FileChange`（HTTP SSE） / src-tauri Tauri host emit 两路并存。spec MODIFIED 已明确 Tauri webview 路径前端 surface 的空 id 守护，HTTP SSE / 浏览器 transport 路径需要等价守护——浏览器 transport.ts 与 Tauri webview 共用 `fileChangeStore.svelte.ts` handler 链，前端守护代码自然复用。**Mitigation**：tasks 加端到端 e2e 测试覆盖浏览器 `?http=1` 模式下 synthetic event 行为，断言 sidebar / dashboard 触发兜底 revalidate 且**不**引发 `loadSessions("")` 类副作用。

- **跨 crate 测试构造复杂度**：cold-cache 集成测试需要构造"FileWatcher 已 mark + ProjectScanCache 空"的精确状态，复用现有 `file_tx_for_test` + `cdt-api` test-utils feature。**Mitigation**：参照 `crates/cdt-api/tests/sse_event_bridge.rs` 既有 inject pattern。

- **SSH polling watcher 改动 off-by-one 风险**：第一次 poll 建 baseline 不 emit / 第二次 poll 起 emit 的时序边界。**Mitigation**：单测覆盖 first-poll baseline / second-poll new path / second-poll deleted path / second-poll size+mtime change 四种场景，反转 fix 验证。

## Migration Plan

无需数据迁移 / 配置迁移。`FileChangeEvent` 字段集合不变，前端 api.ts interface 不动。改动是后端实现位置重构 + lag 路径行为修复。

回滚策略：单 commit revert 即可（如果 archive 后发现问题，按一般 fix 流程开新 change 反向调整）。

## Open Questions

- 是否在本 change 同步把主 spec `Watch project directory additions` Requirement 的 `Existing project session change does not refresh projects` Scenario 重写——它当前规定"已知 project 下 session 改变 MUST NOT 标记项目列表变化"，与本 change 引入的"session first-seen 触发 `session_list_changed=true`"语义不冲突（前者是 `project_list_changed` 字段，后者是 `session_list_changed`），但 spec 阅读上容易混淆。倾向：本 change scope 内不改主 spec 该 Scenario，未来如果 spec purity 重构再处理。

- 是否需要把"前端三个 surface 的守护逻辑"集中到 `fileChangeStore.svelte.ts` 一个 `isStructuralPayload(payload)` helper，避免 Sidebar / DashboardView 各自维护一份漂移。倾向：本 change scope 内**不**做（属于前端重构，不阻塞 bug 修复），followup 留 GitHub issue。

- spec delta MODIFIED Requirement 体内继承自原文的实现细节引用（`RemotePollingWatcher` / `BTreeMap<PathBuf, FileFingerprint>` / `RecommendedWatcher` 等内部类型名 / 部分文件行号）按 spec purity 反模式规则属于"内部模块/类/函数名"应迁出 spec 正文。本 change 因 MODIFIED 必须 paste 完整 Requirement body 而无法独立修复存量反模式——`scripts/check-spec-purity.sh` 的 baseline 锁定单独由 `spec-purity` 类 ratchet change 管理。本 change 新增的 Requirement 与新增段落 SHALL NOT 引入新的反模式（已遵守 ASCII 路径 / 字段名 / 公开 IPC 协议层标识符为限）。倾向：明确告知 reviewer 此为存量继承非本 change 引入；purity baseline 在 archive 时若新增反模式计数 SHALL 走单独 baseline 同步 commit。

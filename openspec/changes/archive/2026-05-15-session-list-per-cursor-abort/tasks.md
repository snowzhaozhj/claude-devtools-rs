## 1. 后端 `cdt-api`：active_scans 改 (projectId, cursor) 双键

- [x] 1.1 改写 `metadata_scan_key(project_id: &str, cursor: Option<&str>) -> String`：返回 `format!("{project_id}|{}", cursor.unwrap_or(""))`，并在 doc-comment 显式声明 `|` 为 reserved 分隔符。
- [x] 1.2 `LocalDataApi::list_sessions` 临界区按新签名构造 `scan_key`，传入当前调用的 `pagination.cursor.as_deref()`；保持 abort + spawn + insert 在同一 sync lock 内完成的 race-free 设计。
- [x] 1.3 `scan_metadata_for_page` 的 `cleanup_key` 参数同步从 caller 传入新格式 key；cleanup 时按完整 (projectId|cursor) key + generation 比较语义不变。
- [x] 1.4 `active_scans: HashMap<String, ScanEntry>` 类型不变（仅 key 编码升级），不破坏既有 `Arc<sync::Mutex<...>>` ownership。

## 2. 后端测试：cursor 维度并存 + 同 cursor 抢占

- [x] 2.1 `crates/cdt-api/tests/list_sessions.rs`（新增或扩展现有 IPC 测试文件）：fixture 1 个 project + 30 个 jsonl，全部 cache miss；并发调 `list_sessions(p, pageSize=20, cursor=null)` 与 `list_sessions(p, pageSize=20, cursor="20")`；订阅 `subscribe_session_metadata()`；断言两次调用的扫描都跑完，receiver 收到的 update 总数 = 30 条（覆盖两页所有 cache miss 条），不少于对应。
- [x] 2.2 同文件加测：连续调 `list_sessions(p, pageSize=20, cursor=null)` 两次（同 cursor）；第一次 spawn 的 task SHALL 被 abort；receiver 收到的 update 总数 SHALL ≤ 20（只有第二次 task 完整推送，第一次的部分 update 可能已收到也可能没 emit）。
- [x] 2.3 `cargo test -p cdt-api --test list_sessions`（或合适测试模块）全绿。

## 3. 前端 `Sidebar.svelte`：sessionsTotal 取 result.total

- [x] 3.1 引入 `let sessionsTotal = $state<number>(0);`，把 `totalSessions = $derived(sessions.length)` 改为 `totalSessions = $derived(sessionsTotal)`。
- [x] 3.2 `loadSessions` 非 silent 路径 IPC 返回后赋值 `sessionsTotal = result.total`；silent 路径合并完后同样赋值；切换 project 时（projectId 变） reset `sessionsTotal = 0`。
- [x] 3.3 `loadMoreSessions` 翻页路径 SHALL **不**改 `sessionsTotal`（注释里点明）。
- [x] 3.4 `npm run check --prefix ui` 全绿。

## 4. 前端单测 + e2e

- [x] 4.1 `ui/src/lib/sessionMerge.test.ts` 不动（merge 语义未变）；新增或扩展现有 sidebar 相关 vitest（如 `ui/src/components/Sidebar.test.ts` 若存在）覆盖 `sessionsTotal` 在 loadMore 后不变；如无现有文件就用 e2e 覆盖。
- [x] 4.2 `ui/tests/e2e/` 加 spec：fixture `multi-project-rich`，首次加载选第一个 project，验证 `session-count-num` 显示 `N/M`（M = 后端返回 total，> sessions.length）；不阻塞——若现有 fixture 不便覆盖，仅靠后端测 + Rust IPC contract test 校验。

## 5. 文档与 spec 校验

- [x] 5.1 `openspec validate session-list-per-cursor-abort --strict` 全绿。
- [x] 5.2 `just preflight`（fmt + lint + test + spec-validate）全绿。

## 6. codex 设计 + 实现二审

- [x] 6.1 design.md 完成后调 `codex review --commit <sha>` 审 D1/D2/D3/D4 决策与 spec delta 是否漏 scenarios。本环境 `Agent({ subagent_type: "codex:codex-rescue" })` 不可用，改走 codex CLI；first-pass 0 问题。
- [x] 6.2 实现 + 测试完成后跑第二轮 codex review（`codex review --base main`）；self-review 找到 metadata scan semaphore 应共享而非 per-task 新建（后端 spec 约束 8 并发上限），第二轮 codex 仍 0 问题。
- [x] 6.3 codex 二轮 + self-review 找到的 bug（semaphore 共享）已修完，`cargo test --workspace` 全过，准备 push + PR。

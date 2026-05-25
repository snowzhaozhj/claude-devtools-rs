## 1. cdt-watch（local watcher first-seen 跟踪）

- [x] 1.1 在 `FileWatcher` 加 `known_sessions: Mutex<HashSet<(PathBuf, String)>>` 字段（与 `known_projects` 同地）；`Default::default()` 初始空集合；构造时不预填
- [x] 1.2 实现 `mark_session_seen(&self, project_id: &str, session_id: &str) -> bool`（HashSet `insert` 包装），key 走 `normalize_path_for_compare` 与 `mark_project_seen` 对称
- [x] 1.3 实现 `unmark_session(&self, project_id: &str, session_id: &str)`（幂等 remove，不返回 bool）
- [x] 1.4 改 `parse_project_event`：写事件路径调 `mark_session_seen` 填 `session_list_changed`；删除事件调 `unmark_session` 后**无条件**填 `session_list_changed=true`；subagent 嵌套分支 SHALL NOT 调 mark/unmark 且字段固定填 `false`；顶层 dir-create 分支字段固定填 `false`
- [x] 1.5 加 unit tests：(a) 已知 project 下首次见 session 填 `true`；(b) 后续 append 填 `false`；(c) 删除已知 session 填 `true`；(d) 删除从未见过的 session 仍填 `true`；(e) subagent jsonl 不进集合；(f) 启动后旧 session 第一次写填 `true`（lazy false-positive）
- [x] 1.6 反转 fix 验证：把 `mark_session_seen` 临时改回固定返 `false` 跑测试集合应红；改回应绿

## 2. cdt-ssh（SSH polling watcher first-seen 对称 + 断连重连 baseline diff）

- [x] 2.1 改 `RemotePollingWatcher::build_change_event`（或对应位置）让 `session_list_changed` 字段基于 `baseline: BTreeMap<PathBuf, FileFingerprint>` 判定：baseline 不含 path（新增）→ `true`；baseline 含 path 但当前 readdir 不返（删除）→ `true`；baseline 含 path 且仍存在但 size/mtime 变化（追加）→ `false`
- [x] 2.2 第一次 poll 不 emit 任何事件（已是 spec 契约，确认 baseline 在 first poll 后包含全部已存在 path）
- [x] 2.3 改 `RemotePollingWatcher::spawn` 签名接受 `Option<BaselineSnapshot>` 参数（断连重连时由 caller 传入上次 baseline 快照）；调用方 `cdt-api::LocalDataApi::attach_remote_watcher` 在 dead-signal monitor 重连路径同步保存 + 传回旧 baseline；首次连接 caller 传 `None`
- [x] 2.4 重连首轮 poll diff 实现：传入旧 baseline 时第一轮 poll 完成后做完整 diff——断连期间新增 path emit `session_list_changed=true`，断连期间删除 path emit `session_list_changed=true + deleted=true`，size/mtime 变化 emit `session_list_changed=false`；diff 完成后新 baseline 替换旧 baseline，进入正常 3s polling 循环；caller 未传旧 baseline 时退化为静默建 baseline 路径
- [x] 2.5 加 unit tests：(a) first poll 静默建 baseline；(b) second poll 新增 path emit `session_list_changed=true`；(c) second poll 删除 path emit `session_list_changed=true`；(d) second poll size/mtime 变化 emit `session_list_changed=false`；(e) 断连重连传入旧 baseline 后首轮做 diff（含新增 / 删除 / size 变化三种）；(f) 重连未传 baseline 时退化为静默建 baseline
- [x] 2.6 反转 fix 验证：把字段临时改回 `false` 跑 second-poll 新增 / 删除 测试应红；改回应绿；重连 diff 临时关掉跑 (e) 应红；改回应绿

## 3. cdt-api（cache 层拆 emit/invalidate + lag synthetic event）

- [x] 3.1 改 `apply_file_event_to_project_scan_cache` 签名返 `EnrichDecision { invalidated: bool, emit_session_list_changed_hint: bool }`（替代当前返 `bool` structural）；`emit_session_list_changed_hint` 值 = "本 event 命中规则 2 unknown_session 判定条件（cache snapshot 视角下）"；`invalidated` 值 = "三档判定决定调用了 `invalidate_local()`"
- [x] 3.2 改 `spawn_unified_cache_invalidator` 内部 loop：emit 字段 = `event.session_list_changed || decision.emit_session_list_changed_hint`（OR 公式）；其他 emit 顺序 / 锁释放 / async parsed 不阻塞 emit 等契约保持不变
- [x] 3.3 改 `RecvError::Lagged(n)` 分支：在 `apply_lag_to_project_scan_cache` 后显式 `file_tx.send(synthetic)` 一条 `FileChangeEvent { project_id: "", session_id: "", deleted: false, project_list_changed: true, session_list_changed: true }`；`tracing::warn!` 标 `missed = n` 与 source 信息
- [x] 3.4 加集成测试 `cdt-api/tests/sse_event_bridge.rs` 或新文件：(a) cold-cache enrich 不被抑制——构造空 `ProjectScanCache` + 注入 raw event with `session_list_changed=true`，断言 `file_tx` 收到 enriched event 仍 `session_list_changed=true`；(b) lag synthetic event——注入 > `CHANNEL_CAPACITY` 个 events 触发 broadcast lag，断言 `file_tx` 收到 synthetic `FileChangeEvent { project_id: "", session_id: "", project_list_changed: true, session_list_changed: true }`；(c) OR 公式——watcher 已填 false 但 cache hint 为 true 时 emit `true`；(d) OR 公式——watcher 已填 true 但 cache hit 让 hint 为 false 时 emit `true`（OR 两源并集）；(e) **local 与 SSH 两路径 first-seen 语义对称契约测试**——表驱动覆盖 `(同 input event 形态) → (同 enriched output 字段)`，断言无论事件来源是本地 watcher 还是 SSH polling watcher，相同 `(project_list_changed, deleted, watcher 跟踪集合命中)` 输入 SHALL 产生相同 `session_list_changed` 字段（codex round 1 GAP #5）
- [x] 3.5 反转 fix 验证：把 emit 公式临时改回 `decision.emit_session_list_changed_hint`（覆盖 watcher 字段）跑 cold-cache 测试应红；改回应绿
- [x] 3.6 文档化接受边角：D4 接受的"reconfigure_claude_root + cache invalidate_all + 无 in_flight_scan 三件事同时发生的极端 race 漏 emit 一次" SHALL 在 `cdt-api/tests/` 添加 `#[ignore = "documented as accepted edge case (design.md::D4 Risks)"]` 测试 case 留 trace，避免未来误以为是 bug 修补打破其他正确路径

## 4. 前端（synthetic event 守护测试 - Tauri webview + 浏览器 transport 双路径）

- [x] 4.1 加 `ui/src/components/Sidebar.test.svelte.ts` 测试用例：handler 收到 `payload { projectId: "", sessionId: "", projectListChanged: true, sessionListChanged: true, deleted: false }` 时 SHALL 触发 `loadProjects(true)` 但 SHALL NOT 触发 `loadSessions("")`（per-session 守护命中）
- [x] 4.2 加 DashboardView 测试 case（inline 现有测试文件）：handler 收到同样 synthetic payload SHALL 触发 `loadData(true)` 兜底全量
- [x] 4.3 加 `ui/src/lib/transport.test.ts` 测试 case：浏览器 transport 路径模拟收到 `PushEvent::FileChange` synthetic payload（空 id），断言归一化后传入同一 fileChangeStore handler 链，与 Tauri webview 路径行为一致（codex round 1 GAP #3）
- [x] 4.4 反转 fix 验证：临时把 Sidebar handler 的 per-session 守护移除，跑 4.1 应红；恢复应绿；同样反转浏览器 transport 守护跑 4.3 应红

## 5. spec validate + ratchet 同步

- [x] 5.1 跑 `openspec validate enrich-via-watcher --strict` 通过
- [x] 5.2 `scripts/check-spec-purity.sh` 通过：本 change MODIFIED 因 paste 完整 body 继承存量反模式（file-watching 16 + ipc-data-api 7），同 commit 在 `scripts/spec-purity-baseline.txt` 加 `change/enrich-via-watcher/file-watching 16` + `change/enrich-via-watcher/ipc-data-api 7` 两行；archive 时随 spec sync 合并到主 spec 计数（codex round 1 GAP #4 同意的存量继承不修）
- [x] 5.3 `crates/cdt-api/tests/ipc_contract.rs` 121 测试 pass，`FileChangeEvent` 字段集合 / camelCase 形态不变（本 change 不引入字段集合变动，无需新增 round-trip case）

## 6. 端到端验证（真数据 e2e）

- [x] 6.1 `e2e-http-verify` PASS：cdt-cli HTTP server 启动成功，新建 jsonl → SSE 660ms 内 emit `session_list_changed=true`，前端 sessionCount +1；删除 jsonl emit `deleted=true + session_list_changed=true`，sessionCount 归零；防 spam 验证：第二次 append 同 session 正确返回 `session_list_changed=false`（lazy false-positive 设计 D2 符合预期）
- [ ] 6.2 `just dev` 桌面端 smoke 由 lead 本机手动确认：qa-engineer subagent CLI-only 无 GUI 跳过；`cargo check` src-tauri 编译通过，新代码会编入桌面 binary。建议 lead 在本机 `just dev` + 在 aiUltron 下新建 jsonl 做 5 秒目视确认（用户原 bug 场景）
- [ ] 6.3 SSH 路径 docker smoke 跳过：本机无 `cdt-ssh-test` 容器；SSH 路径已通过 cdt-ssh 单测 132 个 + cdt-api 集成测试 (e) 对称契约表驱动 6 case 覆盖（task 2.5 + 3.4(e)）。followup：PR 描述列 SSH e2e 为路线图候选

## 7. preflight + 提交

- [x] 7.1 跑 `just preflight`（fmt / lint / test / spec-validate 一把梭）通过（31/31 spec validate + ipc-command-sync OK + workspace clippy/test 全绿）
- [x] 7.2 commit 业务改动 + 测试 + spec delta（同一 commit）

## N. 发布

- [x] N.1 push 分支 + 开 PR（PR #305 https://github.com/snowzhaozhj/claude-devtools-rs/pull/305）
- [x] N.2 wait-ci 全绿（round 1 commit 12477c1 / round 2 commit 82e4a3b / round 3 commit 0081214 三轮全绿）
- [x] N.3 codex 二审通过（round 1 BLOCK 3 bug → 修 commit 82e4a3b；round 2 BLOCK 3 bug → 修 commit 0081214；round 3 PASS 非阻塞 + spec 文本对齐 nit 同 archive commit 修复）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

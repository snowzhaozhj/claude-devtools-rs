## 1. cdt-core：FileChangeEvent 加字段

- [x] 1.1 `crates/cdt-core/src/watch_event.rs::FileChangeEvent` 加 `pub session_list_changed: bool`，套 `#[serde(default, skip_serializing_if = "std::ops::Not::not")]`
- [x] 1.2 `cargo check -p cdt-core` 通过

## 2. cdt-ssh：polling_watcher 构造点补字段

- [x] 2.1 `crates/cdt-ssh/src/polling_watcher.rs:593`（`build_change_event`）补 `session_list_changed: false`（D2：SSH 路径恒为 false，由 unified invalidator enrich）
- [x] 2.2 `cargo check -p cdt-ssh` 通过

## 3. cdt-watch：构造点补字段 + attach_remote 签名扩展

- [x] 3.1 `crates/cdt-watch/src/watcher.rs:200`（顶层 dir-create 分支）补 `session_list_changed: false`
- [x] 3.2 `crates/cdt-watch/src/watcher.rs:218`（subagent 折叠分支）补 `session_list_changed: false`
- [x] 3.3 `crates/cdt-watch/src/watcher.rs:243`（jsonl 主路径）补 `session_list_changed: false`
- [x] 3.4 `crates/cdt-watch/tests/file_watching.rs:45`（test helper）补 `session_list_changed: false`
- [x] 3.5 `crates/cdt-watch/src/watcher.rs::FileWatcher::attach_remote` 签名扩展：加 `cancel: cdt_ssh::CancelToken` 参数（替代内部 `CancelToken::new()`），保留 `RemoteWatcherHandle` 返回值不变
- [x] 3.6 `crates/cdt-watch/src/watcher.rs:862` 的 `attach_remote_broadcasts_schema_compatible_file_event` 测试同步更新签名（构造 `CancelToken::new()` 传入）
- [x] 3.7 `cargo test -p cdt-watch`（单跑 case 即可，整套在 macOS flaky）

## 4. cdt-api：unified invalidator 改造为 file_tx 唯一生产者

- [x] 4.1 `crates/cdt-api/src/ipc/local.rs::spawn_unified_cache_invalidator` 函数签名加 `file_tx: broadcast::Sender<cdt_core::FileChangeEvent>` 参数
- [x] 4.2 修改 invalidator loop 体顺序（D4 emit 时机契约）：
  - sync `apply_file_event_to_project_scan_cache(event)` 拿三档判定结果（返回值改为 `bool` 表示 structural）；锁在函数内部 sync block 末尾自动释放
  - 构造 `enriched_event = FileChangeEvent { session_list_changed: structural, ..raw_event.clone() }`
  - 调 `file_tx.send(enriched_event)` emit（确认锁已释放）
  - async `apply_file_event_to_parsed_cache(event).await` 不阻塞 emit
- [x] 4.3 `apply_file_event_to_project_scan_cache` 函数返回值从 `()` 改为 `bool`（structural=true 时返 true）；`apply_lag_to_project_scan_cache` 保持现有签名（lag 路径不 emit synthetic event）
- [x] 4.4 删除 `crates/cdt-api/src/ipc/local.rs:2253` 的 `bridge_task` 块；调用 `spawn_unified_cache_invalidator` 时传入 `channels.files.clone()`
- [x] 4.5 `start_watcher_pipeline_with_channels` 的返回 `Vec<JoinHandle<()>>` 中删去 `bridge_task` entry

## 5. cdt-api：SSH 路径接入 unified invalidator（D2）

- [x] 5.1 `crates/cdt-api/src/ipc/local.rs::LocalDataApi` 加 `watcher: Option<Arc<FileWatcher>>` 字段；现有 `new()`（`local.rs:1528`）/ `new_with_watcher()`（`local.rs:1631`）两处 `Self { ... }` 初始化分别加 `watcher: None` / `watcher: Some(Arc::new(file_watcher))`
- [x] 5.2 改 `LocalDataApi::attach_remote_watcher`（`local.rs:1760`）：从 `RemotePollingWatcher::spawn(..., self.file_tx.clone(), cancel_token.clone())` 改为 `self.watcher.as_ref().expect("watcher present in attach path").attach_remote(sftp, projects_dir, cancel_token.clone())`。Invariant：`file_tx.is_some()` 必须伴随 `watcher.is_some()`——`attach_remote_watcher` 入口已有 `self.file_tx.as_ref()` guard（`local.rs:1761`），`new()` 路径下 `file_tx=None` 直接 return；构造期保持 `new_with_watcher()` 同时设两个 `Some`，`new()` 同时设两个 `None`
- [x] 5.3 核验 `RemoteWatcherHandle` cancel / dead-signal monitor 路径在新调用方式下行为等价：调用方仍持有 `cancel_token` clone，handle 由 `attach_remote` 返回赋给现有 monitor 持有的字段
- [x] 5.4 `crates/cdt-api/tests/ssh_reconnect_lifecycle.rs` 测试场景核对：SSH connect / disconnect / reconnect / context switch 4 个路径的 file event 仍能被前端 subscriber 收到（enriched form），cancel 路径仍可用
- [x] 5.5 加 SSH 集成测试：`local::tests::ssh_event_enriched_through_unified_invalidator`——构造 fake SSH context，触发 polling_watcher emit `FileChangeEvent { project_list_changed: true }`，断言 `LocalDataApi::subscribe_file_changes` receiver 收到的 enriched event 含 `session_list_changed: true`

## 6. cdt-api：HTTP/SSE 字段 + lag 兜底（D5 + D6）

- [x] 6.1 `crates/cdt-api/src/ipc/events.rs::PushEvent::FileChange` 加 `session_list_changed: bool` 字段（`#[serde(default)]`）；保持 enum 既有 `#[serde(tag = "type", rename_all = "snake_case")]` 不变（snake_case variant tag + 字段保留 Rust 字段名 snake_case，与既有 `project_id` / `project_list_changed` 风格一致）。**禁止**给 enum 加 `rename_all_fields = "camelCase"`——会破坏既有 `project_id` 等字段的 SSE 形态
- [x] 6.2 `crates/cdt-api/src/ipc/events.rs::PushEvent` 加新 variant `SseLagged { source: String, missed: u64 }`，无需 variant 级 rename（既有 `rename_all = "snake_case"` 自动把 `SseLagged` 转 `sse_lagged`；字段 `source` / `missed` 单词无下划线，snake_case 即字面 OK）
- [x] 6.3 `crates/cdt-api/src/http/bridge.rs::spawn_file_bridge`（`bridge.rs:43`）转发 `enriched_event.session_list_changed` 到 `PushEvent::FileChange.session_list_changed`
- [x] 6.4 `crates/cdt-api/src/http/bridge.rs:56` `Err(broadcast::error::RecvError::Lagged(n)) => {}` 改为 `let _ = events_tx.send(PushEvent::SseLagged { source: "file-change".into(), missed: n });`，**禁止**继续吞掉 lag 信号
- [x] 6.5 `crates/cdt-api/src/http/sse.rs::convert_broadcast_result` 路径核验：`Ok(PushEvent::SseLagged { source, missed })` 序列化输出 `{"type":"sse_lagged","source":"file-change","missed":n}` 与既有 `SSE_LAGGED_SENTINEL = r#"{"type":"sse_lagged"}"#` 字符串解析向后兼容（前端只看 `type` 字段；旧 sentinel 缺 source / missed → 前端读 undefined 不报错）
- [x] 6.6 `cargo clippy -p cdt-api --all-targets -- -D warnings` 通过

## 7. cdt-api：测试同步

- [x] 7.1 `crates/cdt-api/tests/project_scan_cache_invalidation.rs::ev` helper 加 `session_list_changed: false` 默认参数
- [x] 7.2 `crates/cdt-api/tests/sse_event_bridge.rs` 4 处 `FileChangeEvent { ... }` 构造点补 `session_list_changed: false`；同 file 内 `PushEvent::FileChange` 模式匹配（`sse_event_bridge.rs:82/120/433`）补 `session_list_changed` 字段
- [x] 7.3 `crates/cdt-api/src/notifier.rs` 3 处构造点（L308 / L324 / L358 build_event helper）补 `session_list_changed: false`
- [x] 7.4 `crates/cdt-api/src/ipc/local.rs:6232` 的 test 构造补字段
- [x] 7.5 `crates/cdt-api/tests/ipc_contract.rs` 加 `file_change_event_session_list_changed_round_trip` 测试 + `push_event_sse_lagged_round_trip` 测试（构造 `PushEvent::SseLagged { source: "file-change", missed: 7 }` 序列化断言 `{"type":"sse_lagged","source":"file-change","missed":7}`，反序列化 round-trip 一致）
- [x] 7.6 `crates/cdt-api/tests/project_scan_cache_invalidation.rs` 加新 scenario 测试：`unified_invalidator_emits_session_list_changed_true_for_unknown_session`、`unified_invalidator_emits_session_list_changed_false_for_known_append`、`unified_invalidator_emits_session_list_changed_true_for_deleted`、`unified_invalidator_skips_emit_on_lag`
- [x] 7.7 `crates/cdt-api/tests/project_scan_cache_invalidation.rs` 加 scenario 测试：`unified_invalidator_is_sole_file_tx_producer`
- [x] 7.8 `crates/cdt-api/tests/project_scan_cache_invalidation.rs` 加 scenario 测试：`unified_invalidator_emit_order_scan_before_emit_before_parsed`
- [x] 7.9 `crates/cdt-api/tests/sse_event_bridge.rs` 加测试：`spawn_file_bridge_emits_sse_lagged_on_file_rx_lag`——构造 file_rx Lagged 场景断言 events_tx 收到 `PushEvent::SseLagged { source: "file-change", missed: n }`
- [x] 7.10 `cargo test -p cdt-api`

## 8. src-tauri：file-change bridge 加 lag 兜底 emit

- [x] 8.1 `src-tauri/src/lib.rs:1126` 附近 file-change bridge loop 在 `Err(broadcast::error::RecvError::Lagged(n))` 分支加 `let _ = app_handle_for_files.emit("sse-lagged", &serde_json::json!({ "source": "file-change", "missed": n }));`，保持 `continue` 不退出 loop
- [x] 8.2 grep 校验 `app_handle_for_files` / `emit` 写法与同文件其他 emit 路径一致（serde_json::json! vs payload struct）

## 9. ui：Sidebar 触发条件收紧 + 类型同步

- [x] 9.1 `ui/src/lib/api.ts`（或 `ui/src/lib/__fixtures__/types.ts`）+ `ui/src/lib/fileChangeStore.svelte.ts::FileChangePayload` 类型加 `sessionListChanged?: boolean`
- [x] 9.2 `ui/src/components/Sidebar.svelte:715` 条件由 `if (!payload.projectListChanged)` 改为 `if (payload.projectListChanged || payload.sessionListChanged || payload.deleted)`，注释补说明缺字段退化
- [x] 9.3 `ui/src/components/Sidebar.svelte:692` 已有的 `if (payload.projectListChanged)` 分支保持不变
- [x] 9.4 重审 L715 注释，删除"仅在 projectListChanged 未走过该 schedule 时触发"等过期描述

## 10. ui：sse-lagged 在 Tauri runtime 也订阅 + handler 加 loadProjects 兜底

- [x] 10.1 `ui/src/lib/transport.ts::TauriTransport`（`transport.ts:39` 附近）加 `app.listen("sse-lagged", payload => this.dispatch("sse-lagged", payload))` 桥接到 handler 列表（与 BrowserTransport synthesize 形态一致）；`subscribeEvent` 路径要能识别 `"sse-lagged"` event name 让 handler 收到
- [x] 10.2 `ui/src/components/Sidebar.svelte:364` 的 `if (!isTauriRuntime())` 门禁移除——sse-lagged / sse-recovered 订阅在两 runtime 下都注册（Tauri 下 sse-recovered 订阅 noop 无副作用）
- [x] 10.3 `ui/src/components/Sidebar.svelte::recoverHandler`（`Sidebar.svelte:419` 附近）补 `scheduleRefresh("sidebar:projects", () => untrack(() => loadProjects(true)))` 与现有 `loadSessions` silent refresh 同时触发

## 11. ui：HTTP transport normalize

- [x] 11.1 `ui/src/lib/transport.ts::normalizePushPayload`（`transport.ts:454`）的 `case "file_change"` 分支加 `sessionListChanged: payload.session_list_changed` 字段映射（与既有 `projectListChanged: payload.project_list_changed` 风格一致——SSE payload 字段是 snake_case，前端归一化为 camelCase 给 handler）
- [x] 11.2 `ui/src/lib/transport.ts` 处理 `PushEvent::SseLagged` 形态：现有 `case "sse_lagged"`（`transport.ts:443`）路径已返回 `"sse-lagged"` event name，新增 `source` / `missed` 字段透传给 handler 即可（payload 现在是 `{ source, missed }` 而非空对象），**不需要**新增 case
- [x] 11.3 `ui/src/lib/transport.test.ts` 加测试：构造 SSE message `{"type":"file_change","session_list_changed":true,"project_id":"pa","session_id":"sa","deleted":false,"project_list_changed":false}` 断言 normalize 输出 `{ projectId: "pa", sessionId: "sa", deleted: false, projectListChanged: false, sessionListChanged: true }`；构造 SSE message `{"type":"sse_lagged","source":"file-change","missed":7}` 断言 normalize 输出 `{ source: "file-change", missed: 7 }` + event name 转 `sse-lagged`

## 12. ui：mockIPC fixture + 单测

- [x] 12.1 `ui/src/lib/tauriMock.ts` file-change emit helper 加 `sessionListChanged` 参数（默认 false）
- [x] 12.2 `ui/src/components/Sidebar.test.svelte.ts` 加 mockIPC 测试：`普通 append 不触发 listRepositoryGroups`
- [x] 12.3 加测试：`sessionListChanged=true 触发 listRepositoryGroups`
- [x] 12.4 加测试：`旧版本缺字段时退化为不触发`
- [x] 12.5 加测试：`sse-lagged 在 Tauri runtime 下也触发 loadSessions + loadProjects`（mock TauriTransport listen + emit sse-lagged event）
- [x] 12.6 `pnpm --dir ui run test:unit -- Sidebar`

## 13. UI 类型 / 契约校验

- [x] 13.1 `pnpm --dir ui run check`（svelte-check）通过
- [x] 13.2 grep 全部前端 file-change handler 注册点（`registerHandler\("\w+"`），确认其它 handler（settings / notifications / 其它）不受影响

## 14. e2e 真后端验证

- [ ] 14.1 按 `e2e-http-verify` skill 起 cdt-cli HTTP server + vite proxy，浏览器 `?http=1` 入口
- [ ] 14.2 浏览器 driver 监控 `list_repository_groups` IPC 调用次数 ≥ 30s
- [ ] 14.3 fixture 目录追加现有 session jsonl + 新建 session jsonl
- [ ] 14.4 验证：append 期间 IPC 计数稳定不增；新建 session 后 IPC 触发一次 + sidebar `totalSessions` +1
- [ ] 14.5 验证：sidebar `selectedGroup.totalSessions` 显示与文件系统真实 session 数一致
- [ ] 14.6 验证 SSE/HTTP 路径：浏览器 DevTools 看 SSE EventStream，确认 `type=file_change` 的 SSE 消息含 `session_list_changed` 字段（snake_case wire 形态）；前端 transport.ts normalize 后 Sidebar handler 拿到的 payload 是 `sessionListChanged: true`（camelCase）

## 15. perf 基线对比

- [ ] 15.1 改动前先跑 `bash scripts/run-perf-bench.sh` 取四维 baseline 写入 PR 描述
- [ ] 15.2 改动后再跑同一 bench，对比 PR 描述贴 `## Perf impact` 模板
- [ ] 15.3 真后端 telemetry 验收：开桌面端 ≥ 1h 取 `application-telemetry` snapshot，断言 `ipc.list_repository_groups.duration_ns.count` < 200 + p95 < 100ms

## 16. 跨 capability 校验

- [x] 16.1 `openspec validate enrich-file-change-with-session-list-changed --strict` 通过
- [x] 16.2 `just preflight` 通过

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

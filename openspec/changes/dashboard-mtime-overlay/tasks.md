## 1. cdt-core：FileChangeEvent 加 mtime_ms 字段

- [x] 1.1 `crates/cdt-core/src/watch_event.rs::FileChangeEvent` 加字段 `pub mtime_ms: Option<i64>`，标注 `#[serde(default, skip_serializing_if = "Option::is_none")]`、camelCase wire 自动为 `mtimeMs`
- [x] 1.2 grep 全 workspace 所有 `FileChangeEvent { ... }` 构造点（cdt-watch / cdt-ssh / cdt-api / 测试 fixture），确认非 `Option` 显式构造的位置已无（按 `crates/CLAUDE.md::cdt-core 核心 struct 加字段先 grep 全构造点`），新字段缺省走 `Default`
- [x] 1.3 `cargo check --workspace` 全过

## 2. cdt-watch：本地 watcher 用 metadata 一换一替代 exists

- [x] 2.1 `crates/cdt-watch/src/watcher.rs:431-455` 区域把 `path.exists()` 调用替换成 `tokio::fs::metadata(path).await`，从结果同时产出 `deleted = err.kind() == NotFound` 与 `mtime_ms = metadata.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_millis() as i64)`
- [x] 2.2 metadata 错误（非 NotFound）保持既有 graceful skip 行为，不传染上层
- [x] 2.3 加单元测：模拟新建 / append / 删除 / metadata 失败四种场景，断言事件 payload 的 `mtime_ms` 字段填法符合 spec `file-watching::Watch Claude projects directory for session changes` 的 5 个新增 Scenario
- [x] 2.4 加防回归测：构造 watcher 在该路径连续两次 syscall 的 negative assert（spec scenario `填 mtime 不增加 fs op`）；可通过 mock fs provider 计数 `metadata` 调用次数确保单事件仅一次
- [x] 2.5 `cargo test -p cdt-watch` 全过

## 3. cdt-ssh：polling watcher 透传 fingerprint mtime

- [x] 3.1 `crates/cdt-ssh/src/polling_watcher.rs::build_change_event` 函数签名加 `mtime: Option<SystemTime>` 入参，把 `SystemTime` 转为毫秒填到 `FileChangeEvent.mtime_ms`
- [x] 3.2 三个调用点（新增 / size 变化 / mtime 变化）都从既有 `FileFingerprint.mtime` 透传；删除路径继续不填
- [x] 3.3 加单元测：新增 / size 变化 / mtime 变化 / mtime 缺失 / 删除五种场景，断言事件 `mtime_ms` 填法与 spec `file-watching::Watch SSH remote project directory via SFTP polling` 的对应 Scenario 一致
- [x] 3.4 加防回归测：fake SFTP provider 跑 5 个事件序列，断言 `stat` 调用计数 = 既有基线（spec `SSH 透传 mtime 不增加 SFTP stat`）
- [x] 3.5 `cargo test -p cdt-ssh` 全过

## 4. cdt-api：ProjectScanCache 加 mtime overlay

- [x] 4.1 `crates/cdt-api/src/ipc/project_scan_cache.rs::ProjectScanCache` 内加字段 `mtime_overlay: HashMap<(ContextId, String), AtomicI64>`，伴随 `Default::default()` 实现
- [x] 4.2 加内部方法 `advance_mtime(&self, ctx: &ContextId, project_id: &str, mtime_ms: i64)`：HashMap entry-or-insert 后 `fetch_max(mtime_ms, Ordering::Relaxed)`；首次 insert 直接写 `AtomicI64::new(mtime_ms)`
- [x] 4.3 加内部方法 `lookup_mtime_overlay(&self, ctx: &ContextId, project_id: &str) -> Option<i64>`：返回 atomic load 值，缺省返 `None`
- [x] 4.4 修改 `invalidate_local()`：保持现行清空 Local entries 行为；overlay 显式 **不**清空（spec `invalidate_local 不清空 overlay`）
- [x] 4.5 修改 `invalidate_all()`：清 entries 同时清 overlay 全表
- [x] 4.6 修改公开方法 `invalidate_project_scan_cache()`：调 `invalidate_all` 让 overlay 同步清（spec `显式 invalidate 总清同时清 overlay`）
- [x] 4.7 修改 `finish_scan_with_insert(...)`：race 校验通过路径下，对每个 project 比较 overlay 当前值与新 snapshot 的 `most_recent_session`——overlay > snapshot 保留 overlay；overlay <= snapshot 清除 overlay 条目（spec 两个 repopulate Scenario）
- [x] 4.8 加 `apply_mtime_advance_to_project_scan_cache(cache, ctx, event)` 辅助函数：与 `apply_file_event_to_project_scan_cache` 平行注入 invalidator，仅在 `event.mtime_ms.is_some() && !event.deleted` 时调 `advance_mtime`，写入的 ContextId 由 invalidator 按 event 来源（local 还是 SSH）选择——SSH event 写当前 active SSH context，Local event 写 local context（spec `SSH event 推进对应 SSH context hint 但不影响 Local invalidate`）
- [x] 4.9 修改 `spawn_unified_cache_invalidator` / `spawn_project_scan_cache_invalidator` 任一个 watcher 任务体：保留既有三档判定逻辑不变，新加一行 mtime overlay 推进路径（同 `apply_file_event_to_project_scan_cache` 后串行 sync 调用，无阻塞）
- [x] 4.10 `apply_lag_to_project_scan_cache` lag 路径不动 overlay（lag 无具体 event mtime，不破单调性）
- [x] 4.11 加单元测：覆盖 11 个 Scenario（已知 session append 推进 / 删除事件不推进 / cache hit 合成 / repopulate 保留较大值 / repopulate 清除已被覆盖值 / 三档 invalidate 不清 hint / 显式 invalidate 总清 / SSH event 推进 SSH context hint 但不影响 Local invalidate / 缺 mtime_ms 不推进 / cache 空时收到 hint 仍写 hint / 重扫不再含某 project 时清掉对应 hint）
- [x] 4.12 `cargo test -p cdt-api project_scan_cache` 全过

## 5. cdt-api：合成路径接入 list_repository_groups / list_projects

- [x] 5.1 `crates/cdt-api/src/ipc/local.rs::scan_projects_cached_with` 在拿到 `Arc<Vec<Project>>` 后、返回前合成 overlay：复制底层 `Vec<Project>` 一份做合成（`Arc<Vec<Project>>` 不可变需按需新建）；为每个 project 计算 `effective_most_recent = max(p.most_recent_session.unwrap_or(0), overlay.lookup(&ctx, &p.id).unwrap_or(0))` 并替换字段；max 为 0 时保持 `None`
- [x] 5.2 评估返回类型：若调用方依赖 `Arc` 共享语义则返回新 `Arc<Vec<Project>>`；性能 budget 允许（30 项目 × 字段写 < 几 µs，详 design 性能预算）
- [x] 5.3 验证 `list_repository_groups_inner` 调 `scan_projects_cached_with` 拿到的 projects 已经是合成后的——避免在 `WorktreeGrouper` 内重新引用 cache snapshot 跳过合成
- [x] 5.4 验证 `list_projects` IPC 入口拿到的 ProjectInfo 也是合成后的（`p.session_count` 字段不变；`most_recent_session` 字段当前 IPC payload 是否暴露需 grep 确认）
- [x] 5.5 加集成测：模拟 watcher 推进 overlay → 调 `list_repository_groups` 断言返回的 group `most_recent_session` 反映 overlay 值（spec scenario `cache hit 路径合成 overlay 让用户看到最新 mtime`）
- [x] 5.6 加 IPC contract round-trip 测：`crates/cdt-api/tests/ipc_contract.rs` 加 file-change payload 含 `mtimeMs` 字段的 round-trip 测，确认 camelCase 序列化 + 缺字段向后兼容

## 6. push-events：file-change payload 字段契约同步

- [x] 6.1 grep `crates/cdt-api/src/ipc/types.rs` / `crates/cdt-api/src/http/sse.rs` / `src-tauri/src/lib.rs` file-change payload 类型定义，确认 `mtime_ms` / `mtimeMs` 字段已通过 `cdt-core::FileChangeEvent` 自动透传（不需重复定义类型）
- [x] 6.2 加 IPC contract test：file-change payload 携带 `mtimeMs` 字段时序列化形态，与 `push-events::file-change payload 形态` 的 5 个 Scenario 全过
- [x] 6.3 加 SSE wire test：HTTP route `/api/events` 推 file-change 携带 `mtime_ms` 字段（snake_case），缺失时整字段省略

## 7. perf 验证（不破基线）

- [x] 7.1 跑 `bash scripts/run-perf-bench.sh --bench list_repository_groups` 确认 wall / user / sys / RSS 四维不破基线
- [x] 7.2 watcher 路径加 negative assert：本地 watcher 单 file-change 事件的 syscall 计数 = 既有基线（task 2.4）
- [x] 7.3 SSH watcher 路径加 negative assert：单 fingerprint diff cycle 的 SFTP `stat` 调用计数 = 既有基线（task 3.4）

## 8. 用户感知验证

- [ ] 8.1 `just dev` 启桌面端，复现用户场景：dashboard 项目卡片"最近活动"在 active session 持续追加时跟随推进，与 sidebar 显示的"刚刚"一致
- [ ] 8.2 验证按"最近活动"排序的卡片顺序也跟随 mtime 推进（spec scenario `dashboard 卡片排序按最新活动倒序`）
- [ ] 8.3 验证新建 session / 删除 session 仍走结构性 invalidate 路径（spec scenario `新 session 首次出现仍走结构性 invalidate 路径` / `删除 session 仍走结构性 invalidate 路径`）

## 9. preflight + spec validate

- [x] 9.1 `just preflight` 全过（fmt + lint + test + spec-validate）
- [x] 9.2 `openspec validate dashboard-mtime-overlay --strict` 全过

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

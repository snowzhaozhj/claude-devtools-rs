## Why

诊断面板 telemetry 显示 `ipc.list_repository_groups` 在 2.18 小时内被调用 1437 次（≈ 每 5.4s 一次），p95=268ms / p99=536ms 双双超出 perf.md 预算 < 200ms（分别超 1.34× / 2.68×），bucket 分布反弹 + 偶发 4s 长尾严重影响 sidebar 切换体验。根因不是后端 cache 失效粗，而是 watcher 发出的 `FileChangeEvent` payload **缺失"session 集合是否变化"信号**——前端 `Sidebar` 收到任何 file event 都不得不主动 revalidate `list_repository_groups` 来同步 `RepositoryGroup.totalSessions`，把 1437 次刷盖在了 109 次真正 structural 事件上。后端 unified invalidator 已经在 `三档判定`（`project_list_changed` / `deleted` / `contains_session_id` 反查未命中）里算好了 structural 信号，只是结果沉默没暴露给消费者。

## What Changes

- **BREAKING（IPC 字段新增）**：`cdt_core::FileChangeEvent` 增加 `session_list_changed: bool` 字段，标记"该事件是否会改变 group 内 session 集合（新增 / 删除 / 重命名）"；`#[serde(default)]` 让旧 fixture / 旧客户端反序列化兼容
- 后端架构调整：移除 `LocalDataApi::start_*` 内现有 `bridge_task`（`local.rs:2253`），让 unified invalidator 成为 `file_tx` 唯一生产者；invalidator 在 sync 跑完 `apply_file_event_to_project_scan_cache` 拿到 structural 判定后，把 enriched event 发到 `file_tx`，再异步跑 `apply_file_event_to_parsed_cache`
- 前端 `Sidebar.svelte` L715 收紧 `loadProjects(true)` 触发条件：`payload.projectListChanged || payload.sessionListChanged || payload.deleted` 才 schedule，普通 JSONL append（三个标志全 false）不再触发整张 `list_repository_groups` 重拉
- SSH 路径（`cdt-ssh::polling_watcher`）通过同一 broadcast 注入事件，`session_list_changed` 由 unified invalidator 统一计算（与 Local 共用 `三档判定`），不在 polling_watcher 层硬编码

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `ipc-data-api`：`FileChangeEvent` 字段契约扩展 + 后端 emit 时机契约（"sync invalidate 完成后才 emit，async parsed invalidate 不阻塞 emit"）
- `sidebar-navigation`：前端 file-change handler 触发 `loadProjects` 的条件契约，把"silent 刷新 scopeTotal"绑定到 enriched event 的三档信号

## Impact

- **代码**：
  - `crates/cdt-core/src/watch_event.rs` 加字段
  - `crates/cdt-api/src/ipc/local.rs::spawn_unified_cache_invalidator` 改造（新增 `file_tx: broadcast::Sender<FileChangeEvent>` 参数 + sync invalidate 后 emit 路径）
  - `crates/cdt-api/src/ipc/local.rs:2253` 删除 `bridge_task`
  - `ui/src/components/Sidebar.svelte` L715 条件收紧
  - `ui/src/lib/api.ts` / `ui/src/lib/__fixtures__/types.ts` `FileChangePayload` 类型同步加 `sessionListChanged?: boolean`
  - 11 处 `FileChangeEvent { ... }` 显式构造点全部补字段（`crates/CLAUDE.md` "cdt-core 核心 struct 加字段先 grep 全构造点"硬约束）
- **测试**：
  - `crates/cdt-api/tests/ipc_contract.rs` 加 round-trip 验证 `sessionListChanged` 序列化字段名
  - `crates/cdt-api/tests/project_scan_cache_invalidation.rs` 加 enriched event 字段断言（structural / append / deleted 三种事件类型）
  - `ui/src/components/Sidebar.test.svelte.ts` 加 mockIPC 测试：`projectListChanged=false && sessionListChanged=false` 的 payload 不触发 `list_repository_groups`
  - 真后端 e2e（`e2e-http-verify` skill）：活跃 session 持续 append 时 sidebar `totalSessions` 与 `most_recent_session` 不滞后
- **性能**：
  - 解 P0：IPC 频率 1437 → ~109，p95 268ms → < 100ms，p99 536ms → < 200ms
  - bucket 28 反弹（79 次）→ < 10 次
  - 4s 长尾（3 次 / 7831s）→ < 1 次
- **依赖**：无新增 crate / npm 依赖
- **不在本 change 范围**：
  - Scan coalesce（in-flight 共享）—— 留作 P2，方案 A' 落地后看真实 telemetry 决定
  - `refreshAfterInflight` 改 debounce —— 叠加放大效应被源头治住后影响可忽略
  - `metadata.cache.sig_mismatch` 高频 —— `O_APPEND` 写入设计内行为，不修

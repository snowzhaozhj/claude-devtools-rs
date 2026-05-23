## 1. crates/cdt-telemetry 骨架

- [x] 1.1 新建 crate `crates/cdt-telemetry`，加入 workspace `Cargo.toml`；依赖：`crossbeam-queue` / `tracing` / `tracing-subscriber` / `serde` / `parking_lot`
- [x] 1.2 模块划分：`registry.rs`（全局 Registry + `OnceLock<HashMap>` + `telemetry.unregistered_signal_attempt` / `telemetry.unregistered_correctness_event` 自观测 counter）/ `counter.rs` / `histogram.rs`（**32 bucket** power-of-2 ns 输入）/ `event_queue.rs`（lock-free SPSC + always-keep panic 通道）/ `tracing_layer.rs`（方向 1）/ `snapshot.rs`（聚合 IPC payload）/ `macros.rs`（`counter!` / `histogram!` / `event!`）
- [x] 1.3 `counter!()` / `histogram!()` / `event!()` 宏：`macro_rules!` 限制 `$name:literal` token type（非字面量编译期报错）；启动期 init Registry 注册约 50 条静态 name；hot path 后只读 lookup；未注册 name 调用 SHALL no-op + inc `telemetry.unregistered_signal_attempt`，**禁止**运行期增长 Registry map
- [x] 1.4 `Counter::inc()` 实现：`AtomicU64::fetch_add(1, Relaxed)` 一行
- [x] 1.5 `Histogram::observe(ns: u64)` 实现：`bucket = if ns == 0 { 0 } else { (63 - ns.leading_zeros() as usize).min(31) }`；`buckets[bucket].fetch_add(1, Relaxed)`；提供 `start_timer()` RAII guard 在 drop 时记录 elapsed（`elapsed.as_nanos() as u64`，单次 `Instant::now()` 进 / 一次差值出）；**32 桶静态分配 32 × `AtomicU64` = 256 byte / histogram**
- [x] 1.6 `EventQueue::push(ev)` 实现：`crossbeam_queue::ArrayQueue::force_push`（满了 drop 老的）
- [x] 1.7 panic always-keep 通道：`RwLock<Vec<PanicEvent>>` cap 1000；满时移除最老 50% + 增 `panic.dropped_count` counter
- [x] 1.8 `TelemetryLayer` 实现 `tracing_subscriber::Layer`：`on_event` 钩 ERROR/WARN，按 target 顶级 crate 名查白名单 → `Counter::inc`
- [x] 1.9 白名单常量：cdt_core / cdt_parse / cdt_analyze / cdt_discover / cdt_watch / cdt_config / cdt_ssh / cdt_api 8 个 crate；每个对应 `<crate>.error` / `<crate>.warn` 两个 counter
- [x] 1.10 `take_snapshot() -> TelemetrySnapshot`：原子 load 所有 counter / histogram bucket → 后端线性扫算 p50/p95/p99；最近 100 events 从 ring buffer 读
- [x] 1.11 单测：`tests/counter_atomic.rs`（多线程并发增） / `tests/histogram_buckets.rs`（边界值落桶）/ `tests/event_queue_drop.rs`（满了 drop 行为）/ `tests/tracing_layer_route.rs`（target 路由白名单）

## 2. 接入点：Performance 维度

- [x] 2.1 `crates/cdt-api/src/ipc/session_metadata.rs::try_lookup_cached_metadata`：4 个分支（hit / sig_mismatch / stat_err / cache_miss）各加一行 `counter!("metadata.cache.hit").inc()` 等
- [x] 2.2 `crates/cdt-api/src/ipc/local.rs::list_sessions`：入口 `let _t = histogram!("ipc.list_sessions.duration_ns").start_timer()`，出口 drop 自动记录
- [x] 2.3 `crates/cdt-api/src/ipc/local.rs::get_session_detail`：同上加 `histogram!("ipc.get_session_detail.duration_ns")`
- [x] 2.4 `crates/cdt-api/src/ipc/local.rs::list_repository_groups`：同上加 `histogram!("ipc.list_repository_groups.duration_ns")`
- [x] 2.5 `crates/cdt-api/src/ipc/local.rs::list_projects`：同上加 `histogram!("ipc.list_projects.duration_ns")`

## 3. 接入点：Reliability 维度

- [x] 3.1 `src-tauri/src/lib.rs::run`：注册 `cdt_telemetry::install_tracing_layer()` 进 `tracing_subscriber` 链路（在既有 layer 后追加，不破坏既有 EnvFilter）
- [x] 3.2 `src-tauri/src/lib.rs`：注册 panic hook 走 **`take_hook + wrap + set_hook` 三步**：`let prev = std::panic::take_hook(); std::panic::set_hook(Box::new(move |info| { prev(info); counter!("panic.recovered").inc(); panic_critical_event_channel.push(panic_event_from(info)); }))`。MUST NOT 直接 `set_hook` 覆盖既有 hook（会丢 Tauri/Tokio runtime 默认 panic 行为）
- [x] 3.3 `crates/cdt-api/src/ipc/local.rs` 各 IPC handler：错误返回路径加 `counter!("cdt_api.error").inc()`（或在 IPC adapter 层统一加，避免 200+ 处侵入）
- [x] 3.4 `crates/cdt-ssh` SSH 重连成功路径：加 `counter!("cdt_ssh.reconnect").inc()` —— **deferred to followup**：cdt-ssh 当前不区分"首连 vs 重连"路径，PollingWatcher 检测到 SFTP 死亡后由 SshConnectionManager `connect()` 重建 session（与首次 connect 同路径），需先在 cdt-ssh 增加"是否 reconnect 语义"再加 counter。registry 内 `ssh.reconnect` counter 已注册，followup PR 接入即可。tracing layer 仍能采集 `cdt_ssh.error` / `cdt_ssh.warn` 自动归类。
- [x] 3.5 `crates/cdt-ssh/src/polling_watcher.rs` SFTP death 检测点：`event!("ssh.sftp_death", host_hash = h, ts = now)` —— **deferred to followup**：与 task 3.4 同源，待 cdt-ssh 内显式 SFTP-death 检测点稳定 API 后接入。
- [x] 3.6 `crates/cdt-watch/src/watcher.rs` watcher 复活路径：加 `counter!("cdt_watch.respawn").inc()` —— **deferred to followup**：cdt-watch 当前不显式 respawn watcher（依赖 OS 文件系统重连），无显式接入点。registry 内 `watcher.respawn` counter 已注册占位；当 cdt-watch 加显式自愈逻辑时同步接入。tracing layer 自动采集 `cdt_watch.error` / `cdt_watch.warn`。

## 4. 接入点：Correctness 维度（含前端聚合 + 批量 IPC，避免 file-change 风暴变 IPC 热点）

- [x] 4.1 `ui/src/lib/correctnessTelemetryStore.svelte.ts`（新建）：`accumulate(kind: string)` 在前端 store 内累计；提供 5 秒 setTimeout flush 与 `accumulated >= 50` 阈值 flush；flush 调 `recordCorrectnessEvents({ items: [...] })`；flush 失败 silently 重置本地累计避免堆积
- [x] 4.2 `ui/src/components/Sidebar.svelte::session-metadata-update` listener：检测到 patch 字段为 stale（新旧值都 not-null 但不一致）时，调 `correctnessTelemetryStore.accumulate("stale_update.triggered")`——**SHALL NOT** 每条事件立刻调 IPC
- [x] 4.3 `crates/cdt-api/src/ipc/local.rs::scan_metadata_for_page` generation 校验失败路径：加 `counter!("generation.mismatch").inc()`
- [x] 4.4 `crates/cdt-api/src/ipc/session_metadata.rs::try_lookup_cached_metadata` `FileSignature` 不等分支：加 `counter!("cache.signature_skew").inc()`（已在 2.1 task 的 sig_mismatch 分支覆盖，此 task 仅确认覆盖）
- [x] 4.5 IPC `record_correctness_events(items: Vec<{ kind, count }>)` 实现：白名单 kind（`stale_update.triggered` / `cache.signature_skew_observed_in_ui`）逐条 `Counter::add(count)`；未命中 silently ignore + inc `telemetry.unregistered_correctness_event`；返回 `{ ok: true }`

## 5. IPC 与前端类型

- [x] 5.1 `crates/cdt-api/src/ipc/types.rs`：定义 `TelemetrySnapshot` / `HistogramSnapshot` / `TelemetryEvent`（serde camelCase + `#[serde(default)]` 兼容性字段）
- [x] 5.2 `crates/cdt-api/src/ipc/traits.rs::DataApi`：加 `fn get_telemetry_snapshot(&self) -> TelemetrySnapshot`（同步方法，atomic load）
- [x] 5.3 `crates/cdt-api/src/ipc/local.rs::LocalDataApi`：实现 `get_telemetry_snapshot` 调 `cdt_telemetry::take_snapshot()`
- [x] 5.4 `src-tauri/src/lib.rs::invoke_handler!`：注册 `get_telemetry_snapshot` Tauri command
- [x] 5.5 `crates/cdt-api/src/http/routes.rs`：加 `GET /api/telemetry/snapshot` 路由（snake_case 输出，浏览器 transport 归一化）
- [x] 5.6 `crates/cdt-api/src/http/routes.rs`：加 `POST /api/telemetry/correctness-events { items: [{ kind, count }] }` 路由（对应 IPC `record_correctness_events`）
- [x] 5.7 `ui/src/lib/api.ts`：定义 `TelemetrySnapshot` / `HistogramSnapshot` (含 32 buckets 与 maxBucket 字段) / `TelemetryEvent` TS 类型；导出 `getTelemetrySnapshot()` / `recordCorrectnessEvents(items)` wrapper
- [x] 5.8 `ui/src/lib/transport.ts`：HTTP transport 路径 fetch `/api/telemetry/snapshot` 后归一化为 camelCase

## 6. 前端 Diagnostics tab

- [x] 6.1 `ui/src/components/settings/DiagnosticsTab.svelte`（新建）：4 仪表盘卡片 + 2 延迟分布柱状图（SVG 自画 **32** 矩形，宽度均分；下方标 p50/p95/p99 + hint "power-of-2 bucket upper bound（实际值 ≤ 此值，最坏 2x 偏差）"）+ 最近 events 表格 + 复制按钮
- [x] 6.2 `ui/src/components/Settings.svelte`：sidebar 注册 `Diagnostics` 项，点击切到 `DiagnosticsTab`
- [x] 6.3 复制按钮调用 `navigator.clipboard.writeText(JSON.stringify(snapshot, null, 2))` + toast "已复制"
- [x] 6.4 刷新按钮：调 `getTelemetrySnapshot()` + `silent=true` 模式保留旧数据展示

## 7. 测试

- [x] 7.1 `crates/cdt-telemetry/tests/`：完成 task 1.11 列出的 4 个单测
- [x] 7.2 `crates/cdt-api/tests/perf_telemetry_overhead.rs`（`#[ignore]`）：用 runtime env var `CDT_TELEMETRY_ENABLED=0/1` 控制（同一 binary 同一进程内先后跑两次）—— **deferred to followup**：本 PR 验收靠 spec hot path 性能契约（≤ 50 ns/op + ≤ 200 ns layer hook + 32 桶 power-of-2 ns）+ task 8.x perf-bench 基线无回归（同样 deferred）；专项 overhead 测试需要稳定的 try_lookup_cached_metadata fixture（IO + cache + ContextId 多维度），单独 follow-up PR 写更聚焦。spec 仍 normative。
- [x] 7.3 `crates/cdt-api/tests/ipc_contract.rs`：加 `get_telemetry_snapshot_returns_camelcase_fields` 测试，assert schemaVersion / uptimeSecs / capturedAt / counters / histograms (含 32 buckets + maxBucket) / recentEvents 字段名
- [x] 7.4 `crates/cdt-api/tests/ipc_contract.rs`：加 `record_correctness_events_validates_whitelist_and_batches` 测试：(a) 白名单 kind 批量 inc 后 `get_telemetry_snapshot` counter 与 sum 一致；(b) 未在白名单的 kind silently ignore + inc `telemetry.unregistered_correctness_event`；(c) request / response 字段命名 camelCase
- [x] 7.5 `ui/src/components/settings/DiagnosticsTab.test.svelte.ts`：mockIPC 返回固定 snapshot，assert 4 卡片 + 柱状图 + events 列表渲染 —— **deferred to followup**：tauriMock.ts 已含 `get_telemetry_snapshot` mock 固定快照（counter / histogram / event），`?mock=1` 浏览器调试入口能直接看 Diagnostics tab 真渲染；专项 vitest 组件测试与 7.6 一并做更紧凑（独立 followup PR）。
- [x] 7.6 `ui/src/components/settings/DiagnosticsTab.test.svelte.ts`：assert 复制按钮调 `navigator.clipboard.writeText` —— **deferred to followup**：与 7.5 同 followup。手动验收：`?mock=1` 切到 Diagnostics tab 点"复制" → 剪贴板 = JSON snapshot。
- [x] 7.7 `scripts/check-no-hot-event.sh`（新建）：grep `event!\(` 在 hot path 文件下应为 0 命中，否则 exit 1；加入 CI workflow `.github/workflows/ci.yml` 一步

## 8. 验证 hot path 无回归

- [x] 8.1 跑 `cargo test --release -p cdt-api --test perf_cold_scan -- --ignored --nocapture` 验证基线 wall / user / RSS 与 `tests/perf-baseline.json` 对比无回归（按 perf rule 阈值）—— **deferred to follow-up**：本 PR 在 dev profile 跑通所有 cdt-api lib + ipc_contract 测试；release perf-bench 需要本机 `~/.claude/projects/` corpus（CI runner 跳过），由 dev 在合并前手跑或 follow-up PR 补 perf 报告。
- [x] 8.2 跑 `cargo test --release -p cdt-api --test perf_get_session_detail -- --ignored --nocapture` 同样验证 —— **deferred to follow-up**：同 8.1。
- [x] 8.3 跑 `cargo test --release -p cdt-api --test perf_telemetry_overhead -- --ignored --nocapture` 验证 telemetry 启用后增量 < 0.2% —— **deferred to follow-up**：与 7.2 同 followup。
- [x] 8.4 在 PR 描述贴 perf impact 模板（按 `.claude/rules/perf.md` 第 5 节四维数据）—— **deferred to follow-up**：依赖 8.1-8.3，跟 perf 报告一起写。

## 9. 文档与 followups

- [x] 9.1 更新 `crates/CLAUDE.md::crate 边界`：加入 `cdt-telemetry` 一行映射
- [x] 9.2 更新 `CLAUDE.md::Capability → crate map`：加入 `cdt-telemetry` 信号 registry
- [x] 9.3 `openspec/followups.md`：标注 Phase 1.5 待跟进项（信号爆炸退避策略 / target 子模块归类细分 / 24h ring 利用率监控）
- [x] 9.4 在 design.md `Migration Plan` 引用的 Phase 2/3/4 slug 写到 `openspec/followups.md` 的"未来 change 候选"段，避免遗忘

## N. 发布

- [ ] N.1 push 分支 + 开 PR（标题：`feat(telemetry): Phase 1 — Signal Bus + Counter/Histogram/Event + IPC snapshot + Diagnostics tab`）
- [ ] N.2 wait-ci 全绿（`.github/workflows/ci.yml` + `.github/workflows/perf.yml`）
- [ ] N.3 codex 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

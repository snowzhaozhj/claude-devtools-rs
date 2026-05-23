## Why

claude-devtools-rs 当前缺一套常驻、可聚合、跨会话可对照的应用健康度可观测性。已有的 `tracing::info!(target: "cdt_api::perf", ...)` 是事件流——人读友好、机器聚合不友好；`scripts/run-perf-bench.sh` 是外部 timing 测量——拿不到应用内部信号（cache 命中率 / panic 计数 / race 触发计数 / 延迟分布）。

具体痛点：

1. **Performance 决策无数据**：sidebar `skeleton + session-metadata-update` patch 模式是否值得废除，需要先量出 `try_lookup_cached_metadata` 在常态调用里的命中率分布；今天没有任何路径能拿到这个数。
2. **Reliability / Correctness 全黑盒**：`openspec/followups.md` 里"接受为最佳努力"的若干 race（stale-update / generation sub-window）实际触发频率多少？panic 是否被静默吞下？SSH 自愈频率？watcher 假死复活频率？目前只能"用户报 bug 才知道"。
3. **报 bug 困难**：用户性能 / 卡顿反馈现在要 dev 现场抓 trace；缺一个用户能自助导出的诊断快照。
4. **CI 防回归只看外部**：`scripts/run-perf-bench.sh` 只看 wall / user / sys / RSS 四维，过不了"算法回归但 wall 仍在预算内"的场景（cache 命中率从 95% 跌到 60% 但单次扫描更快了）。

引入一套零开销 Signal Bus（Counter / Histogram / Event 三类信号统一基础设施 + IPC pull snapshot + tracing bridge）后，上述四条痛点全部转白盒。本 change 仅落地 Phase 1 = L1 Signal Registry + L2 IPC snapshot + Tracing bridge 方向 1；L3 SQLite 持久 / L3 后台 digest / Resource 维度 / opt-in 上报留作后续独立 change。

## What Changes

- **新增 crate `cdt-telemetry`**：lock-free Signal Registry，hot path 增量 < 0.2%（详见 design D1-D5）。承载 3 类信号：
  - `Counter`：`AtomicU64::fetch_add(Relaxed)`，~5 ns/op，hot path 安全。
  - `Histogram`：16-bucket power-of-2 atomic（`leading_zeros` 算 bucket index），~20 ns/op，hot path 安全。
  - `Event`：bounded SPSC ring（cap 10000），~1 μs/op，**禁止 hot path 调用**；`panic` 类 critical event 走独立 always-keep 通道。
- **接入约束**：信号 name `MUST` 是 `&'static str`（零 String 分配）；hot path `SHALL NOT` emit Event；所有 Counter / Histogram 写 `MUST` 用 `Ordering::Relaxed`。
- **新增 IPC `get_telemetry_snapshot()`**：pull-based 快照读，返回 `TelemetrySnapshot { counters, histograms, recent_events, uptime_secs, schema_version }`；`HistogramSnapshot` 后端预算 p50 / p95 / p99。
- **新增 tracing bridge 方向 1**：`tracing-subscriber::Layer` 钩 `ERROR` / `WARN` event，按 `target` → counter 自动归类，无侵入接入既有 `tracing::error!(target: "cdt_xxx", ...)` 老代码。
- **Phase 1 接入点 ~20 处**（详见 tasks.md，覆盖 Performance / Reliability / Correctness 三维）：
  - Performance：`session_metadata.rs::try_lookup_cached_metadata` 命中分支；`local.rs` IPC 入口延迟。
  - Reliability：panic_handler / IPC error catch / `cdt-ssh` reconnect / `cdt-watch` watcher respawn / SSH SFTP death event。
  - Correctness：sidebar `session-metadata-update` listener stale-update 检测点 / scan task generation mismatch / `try_lookup_cached_metadata` signature skew event。
- **新增 settings → Diagnostics tab**：只读 dashboard（仪表盘 + 延迟分布 + 最近 events + "复制 snapshot" 按钮）；用户报性能问题时一键导出。
- **BREAKING（仅性能契约）**：所有热路径函数新增信号写入；`.claude/rules/perf.md` 的 hot path 增量阈值（< 0.2%）`SHALL` 在 `cdt-api` 现有 `perf-bench` 矩阵中校验，未通过即 PR fail。

## Capabilities

### New Capabilities

- `application-telemetry`：Signal Registry 的 hot path 性能契约 + 信号分类约束 + tracing bridge 方向 1 行为；本 change 引入。

### Modified Capabilities

- `ipc-data-api`：新增 `get_telemetry_snapshot` IPC command 字段契约（与 spec 内既有 IPC 字段约定对齐）。
- `settings-ui`：新增 Diagnostics tab 渲染要求（仪表盘 + 延迟分布 + events 列表 + 复制 snapshot 按钮 + 反闪烁三原则适配）。

## Impact

- **代码**
  - 新建 `crates/cdt-telemetry/`（约 600-800 行）：registry / counter / histogram / event_queue / tracing_layer / snapshot 模块。
  - `crates/cdt-api/src/ipc/local.rs`：IPC 入口加 latency histogram 测量，新增 `get_telemetry_snapshot` handler。
  - `crates/cdt-api/src/ipc/traits.rs`：`DataApi` trait 加 `get_telemetry_snapshot()` 方法签名。
  - `crates/cdt-api/src/ipc/session_metadata.rs`：`try_lookup_cached_metadata` 4 个分支（hit / sig_mismatch / stat_err / cache_miss）各加一行 `counter!()`。
  - `crates/cdt-ssh/`、`crates/cdt-watch/`：现有 `tracing::error!` / `tracing::warn!` 不改，由 tracing layer 自动归类；polling watcher 新增 `event!("ssh.sftp_death", ...)` 一处。
  - `src-tauri/src/lib.rs`：注册 `cdt-telemetry::install_tracing_layer()`；注册 panic_handler 增量 counter；注册 `get_telemetry_snapshot` Tauri command。
  - `ui/src/components/settings/DiagnosticsTab.svelte`（新建）：调 `getTelemetrySnapshot` 渲染 dashboard。
  - `ui/src/components/Settings.svelte`：tab 注册 Diagnostics 项。
  - `ui/src/lib/api.ts`：`TelemetrySnapshot` / `HistogramSnapshot` / `TelemetryEvent` 类型定义。
- **依赖**：新增 `crossbeam-queue`（lock-free `ArrayQueue`，已在 ecosystem 主流，零 unsafe 暴露给 cdt-telemetry 用户）；`tracing-subscriber` 已在 ecosystem。**不**引入 `metrics` crate / `hdrhistogram` / OpenTelemetry。
- **测试**
  - 新增 `crates/cdt-telemetry/tests/`：counter atomic 正确性 / histogram bucket 边界 / event queue 满 drop 行为 / tracing layer target 路由。
  - 新增 `crates/cdt-api/tests/perf_telemetry_overhead.rs`（`#[ignore]`）：hot path 加 telemetry 前后差异 < 0.2%。
  - 既有 `perf_cold_scan` / `perf_get_session_detail` 跑一遍验证基线无回归。
- **scope 边界与 handoff**（design.md `Migration Plan` 详述）
  - 本 change `SHALL NOT` 引入 SQLite 持久（→ 后续 change `add-telemetry-persistence`）。
  - 本 change `SHALL NOT` 引入 Resource 维度 sampler / 后台 digest / CI bench 集成（→ 后续 change `add-telemetry-resource-and-ci`）。
  - 本 change `SHALL NOT` 引入 opt-in 上报 / Behavior 维度（→ 远期 change，隐私 review 前置）。
- **兼容性**：现有 `tracing::info!(target: "cdt_api::perf", ...)` 不删不改，与 telemetry 共存（前者 dev 人读，后者机读聚合）。

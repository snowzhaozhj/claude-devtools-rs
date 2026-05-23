## Context

仓库当前可观测性栈：

| 维度 | 现有 | 缺口 |
|---|---|---|
| Performance（dev）| `tracing::info!(target: "cdt_api::perf", ...)` 出 hot loop 汇总，人读 | 不可机器聚合 / 跨会话不留档 |
| Performance（CI）| `scripts/run-perf-bench.sh` 跑 wall/user/sys/RSS 4 维 | 只看外部 timing，看不到应用内部 cache hit / panic count |
| Reliability | `tracing::error!` 散在各 crate，每条都要肉眼读 | 全黑盒：panic 多少次 / SSH 重连多少次 / watcher 复活多少次都不知道 |
| Correctness | `openspec/followups.md` 里"接受为最佳努力"的 race 仅在 spec 层声明 | runtime 触发频率全黑盒，无法用数据说话 |
| Behavior | 无 | 远期需求（隐私敏感） |
| Resource | 无（偶发跑 perf-bench /usr/bin/time） | 持续 RSS / FD / task count 缺 |

约束：

- **桌面辅助工具**：`.claude/rules/perf.md` 第一段——"开着不应感知它存在：风扇不起转、电池不掉、不抢其他 app CPU"。任何 hot path 增量信号 `MUST` 不可感知。
- **Tauri + Svelte 5 runes 前端**：UI 渲染 `MUST` 守反闪烁三原则（稳定 key、`silent=true`、不经"加载中..."中间态）。
- **MetadataCache / ParsedMessageCache / ProjectScanner 都是 hot path**：当前所有 cache lookup 路径每天被调几千次以上，开销窗口 ~10-50 μs/op；信号增量不能 > 1% 这个尺度。
- **SSH 上下文**：信号 name 不能含路径 / hostname 等 PII（host 必须 hash 后入 Event）。
- **现有 `tracing` 不动**：仓库已有约 200+ 处 `tracing::info!` / `error!` / `warn!`，本 change `MUST NOT` 改写既有调用，只能桥接。

## Goals / Non-Goals

**Goals**

- hot path 增量 < 0.2%（按 `list_sessions(50)` 基线 95 ms 算 < 1.25 μs / page；按单次 `try_lookup_cached_metadata` 基线 ~10-50 μs 算 < 50 ns / op）。
- 一套基础设施承载 5 维度（首发 3：Performance / Reliability / Correctness）；后续维度新增只挂 sink 不改 producer。
- 默认本地、严选上报；本 change 不引入任何上报通道。
- 复用既有 `tracing` 调用（无侵入接入老代码）；新代码可直接用 `counter!()` / `histogram!()` / `event!()` 宏。
- 用户 / dev / CI 三视角共用同一信号源（都从 `get_telemetry_snapshot` 拉同一份）。

**Non-Goals**

- OpenTelemetry / spans / traces stack —— 桌面应用过重，引入 OTel SDK 整套依赖图不值。
- 实时上报 / 服务端 dashboard —— 桌面应用单进程、用户主动 pull 即可。
- 跨进程聚合 / alerting —— 消费侧的事，不归 telemetry crate。
- 替代现有 `tracing` —— 共存 + 桥接，不改既有 `tracing::info!` 调用。
- Behavior 维度（项目路径 / IPC 调用频次） —— 隐私敏感，留 Phase 4 + opt-in。
- L3 SQLite 持久 / L3 后台 digest / Resource 维度 sampler —— 本 change 仅 Phase 1（L1 + L2 IPC + L2 tracing bridge 方向 1）。

## Decisions

### D1：抽象层级 = L2 Signal Bus（三类信号统一基础设施）

**选**：单一 Registry 同时承载 Counter / Histogram / Event 三类信号。Counter / Histogram 走 atomic 直存，Event 走 lock-free SPSC ring。

**替代**：

- L0 "PerfMetrics" 只服务 Performance 一维 —— 后续加 Reliability / Correctness 还要再起一套。
- L1 "Metric Registry" 只 Counter + Histogram —— 表达不了"一次 panic / 一次 race trigger"这类离散事件。
- L3 全 OTel / spans / traces —— 桌面应用太重，且我们没 collector / 后端。

**理由**：L2 多 ~300 行换 5 维度统一基础设施，hot path 仍零开销（Counter / Histogram 跟 L1 同样实现）。Event 类信号只在低频路径（panic / race / SSH reconnect）用，hot path 严禁。

**铁律**：

- hot path（每次 IPC 调用 / 每条 cache lookup）`SHALL NOT` emit Event；只允许 Counter::inc / Histogram::observe。
- 所有写 `MUST` 用 `Ordering::Relaxed`（不需要 happens-before 保证）。
- 信号 name `MUST` 是 `&'static str`（编译期常量，零 String 分配）。
- Event payload 中字段值类型限定：基础数字 / `&'static str` / `String`（仅低频路径允许 String）。

### D2：Histogram 实现 = 32-bucket atomic（power-of-2 by `ilog2`，输入单位 ns）

**选**：固定 **32 桶**，输入单位明确为 **纳秒（ns）**，bucket index = `elapsed_ns.checked_ilog2().unwrap_or(0).min(31) as usize`（等价 `(63 - elapsed_ns.leading_zeros()).min(31)` for `u64`，0 做特例处理）；每桶独立 `AtomicU64::fetch_add(Relaxed)`。

```
bucket  range (ns)              ≈ 物理量
  0     [0, 1)                  ── 仅 0 ns 命中（事实上不会观察到）
  1     [1, 2)                  
  2     [2, 4)                  
  ...
  10    [1024, 2048)            ≈ 1-2 μs
  20    [1048576, 2097152)      ≈ 1-2 ms
  27    [134217728, 268435456)  ≈ 134-268 ms      （list_sessions 95 ms 落入）
  30    [1073741824, 2147483648) ≈ 1-2 s
  31    [2147483648, 4294967296) ≈ 2.1-4.3 s      （上限 bucket，事实上的 clamp 边界）
```

实现要点：

- `bucket_index(ns: u64) -> usize`：`if ns == 0 { 0 } else { (63 - ns.leading_zeros() as usize).min(31) }`。
- 32 桶 × `AtomicU64` = 256 byte / histogram；Phase 1 共 4 个 IPC histogram = 1 KB 静态分配。
- `Histogram::start_timer() -> Timer` 返回 RAII guard，guard drop 时调 `observe(elapsed_ns)`；调用方禁止在 hot loop 里 manually `Instant::now()` 进出（详 D9）。

**percentile 字段语义（保守上界）**：

snapshot handler 线性扫 32 桶累计 count，找到首个累计 ≥ `count * percentile / 100` 的 bucket index `i`，**报回 bucket 上界** `2^(i+1) ns`。这是**保守上界估计**——真实 percentile 落在 `[2^i, 2^(i+1))` 区间内，最坏 2x 偏差。spec / IPC 字段名仍为 `p50Ns / p95Ns / p99Ns`，但语义文档 MUST 明确"power-of-2 bucket upper bound, worst-case 2x偏差"。

**替代**：

- `hdrhistogram-rs` —— 精度高（典型 5%）但引入依赖图（zstd / serde 等），桌面应用不需要这么精。
- `metrics` crate + Prometheus exporter —— Prometheus 是服务端配套，桌面应用没人 scrape，引入整套依赖只为用其 histogram primitive 不值。
- 自适应桶宽 / log-linear bucket —— 实现复杂、percentile 校准更难。
- 16 桶 ns 输入（**原方案**）—— bucket 15 上界仅 65 μs，覆盖不到 list_sessions 95 ms 的核心 IPC 延迟，被 codex 二审否决。

**理由**：

- 32 桶 + ns 输入覆盖到 ~4.3 s，桌面应用所有合理 IPC / 文件 I/O 延迟都在范围内（list_sessions 95 ms / get_session_detail 60-74 ms 都落在 bucket 26-27）；
- 自写 ~100 行实现，零依赖、O(1) observe、无锁；
- 2x 最坏精度对"判断 PR 是否回归 / 趋势监控 / 报 issue 时贴 snapshot"够用——这是 telemetry 用途，不是 SLO 保证。要更高精度（< 5%）的场景延伸到后续 change 引入 hdrhistogram。

**精度限制（接受 + 文档显式声明）**：

- p95 真值 95 ms 在 bucket 26 = [67M, 134M] ns 区间，p95Ns 报 134217728（= 134 ms 上界），与真值 95 ms 偏差 ~41%。
- snapshot 文档 / Diagnostics tab UI MUST 在 percentile 数值旁加 hint："power-of-2 bucket upper bound（实际值 ≤ 此值，最坏 2x 偏差）"。

### D3：Event 队列 = `crossbeam-queue::ArrayQueue` SPSC，cap 10000，满了 drop 老的

**选**：

```rust
struct EventQueue {
    ring: ArrayQueue<Event>,  // cap 10000
}
fn push(&self, ev: Event) {
    // ArrayQueue::push 满时返回 Err；drop 一个老的再插入
    let _ = self.ring.force_push(ev);  // crossbeam 0.3+ 提供
}
```

**替代**：

- `tokio::sync::broadcast` —— sender 阻塞慢 receiver，hot path 不安全；且 broadcast 是"多消费者"语义不符（我们只需 IPC handler 拉一次）。
- `crossbeam-channel` unbounded —— 无上限会被 panic flood 撑爆内存。
- 自写 lock-free ring —— 已有库为何重写。

**理由**：

- ArrayQueue lock-free + bounded，零分配（节点固定）；
- 满了 drop 老的而非阻塞 producer —— 关键不变量"hot path 永不阻塞"。

**critical event 例外**：`panic` / `correctness.assertion_failed` 这类不能丢的 event 走独立 `RwLock<Vec<Event>>` always-keep 通道（容量上限同样 1000 条，但满了走"老 50% 移到 dropped_count，新的进 latest 半"——保证最近 panic 一定不丢）。本 change 仅引入 panic 一类 critical event。

### D3b：apply 阶段细节修订 — 用 `parking_lot::RwLock<VecDeque<Event>>` 替代 `crossbeam::ArrayQueue`

apply 阶段实施时发现：`crossbeam::ArrayQueue` 提供 lock-free push 但**没有"读取末尾 N 条不破坏 queue"的 API**——snapshot 路径需要这种语义供 IPC `get_telemetry_snapshot` 返回最近事件，pop 后再重新入队会与并发 push 冲突。

实施改为：

- 普通 EventQueue 用 `parking_lot::RwLock<VecDeque<Event>>` 单一结构，`push` 时 `push_back` + 满了 `pop_front`；`snapshot(n)` 直接 `iter().skip(len - n)`。
- write lock 无竞争 ~50-100 ns；多线程 race 最坏退化几 μs。Event 路径是**低频专用**（hot path 严禁 emit），竞争极低。
- 对外契约保持不变：D3 原文承诺的 "hot path 永不阻塞 producer" 仍成立——hot path 不能 push event，所以阻塞场景不存在。

**性能预算修订**：`Event::push` 单次开销由 D8 表的 ~1 μs 修订为 ~100-200 ns（无竞争）；多线程争抢极端情况下退化到 ~5 μs 仍在原 D8 contractual 上限内。

D3 原决策"lock-free SPSC ArrayQueue"保留为审计记录；实施按 D3b。

### D4：Counter / 信号 name = `&'static str`（macro 限制字面量），运行期白名单校验 + no-op fallback

**选**：

- 所有信号声明用宏 `counter!("metadata.cache.hit")`，**`macro_rules!` 内部限制 `$name:literal` token type**——非字面量字符串编译期报"expected string literal"。这是**编译期能给到的全部静态保证**：保证 name 是字面量 `&'static str`，但**无法**编译期校验"是否在 Registry 注册集合里"（OnceLock 是运行期）。
- 全局 Registry 用 `OnceLock<HashMap<&'static str, AtomicU64>>`：进程启动期 `init_registry()` 一次性 `insert` 所有静态信号 name（约 50 条，编译期 `const` 数组列出全部）；启动后 Registry **冻结**为只读，hot path 仅 `get(name)` lookup。
- **运行期白名单 fallback**：`counter!("xxx")` 宏展开为 `match REGISTRY.get("xxx") { Some(c) => c.fetch_add(1, Relaxed), None => UNREGISTERED.fetch_add(1, Relaxed) }`——未注册 name 触发 `telemetry.unregistered_signal_attempt` counter 增 1（自身用作 self-observability，编译期 hardcoded 始终在白名单），但**不**增长 Registry 内部 map（避免内存泄漏）。这层 fallback 让漏注册的信号在 dev 跑通时 Diagnostics tab 能看到"漏了"，而不是 silent 错误。
- tracing bridge 方向 1 需要把动态 `target: "cdt_ssh"` 转 counter 名 `"cdt_ssh.error"`：用 `OnceLock<HashMap<&'static str, &'static str>>` 预定义白名单（启动期注册 cdt-ssh / cdt-watch / cdt-api / cdt-discover / cdt-parse / cdt-analyze / cdt-config / cdt-core 8 个 target），未在白名单的 target → 不归 counter（避免恶意 / 误用 target 撑爆 map）。

**替代**：

- 所有信号 name 用 `String` —— hot path 每次分配 ~50 ns，且 hash 慢。
- 用 `&str` 但允许动态名 + 运行期注册 —— Registry map 不断长大且无淘汰，慢慢内存涨。
- **完整编译期校验信号 name 在白名单**（**原方案**——被 codex 二审否决）—— `macro_rules!` 没法看到 `OnceLock` 状态，要做编译期校验得换 `proc_macro` + 维护一份 `const SIGNAL_NAMES: &[&str] = &["..."]` 让 macro 在展开期 `match` 字符串字面量。增加构建复杂度（proc_macro crate 单独编译），且白名单分两处（const 数组 + 启动期 init）易漂移。本 change 不采纳；接受运行期 fallback。

**理由**：

- 静态白名单（约 50 条信号 name）启动期一次性 init Registry，hot path 0 分配 0 hash 写 —— 只读 lookup；
- tracing target 白名单边界明确（仅 cdt-* 系列 crate），杜绝外部 crate（reqwest / sqlx 等）的 ERROR 噪音污染；
- macro 仅保证字面量类型不保证内容——文档 / spec 显式声明这一边界。

**实现细节**：Registry 用 `dashmap::DashMap` 还是 `RwLock<HashMap>`？选 **`OnceLock<HashMap>` + 启动期一次性建表 + hot path 只读**——hot path 不存在写需求（信号 name 是编译期已知集合）。

### D5：tracing bridge 方向 1 = `tracing-subscriber::Layer` 钩 ERROR / WARN，按 target 路由 counter

**选**：

```rust
pub struct TelemetryLayer { /* OnceLock<HashMap<&'static str, &'static str>> */ }

impl<S> Layer<S> for TelemetryLayer {
    fn on_event(&self, ev: &Event<'_>, _ctx: Context<'_, S>) {
        let level = *ev.metadata().level();
        if level != Level::ERROR && level != Level::WARN { return; }
        let target = ev.metadata().target();  // e.g. "cdt_ssh::polling"
        // 取顶级 crate 名 (split_once("::") 取第一段)
        let crate_name = target.split("::").next().unwrap_or(target);
        if let Some(counter_name) = WHITELIST.get(crate_name) {
            // counter_name 形如 "cdt_ssh.error" / "cdt_ssh.warn"
            REGISTRY.counter(counter_name).inc();
        }
    }
}
```

**替代**：

- `EnvFilter` 拦截特定 target 后转事件 —— EnvFilter 是过滤器不是 sink，做不到捕获后归 counter。
- 改写所有 `tracing::error!` 为 `counter!()` —— 200+ 处侵入式改造。

**理由**：

- 老代码一行不改，新建一个 layer 即可；
- 性能：layer `on_event` 钩子约 100 ns / event，但 ERROR / WARN 在常态低频（< 100 / 分钟），总开销 < 10 μs / 分钟，可忽略。
- 白名单边界明确，外部依赖 crate 噪音不归。

### D6：IPC = pull-based `get_telemetry_snapshot`，schema_version 字段保留

**选**：

```rust
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TelemetrySnapshot {
    pub schema_version: u32,            // 当前 1
    pub uptime_secs: u64,
    pub captured_at: u64,               // unix millis
    pub counters: BTreeMap<String, u64>,
    pub histograms: BTreeMap<String, HistogramSnapshot>,
    pub recent_events: Vec<TelemetryEvent>,  // ring buffer 最近 N 条（N=100）
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HistogramSnapshot {
    pub count: u64,
    pub buckets: [u64; 16],
    pub p50_ns: Option<u64>,
    pub p95_ns: Option<u64>,
    pub p99_ns: Option<u64>,
    pub max_ns: Option<u64>,
}
```

**替代**：

- 推 push 模式：`subscribe_telemetry()` broadcast —— 用户 / settings UI 不需要持续推；增加 broadcast 路径反而违反"hot path 永不阻塞" Goal。
- 一次返回原始 buckets 让前端算 percentile —— 前端 Svelte 算 percentile 重复劳动且不准，后端 16 桶线性扫即可。

**理由**：

- pull 语义简单，settings UI tab 切换 / 用户点"刷新"按钮触发；
- `schema_version` 字段从首发就预留，未来 Phase 2 加新维度时按 version 升级而不破坏旧 UI；
- size budget：counters ~50 条 × 80 byte ≈ 4 KB / histograms 4 个 × 200 byte ≈ 0.8 KB / events 100 条 × 200 byte ≈ 20 KB，总 < 30 KB —— 远低于 1 MB IPC payload 阈值（详见 `.claude/rules/perf.md`）。

### D7：settings → Diagnostics tab 渲染策略

**选**：

- 现有 `Settings.svelte` tab 注册项加 `Diagnostics`，与 `General` / `Notification` 等同级。
- 新建 `DiagnosticsTab.svelte`：
  - 顶部 4 个仪表盘卡片：cache hit rate / IPC error rate / panic count / SSH reconnect count（卡片用现有 design token，不引新组件库）。
  - 中部延迟分布柱状图（直接 SVG 画 32 个矩形，不引图表库）；柱状图下方显示 p50/p95/p99 数值并加 hint "power-of-2 bucket upper bound（实际值 ≤ 此值，最坏 2x 偏差）"。
  - 底部最近 events 表格（virtualization 暂不需要，N≤100 条）。
  - 顶部右上"复制完整 snapshot"按钮：调 `navigator.clipboard.writeText(JSON.stringify(snapshot, null, 2))`，用户报 issue 时一键贴 GitHub。
- 数据获取：tab mount 时 `getTelemetrySnapshot()` 一次；侧栏标签底部"刷新"按钮触发再拉一次。**不**做轮询（避免抢主线程）。
- 反闪烁：tab 第一次 mount 显 "loading..." 是允许的（不在 hot 用户路径上，settings 切 tab 是低频显式操作）。后续刷新走 `silent=true` 模式，旧数据保留直到新数据到达。

**替代**：

- 引入 `chart.js` / `apexcharts` —— 增加 ~100 KB bundle，得不偿失。
- 实时 SSE push 更新仪表盘 —— 违反"用户 pull"语义，且引入额外 broadcast 路径。

**理由**：UI 简朴优先，dashboard 主要用途是"用户复制 snapshot 给我们"，不是常驻监控面板。

### D8：hot path 性能契约 + 验证方法

**信号开销预算**（含 `Instant::now()` 调用）：

| 信号类型 | 单次开销 | hot path 例 |
|---|---|---|
| `Counter::inc()` | ~5 ns | `metadata.cache.hit` 命中分支 |
| `Histogram::observe()` | ~30-50 ns（含 `Instant::now()` + `leading_zeros` + `fetch_add`）| `ipc.list_sessions.duration_us` 出口 |
| `Event::push()` | ~1 μs（含 `String` 字段分配）| **禁止 hot path**；只用于 SSH reconnect 等低频 |

**hot path 总预算**：单次 `try_lookup_cached_metadata` 增量 ≤ 50 ns（占基线 10-50 μs 的 0.1-0.5%）；单次 `list_sessions(50)` 全程增量 ≤ 1.25 μs（占基线 95 ms 的 0.0013%）。

**验证方法**：

1. 新增 `cdt-api/tests/perf_telemetry_overhead.rs`（`#[ignore]`）：**runtime env var `CDT_TELEMETRY_ENABLED=0/1` 控制 telemetry 是否注册 layer / 启用 hot path 写入**，**同一 binary 同一进程**先后跑两遍负载（10000 次 `try_lookup_cached_metadata` + 1000 次 `list_sessions(50)`），对比 wall time 与 max RSS。这避免 cargo feature flag 编译期切换需要两份 binary 的可重复性问题（早期 design 错误假设可在同一 binary 内 feature 切换被 codex 二审否决）。env var 在 hot path 体现为 `static IS_ENABLED: AtomicBool`（启动期一次 read，运行期只读 atomic load），分支预测命中率高，开销可忽略。
2. 既有 `perf_cold_scan` / `perf_get_session_detail` 跑一遍验证 baseline 无回归（按 `.claude/rules/perf.md` 阈值 wall +20% / user +50% / RSS +30%）。
3. CI smoke：`scripts/run-perf-bench.sh --bench cold_scan` 在 PR pipeline 跑一次。

### D9：禁止热路径同步语义破坏

为避免新接入的 `cdt-telemetry` 调用被错放到错误位置，约束如下：

- `histogram!()` 宏 `MUST` 在 IPC 入口 / 出口测量（一次 `Instant::now()` 进、一次差值出），**禁止**在循环内每次迭代都调用——否则单次 `Instant::now()` ~30 ns 累积成 micros。
- `event!()` 宏 `SHALL NOT` 在 hot path 出现；提交 PR 时 grep `event!\(` 在 `crates/cdt-api/src/ipc/local.rs` / `cdt-discover/` / `cdt-parse/` 等 hot 文件下应为 0 命中（CI 加 `scripts/check-no-hot-event.sh`）。
- `counter!()` 宏可在 hot path 出现，但单次 IPC 调用累计 counter 写次数 `SHOULD` < 100（避免每条消息 / 每个 chunk 都写）。

### D10：Sidebar Correctness 信号前端聚合 + 后端批量 inc（避免 file-change 风暴变 IPC 热点）

**背景**：sidebar 收到 `session-metadata-update` 事件，listener 内若发现"新值与旧值都 not-null 但不一致"判定为 stale-update，需通知后端 inc `stale_update.triggered` counter。但 `session-metadata-update` 在 file-change 风暴下可频繁触发（spec 已明示 SSH 全命中场景仍 broadcast），每条 event 立刻同步调一个 IPC 会把"低频 correctness 信号"事实上变成 IPC 热点——违反 D1 hot path 边界。

**选**：

- **前端聚合**：sidebar 持有进程内 `correctnessCounters: Map<string, number>` Svelte store；listener 检测 stale-update 时**仅本地累计**（无 IPC）。
- **节流 flush**：5 秒 setTimeout 或累计 ≥ 50 条触发一次 `recordCorrectnessEvents(items: { kind: string, count: number }[])` IPC 批量 flush。
- **IPC 后端**：`record_correctness_events` 接受 `Vec<{ kind: String, count: u64 }>`，按白名单（`stale_update.triggered` / `cache.signature_skew_observed_in_ui`）逐条 `Counter::add(count)`；未在白名单的 kind silently ignore + 增 `telemetry.unregistered_correctness_event` counter（自身 hardcoded 注册）。

**替代**：

- 每条 stale-update 立刻调 IPC —— 风暴下 IPC 热点（被 codex 二审否决）。
- 后端按 sessionId 去重 —— sessionId 是 PII 不能跨 IPC 携带；且去重逻辑放后端复杂度更高。
- 完全删 stale-update telemetry —— 损失可观测性，违反 Goal "Reliability + Correctness 转白盒"。

**理由**：

- 前端聚合简单（Svelte store + setTimeout 节流），与现有 sidebar listener 同进程零 IPC overhead。
- 5 秒窗口 + 50 条阈值在常态使用下保证 ≤ 12 IPC / 分钟，远低于 IPC 热点阈值。
- Vue / React 等其他框架移植时聚合逻辑可复用此策略。

**字段语义**：IPC `record_correctness_events` 不返回任何业务数据，仅返回 `{ ok: true }`；前端 fire-and-forget。

## Risks / Trade-offs

- **[风险] 信号爆炸**：`tracing` bridge 自动归类后，新加 `tracing::error!(target: "cdt_xxx", ...)` 都会入 counter。若未来某 crate 把 ERROR 当 INFO 用（频繁 log），counter 会爆涨虚报。**Mitigation**：白名单只覆盖 cdt-* 8 个 crate；ERROR / WARN 真实频率 > 1 / 秒时 layer 内做指数退避（每 1024 条只采样 1 条，仍归 counter）。本 change 暂不实现退避，留作 Phase 1.5 followup（监控 24h 数据后判断是否需要）。

- **[风险] hot path counter 命中率统计偏差**：Counter 用 Relaxed ordering 在多核场景可能出现"读到中间态"——但读出口是 IPC 拉一次（已 atomic load），且统计学意义上误差 < 0.1%（10000 ops / sec 规模下相邻 atomic 写不冲突），不影响"判断命中率高低"的决策粒度。**Mitigation**：snapshot handler 内一次性 atomic load 完所有 counter（< 1 μs），快照一致性保证到"读瞬间"足够。

- **[风险] Event ring buffer 满 drop 关键事件**：panic 类 critical event 走独立 always-keep 通道；其他 Event（如 SSH reconnect）满了 drop 老的，可能丢早期信息。**Mitigation**：本 change 限定 Event 仅用于 SSH SFTP death / SSH reconnect / generation race trigger 三类低频事件，cap 10000 在常态使用 24h 内不会溢满；监控 Phase 1 上线后 ring 利用率，必要时调 cap。

- **[风险] tracing layer 钩子开销在 ERROR 风暴时被放大**：若某 crate 短时间 emit 1000 个 ERROR / 秒，layer 钩子总开销 ~100 μs / 秒 = 0.01% CPU，仍可接受；但若 10000 ERROR / 秒，开销升到 0.1% —— 边缘场景。**Mitigation**：layer 钩子内做"同 target 同 level 在 100 ms 内累计计数 + 批量 fetch_add 一次"——但这增加复杂度；本 change 不做，留作 Phase 1.5 followup。

- **[风险] schema_version 设计约束未来扩展**：u32 版本字段允许 4B 个版本，足够；但若 Phase 2 改 HistogramSnapshot 字段（如新增 sum 字段），旧前端读到新后端 snapshot 会反序列化失败。**Mitigation**：HistogramSnapshot / TelemetryEvent 字段都用 `#[serde(default)]`（Phase 2 加新字段时旧前端读到默认值不报错）；schema_version 仅在新版本前端读旧后端时需要降级 UI。

- **[风险] Diagnostics tab UI 引入 SVG 柱状图代码膨胀**：自画 SVG 柱状图 ~100 行，但维护成本不低（响应式宽度 / tooltip / 颜色主题适配）。**Trade-off**：当前选自画避免引入图表库依赖；Phase 2 若需复杂趋势图（30 天折线图），届时评估引入轻量库（< 30 KB gzipped）。

- **[Trade-off] SQLite 持久 / Resource sampler / opt-in 上报留 Phase 2-4**：用户可能等不及 30 天历史趋势功能。**接受**：本 change 优先把 hot path 安全性 / IPC 契约 / tracing bridge 落地；持久化与上报独立 review。每个 Phase 独立 change，互不阻塞。

- **[Trade-off] 自写 16-bucket histogram 精度 2x**：足以判断"P95 在 1 ms 还是 10 ms 级"但分辨不出"P95 是 1.5 ms 还是 1.9 ms"。**接受**：本 change 用途是"判断回归 / 趋势"，不是 SLO 保证；要更高精度时引入 hdrhistogram 是后续优化。

## Migration Plan

### Phase 划分（本 change = Phase 1）

| Phase | 内容 | 触发条件 | slug 候选 |
|---|---|---|---|
| **1（本 change）** | crates/cdt-telemetry + Counter + Histogram + Event + tracing bridge 方向 1 + 20 处接入 + IPC + Diagnostics tab + perf-bench 验证 | 立即 | `add-telemetry-signal-bus` |
| **2** | SQLite 持久 + 90 天 retention + 历史趋势 UI | Phase 1 上线 1 周后，监控 telemetry 自身无回归 | `add-telemetry-persistence` |
| **3** | Resource 维度 sampler（RSS / FD / tokio task count）+ tracing bridge 方向 2（每 60s digest）+ CI bench 集成 telemetry snapshot | Phase 2 上线 + 用户反馈"想看资源占用" | `add-telemetry-resource-and-ci` |
| **4** | opt-in 上报通道 + Behavior 维度（项目路径 hash / IPC 调用频次）+ 隐私 review pass | 隐私合规 review 完成 + 后端聚合 endpoint 准备好 | `add-telemetry-opt-in-reporting` |

### 部署步骤（Phase 1）

1. 落地 `crates/cdt-telemetry` + 单测 + 集成测试。
2. 接入 20 处信号点 + 验证 hot path overhead 测试。
3. 接入 IPC `get_telemetry_snapshot` + Tauri command 注册。
4. 前端 Diagnostics tab + 复制按钮。
5. 跑 `scripts/run-perf-bench.sh` 验证 baseline 无回归。
6. push + PR + wait-ci + codex 二审 + archive。

### 回滚策略

- Telemetry 全程不影响产品行为；若 hot path 真的回归 > 0.2%，紧急 PR 关闭 `tracing::Layer` 注册（保留 `cdt-telemetry` crate 但不挂 layer）。
- IPC `get_telemetry_snapshot` 出问题时返回空快照，前端 Diagnostics tab 显示 "Telemetry unavailable"，不影响其他 tab。
- 极端情况：删除 `cdt-telemetry` crate 依赖 + 移除 IPC + 移除 Diagnostics tab 一次性回滚。

### Handoff 给后续 Phase（硬约束）

后续 Phase change `MUST` 在 proposal.md 显式引用本 design.md 的 Phase 划分表，验证：

- Phase 2 SHALL 使用本 Phase 引入的 `TelemetrySnapshot` schema 作为输入（不重新定义）。
- Phase 3 资源 sampler SHALL 以 Counter / Histogram 形式接入既有 Registry（不新建并行 registry）。
- Phase 4 上报路径 SHALL 在 `TelemetrySignal` 上加 `reportable: bool` 标签，黑名单（path / sessionId / backtrace）不出本机。

## Open Questions

1. **白名单 target 是否包含 `cdt_ssh::polling` / `cdt_ssh::sftp` 等子模块**？目前 D5 以顶级 crate 名归类（`cdt_ssh` 一律归 `cdt_ssh.error`）；若需要细分到子模块，layer 钩子需要拆分前缀匹配。**临时决定**：先按顶级 crate 归类，Phase 1 上线后看 24h 数据决定是否细分（不阻塞本 change）。
2. **panic_handler 的接入方式**：`std::panic::set_hook` 全局只能注册一个 hook。当前 `src-tauri/src/lib.rs` 是否已有 panic hook？若有，本 change 需要 chain 不能覆盖。**待确认**：apply 阶段 grep `set_hook` 后决定。
3. **Diagnostics tab 是否需要做 i18n**：现有 settings 其他 tab 是否已 i18n？**临时决定**：跟随现有 settings tab 风格（若现有英文则英文，若已 i18n 则跟随）；apply 阶段确认。
4. **`get_telemetry_snapshot` 是否 IPC contract test 覆盖**：按 `frontend-test-pyramid` spec，IPC 字段改动 SHALL 加 `cdt-api/tests/ipc_contract.rs` 测试。本 change 加 1 个 contract test 确保 camelCase 字段名稳定。**确定要加**。

# 性能基线 + 预算 + 防回归

claude-devtools-rs 用 Rust 重写原 TS 项目的根本动机就是性能。本文是**硬约束**——任何 PR 都要参照此规则做性能影响评估。

## 核心原则：低消耗 + 高性能（同时满足）

claude-devtools-rs 是**桌面辅助工具**——不是用户工作主线（不是 IDE / 浏览器 / 编辑器）。用户开着它不应感知它的存在：风扇不应起转、笔记本电池不应明显掉得快、其他 app 不应被抢 CPU。

性能不只是"wall time 快"。一个把串行改并发的优化即使把 real time 砍半，如果 user time 暴涨 5×（多核打满）或 max RSS 涨 3×（cache 不限内存），辅助工具的定位就崩了。

**任何性能改动 SHALL 同时验证四个维度，缺一不可：**

1. **wall time（real）** — 用户实际感知的耗时
2. **user time + sys time** — CPU 实际工作量（多核并发会让 user > real）
3. **CPU 占用核数（user/real ratio）** — 进程平均占用多少核（非系统 CPU 百分比）；详见下方"指标物理意义"
4. **max RSS + peak memory footprint** — 进程内存峰值

**典型反例**：
- 反例 1：把 27 个 project 的 file I/O 串行改 `join_all + Semaphore(32)`，real 从 100ms 降到 30ms，但 user 从 15ms 涨到 200ms（13×）→ 占用 6.6 个核短时间，风扇起转 → reject
- 反例 2：加 LRU cache 让 hot path 跳过重 parse，real 从 300ms 降到 5ms，但 cache 无 byte cap 大会话连开 50 个后 RSS 涨 800MB → reject

## 指标物理意义（避免误解）

`/usr/bin/time -lp` 给的 `user / sys / real` 是该**进程的**累计 CPU 时间，不是系统 CPU 百分比：

- **`user/real`** = 进程在 wall time 期间平均占用的核数
  - `0.13`（cold_scan baseline）= 平均 0.13 个核 ≈ **8 核机 1.6% 系统 CPU**
  - `0.5` = 平均 0.5 个核 ≈ **8 核机 6% 系统 CPU**
  - `1.0` = 单线程满载一个核 = **8 核机 12.5% 系统 CPU**
  - `4.0` = 跑满 4 个核 = **8 核机 50% 系统 CPU** → 风扇起转
- **bench 的 user/real 是单次 IPC 调用 1-2 秒内的爆发峰值**，跟"app 长跑稳态系统 CPU"不是一回事。app 长跑稳态主要看 file watcher / 后台扫描 / idle 时无活动——这些必须人工用 Activity Monitor 看。

## 桌面应用 CPU 分级阈值（辅助工具定位）

claude-devtools-rs 是辅助工具，**单次操作不应打满多核**。即使爆发期（< 200ms），单线程能办的事就不要多核并发。

| 阶段 | 含义 | 用户感知 | 系统 CPU 阈值（8 核机） | bench 等价（user/real） |
|---|---|---|---|---|
| **app idle 稳态** | 开着没操作 | 几乎不可见 | **< 2%**（< 0.16 核） | 不直接测，看 file watcher / metadata scanner 长跑 |
| **后台扫描触发** | file change / fs event 触发的后台扫描 | 风扇不起转 | **< 10%** 短时间（< 0.8 核） | 单次扫 bench user/real **< 0.5** |
| **用户交互峰值** | 用户点 sidebar 选 session / 切 tab / 展开 tool / 搜索 / 改设置等单次操作触发 IPC（`get_session_detail` / `list_sessions` / `get_tool_output` / `search_text`）。从用户操作到屏幕渲染完成 | 单次操作 < 200ms 完成不阻塞 | **< 15%** 几百 ms（< 1.2 核） | bench user/real **< 1.0**（爆发期允许 ≈ 单线程满载一个核） |
| **bench 长跑（CI / dev）** | — | — | — | — |

**核心约束**：单次 IPC handler **绝不允许 user/real > 1.0** 长时间——这意味着调用方在多核上密集计算，违背"辅助工具不抢用户主线 CPU"的定位。short burst（< 50ms）的 user/real ≤ 1.5 可酌情放行，但 PR 描述必须证明无替代方案（cache / 减重复工作 / 改算法）。

**任何 PR**：如果某条 hot path bench 让 user/real 跨过 0.5，SHALL 在 PR 描述说明：(a) 是否 I/O-bound 还是 CPU-bound、(b) 并发改造是否加了 Semaphore 限流（CPU-bound 路径限到 ≤ CPU 核数 / 4）、(c) 是否有更省 CPU 的非并发替代（cache / 减重复工作 / 改算法）。

## 性能预算（关键路径）

这些是冷启动 / 首屏 / 大会话三条关键路径的上限。**违反需要 follow-up 修复**，不能合并 PR 时降低预算。

| 关键路径 | 测量方式 | wall 预算 | CPU 预算 | RSS 预算 | 基线（v0.4.10，2026-05-16，30 project × 538 session）|
|---|---|---|---|---|---|
| **冷启动 list_repository_groups** | `/usr/bin/time -lp cargo test -p cdt-api --release --test perf_cold_scan -- --ignored --nocapture`，bench 内打印 `cold total` + time 给的 user/sys/RSS | **< 200ms**（bench 内） | user/real ≤ **0.5**（≈ 8 核机 6%，I/O-bound 默认；并发改造爆发期可放宽到 ≤ 1.5） | max RSS < **80 MB** | bench 内 95ms · 整进程 real 1.49s / user 0.19s / sys 0.77s（user/real=**0.13** ≈ 1.6% 系统 CPU）/ RSS 59 MB |
| **冷启动 list_projects** | 上同，看 `cold scan` | **< 150ms** | 同上 | 同上 | 87-93ms |
| **首屏 sidebar 可见列表** | 桌面应用启动到 sidebar 首条 session 渲染（人工秒表） | **< 500ms** | — | — | 待测 |
| **大会话 get_session_detail** | `/usr/bin/time -lp cargo test --release -p cdt-api --test perf_get_session_detail -- --ignored --nocapture` | bench 内总 **< 800ms（10k 消息）** | user/real ≤ **0.5** | max RSS < **200 MB** | 1221 msg / 60-74ms · 整进程 real 1.36s / user 0.23s / sys 0.20s（user/real=**0.17** ≈ 2% 系统 CPU）/ RSS 120 MB |
| **app idle 稳态系统 CPU** | Activity Monitor / top 看 claude-devtools-tauri 进程，无操作 60s 平均 | < **2%**（8 核机 < 0.16 核） | — | — | 待测（人工） |
| **app 后台扫描峰值系统 CPU** | 触发 file change / fs event 时 Activity Monitor 看 | < **10%** 短时间（< 1s） | — | — | 待测（人工） |
| **app 用户交互峰值系统 CPU** | 点击 / 搜索 / 切 tab 等单次操作触发的 IPC | < **15%** 短时间（< 200ms） | — | — | 待测（人工） |
| **Tauri IPC payload** | 后端 emit JSON size | 单次返回 **< 1 MB**（>1MB 须走瘦身模式） | — | — | — |
| **Tauri IPC 端到端吞吐** | 实测 webview 端 JSON.parse 完成 | ≈ **6.5 KB/ms**（含反序列化） | — | — | — |

**回归阈值（每条独立判断，违反任一即拒）**：
- wall time 涨 **> 20%** → 性能 bug
- user time 涨 **> 50%**（real 没同步降）→ CPU 反模式（多核打满了但没换来速度）
- user/real ratio 跨过 0.5（从 < 0.5 涨到 > 0.5）且 real 降幅 < 30% → 不合理 CPU 上涨
- user/real **超过 1.0**（占用超过单线程满载一核）即使 real 降也 reject —— 辅助工具单次 IPC 不应打满多核；short burst（< 50ms）放宽到 ≤ 1.5 但需证明无替代方案
- max RSS 涨 **> 30%** → 内存反退
- 涉及 file watcher / metadata scanner / IPC 后台 task 的 PR：必须人工 Activity Monitor 验 idle 稳态系统 CPU < 2%、后台扫描峰值 < 10%、用户交互峰值 < 15%

测量 CPU + 内存的统一命令：`/usr/bin/time -lp <bench-cmd>` —— 输出 `real / user / sys` 三行 + `maximum resident set size` + `peak memory footprint` + `page faults` + `context switches`。Linux 用 `/usr/bin/time -v`。

人工系统 CPU 验证：`top -pid $(pgrep claude-devtools-tauri)` 或 macOS Activity Monitor 看进程 CPU% 列。

## 现有 perf bench 入口

每个关键路径都有 bench tool（`#[ignore]` 不进 CI，手动跑作为定位 + 基线对比）：

- `cdt-api/tests/perf_cold_scan.rs::measure_cold_scan` — 冷启动 scan + grouper 链路
- `cdt-api/tests/perf_get_session_detail.rs::measure_get_session_detail` — 大会话首次打开
- `/perf-bench` skill — 自动跑 + 解析 + 给 "瘦身/不瘦身" verdict

**新增关键路径时**：SHALL 加对应 bench（参照上述模板）+ 把基线数据填进本文预算表。

## 防回归硬约束

### 1. PR 影响评估（强制）

涉及以下任一文件 / 行为的 PR，**SHALL** 在 PR 描述里加 "Perf impact" 段并跑相应 bench 给数据：

- `cdt-discover/` 任何文件（启动 + sidebar 列表必经路径）
- `cdt-api/src/ipc/local.rs` / `session_metadata.rs` / `cache_signature.rs`（IPC 数据流核心）
- `cdt-analyze/` 任何 `build_chunks` / context tracking 路径
- 引入 `tokio::process::Command` / `Command::new` 子进程 spawn（成本极高）
- 在 hot loop 里加 `tokio::fs::read_to_string` 全文件读 / `JSON.parse` 大对象
- 改 `tauri.conf.json` / `src-tauri/Cargo.toml` features（影响 bundle / startup binary size）

模板（**四维齐全**，缺一项视为评估不充分）：
```markdown
## Perf impact
- 关键路径：[冷启动 / get_session_detail / ...]
- wall time：基线 xxx ms → 本 PR yyy ms（±zz%）
- user time：基线 a.aa s → 本 PR b.bb s（±zz%）
- sys time：基线 c.cc s → 本 PR d.dd s（±zz%）
- max RSS：基线 NNN MB → 本 PR MMM MB（±zz%）
- CPU 利用率（user/real）：基线 0.xx → 本 PR 0.yy
- 数据来源：`/usr/bin/time -lp cargo test -p cdt-api --release --test perf_cold_scan -- --ignored --nocapture` 输出
```

四维数据缺任何一项 reviewer 都可拒 PR。**不要**只贴 wall time 一行就声称"性能优化"。

豁免：纯 docs / 注释 / typo / CI 配置改动可不写。

### 2. 反模式清单（**严禁**引入）

历史血泪经验，违反任一即拒：

**wall time 类**：
- **for-loop 内串行 spawn 子进程**：spawn 单次 cold 5–15ms，N 个串行就是 N×。如果真要 spawn N 个用 `futures::future::join_all` 并发，且优先看能不能换纯 fs / lib 调用
- **for-loop 内串行 file I/O**：`for x { tokio::fs::read_to_string(x).await }` 等价上一条。用 `join_all` 并发 + 加 `Semaphore` 限流
- **每次 IPC 都重扫文件 / 重算 chunk**：必须有按 `FileSignature` 的内存 cache（参照 `MetadataCache`）
- **冷启动路径同步阻塞 I/O**：`std::fs::*` / `Command::output().wait()` 都会阻塞 tokio worker；用 `tokio::fs::*` / `Command::output().await`
- **每次返回全量 JSON 而不分页 / lazy**：>1 MB payload SHALL 走"IPC payload 瘦身模式"（参照 CLAUDE.md Conventions）
- **`spawn` 创建新 tokio runtime 当作并发原语**：runtime 是重对象，PR 评审看到当 bug
- **不走 cache fast-path 的 IPC**：详见 `ipc-data-api/spec.md::list_sessions cache 命中`
- **算法 O(N²) 在 N > 100 时**：list / merge / sort 都要看复杂度——本仓 27 project × 534 session 不算大

**CPU 类（与 wall time 同等重要）**：
- **CPU-bound 路径串行改并发不限流**：CPU-bound（parse / serde / 加密 / 压缩）改 `join_all` 不加 `Semaphore` 限到合理并发度（建议 ≤ CPU 核数 / 2，桌面应用避免抢用户其他 app 的 CPU），会让 user time 按并行度倍增、real 几乎不降——典型并发反模式
- **判断 I/O-bound vs CPU-bound 要看 baseline `user/real` 比值**：`< 0.3` 是强 I/O-bound（适合并发，user 不会涨多），`> 0.7` 是 CPU-bound（并发要谨慎，加 `Semaphore` 必备）。本仓 cold_scan baseline `user/real=0.13`、get_session_detail `0.17`，**都是 I/O-bound**，并发改造一般安全
- **加完并发不测 user time**：只看 wall time 降了就觉得赢了——必须四维齐看（见 PR 模板）
- **hot loop 里隐式 `clone()` 大对象**：`Vec<ChatMessage>` / `String` / `HashMap` / `SessionDetail` 等在 hot path 反复 clone 是隐形 CPU 消耗，影响 user time + 触发频繁 alloc/drop。优先用 `Arc<>` 共享、`&` 引用或 `mem::take`
- **同步循环里 `serde_json::from_str` / `to_string` 大 JSON**：JSON 序列化是 CPU 密集，hot loop 里反复跑会让 user time 飙升

**内存类**：
- **加 cache 不设 byte cap 仅设 count cap**：`LruCache::new(1000)` 没有 byte 上限，单条 entry 大时（如 search text vec / parsed message vec）极端场景 RSS 涨百兆。SHALL 同时加 `current_bytes: AtomicUsize` + `max_bytes`，evict 时双闸门检查
- **永久持有全量 `Vec<ChatMessage>` 的 Map**：没有 LRU evict / TTL 的常驻 Map 会让 RSS 单调增长。即使 capacity 限定，单条 entry 增大也会让总 RSS 飙
- **整页 base64 inline 在 IPC payload**：默认场景 image / 大文本 inline 会让 IPC payload 与 webview 端 JSON.parse 双重内存开销，必须走 asset:// URL 或 lazy IPC（参照 CLAUDE.md `IPC payload 瘦身模式`）
- **broadcast channel 缓冲区设过大**：`broadcast::channel(N)` 的 N 直接决定常驻内存上限，每条 event 几十-几百字节 × N。本仓 256 已偏大（除非有充分理由），新 channel 默认 128 起步
- **subscriber 不 drop / receiver 泄漏**：`broadcast::Receiver` / file watcher 退订路径漏，会让旧订阅常驻 + 收不到新事件占内存。新加 broadcast subscriber 时 SHALL grep 退订路径
- **收集全量 `Vec<ParsedMessage>` 仅为最后判一次状态**：典型反模式（见 metadata-streaming-ongoing change）。改流式状态机增量喂消息，不存 Vec

### 3. 新功能性能验收

任何**新增 capability / 改后端算法 / 加 IPC 字段** 的 PR：
- SHALL 在 spec 里加一条 **性能 SHALL**（如 "list_xxx 在 N=500 时 SHALL < 100ms"）
- SHALL 加对应 bench 覆盖该 SHALL
- SHALL 在 PR 描述贴 bench 输出

事后补不算——和 "openspec 行为契约级改动先 propose 再 apply" 同样原则。

### 4. codex 二审增加性能视角

`.claude/rules/codex-usage.md` 已规定每个 PR push 后默认调 codex。**性能相关 PR** 的 codex prompt SHALL 显式列：

```
重点查：
- 是否有 for-loop 内 spawn / 串行 await（应该 join_all + 限流）
- 是否有 hot path 缺 cache（按 FileSignature key）
- 是否有重复 IPC payload 字段（应该 omit 或 lazy）
- 算法复杂度评估
- 并发改造的 Semaphore 限流是否合理（CPU-bound 应限到 ≤ CPU 核数 / 2；I/O-bound 可放宽到 16-32）
- hot path 是否有隐式大对象 clone（Vec / HashMap / String / SessionDetail）；能用 Arc / & / mem::take 替代吗
- 新加 cache 是否有 byte cap（不只是 count cap），evict 路径是否双闸门
- broadcast channel capacity 是否过大；subscriber 退订路径是否漏
- PR 描述四维 perf 数据（wall / user / sys / RSS）是否齐全，user time 涨幅是否在合理范围
```

## 主动定期跑

**每发版前** SHALL 跑以下 bench 并对比上一次基线：

```sh
# 冷启动
cargo test -p cdt-api --release --test perf_cold_scan -- --ignored --nocapture

# 大会话
cargo test --release -p cdt-api --test perf_get_session_detail -- --ignored --nocapture
```

把数据贴到 release PR 描述，方便回溯。

**每次会话开始 + 用户问 "为什么慢"** SHALL 先跑 bench 拿数据再讨论方向，不靠直觉。

## 历史性能事件（学习材料）

按时间倒序：

- **2026-05-15** `perf/cold-start-list-sessions` — list_repository_groups 4030ms → 89ms（45x）
  - 根因：`LocalGitIdentityResolver` 每 project 串行 spawn 3–5 个 `git rev-parse` 子进程
  - 修法：纯 fs 实现，从 `.git` / `HEAD` 文件直接读取，0 个 git 子进程
  - 教训：**有文件可读时绝不 spawn 子进程**——syscall 比 process spawn 快 1000 倍

- **2026-05-14** `multi-session-cpu-cache` + `session-list-cache-fast-path` — list_sessions 全 cache 命中路径
  - 根因：每次 IPC 全部 session 都重扫 JSONL
  - 修法：按 `FileSignature` LRU cache + fast-path 跳过 broadcast 路径

- **2026-04-19** `session-detail-image-asset-cache` — 大会话 image 反复 base64
  - 根因：每次 `get_session_detail` 把所有 image block base64 inline
  - 修法：image 落盘 cache + 走 `asset://` URL

- **2026-04-29 ~ 05-12** 5 轮 IPC payload 瘦身（详见 `feedback_align_with_original.md` 上下文）
  - 根因：default-cap / response content 全量塞 IPC
  - 修法：`OMIT_XXX const + xxxOmitted: bool + get_xxx_lazy IPC` 模式

## 后续性能优化候选清单

按收益排序，开 follow-up issue 或 openspec change 处理：

1. **scan() 顶层 project 目录并发**：当前 `for dir_name in dirs` 顺序处理 27 dir，可改 `join_all` 并发——预期 scan 87ms → ~30ms
2. **head N 行 cwd 抽取的 fallback `read_to_string` 全文件去掉**：大会话 fixture 卡几十 ms，spec 没强制 Local 也要 fallback（SSH 已禁用）
3. **持久化 cwd / git 元数据 cache**：跨进程 cache 让"冷冷启动"也命中（首次 install 后第二次启动直接零扫）
4. **sidebar 列表渲染 `{#each}` 的虚拟滚动 lazy mount**：列表项 > 200 时有微卡顿（现实只有 500 也很流畅，但需要 budget 限定）
5. **chunk-building 复杂度 audit**：`build_chunks_with_subagents` 对 10k 消息会话耗时是否 O(N²)？需 bench 验
6. **WorktreeGrouper 进程内 cache**：当前实现已经很快（2ms），但同进程多次调（如 sidebar refresh）每次重扫——加 in-memory cache 完全避免重复

每条候选都 SHALL 走 openspec propose + 加 bench 验证 + 给数据。

## Context

`WorkflowManifestCache` 当前是三个裸 `HashMap<PathBuf, *CacheEntry>`：

- `entries: HashMap<PathBuf, CacheEntry>` —— manifest 解析结果 `WorkflowItem`（含 agents/phases）
- `journal_entries: HashMap<PathBuf, JournalCacheEntry>` —— 运行态合成 `Vec<WorkflowAgent>`
- `script_entries: HashMap<PathBuf, ScriptCacheEntry>` —— script 解析产物 `ScriptData`（meta + ≤32 KB preview）

三者经 `Arc<Mutex<…>>` 长驻 `LocalDataApi`，key 是磁盘脚本/manifest/journal 文件路径，进程生命周期内只增不减——无 byte/cardinality cap，违反 `perf.md` 内存类反模式。同 capability 的 `MetadataCache`（`session_metadata.rs`）、parsed-message cache（`parsed_message_cache.rs`）已是 `lru::LruCache` + count cap；`cdt-discover` 的 `SearchTextCache`（`search_cache.rs`）是 count + byte 双闸门的完整参照实现。

现状缓解（为何 pre-existing 非紧急）：key 是有限的磁盘文件路径集，单进程增量有界；script preview 已截断 32 KB、文件 >1 MB 不全读，单条上界明确。本 change 把「有界」从「依赖磁盘文件数有限」这一隐式假设升级为显式双闸门。

## Goals / Non-Goals

**Goals:**
- 三个 cache 各自从无界 `HashMap` 改为 `lru::LruCache` + count cap + byte cap 双闸门，对齐 `SearchTextCache` 模式。
- 命中 bump、超限 LRU 淘汰、签名 mismatch 扣减字节——记账闭环无泄漏。
- 行为对外透明：淘汰仅触发重读，IPC 字段 / 序列化 / 结果不变。
- 每个 SHALL 有对应单测（count 淘汰 / byte 淘汰 / mismatch 扣减 / 独立配额）。

**Non-Goals:**
- 不引入 TTL（这些 cache 靠 `FileSignature` 失效，文件不变即长期有效，无 staleness 问题；不像 SSH cache 需 TTL）。
- 不引入 `AtomicUsize`——cache 已在 `Mutex` 临界区内访问，`current_bytes` 用普通 `usize` 即可（参照 `SearchTextCache`，它也是非原子 `usize`，调用方持锁）。perf.md 写的 `AtomicUsize` 是「无锁 cache」场景，本 cache 全程持 `Mutex`。
- 不合并三个 cache 为统一预算（见 D2）。
- 不改 `WorkflowManifestCache::new()` 签名（仍无参默认配额），避免破坏 `local.rs` 两处构造点 + 测试。

## Decisions

### D1：三个 cache 各用独立 `lru::LruCache` + 独立 `current_bytes`/`max_bytes`

每个 cache 一套 `(LruCache, current_bytes: usize, max_bytes: usize)`，count cap 走 `LruCache` 自带容量，byte cap 走手工记账（`SearchTextCache::put` 同款 `push` 返回旧值扣减 + while 循环淘汰至 ≤ max_bytes 保留至少 1 条）。

候选：(a) 三个独立 `LruCache` 各自记账；(b) 把三个 cache 包进一个统一 byte 预算。选 (a)。理由：三类数据 size 分布差异大（script preview 可达 32 KB/条，journal agents 多为小 Vec，WorkflowItem 中等），独立配额更好估、更好测，且直接 3× 复刻 `SearchTextCache` 单 cache 模式，淘汰决策无需跨 cache 协调 LRU 时序。统一预算需要一个全局 LRU 顺序裁决「淘汰哪个 cache 的哪条」，复杂度不值（用户已确认选独立预算）。

### D2：配额取值

| cache | count cap | byte cap | 依据 |
|---|---|---|---|
| `entries` | 256 | 16 MiB | WorkflowItem 中等大小；一个 session 内 workflow 数有限，256 覆盖极端多 workflow session |
| `journal_entries` | 256 | 8 MiB | 合成 agents Vec 较小；与 entries 同数量级 path |
| `script_entries` | 256 | 16 MiB | 单条 preview ≤ 32 KB；256×32 KB ≈ 8 MiB，16 MiB 留 2× 余量含 meta + overhead |

count cap 统一 256（远高于单 session 实际 workflow 数，主要兜底跨 session 累积）；byte cap 按单条上界 × 余量定。这些是粗粒度上界，非精确预算——和 `SearchTextCache` 的「50 MiB 留 1/8 余量」同思路。常量集中定义 + `#[must_use] with_caps` 测试构造器注入小配额跑淘汰单测。

### D3：byte 估算函数

每个 entry 类型一个 `estimate_*_bytes`，计入：固定 overhead 常量（`size_of::<CacheEntry>() + PathBuf 估算`）+ 持有的 `String`/`Vec` capacity。

- `CacheEntry`(WorkflowItem)：递归 sum `run_id`/`name`/各 agent 的 `label`/`result_preview`/`session_id` + phases title + error 的 capacity。
- `JournalCacheEntry`：sum 各 `WorkflowAgent` 字段 capacity。
- `ScriptCacheEntry`：`meta` 的 name + phases titles + `preview` String capacity。

粗粒度即可（byte cap 本就是粗上限）。WorkflowItem/WorkflowAgent 是 `cdt-core` 类型，估算函数放本文件（`workflow_manifest.rs`），不污染 core。

### D4：get/insert 方法对外语义不变，内部改记账

现有 `get`/`insert`/`get_journal`/`insert_journal`/`get_script`/`insert_script` 六个方法的**调用语义**保持——调用方（`resolve_*`）逻辑零改动。仅内部：
- `get*`：`HashMap::get().filter()` → `LruCache::peek()` 判签名后 `get()`（bump）或 `pop()`（mismatch 扣减）。注意 `LruCache` 的 `get` 才 bump，`peek` 不 bump——先 peek 判签名再决定 get/pop，与 `SearchTextCache::get` 完全同构。
- `insert*`：`HashMap::insert` → `LruCache::push` + byte 记账 + while 淘汰（`SearchTextCache::put` 同款）。

### D4b（codex 二审 finding #1）：receiver 由 `&self` 改 `&mut self`，调用点 `mut guard`

`get*` 现为 `&self`，但 `LruCache` 命中要 `get()`(bump) / mismatch 要 `pop()`(扣减) 都需 `&mut self`，故六方法 receiver 全部改 `&mut self`。这不改变**调用语义**（参数 / 返回 / 缓存命中规则不变），但编译层要求三处读侧调用点（`resolve_single_inner` / `read_script_data` / `read_journal_agents`）的 `let Ok(guard) = cache.lock()` 改为 `let Ok(mut guard)`。原 D4 措辞「方法签名不变」不精确——准确说是「调用方逻辑零改动 + receiver mutability 升级」。

### D4c（codex 二审 finding #2/#4）：byte 估算只算 value 不算 PathBuf key + 必须含 meta.phases

- **不把 `PathBuf` key 的 capacity 计入估算**（只算 value 持有的堆字节 + `CACHE_ENTRY_OVERHEAD` 常量补足 key/node 固定开销）。原因：`LruCache::push` 在「同 key 替换」分支返回的是**新传入但未入 cache 的 key**（旧 key 仍在节点），在「容量驱逐」分支返回的是**被驱逐节点的旧 key**——若按实际 path capacity 记账，两分支语义不同需分叉处理。value-only 估算让两分支统一扣减无坑。
- `estimate_script_entry` **必须**计入 `meta.phases`（每个 `WorkflowPhase` title 的 String capacity + 定长），不能只按 preview 32 KB 估——大量 phases 的 script 会让单条远超 preview 心理模型，只算 preview 会系统性低估、byte cap 守不住。

### D5：`lru` 依赖

`cdt-api` 加 `lru = { workspace = true }`（workspace 根已声明，`cdt-discover` 在用）。零新外部依赖引入风险。

## Risks / Trade-offs

- **估算不精确**：byte 估算只算 capacity 不含 `LruCache` 内部 node 真实分配，可能低估 → 实际 RSS 略高于 byte cap。可接受：cap 是粗上界，余量已留；与 `SearchTextCache` 同等精度。
- **淘汰后重读 I/O**：被淘汰条目下次访问重 stat+读盘。对 script（immutable）/manifest 是一次 fs 读，热路径影响极小（cap 256 远大于单 session workflow 数，稳态不触发淘汰）；只有跨大量 session 长驻进程才会淘汰冷 entry，正是期望行为。
- **配额拍脑袋**：256 / 8–16 MiB 是经验值非实测。风险低——这是兜底上界，正常工作集远低于此；若未来实测偏小可调常量。
- **独立预算的类间 churn 失衡（codex finding #5）**：独立预算解决了类间互相挤占，但不覆盖「某类真实工作集远大于另两类」的命中率/IO 放大——如长驻进程几乎只浏览 script preview，则 script 的 16 MiB 频繁淘汰而 journal 的 8 MiB 长期闲置、无法借用。这是独立预算的已知 tradeoff（用户已确认选独立），非内存泄漏；若未来实测某类 churn 显著可单独调该类常量或再评估统一预算。
- **无主动回落（codex finding #6）**：不引 TTL，缓存靠 LRU 替换 + `FileSignature` 失效，count cap 256/类封住条目数上界。长驻进程扫过大量 session 后即便最终停在小 session，仍保留最近 ≤ 3×256 条直到后续访问触发 LRU 替换——不会空闲自动回落。issue #565 验收要求是「有界」，本设计满足（双闸门封死上界）；「空闲回落」非本 change 目标，显式声明为已知限制。
- **`peek` vs `get` 易错**：`LruCache` 命中必须用 `get` 才 bump，签名判定阶段误用 `get` 会对 mismatch 条目也 bump。已由 D4 明确 peek→get/pop 顺序 + 单测覆盖 bump 行为兜底。

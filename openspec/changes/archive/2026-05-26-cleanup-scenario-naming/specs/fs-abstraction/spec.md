# fs-abstraction Spec Delta

## MODIFIED Requirements

### Requirement: `FileSystemProvider` trait 暴露 7 个核心方法

fs provider trait SHALL 暴露以下行为类别（编译时强制实现，default 实现可被 override）：

- **identity**：返回 backend 类型（Local / Ssh / 等）的同步访问器
- **目录探测**：判路径存在；列举目录条目（仅类型）；列举目录条目并附 metadata
- **元数据**：取 path 的 `FsMetadata`（含 size / mtime / `identity: Option<FsIdentity>`）；批量 stat 多 path（default 实现走并发等价 join_all，后端可 override 拿真 batch）
- **读**：全量读为 UTF-8；仅读首 N 行；返回流式 `AsyncRead + Send + Unpin` 句柄
- **写**：原子覆盖写（tmp + rename，rename 失败 best-effort 清理 tmp）；递归创建目录（已存在不报错）；删文件（不存在返 `FsError::NotFound`，不递归删目录）

trait SHALL 保持 dyn-safe（`&dyn` 可用），不引入关联类型。

**`read_dir_with_metadata` override 契约**：default 实现走 1 + N 兜底（`read_dir` 后逐条 stat），性能敏感后端 SHALL override 使用底层协议原生 1-RTT batch。SSH override 可能因 server 未返 mtime 出现部分 entry metadata = None；caller SHALL 把此条视同 cache mismatch（走 cache wrapper miss 路径补齐），实现 SHALL NOT 在 trait 实现层做 per-entry stat fallback（否则退化为 N+1 RTT）。

**原子写契约**：原子覆盖在 reader 角度 SHALL 永远观察到旧内容或新内容整版，永不观察到截断 / 半写状态。Local 后端依赖 OS rename(2) 原子语义；SSH 后端 SHALL 优先走 SFTP `posix-rename` 扩展，不支持时降级（降级路径有极短窗口 reader 可见 target missing，acceptable）。tmp 路径 suffix SHALL 使用进程内单调计数器 + pid hex 防并发碰撞，**不**得依赖 wall-clock 纳秒（Windows 时钟精度并发碰撞）。同 path 多 caller 并发写 SHALL 是 last-write-wins 语义。

**新写方法的 instrumentation 计数**：fs op counter 类型 SHALL 暴露写 / 创建目录 / 删文件三个独立计数字段；wrapper 在三个新方法的入口同步 record，与读侧已有计数路径一致；tracing 输出 SHALL 包含三个新字段。

#### Scenario: open_read 在 Local 上返回流式句柄

- **WHEN** caller 在 Local provider 上调 open_read 且 path 存在可读
- **THEN** SHALL 返回 `Ok(Box<dyn AsyncRead + Send + Unpin>)` 等价句柄
- **AND** caller SHALL 能用 buffered line reader 流式按行读取

#### Scenario: open_read 在 SSH 上走 SFTP 流式句柄

- **WHEN** caller 在 SSH provider 上调 open_read
- **THEN** SHALL 返回 SFTP 流式句柄包装，实现 `AsyncRead + Send + Unpin`
- **AND** caller SHALL NOT 需要 downcast 到 SSH provider 才能流式读

#### Scenario: stat_many 默认实现并发执行

- **WHEN** caller 调 stat_many 且 provider 未 override
- **THEN** 实现 SHALL 并发对所有 path 走 stat
- **AND** 返回结果顺序与 input paths 严格对应

#### Scenario: read_dir_with_metadata 未优化路径走 N+1 RTT

- **WHEN** 某后端未 override read_dir_with_metadata
- **THEN** SHALL 走 trait default：先 read_dir 拿 entries，再对每条 file entry 补 stat
- **AND** 总 op 数 SHALL 为 1 + N（N = file entry 数）

#### Scenario: SSH 路径 read_dir_with_metadata 复用 read_dir 不退化

- **WHEN** 在 SSH context 下调 read_dir_with_metadata
- **THEN** SSH provider SHALL override default impl，复用单次 SFTP READDIR reply 自带的 entry attrs
- **AND** 总 SFTP RTT 数 SHALL 为 1（**SHALL NOT** 退化为 N+1）
- **AND** 部分 entry mtime missing 时 SHALL 在 metadata = None 状态返 caller，由上层 cache 校验决定 fallback——**不**在 trait 实现层补 stat

#### Scenario: 原子写在 Local 上原子覆盖

- **WHEN** caller 在 Local provider 上调原子写且目标 path 已有旧内容
- **THEN** 实现 SHALL 走 tmp + rename
- **AND** 并发 reader SHALL 永远拿到旧 content 或新 content 整版，绝不拿到截断 / 半写中间态
- **AND** rename 失败 SHALL best-effort 清理 tmp（清理失败不向上传播）

#### Scenario: 原子写在 SSH 上原子覆盖

- **WHEN** caller 在 SSH provider 上调原子写远端目标 path
- **THEN** 实现 SHALL 通过 SFTP 写到 tmp 后调原子 rename
- **AND** 远端 SFTP server side OS 提供原子性保证
- **AND** 暂态错误重试遵循既有 SSH retry 策略（指数退避，上限 3 次）

#### Scenario: create_dir_all 不报已存在错

- **WHEN** caller 调 create_dir_all 路径已存在
- **THEN** SHALL 返 `Ok(())`，不返 AlreadyExists 错误

#### Scenario: remove_file 不存在返 NotFound

- **WHEN** caller 调 remove_file 路径不存在
- **THEN** SHALL 返 `Err(FsError::NotFound(path))`

### Requirement: `xtask check-fs-direct-calls` 自动化 H1

系统 SHALL 提供 H1 自动化扫描入口（xtask 命令或等价脚本），扫描业务 crate 内底层 fs API 直调反模式。脚本行为契约：

1. 扫描路径：所有业务 crate 源码（discover / api / config / ssh / cli / watch / analyze / parse / core 内非 provider 实现路径）
2. allowlist：从 fs 抽象 crate 内的 ALLOWLIST 文档解析（豁免准则：design 已分类的 Local-only 业务路径 / SSH 路径有显式 graceful skip / 测试 fixture 写文件）
3. 匹配模式：13 个底层 fs API 直调形态（含 metadata / read / open / read_to_string / read_dir / write / create_dir / create_dir_all / remove_file / remove_dir / remove_dir_all 等）
4. 退出码：non-allowlist 命中时 SHALL 默认非零退出（CI 拒），仅本地诊断 opt-in 时降级为 warn 级输出 + 零退出

#### Scenario: 扫描命令存在且可调用

- **WHEN** 在仓库根触发 H1 扫描入口
- **THEN** 命令 SHALL 存在并产出扫描结果到 stdout
- **AND** 退出码反映检查结果

#### Scenario: allowlist 路径不报警

- **WHEN** 扫描时遇到 fs 抽象 crate 内部 provider 实现的底层 fs API 调用
- **THEN** SHALL NOT 报警（被 allowlist）

#### Scenario: 默认禁直接调用并由 CI 拦截

- **WHEN** CI 跑 H1 扫描（不带本地诊断 flag），业务路径出现一处非 allowlist 的底层 fs API 直调
- **THEN** 进程 SHALL 以非零状态退出，CI step fail
- **AND** 输出 SHALL 含违例文件 / 行号 / 模式 / 总计违规数 + 指向 ALLOWLIST 文档的指针

#### Scenario: 本地诊断绕过

- **WHEN** 开发者本地用诊断 flag 跑扫描
- **THEN** SHALL 以零状态退出 + warn 级前缀列出违规清单
- **AND** CI workflow SHALL NOT 启用此诊断 flag

#### Scenario: ALLOWLIST 文档顶部固化豁免准则

- **WHEN** 阅读 ALLOWLIST 文档
- **THEN** 文档 SHALL 在 allowlist 表之前明示豁免准则（Local-only 业务路径、SSH graceful skip、测试 fixture 等）
- **AND** 任何新加 ALLOWLIST 行的 PR SHALL 在 PR description 引用对应 design 决策的锚点

### Requirement: fs-related cache 必须采用"单实例 + ContextId key 前缀"拓扑

任何持有 `FsMetadata` / `FsSignature` / 解析后的 jsonl 消息 / `ProjectScanner::scan` 结果等 fs-derived 数据的 cache SHALL 采用以下拓扑：

1. **单实例**：`LocalDataApi` 持有该 cache 的**一个** `Arc<Mutex<...>>` / `Arc<RwLock<...>>` 实例，**不得**为每个 `ContextId` 创建独立 cache 实例
2. **key 含 ContextId 前缀**：cache 的 key 类型 SHALL 是 `(ContextId, ...)` 形式 tuple（或等价 struct），其中 `ContextId` 是第一成员
3. **LRU 容量按全局计算**：容量上限对所有 `ContextId` 总和适用，不按 context 拆分配额
4. **switch_context 时不必清 cache**：不同 `ContextId` 的 entry 自然不命中（依赖 Hash/Eq 隔离），TTL + signature 校验照常工作

本 change **不**改 `MetadataCache` / `ParsedMessageCache` 现状（PR-B/C 才动），但本 Requirement 是 PR-B/C 必须遵循的 SHALL 句——若 PR-B/C 选了"每 ContextId 一个实例"拓扑，违反本 Requirement，spec validate 应拒。

#### Scenario: cache 实例只有一个

- **WHEN** 检查 `LocalDataApi` 字段
- **THEN** `metadata_cache` SHALL 是单一 `Arc<Mutex<MetadataCache>>` 字段，**不得**是 `HashMap<ContextId, Arc<Mutex<MetadataCache>>>` 类型

#### Scenario: cache key 含上下文身份

- **WHEN** 检查 `MetadataCache` / `ParsedMessageCache` 内部 `HashMap` 类型
- **THEN** key 类型 SHALL 是 `(ContextId, PathBuf)` 或等价 newtype，**不得**仅 `PathBuf`

#### Scenario: 跨 context 复用同一 cache 实例

- **WHEN** 用户在 Local context 与 SSH context A 之间频繁切换
- **THEN** 同一 `MetadataCache` 实例 SHALL 同时持有两个 context 的 entry
- **AND** SSH context A 的 entry SHALL NOT 因切回 Local 被自动清除（依赖 LRU + TTL 自然淘汰）

### Requirement: `BackendPolicy` enum 雏形定义

fs 抽象 crate SHALL 定义 backend 行为策略 struct + 三个独立 enum 字段，作为业务路径**选择后端相关行为**的真相源：

- **InitialLoadPolicy**：表达"首屏加载策略"（一次性 vs 增量补全），至少含 `FullEager` 与 `SkeletonThenStream` 两个 variant
- **PrefetchPolicy**：表达"翻页预取策略"（不预取 vs 预取下一页），至少含 `None` 与 `PrefetchNext` 两个 variant
- **StaleCheckStrategy**：表达"5min stale 判定策略"，至少含 `LocalClock5min`（用本机 mtime 比对）与 `SkipUntilClockSync`（远端 mtime 跨 clock domain 不可比对）

`BackendPolicy` struct SHALL 是 Copy + Eq + Clone + Debug；字段 SHALL 是 primitive 或 Copy 类型（bool / u8 / Copy enum），**禁止**含 `Arc<dyn Trait>` / `Box<dyn Trait>` / `Vec<T>` / `String` 等非 Copy 字段——业务侧的 trait object 与 Clone 类型策略 SHALL 放在更高层（业务 IPC 层 backend resolvers）与 `BackendPolicy` 配套。

`PrefetchPolicy` 与 `InitialLoadPolicy` SHALL 是**正交字段**——**不得**把 `PrefetchNext` 塞进 `InitialLoadPolicy` 当 variant。

本 capability SHALL 提供三个 const 构造器：Local / SSH / HTTP 各一个，每个完整覆盖所有字段值。业务 callsite SHALL 通过 `BackendPolicy` 字段读取 backend-specific 行为，**禁止**在业务算法层直接 `match fs.kind()` 表达策略——`fs.kind()` 仅允许在策略**派生**点（顶层 helper 或 backend resolver 内部）使用。

#### Scenario: Local policy 含 SkeletonThenStream + 本机 stale

- **WHEN** 取 Local backend 的 policy 构造器结果
- **THEN** SHALL 返回 `initial_load_policy = SkeletonThenStream`、`max_round_trips_for_initial_page >= 2`、`supports_incremental_updates = true`、`prefetch_policy = None`
- **AND** SHALL 返回 `supports_memory = true`、`supports_subagent_scan = true`、`stale_check_strategy = LocalClock5min`

#### Scenario: SSH policy 含 FullEager + 跳过 stale

- **WHEN** 取 SSH backend 的 policy 构造器结果
- **THEN** SHALL 返回 `initial_load_policy = FullEager`、`max_round_trips_for_initial_page = 1`、`supports_incremental_updates = false`、`prefetch_policy = None`
- **AND** SHALL 返回 `supports_memory = true`、`supports_subagent_scan = false`、`stale_check_strategy = SkipUntilClockSync`

#### Scenario: HTTP policy 按 Local 数据源语义填

- **WHEN** 取 HTTP backend 的 policy 构造器结果
- **THEN** initial-load 相关字段 SHALL 保持现状（`FullEager` / `max_round_trips_for_initial_page = 1`）
- **AND** 其余字段按 Local 数据源语义填（`supports_memory = true` / `supports_subagent_scan = true` / `LocalClock5min`）

#### Scenario: PrefetchPolicy 与 InitialLoadPolicy 正交

- **WHEN** 显式构造 `BackendPolicy` 含 `SkeletonThenStream` + `PrefetchNext`
- **THEN** SHALL 编译通过且 `==` 自身（两字段独立可组合）
- **AND** `InitialLoadPolicy` 内 SHALL NOT 出现 `PrefetchNext` variant

#### Scenario: BackendPolicy 可按值复制并相等比较

- **WHEN** 编译 fs 抽象 crate
- **THEN** `BackendPolicy` SHALL derive Copy + Clone + PartialEq + Eq + Debug
- **AND** 所有字段 SHALL 是 primitive 或 Copy 类型，**禁止**含 `Arc<dyn Trait>` / `Box<dyn Trait>` / 非 Copy 容器

#### Scenario: 业务代码通过 BackendPolicy 字段选择行为

- **WHEN** 业务 IPC handler 需要根据后端类型选择行为
- **THEN** handler SHALL 读 `BackendPolicy` 字段，**不得**新增 `if fs.kind() == Ssh` / `let is_remote = ...` / `matches!(fs.kind(), ...)` 等等价直接派生
- **AND** `fs.kind()` 比对仅允许出现在策略**派生**点（顶层 helper / backend resolver 内部 / fs 抽象 crate 自身实现）

#### Scenario: StaleCheckStrategy 至少含本机时钟与跨时钟域两种策略

- **WHEN** 编译 fs 抽象 crate
- **THEN** `StaleCheckStrategy` SHALL 至少含 `LocalClock5min` 与 `SkipUntilClockSync` 两个 variant
- **AND** SHALL derive Copy + Clone + PartialEq + Eq + Debug
- **AND** 调用方对该字段 match SHALL 通过 exhaustive 检查

### Requirement: Provider instrumentation 入口可观测 fs op 次数

fs 抽象 crate SHALL 提供 `InstrumentedFs<P>` wrapper（`P` 是 fs provider）+ counter 类型 + counter 入口函数，让业务调用方可在每个 IPC command 边界统计 fs 操作次数（stat / read / read_dir / read_dir_with_metadata / read_to_string / read_lines_head / open_read / stat_many / write_atomic / create_dir_all / remove_file 各计数）。

**注入机制契约**：counter 通过 `InstrumentedFs` wrapper 在 trait 调用边界自动计数，**不**要求每个 provider 实现内嵌 record hook。具体语义：

1. wrapper 实现 fs provider trait，每个 trait 方法内部先 record 当前 counter，再 delegate 到 inner provider
2. 调用方注入 fs handle 时包一层 wrapper；测试 fake provider 同样包 wrapper 即可，不需要修改 fake 内部代码
3. 未包 wrapper 的 fs handle 调 trait 方法不计数（向后兼容）

counter 入口 SHALL 满足：

1. 基于 task-local 实现，避免全局 atomic 让并发 IPC command 互相干扰
2. async wrapper 函数让调用方包住代码块，结束后拿计数
3. wrapper 在 trait 调用边界自动 record，无需 provider 实现配合
4. 与日志 facade 集成——counter Drop 时自动 emit 一条结构化 event，含每种操作的次数

#### Scenario: fs op 在 trait 边界自动计数

- **WHEN** 调用方包 wrapper 后用 counter 入口跑一段含若干 fs op 的代码
- **THEN** 返回的 counter snapshot SHALL 含每种 op 的实际计数
- **AND** provider 实现 SHALL NOT 含任何 counter 调用（计数发生在 wrapper 层）

#### Scenario: 调用方未启用 instrumentation 时不计数

- **WHEN** 调用方直接用 provider（未包 wrapper）+ 调 counter 入口
- **THEN** counter snapshot SHALL 全 0
- **AND** SHALL NOT panic（向后兼容）

#### Scenario: counter 不跨 task 污染

- **WHEN** 两个并发 task 各自调 counter 入口 + 各自的 wrapper
- **THEN** 两 task 的计数 SHALL 互不影响（依赖 task-local 隔离）

#### Scenario: fs op counter 入口结束时输出诊断

- **WHEN** counter 入口闭包正常结束
- **THEN** SHALL emit 一条结构化日志 event 含全部计数字段

### Requirement: 本 change 零业务变化下性能基线不退化

本 change 是基建 PR-A，原则上**零业务代码变化**——但 trait 加 `Box<dyn AsyncRead>` 动态分发改了底层 LocalFileSystemProvider 内部路径（之前调用方拿到 inherent typed File，现在拿 Box dyn）。系统 SHALL 通过两套性能 gate 验证零退化：

1. **端到端 baseline 校验**：`cargo test --release -p cdt-api --test perf_cold_scan -- --ignored --nocapture` 与 `perf_get_session_detail` 在本 change apply 前后各跑 **5 次**取 min / median / stddev。回归判据：
   - median 退化 > 5% → 拒
   - stddev > 8ms（baseline 95ms 的 ~8%）→ 拒（说明引入了不稳定性）
   - min 退化 > 8% → 拒
2. **Local micro benchmark**（D4 量化要求）：新增 `crates/cdt-fs/benches/open_read_overhead.rs`，对比同 jsonl 文件（~500KB 与 ~5MB 两个 size）走 `tokio::fs::File::open + BufReader::lines` 直读路径 vs 走 `FileSystemProvider::open_read` dyn 路径，跑 10 次取 min / median / stddev。dyn 路径 SHALL 在 median 上 ≤ 直读路径 × 1.3（vtable overhead 上限），超过则拒

性能 gate SHALL 在本 change apply commit 上有 reproducible 数据（PR 描述贴 `/usr/bin/time -lp` 四维输出 + micro bench 结果），不只口头声称"零变化"。

#### Scenario: 端到端 baseline 不退化

- **WHEN** apply 本 change 后跑 `perf_cold_scan` 5 次
- **THEN** median SHALL ≤ 主线 baseline × 1.05
- **AND** stddev SHALL ≤ 8ms

#### Scenario: open_read 动态分发路径开销不超单态化的 1.3x

- **WHEN** 跑 `cargo bench -p cdt-fs --bench open_read_overhead` 10 次
- **THEN** `fs.open_read` dyn 路径的 median 耗时 SHALL ≤ `tokio::fs::File::open` 直读路径 × 1.3
- **AND** 若超过 1.3x，本 change PR review 拒，需重新评估 D4 决策（关联类型 vs dyn dispatch trade-off）

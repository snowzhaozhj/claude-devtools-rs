## MODIFIED Requirements

### Requirement: `cdt-fs` crate 是 fs 抽象的唯一物理位置

系统 SHALL 把所有文件系统抽象类型（fs provider trait、其 Local 实现、`FsError` / `FsMetadata` / `FsKind` / `FsIdentity` / `DirEntry` / `EntryKind` / `ContextId` / `BackendPolicy` / `InitialLoadPolicy`）的**真相源**集中于专用的 fs 抽象 crate。其它 crate（业务侧 / 历史 discover crate）SHALL 通过 re-export 兼容历史 import 路径，**不得**重新定义同名类型。业务 crate SHALL 直接依赖 fs 抽象 crate，不得通过历史 discover crate 间接拿 fs 抽象。

fs 抽象 crate SHALL **不**依赖任何业务 crate；允许的运行时依赖仅限通用基础设施（async runtime / async-trait / 错误派生 / 日志 facade）。

#### Scenario: 业务 crate 直接依赖 fs 抽象 crate

- **WHEN** 业务 crate 需要 fs provider trait
- **THEN** 其 manifest SHALL 直接依赖 fs 抽象 crate
- **AND** import 首选直接路径；通过历史 discover crate 的间接 import 仅作为兼容期保留

#### Scenario: 兼容性 re-export 保持等价

- **WHEN** 老代码通过历史 discover crate import fs 抽象类型
- **THEN** 编译 SHALL 成功，运行时行为与直接 import fs 抽象 crate 完全一致

#### Scenario: fs 抽象 crate 不依赖业务 crate

- **WHEN** 检查 fs 抽象 crate 的 manifest 依赖
- **THEN** SHALL NOT 含任何业务 crate（discover / api / config / ssh / cli / watch / analyze / parse / core）
- **AND** 允许的依赖仅限运行时 / 错误 / 日志基础设施

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

#### Scenario: read_dir_with_metadata default impl 是 N+1 RTT 兜底

- **WHEN** 某后端未 override read_dir_with_metadata
- **THEN** SHALL 走 trait default：先 read_dir 拿 entries，再对每条 file entry 补 stat
- **AND** 总 op 数 SHALL 为 1 + N（N = file entry 数）

#### Scenario: SSH override read_dir_with_metadata 复用 read_dir 不退化

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

### Requirement: H1-H6 六条硬契约 SHALL 通过 enforce 机制守护

本 capability SHALL 守护以下六条硬约束（H1-H6），作为 fs 抽象边界的代码组织契约。每条 SHALL 由对应 enforce 机制（自动化测试 / xtask / 单测 / PR review）守护，**不**单独依赖独立的散文档存在。具体守护脚本 / 测试名 / allowlist 内容随代码组织演进，行为契约由本 Requirement 兜底。

- **H1**：业务路径**禁止**直调底层 OS fs API（统一走 fs 抽象 trait）；豁免清单 SHALL 单源住在 fs 抽象 crate 内的人类可读 allowlist 文件，xtask 与 build-time 集成测试 SHALL 在运行时 parse 此文件作为唯一输入。**Enforce**：xtask 默认 fail-on-match（CI 拒绝违例），本地诊断模式可降级为 warn-only。
- **H2**：hot path（list / 翻页 / 详情）**禁止** N 次串行 stat / read；SHALL 走带 metadata 的 batched API。**Enforce**：fs op counter instrumentation tracing histogram + 集成测试用 fake provider 断言 fs op 上限 + PR review checklist。
- **H3**：业务**算法**代码 `fs.kind() == Ssh` 默认拒；业务**策略**层允许但 SHALL 配 ADR + inline 注释，且**只允许选 BackendPolicy 字段值，不允许复制业务算法**。**Enforce**：PR review checklist 按 D6 分类表逐行复核。
- **H4**：HTTP backend SHALL 默认 FullEager + max_round_trips_for_initial_page=1；Tauri 本地 backend SHALL 默认 SkeletonThenStream；transport 层抽象延后处理。**Enforce**：BackendPolicy 构造器单测断言 + wire 时单测断言 HTTP backend policy。
- **H5**：fs provider trait **不**承担分页 / 排序语义——按 mtime 拿前 N 走更高层（session index / project repository），不污染 fs trait。**Enforce**：集成测试 AST parse trait 方法签名，禁含 `Cursor / Offset / Limit / SortBy / Order` 类型。
- **H6**：`FsError` 必须可操作 —— `is_retryable / should_invalidate_cache / is_likely_channel_dead` 元方法是 trait 契约的一部分。**Enforce**：单测覆盖每个 variant 的元方法返回值。

#### Scenario: H1 allowlist 单源真相

- **WHEN** xtask / build-time 集成测试需要 H1 allowlist
- **THEN** 数据来源 SHALL 是 fs 抽象 crate 内的 ALLOWLIST 文档；**禁止** xtask 源码或测试源码硬编码 allowlist 副本

#### Scenario: H5 fs trait 不暴露排序参数

- **WHEN** CI 跑 trait 方法签名检查
- **THEN** SHALL 自动化检验 trait 方法参数 type 不含 `Cursor` / `Offset` / `Limit` / `SortBy` / `Order`
- **AND** 测试 fail 时 CI 拒，错误信息含 method name + violating arg type + 指向本 spec H5 + design.md

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

#### Scenario: 默认 fail-on-match（CI enforce）

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

#### Scenario: wrapper 在 trait 边界自动计数

- **WHEN** 调用方包 wrapper 后用 counter 入口跑一段含若干 fs op 的代码
- **THEN** 返回的 counter snapshot SHALL 含每种 op 的实际计数
- **AND** provider 实现 SHALL NOT 含任何 counter 调用（计数发生在 wrapper 层）

#### Scenario: 未包 wrapper 不计数

- **WHEN** 调用方直接用 provider（未包 wrapper）+ 调 counter 入口
- **THEN** counter snapshot SHALL 全 0
- **AND** SHALL NOT panic（向后兼容）

#### Scenario: counter 不跨 task 污染

- **WHEN** 两个并发 task 各自调 counter 入口 + 各自的 wrapper
- **THEN** 两 task 的计数 SHALL 互不影响（依赖 task-local 隔离）

#### Scenario: tracing emit on Drop

- **WHEN** counter 入口闭包正常结束
- **THEN** SHALL emit 一条结构化日志 event 含全部计数字段

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

#### Scenario: BackendPolicy 是 Copy + Eq 类型

- **WHEN** 编译 fs 抽象 crate
- **THEN** `BackendPolicy` SHALL derive Copy + Clone + PartialEq + Eq + Debug
- **AND** 所有字段 SHALL 是 primitive 或 Copy 类型，**禁止**含 `Arc<dyn Trait>` / `Box<dyn Trait>` / 非 Copy 容器

#### Scenario: 业务代码通过 BackendPolicy 字段选择行为

- **WHEN** 业务 IPC handler 需要根据后端类型选择行为
- **THEN** handler SHALL 读 `BackendPolicy` 字段，**不得**新增 `if fs.kind() == Ssh` / `let is_remote = ...` / `matches!(fs.kind(), ...)` 等等价直接派生
- **AND** `fs.kind()` 比对仅允许出现在策略**派生**点（顶层 helper / backend resolver 内部 / fs 抽象 crate 自身实现）

#### Scenario: StaleCheckStrategy enum 至少包含 LocalClock5min 与 SkipUntilClockSync

- **WHEN** 编译 fs 抽象 crate
- **THEN** `StaleCheckStrategy` SHALL 至少含 `LocalClock5min` 与 `SkipUntilClockSync` 两个 variant
- **AND** SHALL derive Copy + Clone + PartialEq + Eq + Debug
- **AND** 调用方对该字段 match SHALL 通过 exhaustive 检查

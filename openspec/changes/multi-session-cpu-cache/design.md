## Context

诊断起点：用户报告"多个会话同时活跃时 CPU 占用经常飙高"。前端已有 `scheduleRefresh` 250ms leading+trailing 节流（`ui/src/lib/fileChangeStore.svelte.ts`），但后端有两条独立链路完全绕过前端节流：

1. `NotificationPipeline::process_file_change`（`crates/cdt-api/src/notifier.rs:91`）：每条 `FileChangeEvent` 直接 `parse_file()` 整个 JSONL（`cdt-parse::file::parse_file` line-by-line stream），再跑所有 trigger 的 `detect_errors`。多 session 并发活跃 → 每秒数十次全文件 parse。
2. `extract_session_metadata`（`crates/cdt-api/src/ipc/session_metadata.rs:114`）：line-by-line 读取整个 JSONL + 构造 `Vec<ParsedMessage>` 给 `check_messages_ongoing` 用。`list_sessions` 后台扫描（`crates/cdt-api/src/ipc/local.rs:582`）每次 spawn 都对**整页** session 全跑——即便只有一个 session 真有文件变化。Sidebar 每次 file-change 都触发 `loadSessions(silent)`，N session 项目下意味着 N 倍 CPU。

两处的共同点：**重复处理同一份未变化的文件**。文件 `FileSignature`（mtime + size + identity，详 D1b/D1d）是 Linux/macOS/Windows 通用的廉价 stat 字段组合，在常规 append-only 写入路径下足以判定 JSONL 是否真有新内容（best-effort，详 D1d）。

约束与边界：
- 行为契约**接近等价**（best-effort）。缓存命中时返回的结果在常规 append-only 写入路径下与原路径等价；但承认 inode reuse + mtime/size 撞车的极端边界 case（详 D1d）—— 此时漏检会被下一次任何文件变化的 file-change 自然恢复。
- JSONL 是 append-only 文件，正常路径下 size 单调递增；rotate / truncate 极少见。
- 多线程并发访问：notifier 单 task loop 只有一个调用者，无并发写；`extract_session_metadata` 同时被 `list_sessions_sync`（HTTP）+ `list_sessions` 后台扫描 + `get_session_detail`（refreshDetail 不调，但其它路径调）调用，需要 `Mutex` 或 `RwLock` 保护缓存 map。
- 内存上限：每个 cache entry ≈ `SessionMetadata` ≈ 数百字节，200 entries = 几十 KB，可忽略。

## Goals / Non-Goals

**Goals:**
- 多 session 活跃场景下，notifier CPU ≈ 0（仅 stat 调用）；list_sessions 元数据扫描 CPU ∝ 真有变化的 session 数（而非整页）
- 实现保守：`FileSignature` 任一字段（mtime / size / identity）不一致 / stat 失败 / 任何异常都走 cache miss 重 parse 路径
- 行为零回归：现有 notifier / session_metadata 单测全部不动通过
- 缓存契约写进 spec，让未来 reviewer 知道为啥有这层

**Non-Goals:**
- **不**做真增量 parse（按 byte offset 续读 JSONL）——增量 parse 涉及解析状态保留、行边界对齐、tool_use/tool_result 跨行链接等复杂性，本 change 不引入。整文件 parse + 缓存命中跳过已经能解决多会话场景的核心 CPU 热点；增量 parse 留作未来优化。
- **不**做后端 file-change 事件 coalesce（跨 session 短窗口合并）——每条事件都走缓存检查后秒返回，coalesce 收益边际且增加事件丢失风险。
- **不**改前端节流窗口——250ms 已经够用。
- **不**新增依赖（`lru` crate 不引入，自实现简易 LRU）。
- **不**优化 `get_session_detail` 路径——该路径已通过前端 250ms `scheduleRefresh` 节流，且 detail 路径用户主动打开 session 才进入，不是多会话被动 CPU 热点。

## Decisions

### D1（历史初版，被 D1b/D1d superseded）：cache key 应包含 mtime + size

候选方案：
- A（初版采纳，后被 D1b 升级为 `FileSignature`）：`(SystemTime mtime, u64 size)` 作组合 key
- B：仅 mtime
- C：内容 hash（blake3）

> 注：本节是 design 的历史决策记录，**最终实现按 D1b（加 identity 维度）+ D1d（best-effort 限制）**。保留 D1 是为审计候选 A 何以演进至 `FileSignature`。

选 A（初版理由）：
- mtime 单独不足：macOS HFS+ / APFS mtime 精度 1s，单秒内 append 多次 mtime 不变（虽然现代 APFS 有 ns 精度，但保守起见叠加 size 兜底）
- Windows NTFS mtime 默认 100ns 精度，但 stat 调用时机和实际写入时机有微妙差异
- size 廉价（同一次 `tokio::fs::metadata` 调用就能拿到）
- 内容 hash 需要读全文件，违背"省 IO"的初衷
- JSONL append-only 特性下，size 单调递增，size 变小（truncate / rotate）走 miss 分支重 parse 是正确兜底

### D1b（修订采纳）：cache 签名加 (dev, ino) 兜底文件身份替换

codex 异构审查发现：D1 的 ``FileSignature`` 在"同尺寸文件被 rename 替换"场景下可被欺骗：

- 攻击/事故路径：用户在外部把 `<session>.jsonl` mv 到备份名，再放一份**完全相同 size、相同 mtime（手动 `touch -t`）但内容不同**的文件回原位 → notifier 缓存假命中、永久跳过 `parse_file`
- 现实路径：日志 rotate 工具偶尔会先生成新文件再 rename 替换；若 rotate 工具以原 `FileSignature` 替换（罕见但可能）也会假命中
- JSONL 一般是 Claude Code 原地 append，不走 rotate；但作为缓存契约，"等价"需要更强保证

修订采纳：cache key 在 Unix 上加 `(dev: u64, ino: u64)` 维度，构成 `(dev, ino, mtime, size)` 四元组；`tokio::fs::metadata` 返回的 `Metadata` 在 Unix 通过 `MetadataExt::dev() / ino()` 拿到。Windows 上 `MetadataExt::file_index() / volume_serial_number()` 提供等价语义（NTFS file ID + volume serial）。

跨平台抽象：定义内部 `FileSignature` struct，含 `mtime: SystemTime`、`size: u64`、`identity: FileIdentity`，其中 `FileIdentity`：
- Unix：`(dev: u64, ino: u64)`
- Windows：`(volume_serial: u32, file_index: u64)`
- 其它（不应触发，但兜底）：仅依赖 `FileSignature`（退化）

`FileSignature` 整体 `PartialEq`：所有字段都一致才算命中。任一不一致（含 identity 变化即 inode/file_index 替换）走 cache miss。

代价：每次 stat 多读 2 个 `u64` 字段，性能影响可忽略。spec 措辞需描述身份维度的纳入；具体措辞最终走 D1e 的 best-effort 表述。

### D1c（澄清）：spec delta 的"完全等价"措辞调整为 best-effort

按 D1b，原 proposal 与 spec 中"行为契约零变化 / 缓存命中等价"的措辞需要明确：等价性建立在 `FileSignature`（含 dev/ino 或等价 file identity）一致的前提下，且**仅在常规 append-only 写入路径下成立**。极端绕过路径（详 D1d）由 spec 显式列为 best-effort。

### D1d（修订采纳）：承认 inode reuse + mtime/size 撞车的 best-effort 限制

codex 第二轮二审发现：D1b 加的 `(dev, ino)` 维度仍有 inode reuse 漏洞——

- 路径：file A 被删除 → file B 在同一秒（HFS+ / 旧 ext 1s mtime 精度）被分配同 inode + 写入相同 size + 系统时钟位于同一秒 → `FileSignature` 三元组撞车（mtime 秒级一致 + size 一致 + identity 一致）
- 概率：极低（要 inode 复用 + size 撞车 + mtime 秒级撞车三连）；现实中 Claude Code 不会主动 delete-then-recreate session JSONL，rotate 工具也罕见。但 spec 不能写"完全等价"否则 codex 二审持续不通过

候选方案：
- A（**采纳**）：承认 best-effort，spec 措辞下调为"在常规 append-only 写入路径下等价"，列出已知例外。漏检由下次任何文件变化的 file-change 自然恢复（Claude Code 持续 append 会让 size 单调增加 → 必然 cache miss → 重 parse）
- B：再加 ctime（Unix `metadata.ctime() / ctime_nsec()`）+ Windows `ChangeTime`。但 Windows `std::fs::Metadata` 不直接暴露 `ChangeTime`，需要 `windows-sys` 调 `GetFileInformationByHandle` 拿 `FILE_BASIC_INFO.ChangeTime`，引入平台特异 unsafe 代码与 std 之外的 syscall——成本高于收益
- C：内容 hash（blake3 头部 N KB）。读 IO 开销违背"省 IO"初衷

选 A 因为：
- 漏检影响极小：notifier 漏一次 detect 不影响后续；metadata 漏检让 sidebar 短暂显示旧 message_count 至下次 file-change（≤ 100ms 后）
- B 引入 Windows 平台特异 syscall 与 unsafe，code 复杂度激增
- C 违背初衷
- 真正想强等价只能走 hash，与本 change 目标矛盾

实施层影响：`FileSignature` 仍按 D1b 设计（mtime + size + identity），但 spec 措辞调整为 best-effort（详下一条）。

### D1e（spec 措辞最终版）：spec 用"在常规 append-only 写入路径下等价"

最终 spec delta 中所有"完全等价 / 完全一致"措辞统一改为：
- "调用方 SHALL 通过 `FileSignature` 比对决定是否走 cache hit 路径"（描述实现机制，不做行为契约承诺）
- Requirement 顶部 Purpose 段加一句"等价性是 best-effort：在常规 append-only 写入路径下成立；inode reuse + mtime/size 撞车等极端场景可能假命中，由后续 file-change 自然恢复"
- 删除"行为契约零变化"措辞

### D2：缓存数据结构——自实现 LRU 而非引入 `lru` crate

候选方案：
- **A（采纳）**：`HashMap<K, (FileSignature, V)>` + `VecDeque<K>` 自实现 LRU
- B：引入 `lru` crate
- C：无淘汰的 `HashMap`（按 cap 上限直接 drop 全清）

选 A 因为：
- 200 entry 容量、淘汰频率低（用户 active session 数稳定），简易 LRU 即可
- workspace 已有 `parking_lot::Mutex`（不算新依赖）/ `std::sync::Mutex`，HashMap+VecDeque 都在 std 里
- 引入 `lru` crate 多一个供应链依赖，对这种廉价场景不值
- 完全 drop 全清在抖动场景下会反复全 miss，不可取

### D3：缓存的所有权与生命周期

候选方案：
- **A（采纳）**：cache 由 `NotificationPipeline` / `LocalDataApi` 各自持有，独立 `Arc<Mutex<MetadataCache>>` 字段
- B：全局 static / `OnceLock` 单例
- C：每次 `list_sessions` 调用临时构造（不跨调用持久化）

选 A 因为：
- 测试可注入：`make_pipeline` 已用 tempdir + 自定义 manager，cache 同样可以新建独立实例
- 跨调用持久化是缓存价值的核心——B 和 C 不能持久化跨进程外都不行
- 每个组件持有自己的 cache 隔离故障域：notifier cache 抖动不影响 metadata cache
- 不引入 global state 让 ownership 复杂化

### D3b（修订采纳）：metadata cache 必须由 `LocalDataApi` 持有

codex 异构审查发现：原 tasks 2.2 与 D3 决策冲突——tasks 写了"用 `OnceLock<Mutex<MetadataCache>>` 模块级单例"，但 D3 已采纳"`LocalDataApi` 各自持有"。冲突会导致：

- 测试串行/并行跑互相污染 cache 状态
- 现有 `LocalDataApi` 已通过 `make_local_data_api` / `new_with_xxx` 注入基础设施，单例与现有 ownership pattern 不一致
- 多 `LocalDataApi` 实例（HTTP server + Tauri IPC 各自构造场景）会共享单例 cache，cache invalidation 行为不可控

修订采纳：metadata cache 严格按 D3 由 `LocalDataApi` 持有 `metadata_cache: Arc<std::sync::Mutex<MetadataCache>>` 字段。`extract_session_metadata` 不改函数签名（保持纯函数），缓存 lookup/insert 的 wrapper 函数 `extract_session_metadata_cached(cache, path)` 作为 `LocalDataApi` 内部辅助函数（非公开 API）。`scan_metadata_for_page` 函数签名加一个 `cache: Arc<...>` 参数，从 spawn 处传入。

测试隔离：每个测试新建独立 `LocalDataApi` 实例（已是现有 pattern），cache 与实例同生命周期，零状态污染。

tasks 2.2 / 2.5 / 2.6 同步改写：删除 `OnceLock` 单例与 cached/uncached 拆函数的设计，改为 `LocalDataApi` 字段 + 私有 wrapper。

### D4：Cache invalidation——被动 vs 主动

候选方案：
- **A（采纳）**：纯被动——每次访问时 stat + 比对 mtime/size，不一致就 miss
- B：主动 invalidate——订阅 file-change 事件，事件到来主动 evict cache entry
- C：A + B 混合

选 A 因为：
- 被动方案逻辑简单：cache 不需要订阅事件、不需要双向通信
- file-change 事件本身就是访问触发器（notifier / list_sessions 的入口都受 file-change 驱动），被动 stat 与主动 invalidate 时序基本等价
- B 增加跨 task 通信复杂度，且 broadcast::Receiver lagged 时可能漏 invalidate 反而不安全
- C 没有额外收益

### D5：metadata cache 的 key——按文件 path 还是 (project_id, session_id)

候选方案：
- **A（采纳）**：用 `PathBuf`（canonical 后）作 key
- B：`(project_id, session_id)` tuple

选 A 因为：
- `extract_session_metadata(path)` 已经接受 `&Path`，封装层只在调用处加 cache 包装
- HTTP 路径（`list_sessions_sync`）和 IPC 路径（`list_sessions`）传的 path 一致（都从 `list_sessions_skeleton` 来），跨入口共享缓存
- B 需要在 `extract_session_metadata` 加 project/session 参数，污染纯函数签名
- 注意：mtime/size 已经隐含定位文件，path 只是 lookup key，路径不一致同 inode 重复 cache 边界 case 极罕见且影响仅是 cache miss 一次重算，可接受

### D6：notifier cache 的 key——`(project_id, session_id)` tuple 而非 path

候选方案：
- **A（采纳）**：用 `(String, String)` 即 `(project_id, session_id)`
- B：用 `PathBuf`

选 A 因为：
- `FileChangeEvent` 直接带 `project_id` / `session_id`，构造 path 是后续步骤
- notifier 在 `process_file_change` 入口就能用 event 的两个 String 字段直接 lookup，避免不必要的 path 拼接
- A 与 D5 不同的取舍是因为：notifier 链路 event 是 source，path 是衍生品；metadata 链路 path 是被传入参数

### D7：cache 命中后的 trigger 变化处理

边界 case：用户在文件未变期间**修改了 trigger 配置**（启用/禁用某条 trigger）。如果 notifier 缓存只看 `FileSignature` 命中即整段跳过，trigger 配置变化期间漏检。

候选方案：
- A：cache 只缓存"已 parse 的 messages 数量"，不缓存 detect_errors 的结果。每次进入仍 acquire trigger 列表 + stat；`FileSignature` 一致时跳过 parse_file，但仍跑 detect_errors—但 detect_errors 输入是已缓存的 messages slice。
- B：cache 缓存 `Vec<DetectedError>`，trigger 变化时全清缓存
- C：cache 缓存 messages，trigger 变化时也只清 detect 结果不清 messages

实测确认：
- 重新审视 `notifier.rs:104` 的 `detect_errors(&messages, &triggers, ...)` —— `detect_errors` 输入是 messages slice，CPU 主要在 parse_file（O(整文件)），detect 是 O(messages × triggers) 但每条仅做 regex match（数 μs）
- A 即"缓存 messages，跳过 parse 但仍跑 detect"是最佳方案：
  - 命中时：避免 parse_file 这个真正的 CPU 大户
  - 仍跑 detect：trigger 配置变化立即生效
  - `add_notification` 已按确定性 id 去重，重跑 detect 不产 dup 通知（CLAUDE.md notifier 段已记录）
- B 复杂且 trigger 变化触发全清浪费 cache
- C 与 A 等价但实现更绕

**修订**：D7 还有一个考量——缓存 `Vec<ParsedMessage>` 体积可能很大（大会话几 MB）。**最终选 A 的精简版**：缓存仅 ``FileSignature`` 标志位+空 marker，命中时直接 `return`（跳过 parse + detect 整段）。代价是 trigger 配置变化期间漏检——但仅在该 session 文件**未变化**期间漏检；下次该 session 任意 append 触发 file-change 即正常处理。用户改 trigger 几乎总是在静止期，且新 trigger 设计上是为未来错误服务的，漏检"过去几秒静默期"无实际损失。

### D7b（修订采纳）：notifier cache 命中即整段跳过

最终决策：notifier cache 命中（`FileSignature` 与缓存一致）时**整段跳过** parse + detect，不重跑 detect_errors。

理由：
- D7-A 仍要 parse_file（虽然可缓存 messages）—— 但缓存 `Vec<ParsedMessage>` 内存压力大（几 MB × 200 entries = 几百 MB）不可接受
- 仅缓存 ``FileSignature`` 而 messages 不缓存 = 命中时仍要 parse_file = 没省 CPU
- "trigger 配置变化期间漏检"是可接受的——用户行为模式上 trigger 配置变化总伴随其它 session 操作（保存配置文件本身可能触发 file-change），而且静止期漏检在下次任意 append 时立即恢复
- 简化实现：notifier cache value 只需存 `FileSignature` 不需要存任何 parsed 结果

### D8：metadata cache 缓存内容——整个 `SessionMetadata` 而非分片

候选方案：
- **A（采纳）**：缓存整个 `SessionMetadata { title, message_count, is_ongoing, git_branch }`
- B：分别缓存各字段
- C：仅缓存 line count，title 等仍每次扫前 200 行

选 A 因为：
- `SessionMetadata` 整个结构体很小（几百字节），整体缓存简单清晰
- B/C 增加复杂度无收益
- **但 `is_ongoing` 字段含 `is_file_stale` 时间敏感判定**：mtime 没变但当前 wall clock 推进 5 分钟后，"5 min stale" 判定会从 false 翻 true。需要在缓存 lookup 时**重新跑** `is_session_stale` 这一步，而不是直接返回缓存的 `is_ongoing`——只缓存 `messages_ongoing` 这一半，`is_ongoing = messages_ongoing && !is_file_stale(...)` 在 lookup 时实时计算。

### D9：测试策略

- 单元测试：
  - notifier cache：相同 `FileSignature`（含 identity）跳过 parse（用 mock parse_file 计数 / 用 tempdir 实测 add_notification 调用次数）
  - notifier cache：identity 变化（`#[cfg(unix)]` 用 rename 替换文件覆盖 inode）触发 miss
  - notifier cache：mtime 变化（touch 文件）触发重 parse
  - notifier cache：size 变小（truncate）触发重 parse
  - notifier cache：cap 上限触发 LRU 淘汰
  - metadata cache：相同 path+`FileSignature` 命中
  - metadata cache：identity 变化触发 miss（`#[cfg(unix)]` rename 替换文件）
  - metadata cache：mtime 变化触发 miss
  - metadata cache：cache 命中但 wall clock 推进让 stale 翻转
  - metadata cache：truncate 触发 miss
  - metadata cache：LRU 淘汰
- 集成测试：现有 `crates/cdt-api/tests/notifier_*.rs` / `metadata_*.rs` 不变全过
- 性能验证：手动用多 session fixture 跑 `just dev`，开 Activity Monitor / `top -pid <cdt>` 观察 CPU——无自动化 baseline benchmark（多会话场景缺现成 fixture），由 codex CR + 用户实测覆盖

## Risks / Trade-offs

- **[mtime 精度不足导致缓存假命中]** → mtime 1s 精度系统下，单秒内多次 append 可能让 mtime 不变。叠加 size + identity 比对兜底——常规 append 后 size 必然变化，`FileSignature` 命中 = 文件在常规写入路径下未变。
- **[inode reuse + mtime/size 撞车假命中]**（见 D1d）→ 接受为 best-effort 限制。spec 明确"在常规 append-only 写入路径下等价"；现实中 Claude Code 不主动 delete-then-recreate session JSONL，加上后续 file-change 自然恢复，影响可忽略。
- **[truncate 后 size 变小但 mtime 偶尔不更新]** → 落入 miss 分支重 parse，正确兜底（size 维度生效）。
- **[LRU 抖动场景反复 miss]** → 用户同时活跃 session 数 < 200 时 LRU 不淘汰；> 200 时按 LRU 顺序淘汰最久未访问的，符合直觉。命中 bump 到队首避免冷热混淆。
- **[trigger 配置变化期间漏检]**（见 D7b）→ 接受，详 D7b 理由。
- **[wall-clock 推进让 stale 翻转]**（见 D8）→ 缓存只存 `messages_ongoing`（不随时间变），`is_ongoing` 由 lookup 时实时计算 stale 状态合成。
- **[Mutex 锁竞争]** → 缓存访问极快（HashMap lookup + `FileSignature` byte-equal 比对），临界区 < 1μs；标准 `std::sync::Mutex` 即可，不需要 RwLock。

## Migration Plan

无 migration——纯实现优化，缓存初始为空，第一次访问填充，后续命中。无配置开关、无回滚专用代码。如需紧急回滚：单 commit revert，缓存层移除即恢复全 parse 行为。

## Open Questions

- **Q1**：metadata cache 的容量 200 是否够？用户极端场景项目 > 200 session 时，分页只看一页，cache 足够。多项目并发查看时也只在 active project 集中，超 200 极罕见。如果用户实际反馈不足再调。
- **Q2**：是否给 notifier cache 也加 LRU 淘汰？同样 200 entries 上限。是的——notifier 跨 session 长期运行同样可能堆积。

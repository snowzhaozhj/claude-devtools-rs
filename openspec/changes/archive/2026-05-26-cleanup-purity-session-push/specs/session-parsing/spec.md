# session-parsing Specification (delta)

## MODIFIED Requirements

### Requirement: `extract_session_metadata` 按 `FileSignature` 缓存

数据 API 层 SHALL 持有一个内部 LRU 缓存（不使用全局单例），以 `(ContextId, PathBuf)` 复合 key 记录上一次扫描时的 `(FileSignature, title, message_count, messages_ongoing, git_branch)`。其中 `ContextId` 由文件系统抽象层定义（详 `openspec/specs/fs-abstraction/spec.md` §`ContextId` 三元组作为 cache key 前缀），Local 模式下 SHALL 为 `ContextId::local(claude_root)`，SSH 模式下 SHALL 为 `ContextId::ssh(host_signature, remote_home)`。`FileSignature` MUST 至少包含：

- `mtime`：文件最后修改时间
- `size`：文件字节数
- `identity`：文件身份 —— Unix `(dev, ino)`；Windows 与其它平台退化为空（详 design D1f）

**等价性是 best-effort**：在常规 append-only 写入路径下，`FileSignature` 字段 byte-equal 即视为文件未变。inode reuse + mtime/size 三维同时撞车的极端场景可能假命中，由后续任何文件变化的 file-change 自然恢复。

再次调用相同 `(ContextId, path)` 时 SHALL 先通过文件系统抽象层的 stat 操作抓取文件元数据并构造 signature；若 byte-equal 等于缓存记录 THEN MUST 直接返回基于缓存数据合成的 `SessionMetadata`，**不**再逐行重读全文件；否则正常扫描并把结果写回缓存。SHALL NOT 绕过文件系统抽象层直接调用平台 fs API —— stat 必须通过抽象层走当前 active context 的 provider，保证 Local context 与 SSH context 各自命中正确的 provider 实例，避免跨 context fs 实例错配。

由于 `is_ongoing` 字段含时间敏感判定，缓存 MUST 仅缓存"基于消息序列结构"的 `messages_ongoing` 中间值（即活动状态判定算法的结果），而 `is_ongoing = messages_ongoing && !is_session_stale(signature.mtime, now)` MUST 在每次 lookup 时根据当前 wall clock 实时计算合成——不得直接缓存 `is_ongoing` 终态。

缓存 SHALL 在以下任一条件下走 cache miss：

- 缓存中无该 `(ContextId, path)` 组合（即使同 path 不同 ContextId 也 miss）
- `mtime` / `size` / `identity` 任一不一致
- stat 操作返回错误

缓存容量 SHALL 上限 2000 entries（全局总和，跨 `ContextId` 共享同 pool），按 LRU 淘汰；命中时 MUST 把命中 key bump 到队首避免冷热混淆。

**cache miss 后的扫描路径** MUST 通过文件系统抽象层的 open_read 操作拿到异步读取流，包装到 32 KiB 缓冲区的流式读取器后逐行解析（32 KiB 与 SFTP packet 上限对齐）；SHALL NOT 绕过文件系统 trait 直接打开文件。

#### Scenario: 相同 `(ContextId, path)` `FileSignature` 不变命中缓存

- **WHEN** 调用 metadata 缓存 wrapper 且 stat 操作拿到的 `FileSignature` 与缓存记录 byte-equal
- **THEN** MUST 直接返回基于缓存数据合成的 `SessionMetadata`，且 SHALL NOT 再逐行读全文件
- **AND** SHALL NOT 绕过文件系统抽象层直接调用平台 fs API

#### Scenario: mtime 不一致触发重扫

- **WHEN** 调用 metadata 缓存 wrapper 且 stat 操作拿到的 `mtime` 与缓存记录不同
- **THEN** MUST 走 line-by-line 全文件扫描路径——但 SHALL 通过文件系统抽象层的 open_read 操作而非直接打开文件，并以新 `FileSignature` 与新结果覆盖缓存

#### Scenario: 文件被 rename 替换（inode 变化）触发重扫（仅 Unix）

- **WHEN** 调用 metadata 缓存 wrapper 且 stat 操作拿到的 `identity`（Unix `(dev, ino)`）与缓存记录不同 —— 即便 mtime 与 size 巧合相同
- **THEN** MUST 走 cache miss 分支重新扫描
- Windows 与其它平台 identity 退化为 `None`，此 Scenario 由 mtime/size 维度兜底（best-effort，详 design D1f）

#### Scenario: 缓存命中后实时重算 stale 状态

- **WHEN** 缓存命中（`(ContextId, path)` key 等 + `FileSignature` 一致），且缓存条目的 `messages_ongoing = true`，且当前 wall clock 距 `mtime` 已超过 5 分钟 stale 阈值
- **THEN** 返回的 `SessionMetadata.is_ongoing` MUST 为 `false`（`messages_ongoing && !stale = true && !true = false`）；缓存 SHALL NOT 因此被 invalidate（`FileSignature` 仍正确反映文件未变，下次访问还能复用其它字段）

#### Scenario: 文件 size 变小触发重扫

- **WHEN** 调用 metadata 缓存 wrapper 且 stat 操作拿到的 `size` 比缓存记录小
- **THEN** MUST 走 cache miss 分支重新扫描

#### Scenario: stat 失败时走 cache miss

- **WHEN** 调用 metadata 缓存 wrapper 但 stat 操作返回错误
- **THEN** MUST 走原路径（由内部 open_read 自身决定返回空 `SessionMetadata`），且 SHALL NOT 把空结果写入缓存

#### Scenario: 缓存超过容量按 LRU 淘汰

- **WHEN** 缓存已达 2000 entries 时再调用一个新 `(ContextId, path)` 组合
- **THEN** MUST 淘汰当前最久未访问的条目后再写入新条目，缓存大小始终 ≤ 2000
- **AND** 容量上限是跨 `ContextId` 全局总和，不按 context 拆分配额

#### Scenario: 缓存命中时把 key bump 到队首

- **WHEN** lookup 在缓存中命中 `(ContextId, path)`
- **THEN** MUST 把该 `(ContextId, path)` 组合的 LRU 位置移到队首（最新访问），后续淘汰循环中该 key 不会被冷热顺序错误淘汰

#### Scenario: Local 与 SSH 同字面 path 不串扰

- **WHEN** Local context 写入 cache `(ContextId::local(local_root), "/foo/s.jsonl")`，随后切换到 SSH context 后查询同字面 path `(ContextId::ssh(host_sig, remote_home), "/foo/s.jsonl")`
- **THEN** 查询 SHALL miss —— 不同 `ContextId` 即不同 cache key，即使 path 字面相同
- **AND** Local entry SHALL NOT 被 SSH 查询误命中或覆盖
- **AND** 切回 Local 后再查同 Local key SHALL hit（cache 仍持有该 entry）

#### Scenario: 不同 SSH host 之间不串扰

- **WHEN** 用户连 SSH host A 写入 cache，再切到 SSH host B 查询同字面 path
- **AND** host A 与 host B 的 `HostSignature.config_digest` 不同
- **THEN** host B 的查询 SHALL miss —— `ContextId` 不等

#### Scenario: scanner 通过文件系统抽象层切换异步读取流

- **WHEN** cache miss 后触发扫描路径
- **THEN** 函数体 SHALL 通过文件系统抽象层的 open_read 操作拿到异步读取流，并用 32 KiB 缓冲区的流式读取器包装后逐行喂状态机（活动状态 / 标题 / 计数 / git branch）
- **AND** 函数体 SHALL NOT 绕过文件系统 trait 直接打开文件
- **AND** SSH context 下扫描路径的 fs op 计数 SHALL 为 1 个 open_read（不论文件大小，缓冲读取器内部分多次读取但都通过同一异步读取流）

### Requirement: metadata 缓存 ownership 由 `LocalDataApi` 持有

数据 API 层 SHALL 通过独立的互斥锁字段持有缓存实例。所有构造器 MUST 初始化为空 cache。**禁止**用全局单例 ——多个数据 API 实例（HTTP server + Tauri IPC 各自构造）必须各自独立持有 cache，互相不共享。

数据 API 层 SHALL 提供获取当前 active 文件系统 provider 与 context 的 inherent 方法（relaxed 变体用于 cache 内部路径 / 测试 helper；strict 变体用于 user-facing IPC handler，SSH disconnect 中间态返 `not_found`）以支撑 `(ContextId, PathBuf)` cache key 拓扑：

- 函数内部 SHALL 单次读 SSH manager 的 active context id 决定走 SSH 还是 Local 分支
- SSH 分支：若同一 active context 的 provider 仍存在，SHALL 返回 provider + remote_home + context_id；任一 lookup miss 时 relaxed 变体 SHALL 安全降级到 Local 分支（disconnect 中间态等并发场景），strict 变体 SHALL 返错误
- Local 分支：SHALL 返回本地 fs handle + projects_dir + local context_id，fs 与 ctx **来自同一快照**
- `switch_context` / `ssh_connect` / `ssh_disconnect` 实现 SHALL NOT 主动清空 metadata cache，且 SHALL NOT 持有外部 current_context_id 字段（依靠 ContextId Hash/Eq 隔离 + LRU 自然淘汰，与 `openspec/specs/fs-abstraction/spec.md` §"fs-related cache 必须采用'单实例 + ContextId key 前缀'拓扑"第 4 条一致）

数据 API 层 MUST NOT 持有静态 fs provider 字段或等价静态注入——避免死字段污染；cache callsite 一律走 active_fs_and_context 方法拿当前 active provider。

`extract_session_metadata` 自身 MUST 保留为 path-only 公开函数（兼容现有调用方 / 单测），内部以本地 fs handle 包装；SSH-aware 入口 SHALL 通过带 fs 参数的变体暴露给 cache wrapper。缓存查询 wrapper MUST 作为内部辅助函数，由数据 API 层的方法或后台扫描任务调用。wrapper 的签名 SHALL 接受 fs provider 与 context_id 参数，**禁止**在 wrapper 内部绕过文件系统抽象层或硬编码 fs provider 类型。

**SSH callsite 接入 cache wrapper**：骨架阶段与分页扫描在 SSH active context 下 SHALL 走与 Local context 完全一致的 cache wrapper 调用——**不再**走临时保留的 inline 早退路径。

SSH list 路径**hot path（用户感知）SHALL 走 cache hit trust**：UI 立刻拿 in-memory cache 内容渲染列表（0 fs op），不等 stat RTT；后台 spawn 异步校验任务校验 cache freshness，每条改动通过 SSE event 推差量给 UI 增量更新（与 Local 现有 SkeletonThenStream 体验一致）。**朴素 per-session 串行 stat 路径 SHALL NOT 出现在 SSH list hot path 用户感知阻塞段** —— SFTP 全锁串行会直接超 sidebar 首屏预算。

SSH 后台校验路径：spawn 异步校验任务 per project，内部 per-session 调缓存 wrapper（stat 拿 signature → cache mismatch → 通过文件系统抽象层重 parse）。per-project N→1 stat batch 优化（SFTP READDIR reply 自带 entry attrs）SHALL 留 follow-up 实施。

fs op 计数：

- Hot path cache hit（用户切回已访问 SSH host）：UI 渲染 fs op = 0（via trust cached lookup）；后台异步校验任务 per session stat 校验 signature，mismatch 时 open_read 重 parse
- Cold start 首次连 SSH：UI 立即返 SessionSummary 骨架（title=None / message_count=0）；spawn 异步刷新任务 per session，metadata 通过 SSE 推差量

新加 cache helpers（follow-up wire 入 batch 路径用，本 segment 仅 trust cached lookup 在 SSH list hot path 使用）：
- `lookup_with_known_signature`：用调用方提供的 signature 直接查 cache，跳过内部 stat
- `lookup_trust_cached`：hot path cache hit trust，不校验 signature 直接返 entry

#### Scenario: 多个数据 API 实例独立持有 cache

- **WHEN** 测试或运行时构造两个数据 API 实例 A 与 B
- **THEN** A 的 metadata cache 与 B 的 metadata cache MUST 是独立实例，A 中的缓存写入 SHALL NOT 影响 B 中的 lookup 结果

#### Scenario: `extract_session_metadata` 保持 path-only 公开签名

- **WHEN** 现有调用方（含单元测试）直接调用 `extract_session_metadata(path)`
- **THEN** 该函数签名 MUST 保持 path-only 公开入口，内部 SHALL 用本地 fs handle 包装到 fs-aware 变体

#### Scenario: 数据 API 层不持静态 fs 字段也不持 current_context_id 字段

- **WHEN** 检查数据 API 层 struct 定义
- **THEN** SHALL NOT 含静态 fs provider 字段
- **AND** SHALL NOT 含显式 current_context_id 字段
- **AND** SHALL 提供 active_fs_and_context relaxed + strict 两个 inherent 方法

#### Scenario: active_fs_and_context 让 fs 与 ctx 来自同一快照

- **WHEN** 调用方在任意时刻调 active_fs_and_context
- **THEN** 返回的 (fs, projects_dir, ctx) 三元组 SHALL 自洽：fs 为 Local 时 ctx 为 local context id；fs 为 SSH 时 ctx 的 host_signature 等于该 SSH provider 在 connect 时计算的签名
- **AND** SHALL NOT 存在"fs 是 Local provider 但 ctx 是 SSH ContextId"或反之的不一致组合

#### Scenario: ssh_connect 强制 disconnect 旧 active 期间 cache 不被串扰

- **WHEN** 在 SSH context A 已 active 时调 `ssh_connect(host_B_request)`，触发强制 disconnect 旧 active 流程
- **AND** 在 disconnect 旧 active 与 connect 新 host 之间的并发窗口内，另一个 IPC 调用走 active_fs_and_context
- **THEN** 该并发调用 SHALL 拿到自洽的 (Local fs, Local ctx) 或 (SSH B fs, SSH B ctx)，而 SHALL NOT 拿到混合的 (Local fs, SSH A ctx)

#### Scenario: ssh_disconnect 不清 cache

- **WHEN** 调用 `ssh_disconnect("ssh-host-A")`
- **THEN** SHALL NOT 清空 cache 中该 ContextId 的 entry
- **WHEN** 用户随后 `ssh_connect` 同 host A（reconnect 后 host_signature 相同）
- **THEN** 同 ContextId 的 cache entry SHALL 立即可用（无需冷扫）

#### Scenario: SSH 路径 hot path cache hit trust

- **WHEN** 骨架阶段在 SSH active context 下执行 page 处理且 cache 中持有该 ContextId 的 entry
- **THEN** UI 渲染路径 SHALL 直接拿 cache 内容（0 fs op），不等 stat 校验
- **AND** SHALL NOT 出现 cache lookup 早退
- **AND** SHALL NOT 出现 per-session 串行 stat 校验

#### Scenario: SSH 后台 scan task 走异步校验 + SSE 推差量

- **WHEN** UI 渲染完 cache hit trust 内容后（或 cache miss 时启动）
- **THEN** SHALL spawn 后台异步校验任务 per project；task 内部 per-session 调缓存 wrapper
- **AND** 每条改动通过 `session_metadata_update` SSE event 推差量给 UI 增量更新
- **AND** task SHALL NOT 阻塞 list_sessions IPC 响应——hot path 走 cache trust 立即返回
- **AND** task SHALL 注册 abort handle 到 active_scans map；context 变更入口 SHALL 递增 generation 并按 prev ContextId 精确 abort 已注册的 scan handle
- **AND** 后台异步校验任务内部每次广播前 SHALL check context_generation 是否匹配；mismatch 时 silent drop

#### Scenario: 冷启动 SSH list_sessions（cache 无 entry）

- **WHEN** 用户首次连 SSH host A 调 `list_sessions`，cache 中无该 ContextId 的 entry
- **THEN** UI 立即返 SessionSummary 骨架（title=None / message_count=0）走 SkeletonThenStream
- **AND** spawn 后台异步校验任务 per project
- **AND** SHALL 通过 `session_metadata_update` SSE event 推骨架 + 增量 metadata 给 UI

### Requirement: `extract_session_metadata` 流式判定 isOngoing 不收集全量消息向量

`extract_session_metadata` 的 fs-aware 变体 SHALL 流式判定 `messages_ongoing`：在 JSONL 逐行解析的 loop 内，将每条 `ParsedMessage` 即时喂给 `IsOngoingStateMachine` 的 `feed(&msg)` 接口，并在文件读取完毕后调用 `state_machine.finalize()` 得到最终 `messages_ongoing` 值。该函数 MUST NOT 在内存中保留全量消息向量 —— 即 `messages_ongoing` 的计算路径上不得 collect 全量解析结果到容器。

`IsOngoingStateMachine` SHALL 提供以下公开接口：
- `new()`：构造空状态机（ongoing = false，shutdown_tool_ids 为空集）
- `feed(&mut self, msg)`：吃一条消息，按 assistant / user 分发并更新内部状态
- `finalize(self) -> bool`：消费状态机得到最终 is_ongoing 判定

`IsOngoingStateMachine` 流式喂入的最终结果 SHALL 与既有 `check_messages_ongoing` 切片版在任意有限消息序列上完全等价。`check_messages_ongoing` MAY 内部委托给状态机（thin wrapper），公开签名保持切片入参 + bool 返回。

#### Scenario: 流式状态机不在内存保留全量 ParsedMessage

- **WHEN** 调用 fs-aware 变体处理一个含 N 条消息的 JSONL 文件
- **THEN** 函数实现路径 SHALL NOT 创建全量消息容器以累积全部解析结果用于 is_ongoing 计算
- **AND** 实际驻留内存峰值 SHALL 不随 N 线性增长（仅状态机自身字段 + 当前正解析的单行消息）

#### Scenario: 状态机与切片版结果等价

- **GIVEN** 一组覆盖 normal completed / ongoing tool-use / interrupted / teammate-message / shutdown_response / resumed-after-interrupt 六类典型场景的 fixture 消息序列
- **WHEN** 用状态机 feed + finalize 流式处理
- **AND** 用切片版处理同一序列
- **THEN** 两种处理方式 SHALL 在每个 fixture 上返回相同 bool 结果

#### Scenario: 空消息序列返回 false

- **WHEN** 在新建的状态机上不调用任何 feed，直接 finalize
- **THEN** SHALL 返回 false（与切片版空输入一致）

#### Scenario: SHUTDOWN_RESPONSE tool 跨消息追踪

- **GIVEN** 序列：assistant 消息含 `tool_use { id: "tu-shutdown", name: "SendMessage", input: { type: "shutdown_response", approve: true } }`，紧随 user 消息含 `tool_result { tool_use_id: "tu-shutdown", ... }`
- **WHEN** 依次 feed(assistant_msg); feed(user_msg); finalize()
- **THEN** 状态机内部 shutdown_tool_ids SHALL 在 feed assistant 时插入 "tu-shutdown"
- **AND** feed user 时识别匹配的 tool_use_id，将对应事件归类为 ending，最终 finalize SHALL 返回 false

#### Scenario: extract_session_metadata 公开签名保持纯函数语义

- **WHEN** 现有调用方直接调用 `extract_session_metadata(path)`
- **THEN** 该函数签名 SHALL 保持 path-only 公开入口不变
- **AND** 行为 SHALL 与既有语义完全一致（含 is_ongoing 取值，仅内部实现改流式）

### Requirement: `get_tool_output` 与 `get_image_asset` 走 parsed-message LRU 缓存

数据 API 层 SHALL 持有一个内部 parsed-message LRU 缓存（不使用全局单例），以 `(ContextId, PathBuf)` 二元组为 key（**MUST** 把 `ContextId` 作为 key 的第一成员；裸 `PathBuf` 作 key **MUST NOT** 出现），缓存值为 `(FileSignature, Arc<Vec<ParsedMessage>>)` 二元组。`get_tool_output` 与 `get_image_asset` MUST 在调用 parse 之前先查该缓存，命中时 MUST 直接复用缓存中的共享引用、SHALL NOT 重读 JSONL 全文件，亦 SHALL NOT 重新执行 line-by-line parse。

`FileSignature` 等价性 MUST 与 MetadataCache 同源（`(mtime, size, identity)` 三元组，identity 在 Unix 上为 `(dev, ino)`，Windows 与其它平台退化为 `None`），best-effort 语义与 `extract_session_metadata` 按 `FileSignature` 缓存 Requirement 完全一致。

stat 路径 MUST 走文件系统抽象层的 stat 操作（而非直接调用平台 fs API）；构造 `FileSignature` MUST 走标准化工厂方法。

缓存容量 SHALL 上限 50 entries，按 LRU 淘汰；容量按全局计算（**所有 `ContextId` 共享同一上限**，不按 context 拆配额）；命中时 MUST 把命中 key bump 到队首避免冷热混淆。

**SSH callsite 接入 cache wrapper**：`get_tool_output` / `get_image_asset` 在 SSH active context 下 SHALL 走与 Local context 完全一致的 cache wrapper 调用。

#### Scenario: `get_tool_output` 命中缓存时不重读 JSONL

- **WHEN** 同一 session 文件未变（`FileSignature` 一致），调用方再次调 `get_tool_output`
- **THEN** 第二次调用 MUST 直接从缓存读取共享消息序列，SHALL NOT 重读 JSONL 全文件

#### Scenario: `get_image_asset` 命中缓存时不重读 JSONL

- **WHEN** 同一 session 文件未变，调用方再次调 `get_image_asset`
- **THEN** 第二次调用 MUST 直接从缓存读取共享消息序列，SHALL NOT 重读 JSONL 全文件

#### Scenario: `FileSignature` 不一致走 cache miss

- **WHEN** stat 拿到的 `FileSignature` 与缓存记录任一字段不一致
- **THEN** MUST 走 cache miss 分支，重新解析全文件

### Requirement: parsed-message 缓存按 file-change 广播主动失效

数据 API 层的 watcher 构造路径 SHALL 在 spawn 自动通知管线的同时，额外 spawn 一个后台 task，订阅 file watcher 广播，对每条 `FileChangeEvent` 推算出 cache key 的 `PathBuf` 部分。

**ContextId 推算**：该后台 task SHALL 在构造时一次性合成 Local ContextId（**watcher 是本地 fs 的硬不变量**，永远不会触发远端 SSH 文件事件），循环内每次事件复用同一个 ctx 与推算出的 path 一起作为 cache key。

**stat 校验语义**：收到事件后 task MUST 先通过本地 fs handle 的 stat 操作拿当前 `FileSignature`，与 cache 中记录的 signature 比对：
- 两者一致 → SHALL NOT 移除（视为 spurious watcher 事件——典型场景：启动期偶发"无内容变化"事件、metadata-only touch、跨平台 backend 行为差异等）
- 两者不一致 → MUST remove 让下次 lookup 重 parse
- stat 失败（文件被删 / 权限）→ MUST remove 保守剔除

该失效路径与 `FileChangeEvent.deleted` 字段无关——文件被删 / 改 / 新建都同样进入"stat → 比对 signature → 决定 remove"流程。

不带 watcher 的构造路径 SHALL NOT 启动该订阅 task；此场景仅依赖被动 `FileSignature` 失效路径兜底。

广播订阅 lag（接收端因容量打满丢失事件）时 SHALL 静默继续 loop——lag 仅代表事件激增，下次 lookup 由被动 `FileSignature` mismatch 兜底，不影响正确性。广播关闭时 task SHALL 退出。

#### Scenario: 文件真改后 file-change 广播主动 invalidate

- **WHEN** watcher 构造且缓存中已有某 session 的 parsed-message 条目
- **AND** session JSONL 文件被追加 / 重写（mtime+size 变化）
- **AND** file watcher 广播一条对应 `FileChangeEvent`
- **THEN** 后台 invalidate task MUST 先 stat 拿当前 `FileSignature`、与 cache 记录比对、发现不一致后 remove 该条目

#### Scenario: spurious file-change 事件 SHALL NOT 错杀有效 cache

- **WHEN** 缓存中已有某 session 的 parsed-message 条目
- **AND** file watcher 发出了一条 `FileChangeEvent`，但目标文件内容 / mtime / size 实际未变
- **THEN** invalidate task MUST stat 拿当前 `FileSignature` 与 cache 记录比对，发现两者一致后 SHALL NOT remove 条目

#### Scenario: 文件被删时 stat 失败走保守 remove

- **WHEN** 缓存中已有某 session 的 parsed-message 条目
- **AND** file watcher 广播事件之后文件已不存在
- **THEN** stat 失败，task MUST remove 条目

#### Scenario: 不带 watcher 构造不启动 invalidate 订阅

- **WHEN** 数据 API 层由不带 watcher 的构造器实例化
- **THEN** SHALL NOT spawn 任何订阅 file watcher 的后台 task

#### Scenario: invalidator 用 Local ContextId 推算 cache key

- **WHEN** Local callsite 以 key `(ContextId::local(projects_dir), path)` 写入 cache
- **AND** file watcher 随后广播对应事件、文件内容已变
- **THEN** invalidator 推算的 ContextId MUST 等于 Local ContextId（与写入 key 一致），并成功 remove 该 entry
- **AND** runtime 切 SSH context 不影响 invalidator 行为（watcher 是 Local 视角的硬绑定）

### Requirement: parsed-message 缓存 ownership 由 `LocalDataApi` 持有

数据 API 层 SHALL 通过独立的互斥锁字段持有 parsed-message 缓存实例。所有构造器 MUST 初始化为空 cache。**禁止**用全局单例 ——多个数据 API 实例必须各自独立持有 cache，互相不共享。

构造器扩展 MUST 遵循"现有构造器签名不变 + 内部初始化新字段"模式。

`switch_context` / `ssh_connect` / `ssh_disconnect` 三个方法 SHALL NOT 主动清空 parsed-message cache —— 不同 `ContextId` 的 entry 自然不命中（依赖 key 的 Hash/Eq 隔离），signature 校验照常工作；reconnect 同 host 时可复用旧 entry。

#### Scenario: 多个数据 API 实例独立持有 parsed-message cache

- **WHEN** 测试或运行时构造两个数据 API 实例 A 与 B
- **THEN** A 的 parsed-message cache 与 B 的 MUST 是独立实例，A 中的缓存写入 SHALL NOT 影响 B

#### Scenario: 不改现有构造器签名

- **WHEN** 既有调用方按现有签名调用构造器
- **THEN** 签名 MUST 保持不变；parsed-message cache 字段 MUST 在构造器内部初始化为空

#### Scenario: switch_context / ssh_connect / ssh_disconnect 不清 parsed-msg cache

- **WHEN** 用户在 Local context 下写入 cache 若干 entry，再 ssh_connect 切 SSH、再 ssh_disconnect 切回 Local
- **THEN** Local entry SHALL 保留；reconnect 同 host 时 SSH entry 可复用

#### Scenario: cache miss 路径走 fs-aware parse

- **WHEN** parsed-message cache miss 后触发 fallback parse
- **THEN** SHALL 调用 fs-aware 版本的 parse（内部走文件系统抽象层的 open_read 操作）
- **AND** 旧版 path-only parse 入口 SHALL 保留作为兼容入口

### Requirement: Title length is bounded by TITLE_MAX_CHARS constant

`extract_session_metadata` 提取的 `SessionSummary.title` 最终字符数 SHALL ≤ 500（Unicode char 计数，不是 byte 数）。所有截断路径（teammate summary fast-path / slash-with-args 直接路径 / 普通 sanitize 路径）SHALL 调用同一截断 helper，禁止散落不同 magic number。

截断上限常量 SHALL 定义为 500 并在同 crate 测试中可引用。

#### Scenario: Plain-text title longer than 500 chars is truncated at 500

- **WHEN** session 第一条 user 消息 content 为 700 个中文字符的纯文本
- **THEN** `extract_session_metadata.title.unwrap().chars().count()` SHALL ≤ 500

#### Scenario: Slash with args longer than 500 chars is truncated at 500

- **WHEN** session 第一条 user 消息为 slash command + 700 字符 args
- **THEN** `extract_session_metadata.title.unwrap().chars().count()` SHALL ≤ 500

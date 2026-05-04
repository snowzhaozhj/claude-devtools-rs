## Context

会话详情页的 Compact 与 System 气泡渲染相比 `../claude-devtools` Electron 原版差距明显：

- **Compact**：当前是一行 `<span>Compact</span>` + 裸 `summaryText` 文本；原版 (`CompactBoundary.tsx`) 是 amber 风格折叠 button 头（ChevronRight + Layers + "Compacted" 标签 + token delta `{pre}→{post} ({delta} freed)` + Phase N 徽章 + 时间），点击展开 markdown 渲染。
- **System**：当前是 `<pre class="system-pre">` 裸文本；原版 (`SystemChatGroup.tsx`) 是左对齐 `rounded-2xl rounded-bl-sm` 气泡。

Compact 视觉对齐被一个数据缺口卡住——后端 `cdt-core::ContextPhaseInfo` 已经持有 `compaction_token_deltas: HashMap<chunk_uuid, CompactionTokenDelta>` 与 `ai_group_phase_map: HashMap<ai_group_id, phase_number>`，但 `SessionDetail` IPC payload 没把这俩数据关联到 `CompactChunk` 的字段里，前端无从读 token delta / phase number。

附带的 Sidebar `.session-meta` flex 行视觉折断（"刚刚" 被 CJK 按字符断成"刚 / 刚"）是纯 CSS bug——`.session-time` 没设 `nowrap`、`.session-branch` 没设 `min-width: 0` 导致 flex 子元素互相挤压。

利益相关方：项目用户（消费 Compact 视觉信息）；本仓库其它 chunk 渲染逻辑（避免被新字段污染）；`cdt-analyze::chunk::builder` 算法层（保持其纯净不依赖 `ContextPhaseInfo`）。

## Goals / Non-Goals

**Goals:**

- `cdt-core::CompactChunk` 新增可选 `tokenDelta` / `phaseNumber` 字段，IPC 序列化 camelCase
- 派生逻辑放在 `cdt-api` 的 `SessionDetail` 组装层，不污染 `cdt-analyze::chunk::builder`
- 前端 Compact 视觉重做对齐 `CompactBoundary.tsx`：折叠头 + token delta + Phase 徽章 + 默认折叠 + 展开 markdown
- 前端 System 视觉对齐 `SystemChatGroup.tsx`：气泡容器 + 左对齐 + max-width 85%
- Sidebar `.session-meta` 子元素 flex 行为修复：消息数 / 时间 nowrap，分支名 ellipsis
- IPC contract test 覆盖新字段；fixture 加示例数据

**Non-Goals:**

- 不改 `cdt-analyze::chunk::builder` 的 chunk emission 算法（既有 `Emit CompactChunks at compaction boundaries` Requirement 行为不变）
- 不反转 PR #38 的 Sidebar git 分支 per-session 显示位置（不搬回 SidebarHeader）
- 不改 AIChunk / UserChunk 的渲染样式
- 不改 `ContextPhaseInfo` 数据结构（D1d 最终算法**也不消费它**——派生层从 chunks 自身独立计算 tokenDelta 与 phaseNumber，详见 D1d）

## Decisions

### D1: phaseNumber 语义——"compact 之后第一个 AIChunk 的 phase"

**问题**：原版 `CompactBoundary.startingPhaseNumber` 是 "compact 之后开始的新 phase 编号"。Rust 端 `ai_group_phase_map: HashMap<ai_group_id, u32>` 按 AI group ID 索引；CompactChunk 自身没有 ai_group_id（它代表 compaction 边界，不是 AI 响应）。需要确定如何从 CompactChunk uuid 反查 phase。

**候选**：

- **(A)** 在 `SessionDetail` 组装层遍历 chunks，对每个 `CompactChunk[i]` 找 `chunks[i+1..]` 中第一个 `AIChunk`，取其 `responses[0].uuid`（= ai_group_id），查 `ai_group_phase_map[uuid]` → `Option<u32>`。compact 后没有 AIChunk 时返回 `None`。
- (B) 在 `cdt-analyze::context` 的 `ContextPhaseInfo` 里加新字段 `compact_to_phase_map: HashMap<compact_uuid, phase_number>`，提前算好。
- (C) 不携带 `phaseNumber` 字段，前端自己从其它数据推断。

**选 (A)**，理由：

1. 单点派生在最终 IPC 组装层，不污染 `cdt-analyze` 的纯算法（保 D2 原则）
2. 数据已经在 `ContextPhaseInfo` 里齐了，再加 map 是冗余
3. compact 后没 AIChunk 的边界（用户在 compact 后立即关掉 session）只占极少数，`None` 让前端不显示徽章是合理 fallback
4. 性能：每个 CompactChunk O(N) 找下一个 AIChunk，最坏 O(N×M)，但 M 通常 ≤ 5（一个 session 罕有超过 5 次 compaction），实际 O(N)

**边界**：

- compact 后**没有** AIChunk → `phaseNumber = None`，前端跳过 Phase 徽章
- compact 后第一个 chunk 不是 AIChunk（罕见，比如 user/system）→ 继续往后找直到 AIChunk 或 chunks 结束
- `ai_group_phase_map` 不含该 ai_group_id → `phaseNumber = None`（保守 fallback）

### D1b: phaseNumber 改用 `ContextPhaseInfo::phases[i].compact_group_id` 反查（修订 D1）

**触发**：codex 二审找到 D1 在**连续 compact** 场景下错算（`A → B → AI(phase=3)` 时 D1 让 A 和 B 都拿 phase 3，但原版 `groupTransformer.ts:295-303` 是每遇 compact 自增——A=phase 2、B=phase 3）。保留 D1 决策审计，本块为修订。

**新候选**：

- (D) 直接反查 `ContextPhaseInfo::phases: Vec<ContextPhase>`——每个 `ContextPhase { phase_number, compact_group_id, .. }` 已经记录了"phase N 由哪个 compact 触发"（`compact_group_id` 字段，见 `cdt-analyze::context::session::compute_session_context` 第 195 行）。**对一个 compact uuid `c`，phaseNumber = `phases.iter().find(|p| p.compact_group_id.as_deref() == Some(c)).map(|p| p.phase_number)`**

**选 (D)**，理由：

1. **数据语义直接对齐**——`ContextPhaseInfo::phases[i].compact_group_id` 已记录 phase 由哪个 compact 引导，不再依赖"扫 chunks 找下一个 AIChunk"
2. **正确处理连续 compact**——`A → B → AI(phase=3)` 时 phases 数组形如 `[(1, None), (2, Some("A")), (3, Some("B"))]`，A 反查 → phase 2、B 反查 → phase 3，对齐原版 TS
3. **不依赖 chunks 序列扫描**——派生函数变成 O(N×K) 其中 K = phase 数（≤ compact 数），常见会话 K ≤ 5，几乎 O(N)；如成为热点改用 HashMap 反向索引
4. **边界更清晰**——compact uuid 不在任何 phase 的 `compact_group_id` 字段时（理论不应发生，是 cdt-analyze 内部一致性 bug）`phaseNumber = None`，让前端 fallback

**新边界**：

- 一个 compact uuid 不被任何 `phases[i].compact_group_id` 引用 → `phaseNumber = None`
- `ContextPhaseInfo::phases` 为空（无 phase 信息可用）→ 所有 compact 的 `phaseNumber = None`
- 不再依赖"compact 之后扫 AIChunk"语义，原 D1 的两个 fallback Scenario（"compact 后无 AIChunk"、"AIChunk uuid 不在 ai_group_phase_map"）合并为单一 fallback "phases 数组中无匹配 compact_group_id"

### D1c: phaseNumber 改用 chunks 顺序 counter（修订 D1b——第二轮 codex 验证发现 D1b 仍未真正解决 Bug 1）

**触发**：第二轮 codex 验证发现 `cdt-analyze::context::session.rs:81-104,187-197` 的实现里，`phases` 数组**只在 phase 有 first/last AI 时才 push**。连续 compact `A → B → AI` 时：

1. phase 1 有 AI → push `(1, None)`
2. 遇到 A：`current_phase_number=2`、`current_phase_compact_group_id=Some("A")`，但此 phase 还没 AI 进入
3. 遇到 B：`current_phase_number=3`、`current_phase_compact_group_id=Some("B")`——**A 的 compact_group_id 已被覆盖丢失**
4. AI 进入 → 末尾 push `(3, Some("B"))`
5. 最终 `phases = [(1, None), (3, Some("B"))]`——A 完全消失，D1b 反查 c-1 (= A) 得 `None`，Bug 1 仍未解决

**新候选**：

- (E) 在 `apply_compact_derived` 派生函数内维护一个 `compact_counter: u32 = 1`，**按 chunks 顺序遍历**，每遇 `Chunk::Compact(c)` 就 `compact_counter += 1`，立即赋 `c.phase_number = Some(compact_counter)`。完全不依赖 `ContextPhaseInfo::phases` 数组，对齐原版 TS `groupTransformer.ts:295-303` 的 `let phaseCounter = 1; for compact: phaseCounter++; compact.startingPhaseNumber = phaseCounter` 一对一语义

**选 (E)**，理由：

1. **真正修复 Bug 1**——连续 `A → B`：A 时 counter 1→2 → A.phaseNumber=2；B 时 counter 2→3 → B.phaseNumber=3。每个 compact 立即得到自己的编号，不被后续 compact 覆盖
2. **不依赖 `phases` 不一致状态**——`cdt-analyze::context::session.rs` 的 `phases` push 条件依赖 first/last AI 存在，在"compact 后无 AI"或"连续 compact"场景下数组结构与 phaseNumber 语义不对齐；新算法绕开该数据
3. **算法极简 O(N)**——单趟扫描，无嵌套查找，更易测试
4. **与 cdt-analyze 内部 phase 编号语义一致**——`cdt-analyze::context::session.rs:101` 同样在遇到 compact 时 `current_phase_number += 1`，即"compact 触发新 phase"语义。chunks 顺序的第 i 个 compact 触发第 i+1 个 phase（1-based 起点），与原版 TS 与 cdt-analyze 内部完全对齐
5. tokenDelta 派生**保持 D1 不变**：仍按 `compaction_token_deltas.get(&c.uuid).copied()` 查 HashMap，该数据无 phases 数组的不一致问题（tokenDelta 在 `session.rs:142-150` 是有 pre/post AI 时才 insert，缺数据时 `None` 是合理 fallback，无需修订）

**新边界**：

- chunks 中无任何 `Chunk::Compact` → 派生函数不做任何修改（counter 也不前进）
- 派生**仅**取决于 chunks 序列中 compact 的相对顺序，不取决于 `ContextPhaseInfo.phases` 内容
- D1b 引入的"phases 数组中无匹配 compact_group_id → None"边界**作废**——新算法对每个 compact 都 emits `Some(n)`，不再有 None 的 phaseNumber 路径（除 `enabled=false` 回滚）

**作废说明**：D1b 的"phases.find 反查"实现 + 对应 fallback Scenario 在本 change 不被采用；但保留 D1b 文本作为决策审计（让 reviewer 看到完整推理路径）。spec delta 与 tasks 同步更新到 D1c 算法。

### D1d: tokenDelta 也改为派生层独立计算（修订 D1——第三轮 codex 验证发现 D1c 仅修了 phaseNumber，tokenDelta 在连续 compact 时仍与原版不齐）

**触发**：第三轮 codex 验证发现 `cdt-analyze::context::session.rs:101-104,131-143` 的 `compaction_token_deltas` insert 逻辑里，连续 `A → B → AI` 时 `current_phase_compact_group_id` 在每个 compact 被覆盖，最终只有 B 的 uuid 作为 key insert 到 map，A 反查得 `None`——D1 (A) 给 A 派生 `tokenDelta = None`。原版 `groupTransformer.ts:305-315` 对每个 compact 独立调 `findLastAiBefore` / `findFirstAiAfter` 算 delta，A 和 B 都拿到（值相同）。

**新候选**：

- (F) `apply_compact_derived` **完全独立于 `ContextPhaseInfo`**，从 chunks 自身计算 tokenDelta：对每个 `Chunk::Compact(c)` at index `i`，找 `chunks[..i]` 里最后一个 `Chunk::Ai`、`chunks[i+1..]` 里第一个 `Chunk::Ai`，分别取它们的 last response total tokens / first response total tokens，算 `CompactionTokenDelta { pre, post, delta = post - pre }`。两个 helper（`find_last_ai_before` / `find_first_ai_after`）+ 两个 token 累加 helper（对齐 `cdt-analyze::context::session.rs:220-242` 的 `assistant_total_tokens` / `get_last_assistant_total_tokens` / `get_first_assistant_total_tokens`，仅 12 行，**不**跨 crate import，**不**改 cdt-analyze 公共 API，在 `cdt-api` 派生模块内独立实现）

**选 (F)**，理由：

1. **真正修复 Bug 6**——连续 `A → B → AI` 时 A 和 B 各自的 `findLastAiBefore` / `findFirstAiAfter` 命中同一对 AI，得到相同 `tokenDelta`，对齐原版
2. **派生层完全独立**——不再依赖 `ContextPhaseInfo` 数据；`apply_compact_derived` 函数签名简化为 `(chunks: &mut [Chunk], enabled: bool)`，删 `context_info` 参数。配合 D1c 让派生层语义自洽：phaseNumber 用 chunks 顺序 ordinal、tokenDelta 用 chunks 邻接 AI 算 delta，**两个字段都仅依赖 chunks 自身**
3. **不污染 chunk-building 与 context-tracking 算法**——D2 的"不动 cdt-analyze"原则保持
4. **复用极简**——12 行 helper 在派生模块内独立实现，不需要 pub 化 cdt-analyze 内部 fn 也不需要 cross-crate import；内联是更内聚的设计

**新边界**：

- compact 之前没有 `Chunk::Ai` → `tokenDelta = None`（无法算 pre）
- compact 之后没有 `Chunk::Ai` → `tokenDelta = None`（无法算 post）
- 命中的 AI `responses` 全部 `usage = None`（罕见数据缺失）→ `tokenDelta = None`
- chunks 中无任何 compact → 派生函数零工作

**作废说明**：D1 (A) 的 "`compaction_token_deltas.get(&c.uuid)` 查 cdt-analyze 已计算的 map" 在本 change 不被采用；但保留 D1 文本作为决策审计。spec delta 与 tasks 同步更新到 D1d 算法。

### D2: 字段在 cdt-core 还是 cdt-api 派生

**问题**：`CompactChunk` 是 `cdt-core` 的纯数据 struct。`tokenDelta` / `phaseNumber` 是派生关联字段。在哪个层填？

**候选**：

- **(A)** `cdt-core::CompactChunk` struct 上加字段；`cdt-analyze::chunk::builder` 算法层产出时填 `None`；`cdt-api::session_detail` 组装最终 IPC payload 时**基于 chunks 自身**后置派生填充（D1d 最终方案；原 D2 草稿曾写"基于 `ContextPhaseInfo` 后置填充"，被 D1d 修订为"基于 chunks 自身"）
- (B) `cdt-core` 不加，仅在 `cdt-api` 内部新增 `CompactChunkWithDerived` wrapper struct，IPC 层序列化扁平化
- (C) `cdt-analyze::chunk::builder` 直接消费派生数据源，emit chunk 时就填好

**选 (A)**，理由：

1. **保 chunk-building 算法纯**——builder 接收 `ParsedMessage`s 流，emit Chunk，不依赖任何 phase / token 派生数据源。这是 `chunk-building` capability 的既有契约（Non-Goal "不改 chunk emission 算法"）
2. **避免 wrapper 类型污染**——(B) 让 IPC 层 / 非 IPC 层（HTTP 也用 SessionDetail）类型不一致
3. 字段是 `Option<T> + #[serde(default, skip_serializing_if = "Option::is_none")]`——builder 产 `None` 是合法默认值，不会引起既有 fixture / 测试的破坏
4. 老前端不感知（serde skip_serializing_if 让 IPC payload 中字段缺失），新前端按 `?? null` 兼容
5. **派生数据源**：D2 选 (A) 时未定派生数据源（早期假设是 `ContextPhaseInfo`），最终由 D1d 决定为 "chunks 自身的邻接 AI"——不依赖任何 `cdt-analyze` 计算结果，派生层语义自洽

### D3: 视觉与契约耦合度——一个 PR 落地两类改动

**问题**：本 change 包含 (a) IPC 字段透出（行为契约改动）+ (b) 前端视觉对齐 + (c) Sidebar CSS bug 修复。视觉部分不属于 spec 行为契约，但用户明确选择"全部一把走 openspec"。

**决策**：

- **spec delta 只覆盖 IPC 字段部分**：`chunk-building` ADD `CompactChunk derived metadata` Requirement；`ipc-data-api` ADD `Expose CompactChunk derived metadata in SessionDetail` Requirement
- **proposal / tasks 列全部 task**（含视觉与 Sidebar CSS）
- **无 MODIFIED 既有 Requirement**——不改既有 chunk emission 算法、不改既有 IPC 字段语义；仅"新增字段"用 ADDED 更稳，避免 archive 顺序坑（CLAUDE.md "archive 顺序坑"）

### D4: 前端 Compact 折叠状态作用域——per-chunk 局部 state

**问题**：用户展开一个 Compact 又切到另一会话再切回，期望折叠状态恢复 default（折叠）还是保留？

**决策**：每个 Compact 用 Svelte 5 `$state` 局部维护 `isExpanded: boolean` 默认 `false`。切 tab / 切 session 都会让组件 unmount → 状态丢失，重新 mount 时回到默认折叠。**不**走 `tabStore` 持久化——折叠状态属于"易失 UI 状态"，对齐原版 `useState(false)` 局部 state 语义（不进 redux store）。

### D6: 回滚开关从 `const` 改为派生函数参数（修订 D2 / D3 隐含的 const 实现）

**触发**：codex 二审指出 `const COMPACT_DERIVED_ENABLED: bool` 在测试时不可切换为 `false`，无法测试 spec delta 中"Rollback flag disables derivation" Scenario。

**决策**：派生函数 signature 加 `enabled: bool` 参数。生产代码（`get_session_detail` 真实组装路径）调用时传 `COMPACT_DERIVED_ENABLED` 顶部 const；测试代码直接传 `true` / `false`。这样：

- 顶部 const 保留作为"生产开关"语义不变（统一回滚点）
- 测试用例可单独验 `enabled=false` 路径
- 与既有 OMIT 模式（如 `apply_subagent_messages_omit`）相比是显式 bool 参数（更易测）

**最终 signature**（结合 D1d 删 `context_info`）：`apply_compact_derived(chunks: &mut [Chunk], enabled: bool)`。原 D6 草稿曾写 `(chunks, context_info: &ContextPhaseInfo, enabled)`，被 D1d 修订（派生独立于 `ContextPhaseInfo`），实施 SHALL 按 D1d 的两参数版本。

### D5: SVG 图标——加到 `lib/icons.ts`

**问题**：原版用 `lucide-react` 的 `ChevronRight` / `Layers` / `Terminal`。本仓库用自定义 `lib/icons.ts` 导出 lucide 风格 path 常量（`MESSAGE_SQUARE` 等），`BaseItem` 通过 `svgIcon` prop 渲染。

**决策**：在 `ui/src/lib/icons.ts` 新增 `LAYERS` / `CHEVRON_RIGHT` 常量（path d 字符串），与已有常量风格一致；`SessionDetail.svelte` 直接 inline 用 `<svg>` 引用这些常量。`Terminal` 已存在（system header 已用），不重复加。

## Risks / Trade-offs

- **Risk**（**已废弃，保留审计**）：D1b 的 `phases.find` 反查每 compact 一次 O(K)。该方案被 D1c 替换，不再适用
- **Risk**: D1c+D1d 的 `apply_compact_derived` 单趟扫 chunks O(N) 计 ordinal 与定位 compact；每个 compact 再做一次 `find_last_ai_before` + `find_first_ai_after`（向前 / 向后扫到第一个 AI），最坏 O(N×M)（M = compact 数）
  → Mitigation: M 实际 ≤ 5；单趟 N 扫 + 内层 N/2 扫总开销 < 1 ms（实测可在 perf-bench 验证）；如成为热点改用单趟双指针扫描预算 `compact_idx → (last_ai, first_ai)` 复杂度降到 O(N)
- **Risk**: `cdt-core::CompactChunk` 加字段会破坏所有构造点
  → Mitigation: 两个字段均 `Option<T> + #[serde(default)]`，`Default::default()` 自动覆盖；现有 fixture / `cdt-analyze::chunk::builder::tests` / `cdt-core` roundtrip test 用 struct 字面量构造的地方会编译失败，但用 `..Default::default()` 的不会。**实施时 SHALL 先 grep 全所有 `CompactChunk {` 构造点统一改完再 cargo check**（CLAUDE.md "核心 struct 加字段先 grep 全构造点"硬约束）
- **Risk**: 前端 Compact markdown 渲染 summaryText 比当前裸文本慢
  → Mitigation: 默认折叠（用户不展开就不渲染 markdown）；展开后用现有 `lazyMarkdown` 管线，性能与 AIChunk markdown 渲染相当
- **Risk**: System 气泡 max-w 85% 在窄 sidebar 下可能溢出
  → Mitigation: 按原版同步设 `max-w-[85%]` + 容器 flex justify-start；在 sidebar 折叠 / 双 pane 窄场景下视觉与原版一致（原版同样限制）
- **Trade-off**: 选 D1d (F) "派生层完全独立于 `ContextPhaseInfo`，从 chunks 邻接 AI 自算 token delta" 而非 D1 (A) "复用 cdt-analyze 已计算的 `compaction_token_deltas` map"——后者更省 CPU（map 已建好直接 `.get()`），但 cdt-analyze 内部 `current_phase_compact_group_id` 在连续 compact 时被覆盖（见 `cdt-analyze::context::session.rs:101-104`），map key 仅含最后一个 compact uuid，前序 compact 拿到 `None`，与原版 `groupTransformer.ts:305-315` 对每个 compact 独立 `findLastAiBefore/findFirstAiAfter` 行为不齐。D1d 接受每 `get_session_detail` 重算的 CPU 代价（O(N×M) 实测 < 1 ms）换取与原版完全一致的 tokenDelta 行为。如成为热点改单趟双指针预算 last/first AI（Risks 段已注）。

## Migration Plan

无破坏性改动——`tokenDelta` / `phaseNumber` 是新增 optional 字段：

- 老前端：`?? null` fallback，UI 不显示徽章（视觉退化但功能不破）
- 老 fixture：`#[serde(default)]` 让 deserialize 时字段缺失自动填 `None`
- 旧 IPC 调用方（HTTP）：收到的 payload 多两个可选字段，旧 client 忽略不解析也无影响

回滚：在 `cdt-api::session_detail` 派生入口加 `const COMPACT_DERIVED_ENABLED: bool = true` 顶部回滚开关；设 `false` 时直接传 `None` / `None` 即可（前端 fallback 与无新字段时一致）。

## Open Questions

- 无。D1–D5 均已决策；实施按 tasks.md 推进。

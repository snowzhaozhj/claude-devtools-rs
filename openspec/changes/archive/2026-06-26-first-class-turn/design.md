## Context

turn 当前被重造三遍（#542）：后端 `turn_index` 是 `cdt-analyze::context::session.rs` 循环里的副产品计数器（#541 已改为「真实用户消息锚定」，但仍是内联会计）；前端 `turnIndex + 1` 反建 "Turn N"；未来 API 再造一套 → 与桌面分叉。

领域逻辑（chunk-building / tool-linking / `semantic_steps`）**已在 `cdt-analyze` 共享**，桌面（经 Tauri IPC → `cdt-api`）与 CLI/MCP（经 `cdt-query`）都消费。缺的只是 turn 边界本身没有共享权威。本 change 补这一块。

**前置**：#540/#541 已 merge（commit `7db5c526`），turn 锚定已是「真实用户消息」，诊断守卫 `crates/cdt-api/tests/corpus_turn_fidelity.rs`。

**经过的论证（数据 + 权威 + 异构二审）**：本设计的每个决策都有背书——本机 843 session 实测、claude-code-guide 对 Claude Code turn 语义的权威查证、codex 对抗审查（12 finding 逐条过筛）。关键结论见各 Decision。

## Goals / Non-Goals

**Goals:**
- 抽 `derive_turns(chunks) -> Vec<Turn>` 作 turn 边界单一权威，落 `cdt-analyze`；桌面 context-tracking 与未来 API 共消费，消除分叉。
- turn 定义与 Claude Code 的 Stop-界定 turn 对齐（不是发明）。
- 修掉 `injection.id` 对 turn 号的潜伏耦合（codex C2）。

**Non-Goals:**
- 不做 API（`get_turn` 等）——属 `redesign-cli-mcp-api`。
- 不碰 chunk 渲染 / IPC wire 形状（仅 `injection.turnIndex` 与 `injection.id` 取值变）。
- 不收敛桌面 TS `buildDisplayItems`（见 D6）。
- 不 port `buildDisplayItems` / 不建 turn 的 step 内容——那是 API 需要的，编号不需要。

## Decisions

### D1：turn = Claude 自己的 Stop-界定 turn（不是发明一个概念）

- **选择**：turn = 「一条驱动输入 + 其后所有 AI 响应 + 工具调用，直到 assistant 停下等下一条驱动输入」。
- **背书（权威）**：claude-code-guide 查证 Claude Code 对 transcript turn 的定义与此**字节级一致**；`stop_reason: end_turn` 可靠标记一轮结束。
- **背书（数据）**：`471bc334`（2 用户消息 / 26 compact）的 assistant `stop_reason` 序列——压缩之间全是 `tool_use`，唯一 `end_turn` 在最末。证明 compact **发生在一轮 response 中途**、不是 turn 边界 → 这场就是 2 个 turn（与折叠一致）。
- **拒绝（Agent SDK `max_turns` per-inference）**：每次模型 round-trip 算一 turn——那是 SDK 概念，非 CLI transcript 的 turn，AI/用户都不按它理解对话。

### D2：derive_turns 作单一权威，落 cdt-analyze；层序归正

- **选择**：`derive_turns(chunks) -> Vec<Turn>` 落 `cdt-analyze`（core，sync、无 runtime 依赖）。层序固定：`chunks → derive_turns(纯结构) → { context-tracking 标注 / 桌面 "Turn N" / 未来 API get_turn }`。
- **理由**：turn 边界比「每轮注入多少 context」更基础，理应在共享层算一次、所有人消费。现状把它关在 context 函数里，API 想要 turn 就得拖整套 context 机器或自己重造（第三遍）。
- **代价**：动 `cdt-analyze` + context-tracking。正当性来自 API（`redesign-cli-mcp-api`）也要消费它——两个消费者 → 不抽就是第三次重造。**若 API 不做，本 change 退化为「直接在 session.rs 修编号」即可，不必抽 derive_turns**（YAGNI 边界，已与用户确认 API 确定要做）。

### D3：Turn 身份与 driver 模型

- **选择**：
  ```
  Turn { index: u32, driver: TurnDriver, member_chunk_ids: Vec<ChunkId>, ... }
  TurnDriver = User(UserChunkId)
             | Teammate(Vec<TeammateMsgUuid>)   // 一个 AIChunk 可批量携带 N 条
             | Headless                          // 首个 driver 之前的内容（实测近乎为空）
  ```
- **driver 优先级（每个 AIChunk 取一个）**：消费了 UserChunk → `User`；否则携带 incoming teammate-message → `Teammate`；否则折叠进前一 turn（无前驱则 `Headless` turn 0）。
- **消除 aiGroupId 双语义**：现状正常 turn 锚 AIChunk.chunk_id、被打断锚 UserChunk.chunk_id（codex 早先点名「aiGroupId 背两义」）。Turn 显式拥有 index + driver + 成员 chunk，不再借单个 chunk id 漂移。
- **N 条 teammate-message 批量进一个 AIChunk = 1 个 turn**（一次 AI 响应 = 一次交换），`Teammate` 装全部 uuid（codex W8：driver 不能是单值）。与「N 条 UserChunk = N turn」的不对称是正当的——UserChunk 是独立时间线事件，teammate-message 被 builder 打包进一个 AIChunk。

### D4：turn 边界 = 按 driver 切时间线（一条规则统一所有边界）

- **规则**：`derive_turns` 把 chunk 时间线按 driver 切。**每个 driver（UserChunk，或没消费 UserChunk 却携带 teammate-message 的 AIChunk）开一个新 turn；其后所有 chunk（续写 AIChunk / Compact / System）归属最近 driver 开的 turn；首个 driver 之前的 chunk = turn 0（Headless）。** Compact / System 永不开 turn。
- **统一收口 codex C3/C4/C5**（一条规则、非多个补丁）：
  - `[Compact, AIChunk]`：Compact 不开 turn，AIChunk 在首 driver 前 → turn 0 Headless。
  - `[AIChunk, User, AIChunk]`：首 AIChunk 在 driver 前 → turn 0；User → turn 1；次 AIChunk 归 turn 1。**两个 AIChunk 都「归当前 turn」，开头那个恰好 IS turn 0——同一条规则，非 codex 担心的「不一致」。**
  - `[teammate→AIChunk, User]`：teammate-carrying AIChunk = driver → turn 0(Teammate)；User → turn 1。
- **被打断 turn 保留 #541 行为**：UserChunk 后无 AI group → 仍是 turn，answer=null、member_chunk_ids 仅含 UserChunk。
- **可接受的边角**（spec 写明）：会话以 AI/teammate 内容开头时，第一条人类消息 = turn 1 而非 0（因为前面真有内容）。实测「headless」会话全是 teammate 会话（turn 0 = lead 派活，真实交换），纯 Headless 近乎为空。

### D5：折叠无驱动 AI 续写（compact-split）——因 phase 已管 compact

- **选择**：被 compact / 中断切出、无驱动输入的续写 AIChunk **折进所属 turn**，不各占 turn 号。**反转 #541 spec 第 238 行「AI-only group 也占一个 turn 序号」。**
- **理由（合理性，非「改动小」）**：compact 已由一等概念 `phase` 表达（每 compact 一个 phase + `compaction_token_delta`，聊天流有分隔线、IPC 有 `phase_info`）。让 turn 编号**也**编码 compact 是冗余，且污染 turn 语义——`471bc334` 会被记成 15 个 turn，「第 15 问」不存在。折叠后 turn = 用户问的次数，compact 走 phase，两轴各管各、不冗余。
- **turn 与 phase 正交**：一个 turn 可跨一个 phase 边界（一次提问内部压缩了一下），天经地义。折叠**不丢任何信息**——压缩事实、token delta、分隔线全由 phase 记着，折叠只改 turn 标签。
- **背书**：D1 的 `stop_reason` 数据 + guide「auto-compaction 对用户透明」直接印证 compact 非 turn 边界。
- **数据影响**：实测仅约 55 处 AIChunk（0.69%）标签位移，集中在重压缩会话。

### D6：不收敛桌面 TS buildDisplayItems —— 用 spec 锁契约，不合并实现

- **选择**：本 change 只统一 turn **边界**；未来 API 的 step 派生（port `buildDisplayItems`）**不与桌面 TS 合并**。靠 `turn-model` / API spec 锁契约 + corpus 差分测试（TS 当 oracle）防漂移（差分测试属 API change，非本 change）。
- **理由**：领域逻辑（chunk-building / tool-linking / `semantic_steps`）已共享在 Rust core，`buildDisplayItems` 只是其上的**呈现编排**。收敛要让桌面经 IPC 消费 Rust 派生的 step → 把高频迭代的桌面 UX 焊到后端 wire 契约，且赌上 SessionDetail hot path。**优化低频的编排重复、牺牲高频的 UX 迭代，是反向取舍。**
- **与 #542 一致**：#542 要消灭的是「领域计算重造」（turn 编号有唯一正解、被纠缠）；buildDisplayItems 是坐在已共享数据上的呈现编排，可被 spec 锁成「多实现 conform 同一契约」——这正是本仓「spec 是唯一真相、实现可多份」的既定模式。
- 此决策记此处作锚点；落地在 API change，本 change 不实现。

### D7：修 injection id 派生（chunkId-based，去掉 id↔turn 耦合）—— codex C2

- **选择**：`ContextInjection.id` 从 `format!("tool-output-ai-{turn_index}")`（及 thinking/task-coord/user-msg 同款）改为按 **AIChunk chunkId** 派生（如 `tool-output-{chunkId}`）。
- **根因**：injection 属于确定的 AIChunk（本就带 `ai_group_id` = chunkId），id 凭什么依赖 turn 号？旧方案隐含「turn ↔ AIChunk 1:1」假设——折叠后该假设破，id 撞车（两个 AIChunk 共享 turnIndex 0 → 都叫 `tool-output-ai-0`）。**这是去掉潜伏耦合，不是打补丁。**
- **背书**：grep `aggregator.rs:44-56` 实锤 4 处 `*-ai-{turn_index}`；前端只把 `inj.id` 当 `{#each}` key / 展开态 key 用、不解析格式（grep `(inj.id)`、`openTurns.has(inj.id)`），故改格式受控，且正好根治撞车。
- **唯一性**：每个 AIChunk 每类至多一条聚合 injection，`{category}-{chunkId}` 唯一。

### D8：turn-model 立为新 capability（vs 并入 context-tracking）

- **选择**：新建薄 capability `turn-model` 拥有 `derive_turns` 契约 + Turn/Driver 模型 + 边界规则；`context-tracking` 改为**消费**它。
- **理由**：#542「一等公民」本意 = 单一 owner；且未来 API 也消费它（跨 capability 共享）。并入 context-tracking 会让「turn 是什么」继续从属于「每轮注入多少 context」，层序仍颠倒。
- **保持薄**：`turn-model` 只定边界派生；step 内容（buildDisplayItems）、API 字段都不在它名下。

### D9：`[User, Compact, AIChunk]` 退化场景——A0 折进 U 的 turn（codex 二审 F1/F4/F10）

- **选择（用户确认折叠）**：序列 `[UserChunk(U), Compact, AIChunk(A0)]`（U 后直接压缩、再出 A0、中间无用户消息）中，A0 **折进 U 的 turn**（turn 跨 phase），U **不算被打断**——A0 视为 U 压缩后的延续响应。
- **理由**：对齐 D1 的 Stop 语义（压缩是一轮内部事件，非 turn 边界）；现实中此序列多为「窗口已满 → 先压缩 → 再答 U」。且与 D5「折叠 AI-only」一致——若改判 A0 为独立 headless turn，反而让 AI-only 占独立号、自相矛盾。
- **拒绝（保持 #541 被打断）**：U 算被打断、A0 另算 headless turn——与 D5 张力，且把「窗口满压缩后继续」误读成「中断 + 新轮」。
- **承载缺口（与折叠正交，效果不变）**：U 的 `user-message` injection 在其无 AI group 的压缩前 phase 仍无承载点 → 仍不出现在任何 phase 的 `contextInjections`。这是 injection 累积的 phase-bound 机制，**与 turn 归属是两件事**——折叠只改 A0 的 turn 号、不改 U injection 的孤立。归因从「中断」改为「phase 重置承载缺口」。

### 「不引入问题」硬不变量（codex 已逐条对抗，进 spec scenario 守护）

1. **wire 形状不变**：`turnContextStats` 仍 key=chunkId、`injectionsByPhase` 仍 key=phase number、前端 `data-chunk-id` 锚 + `getPerTurnStats(stats, chunkId)` 反查不变。变的只有 `injection.turnIndex` 取值、`injection.id` 派生。
2. **phase / compaction delta 独立于 turn_index**：phase 用 ai_group_id+phase_number，delta 用 compact_id+AI usage——folding 不触碰。
3. **#541 被打断 turn：常见场景保留、退化场景重诠释**：用户连发 / 末尾打断等常见被打断场景行为不变（仍占一个 turn、answer=null，`corpus_turn_fidelity` 守卫继续过）。但 `[User, Compact, AIChunk]` 退化场景**被重新诠释**（D9，codex 二审 F1/F4/F10）——这条不再是「原样保留 #541」。
4. **id 唯一**：chunkId-based 派生保证折叠后仍唯一（D7）。

### codex 12 finding 处置（已逐条数据/代码过筛）

- **进设计（已验证真）**：C2/I12（修 id 派生，D7）、W9（#541 受影响 Scenario 改写）、W8（driver 带列表，D3）。
- **证伪/非问题**：C4（同一条规则，D4）、W10-resumed（guide 确认 resume append 同文件、无 turn 概念、无错位）、W7（driver=User，teammate 作 step，不丢）。
- **规则已覆盖**：C1（并入 W9，仅 turnIndex 断言更新、非行为冲突）、C3/C5（D4 边角，spec 写明）、W6（phase 层正交，不在本 change）。
- **挂起/兜底**：I11（TS/Rust 漂移→API change 的差分测试覆盖）、W10-subagent（subagent 会话有自己的任务-prompt driver，疑非问题）。

**codex 二审（审落盘草案）逐条采纳**：F1/F4/F10（`[User,Compact,AI]` 折叠语义 → D9 定夺 + spec scenario）、F2/F3/F5（折叠后 `Compute cumulative stats per turn` / `Per-turn context stats exposure` 的「per-turn」命名失真 → MODIFIED 这两个 Requirement，body 澄清 stats 是 per-AI-group、key=chunkId、消费方 SHALL NOT 假设 `turnContextStats[turnIndex]`；title 与 body 同步抽象留 cleanup followup）、F7（id 派生收窄为 AIChunk-scoped + 被打断 user-message 锚 UserChunk.chunkId）、F6（turn-model 补 4 边界 scenario）、F9（turn-model 的 CLI/MCP SHALL 改为前瞻意图、可验收契约只覆盖 context-tracking + 桌面）、F8（每类每 AIChunk 至多一条的唯一性前提写进 spec 约束）。codex 认可 D1/D2/D5/D7/D8 站得住。

## Risks / Trade-offs

- [桌面 "Turn N" 标签位移（~55 处，重压缩会话）] → 记 CHANGELOG（Changed）；corpus 守卫断言 `turn 计数 == 驱动输入数`。
- [#541 spec Scenario 改写] → 同 commit 改 spec delta + 受影响断言；codex 列的清单采纳。
- [injection id 格式变] → 前端仅当 key 用、不解析格式（已验证），受控；ipc_contract fixture 同步。
- [derive_turns 重构波及 session.rs 精密循环] → 4 条不变量 + corpus 守卫 + codex 对抗验证兜底；这是「块① 单独审」的最强理由。

## Migration Plan

1. `turn-model`：新增 `derive_turns(chunks)->Vec<Turn>` + `Turn`/`TurnDriver`（D3/D4），含 AI-only 折叠（D5）；单测覆盖 4 场景（compact 跨界 / headless / teammate / 中断）+ corpus 守卫。
2. `context-tracking`：`session.rs` 改消费 `derive_turns` 标注 `turnIndex`；`aggregator.rs` 改 id 按 chunkId 派生（D7）。
3. 更新 `context_tracking.rs` / `ipc_contract` 受影响断言（turnIndex 值、AI-only 占号、id 格式）为预期新值。
4. codex 逐条对抗验证 4 不变量。
5. CHANGELOG 记桌面 "Turn N" 标签纠错。
- **回滚**：本 change 触及桌面 turn 标签源，回滚需连带还原 session.rs + aggregator——作一个原子单元 review。

## Open Questions

- `derive_turns` 接口是否需要携带 phase 边界引用（便于 API 后续展示「turn 内何处压缩」），还是 turn 与 phase 完全解耦由消费者各取——倾向解耦，reviewer 可挑战。
- `Headless` driver 实测近乎为空，是否保留——倾向保留作防御性兜底（resumed/fork 退化文件），标注「实测罕见」。

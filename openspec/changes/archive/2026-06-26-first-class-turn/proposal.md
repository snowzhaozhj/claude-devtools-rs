## Why

turn（一来一回的对话轮）是用户在桌面看的、AI 调未来 `get_turn` 要的、人脑里想的基本单位，但现在它被**重造三遍**（GitHub issue #542）：

1. **桌面后端**：`turn_index` 是 `cdt-analyze::context::session.rs` 循环里的**副产品计数器**，埋在 context-injection 会计里。
2. **桌面前端**：从 `injection.turnIndex + 1` **反建** "Turn N"。
3. **未来 API**（cdt-query `get_turn`）：目前无 turn 概念，一加又要再造一套 → 与桌面 turn 序号分叉。

本 change 把 turn 升为**共享 core 一等公民**：抽 `derive_turns(chunks) -> Vec<Turn>` 作 turn 边界的**单一权威**，桌面 context-tracking 与未来 CLI/MCP API 共同消费，消除分叉。

**关键事实（权威确认）**：claude-code-guide 查证 Claude Code 对 transcript "turn" 的定义是「一条用户消息 + 其后所有 assistant 响应 + 工具调用，直到 assistant 停下等待下一条用户消息（`stop_reason: end_turn`）」。本 change 的 turn 定义与之**字节级对齐**——我们不是发明 turn，而是把 Claude 真实的 Stop-界定 turn 派生出来。

本 change 是 `redesign-cli-mcp-api`（CLI/MCP `get_turn` API）的**地基**，单独先行落地、先 merge，API change 再消费稳定的 `derive_turns`。

## What Changes

- 抽 **`derive_turns(chunks) -> Vec<Turn>`** 落 `cdt-analyze`，作 turn 边界 + 编号的**单一权威**。`Turn { index, driver, member_chunk_ids, ... }`，`driver = User | Teammate(Vec) | Headless`。
- **context-tracking(`session.rs`) 改为消费 `derive_turns`** 标注 `injection.turnIndex`，不再内联自增 `turn_index`。
- **AI-only group 折叠**：被 compact / 中断切出、无驱动输入的续写 AIChunk 折进所属 turn，不再各占独立 turn 号（**反转 #541 spec 第 238 行「AI-only group 也占一个 turn 序号」**——为原则性理由：turn 该指「一次问答」，compact 续写是同一轮内部的延续，由正交的 `phase` 表达压缩边界）。
- **修 injection id 派生**：`ContextInjection.id` 从「按 turn 号拼」（`tool-output-ai-{turn}`）改为「按 AIChunk chunkId 拼」（codex 二审 C2：折叠后 turn 号不再 1:1，旧方案会 id 撞车）。
- **唯一用户可感知变化**：人类会话里 compact 切分的 "Turn N" 标签纠错（实测 843 会话中约 55 处 AIChunk，0.69%，集中在重压缩会话），如「Turn 2」→「Turn 1」。聊天流渲染、token 统计、phase、导航**全不变**。

**不做**（划清边界）：

- 不做 API（7 工具 / `get_turn`）—— 属 `redesign-cli-mcp-api`。
- 不碰 chunk 渲染（聊天流、teammate-message 嵌在 AIChunk 的样子）—— wire 形状不变（仅 `injection.id` 取值与 `turnIndex` 取值变）。
- 不收敛桌面 TS `buildDisplayItems` 与未来 Rust 派生——领域逻辑（chunk-building / tool-linking / semantic_steps）已共享，buildDisplayItems 是呈现编排，收敛会把高频桌面 UX 焊到后端；改由 spec 锁契约 + corpus 差分测试防漂移（差分测试本 change 不建，属 API change）。

## Capabilities

### New Capabilities

- `turn-model`：turn 边界派生的单一 owner——`derive_turns` 契约、turn 定义（Claude Stop-界定）、5 条边界规则（按 driver 切时间线）、`TurnDriver` 模型、AI-only 折叠语义、turn 与 phase 的正交关系。`context-tracking` 消费它；未来 `redesign-cli-mcp-api` 也消费它。

### Modified Capabilities

- `context-tracking`：`turn_index` 改为消费 `turn-model` 的 conversation-turn（折叠值），不再内联自增；`ContextInjection.id` 改按 chunkId 派生（去掉 id↔turn 耦合）；turn 可跨 phase（turn 与 phase 正交）。`turnContextStats`（key=chunkId）/ `injectionsByPhase`（key=phase number）的 wire 锚契约**不变**。受影响的 turn-anchoring / 多-phase Scenario 同步改写。

## Impact

- **代码**：`cdt-analyze` 新增 `derive_turns` + `Turn`/`TurnDriver` 类型；`context::session.rs` 改消费；`context::aggregator.rs` 改 id 派生。`cdt-api::get_session_detail` hot path 不改算法（仍消费已有输出，`turnContextStats` 仍 key=chunkId）。
- **桌面前端**：`injection.turnIndex` 取值变（标签纠错）；`injection.id` 格式变（前端仅当 `{#each}` key 与展开态 key 用，不解析格式，受控）。无渲染结构改动。
- **数据背书**：本机 843 session / 9138 UserChunk / 7945 AIChunk 实测——99 个 AI-only group（1.25%），其中 headless 44（实为 teammate 会话，Teammate driver，0 标签位移）/ compact 后续写 51 / 纯连续 4 / 本地命令 0。真正标签位移 ≈ 55 处（0.69%），集中在极少数重压缩会话（最坏 `471bc334`：2 用户消息 / 26 压缩 / 旧记 15 turn → 折叠后 2 turn，已用 `stop_reason` 验证对齐 Claude Stop-turn）。
- **测试**：`context_tracking.rs` 多数断言 key=ai_group_id（chunkId）不变；少数断言 turn_index 具体值 / AI-only 占号的 Scenario 按折叠后新值更新（预期契约变更）；`ipc_contract` turnIndex/aiGroupId/id fixture 同步。新增 corpus 守卫断言 `turn 计数 == 驱动输入数`。

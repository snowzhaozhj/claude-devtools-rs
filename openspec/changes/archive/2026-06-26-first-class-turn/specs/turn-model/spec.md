## ADDED Requirements

### Requirement: Turns derived once as a single shared authority

系统 SHALL 从会话的 chunk 序列在**单一共享位置**派生 turn 结构，作为 turn 边界的唯一权威，供当前消费方——context-tracking 的 injection 标注与桌面 "Turn N" 标签——消费，任何消费方 SHALL NOT 各自重新发明一套 turn 编号。该派生 SHALL 作为同一权威基础供未来 CLI/MCP turn 查询消费，以从源头消除「桌面 Turn N 与 API turn 序号分叉」（未来 API 的消费契约由其自身能力规约，不在本能力验收范围）。

#### Scenario: 桌面 "Turn N" 与 context-tracking 标注同源

- **WHEN** 桌面渲染某会话的 "Turn N" 标签，且 context-tracking 为同一轮的 injection 标注 turn 序号
- **THEN** 两者的 turn 序号 SHALL 来自同一派生结果、对同一轮给出相同的 index，而非各自计算

#### Scenario: context-tracking 不自行计数 turn

- **WHEN** context-tracking 为某条 injection 标注其所属 turn
- **THEN** 该 turn 序号 SHALL 取自共享派生结果，而非在 context 累计循环内独立自增

### Requirement: A turn spans one driving exchange aligned with Claude's stop boundary

一个 turn SHALL 表示**一个驱动输入**（一条真实用户消息；或在没有用户消息驱动时，一条进入的 teammate 消息）连同其后所有 assistant 响应与工具调用，直到 assistant 停下等待下一个驱动输入——与 Claude Code 以 `stop_reason: end_turn` 界定的一轮对话对齐。中途的自动压缩 SHALL NOT 结束当前 turn。

#### Scenario: 一次提问及其完整响应配对为一个 turn

- **WHEN** 用户发一条消息，assistant 经多次工具调用后给出最终响应并停下
- **THEN** 系统 SHALL 产出恰好一个 turn，包含该用户消息及其后全部响应与工具步骤

#### Scenario: 一轮响应中途发生自动压缩仍是同一个 turn

- **WHEN** assistant 响应过程中触发自动压缩（context 窗口满），压缩后 assistant 未等待新用户输入即继续完成响应
- **THEN** 压缩前后的内容 SHALL 归属**同一个 turn**；压缩边界 SHALL 由 phase（而非新增 turn 序号）表达

### Requirement: Turn boundaries partition the chunk timeline by driver

系统 SHALL 按驱动输入切分 chunk 时间线：每个驱动输入开启一个新 turn；其后所有 chunk（续写的 assistant 响应、压缩标记、系统输出）SHALL 归属于最近一个驱动输入开启的 turn；第一个驱动输入之前的所有 chunk SHALL 归属于一个无驱动的 turn 0（headless）。压缩标记与系统输出 SHALL NOT 开启新 turn。turn 序号 SHALL 单调递增、连续无空洞。

#### Scenario: 压缩后无新驱动的续写归入当前 turn

- **WHEN** 会话序列为「用户消息 U → 响应 A0 →（压缩）→ 续写 A1 → 用户消息 U2」，A1 之前没有新的驱动输入
- **THEN** A0 与 A1 SHALL 归属 U 开启的同一个 turn；U2 SHALL 开启下一个 turn

#### Scenario: 会话以非用户内容开头时首个驱动之前归 turn 0

- **WHEN** 会话在第一条驱动输入之前已有 assistant 内容（如 resumed/fork 退化前缀）
- **THEN** 这些前缀内容 SHALL 归属 headless 的 turn 0；其后第一条真实用户消息 SHALL 开启 turn 1

#### Scenario: 被打断的 turn 仍占一个 turn

- **WHEN** 一条用户消息之后，在下一个驱动输入 / 压缩 / 会话结束之前没有任何 assistant 响应
- **THEN** 该用户消息 SHALL 仍开启一个 turn，其最终响应为空（answer = null）

#### Scenario: 压缩后紧接续写折回该用户的 turn（无 AI group 在压缩前）

- **WHEN** 会话序列为 `[User(U), Compact, AIChunk(A0)]`，U 与 Compact 之间没有任何 AI group，A0 之前没有新的驱动输入
- **THEN** A0 SHALL 折进 U 开启的 turn（turn 跨越该压缩边界），U **不**被判为被打断
- **AND** A0 SHALL NOT 单独占一个 turn 序号

#### Scenario: 会话以压缩标记开头

- **WHEN** 会话序列以 `[Compact, AIChunk]` 开头（压缩标记在第一个驱动输入之前）
- **THEN** Compact SHALL NOT 开启 turn；A0 在首个驱动之前 → 归属 headless 的 turn 0

#### Scenario: 连续多个压缩标记不各开 turn

- **WHEN** 会话序列含相邻的 `[Compact, Compact, AIChunk]`
- **THEN** 两个 Compact SHALL 均不开启 turn；它们之间的压缩边界由 phase 历史表达，turn 层不为其各增序号

#### Scenario: teammate 消息前缀使首个真实用户消息成为 turn 1

- **WHEN** 会话以一条进入的 teammate 消息（驱动了首个 AIChunk）开头，其后才出现第一条真实用户消息
- **THEN** teammate 驱动的 AIChunk SHALL 为 turn 0（teammate driver）；第一条真实用户消息 SHALL 开启 turn 1

### Requirement: A turn carries a stable identity and a typed driver

每个 turn SHALL 拥有自身的 index 身份、一个类型化的 driver、以及对其组成 chunk 的显式引用，而非借用某个组成 chunk 的 id 来表示自身。driver SHALL 区分三类：用户消息驱动、teammate 消息驱动（可携带多条消息标识）、无驱动（headless）。当一个 assistant 响应批量承载多条进入的 teammate 消息时，系统 SHALL 仍将其计为一个 turn（一次响应 = 一次交换），driver 记录该批全部 teammate 消息的标识。

#### Scenario: teammate 会话每次响应计一个 turn

- **WHEN** 一个 teammate 会话（无真实用户消息）由进入的 teammate 消息逐轮驱动，每轮一次响应
- **THEN** 系统 SHALL 为每次响应产出一个 turn，其 driver 为 teammate 类型

#### Scenario: 一次响应批量承载多条 teammate 消息仍计一个 turn

- **WHEN** 多条 teammate 消息先后到达、被同一次 assistant 响应一并处理
- **THEN** 系统 SHALL 计为一个 turn，driver SHALL 记录该批全部 teammate 消息标识，而非拆成多个 turn

#### Scenario: 用户消息驱动优先于同响应内的 teammate 消息

- **WHEN** 一次 assistant 响应既回应了一条真实用户消息，又在其中承载了进入的 teammate 消息
- **THEN** 该 turn 的 driver SHALL 为用户类型；其中的 teammate 消息作为该 turn 内的步骤呈现，不另开 turn

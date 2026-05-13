## MODIFIED Requirements

### Requirement: Subagent 卡片与 Task tool 就地交错渲染

SessionDetail 渲染 `AIChunk` 时 MUST 按 `semantic_steps` 顺序依次输出 DisplayItem，使 subagent 卡片与其对应 Task / Agent 调用**时序相邻**；同时 UI SHALL 跳过与 subagent 已关联的 `Task` 或 `Agent` `tool_execution`，避免同一个逻辑调用同时以"工具调用行"和"Subagent 卡片"两种形式重复显示。

**前端跳过判定的工具名集合 MUST 与后端 `cdt-parse::is_task`（`crates/cdt-parse/src/parser.rs`）保持一致——当前为 `{ "Task", "Agent" }`**。后端 `resolve_subagents` 把这两类工具识别为 task 调用并尝试关联 SubagentProcess，前端 `displayItemBuilder` 在判定"已关联 subagent 的工具跳过 ToolItem"时 SHALL 同时识别这两个工具名；增减工具名时 SHALL 同步前后端两处实现并补对应 Scenario。

#### Scenario: Task 调用后紧随 Subagent 卡片
- **WHEN** AIChunk 含 `Read` → `Task(t_task)` → `Grep` 三个 tool_execution，且 `Task(t_task)` 已解析出 subagent A
- **THEN** UI SHALL 依序渲染：Read tool item → Subagent 卡片（A）→ Grep tool item；SHALL NOT 在 Grep 之后再输出 Subagent

#### Scenario: Task 去重
- **WHEN** `chunk.subagents` 中某 subagent 的 `parentTaskId = t_task`，且 `chunk.toolExecutions` 也含 `toolUseId = t_task, toolName = "Task"`
- **THEN** `displayItemBuilder` SHALL NOT 为该 `tool_execution` 步骤输出 `DisplayItem.type === "tool"`；subagent 卡片 SHALL 是该 Task 的唯一可见代表

#### Scenario: Agent 去重
- **WHEN** `chunk.subagents` 中某 subagent 的 `parentTaskId = t_agent`，且 `chunk.toolExecutions` 也含 `toolUseId = t_agent, toolName = "Agent"`
- **THEN** `displayItemBuilder` SHALL NOT 为该 `tool_execution` 步骤输出 `DisplayItem.type === "tool"`；subagent 卡片 SHALL 是该 Agent 调用的唯一可见代表

#### Scenario: Orphan Task 保留显示
- **WHEN** 某 Task `tool_use` 未匹配任何 subagent（`Resolution::Orphan`）
- **THEN** 对应的 `tool_execution` SHALL 照常渲染为 Tool item（Default viewer），不受去重影响

#### Scenario: Orphan Agent 保留显示
- **WHEN** 某 Agent `tool_use` 未匹配任何 subagent（`Resolution::Orphan`）
- **THEN** 对应的 `tool_execution` SHALL 照常渲染为 Tool item（Default viewer），不受去重影响

## ADDED Requirements

### Requirement: SubagentCard 在 ongoing 期间主动重拉 trace

SubagentCard MUST 监听 `(process.isOngoing, process.endTs, process.messagesTotalCount)` 三元组组成的版本指纹；当版本递增**且**该卡片处于已展开状态（用户曾触发过 `getSubagentTrace`，本地缓存 `messagesLocal` 非空）**且**`process.isOngoing === true` 时，SHALL 自动调用 `getSubagentTrace(rootSessionId, process.sessionId)` 重拉新 trace 并替换 `messagesLocal`。

未展开的 SubagentCard SHALL NOT 因版本变化主动发 IPC（仅清本地 stale 缓存或保持 `null`，等待用户下次展开时按既有 lazy 路径拉取），避免 ongoing 大会话内 N 个未展开卡片每次父 refresh 都触发 IPC 风暴。

同一 `process.sessionId` 同时收到多次版本变化 SHALL 通过 inflight 去重，但 inflight 复用 key MUST 为 `${sessionId}|${messagesVersion}` 联合 key，**不**仅按 sessionId 复用。理由：仅按 sessionId 复用时，旧版本（版本 N）的 Promise 在 pending 期间版本递增到 N+1，新触发的重拉若复用旧 Promise 会把版本 N 的旧 trace 写入 `messagesLocal`，且因 effect 认为"已在拉取中"而不再排第二轮——版本 N+1 的新 chunks 永远拿不到。等价替代实现：仅按 sessionId 复用但 Promise settle 后 SHALL 检查"当前版本 == fetch 时版本"，不等则视为 stale 并立即触发新一轮重拉。

#### Scenario: 已展开 ongoing subagent 在版本递增时主动重拉

- **WHEN** SubagentCard 已展开（`messagesLocal !== null`）且 `process.isOngoing === true`
- **AND** 父 session refresh 后 `process.messagesTotalCount` 从 5 变为 8
- **THEN** SubagentCard SHALL 自动调 `getSubagentTrace(rootSessionId, process.sessionId)` 重拉，并把返回的 `Vec<Chunk>` 替换到 `messagesLocal`，UI 渲染的 ExecutionTrace SHALL 立即反映新增的 chunks，**无需**用户折叠重开

#### Scenario: ongoing 翻转到 done 时同步最终状态

- **WHEN** SubagentCard 已展开，`process.isOngoing` 从 `true` 翻转到 `false`（subagent 收尾）
- **AND** `process.endTs` 从 `null` 变为具体时间戳
- **THEN** SubagentCard SHALL 触发最后一次 `getSubagentTrace` 重拉，让 UI 同步到 subagent 完成态的完整 trace

#### Scenario: 未展开卡片不主动重拉

- **WHEN** SubagentCard 未展开（`messagesLocal === null`），`process.messagesTotalCount` 在多次父 refresh 中递增
- **THEN** SubagentCard SHALL NOT 发 `getSubagentTrace` IPC；用户首次展开时 SHALL 走既有 lazy 路径拉一次最新 trace

#### Scenario: 同 sessionId 同版本并发触发 inflight 复用

- **WHEN** SubagentCard 已展开，`messagesVersion = "1|_|5"` 触发 `getSubagentTrace`（尚未 settle）
- **AND** 同 sessionId 同版本 `"1|_|5"` 因 effect 重跑再次触发
- **THEN** 第二次 SHALL 复用第一次的 Promise（key `${sessionId}|1|_|5` 命中），SHALL NOT 并发发起第二次 IPC

#### Scenario: 同 sessionId 跨版本不复用旧 Promise

- **WHEN** SubagentCard 已展开，`messagesVersion = "1|_|5"` 触发 `getSubagentTrace`（Promise A 尚未 settle）
- **AND** pending 期间版本递增到 `"1|_|8"`，新一轮重拉触发
- **THEN** 第二次 SHALL 视为新版本 fetch（key `${sessionId}|1|_|8` 不命中旧 inflight），SHALL 发起 Promise B；Promise A settle 时**不应**把版本 5 的旧 trace 写入 `messagesLocal`（fetch 时版本与当前版本不等，结果 SHALL 被丢弃或被 Promise B 的结果覆盖）

#### Scenario: 老后端缺 messagesTotalCount 字段降级

- **WHEN** 旧后端响应不含 `messagesTotalCount`（JSON 反序列化为 `undefined`）
- **THEN** 版本指纹三元组中 `messagesTotalCount` 视为 `undefined`，版本永远是常量，主动重拉 effect SHALL NOT 触发；行为退化为既有 lazy 路径（用户折叠重开才能看到新内容），SHALL NOT 报错或卡死

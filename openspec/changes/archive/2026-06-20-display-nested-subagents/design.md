## Context

会话目录 `<root>/subagents/agent-*.jsonl` 扁平存放该 root 会话的所有 subagent transcript,含嵌套层(subagent 内部再 spawn 的子 subagent)。当前:

- 主会话打开走 `build_chunks_with_subagents(messages, candidates)`(`local.rs:2656`),用 `resolve_subagents` 三阶段匹配把主会话的 `Agent`/`Task` 调用解析成 `Process`,attach 到 `AIChunk.subagents`,只覆盖 **level-1**。
- 展开某个 subagent 时走 `get_subagent_trace`(`local.rs:4179`),它对子 transcript 用**非递归** `build_chunks`,子里的 `Agent` 调用只产一个 `ToolExecution`,前端按普通工具渲染——这就是嵌套层无法展开的根因。

关键已查实事实(决定方案可行性):

1. 父执行链里每个 spawn 子 agent 的 `Agent` 调用,其 JSONL 顶层 `toolUseResult.agentId` = 子 agent 的 session id,`cdt-parse` 已提取为 `ToolExecution.result_agent_id`(`tool_execution.rs:70`),已 camelCase `resultAgentId` 透传前端。
2. 前端递归渲染基建已就位(`session-display` spec §`Subagent 内联展开 ExecutionTrace`):`ExecutionTrace` 对 `DisplayItem.type === "subagent"` 渲染 `SubagentCard`,`rootSessionId` 一路向下传递,`messagesOmitted` 首展开懒拉,深度上限已实现。
3. `get_subagent_trace(rootSessionId, subSessionId)` 已能按子 session id 在 `<root>/subagents/` 定位文件并返回 `Vec<Chunk>`。

即:数据已解析、前端能递归、懒加载端点已存在;唯一缺口是 `get_subagent_trace` 返回的子 chunks 没把嵌套 `Agent` 调用标成 subagent item。本设计经 codex 异构二审(7 finding,0 critical),Decisions 已吸收其修正。

## Goals / Non-Goals

**Goals:**

- 展开任意 subagent 时,其内部 spawn 的子 subagent 可作为内联卡片继续展开,逐层深入。
- 不引入新文件 IO / 单次 payload 与今天展开 level-1 同量级。(原"不触碰 `get_session_detail` 主路径"于 apply 阶段修正——内联未裁剪路径也需 promote,见 D1b;仍为纯内存零 IO。)
- 前端零渲染改动,复用既有递归 `SubagentCard` + 深度保护。

**Non-Goals:**

- 不在主会话打开时预解析整棵嵌套树(会撞穿 payload 与 build 预算)。
- 不为骨架引入精确实时状态(`is_ongoing` / 计数);状态以首次展开懒拉为准(见 D4)。
- 不修 `find_subagent_jsonl` 兜底路径歧义(main 既有隐患,见 D7,范围外)。
- 不新增 trace 级缓存(可选后续优化,见 Risks)。

## Decisions

### D1:骨架升级作为 `get_subagent_trace` 的后处理,而非递归 resolver

`get_subagent_trace` 在 `build_chunks` 后调用新纯函数 `promote_result_agent_tasks(chunks)`,把带 `result_agent_id` 的 `Agent`/`Task` `ToolExecution` 升级成骨架 subagent。**不**改成递归 `build_chunks_with_subagents`。

- 备选(已否决):让 `get_subagent_trace` 走 `build_chunks_with_subagents` + candidate 池递归。problem:`candidate_to_process` 依赖 candidate 的完整 `messages` 派生 `header_model` / `messages_total_count`,且会 clone 子完整 messages,一层 fan-out 100 子就把整层内联进单次 IPC,撞穿 payload 预算;还需建父子图 / 缓存,工作量放大一个数量级。
- 取舍:骨架升级只消费已解析的 `result_agent_id`,零新 IO,每层只在被展开时拉一个 transcript。

### D1b:内联 subagent messages 路径同样需 promote(apply 阶段发现,修正 D1 单点假设)

D1 原假设 promote 只需挂在 `get_subagent_trace` 单一返回路径,前提是 subagent messages 在首屏 IPC 总被裁剪(`messagesOmitted=true`)→ 前端必走 `getSubagentTrace` 懒拉。**apply 阶段真实数据验证(HTTP `?http=1` 页面)证伪此前提**:`LocalDataApi::get_session_detail` 按 ipc-data-api spec 返回**完整未裁剪**数据,裁剪只发生在 Tauri command handler 的 `apply_display_omissions`;HTTP route / MCP / CLI 消费者 + Tauri 回滚裁剪开关时,subagent `messagesOmitted=false`,前端**直接渲染内联 `Process.messages`**,绕过 `get_subagent_trace`,内联层的嵌套 `Agent` 调用因此仍显示为普通工具(用户实测复现:level-1 subagent 内 `Trace build_graph_profiled callers` 等嵌套调用为裸工具)。

- 修正:`parse_subagent_candidate`(构建 candidate `messages` → 内联 `Process.messages`)在 `build_chunks` 后**同样**调 `promote_result_agent_tasks`。两处共用同一纯函数,语义一致(`chunk-building::Promote nested Agent calls to skeleton subagents` Requirement body 已泛化为"`build_chunks_with_subagents` 不可用的路径",本就涵盖内联路径,只是 D1 文字漏列)。
- 性能:promote 是纯内存 O(chunks/候选)变换,零新文件 IO;每个 candidate 在 scan 阶段已 `build_chunks`,promote 仅多一次同数据线性扫描。实测 7f59237e(63 个 level-1 candidate)内联升级出 96 个 level-2 骨架、0 个裸嵌套 Agent 工具,主路径 wall 无可感知回归(纯内存,不触发 perf.md 反模式)。
- 取舍:此修正让 promote 进入 `get_session_detail` 主路径(原 D1 声明的"不碰主路径"作废)——但代价是已 build 好的 chunks 上一次零 IO 线性扫描,远低于"内联嵌套层永远显示为工具"的功能缺陷。
- 验证:HTTP 页面端到端确认——`a179ec85`(level-2)从裸工具变为可展开骨架卡片,展开后懒拉出其自身 `Execution Trace`(2 tool calls)。

### D2:骨架必须回填 `parent_task_id = exec.tool_use_id`(否则重复渲染)

前端 `displayItemBuilder.ts` 靠 subagent 的 `parentTaskId` 命中来**跳过**原始 `Agent`/`Task` 工具 item。骨架若不回填 `parent_task_id`,前端会**同时**渲染骨架 subagent 卡片与原始 Agent 工具块,造成重复。

- 同时:`SubagentSpawn.placeholder_id` MUST 等于骨架 `Process.session_id`,与现有完整 resolve 路径(`builder.rs:590-604`)一致,前端才能正确去重 + 定位。
- 这是 codex 二审的首要 finding,作为硬约束写进 spec。

### D3:骨架 `Process` 字段策略

由 `ToolExecution` 就地合成,字段固定:

- `session_id = exec.result_agent_id`
- `subagent_type` 取自 `exec`(沿用现有 subagent_type 来源)
- `description` 取自 `exec.input.description`,**加字符上限**(见 D5)
- `parent_task_id = Some(exec.tool_use_id)`(D2)
- `spawn_ts = exec.start_ts`,`metrics = Default::default()`
- `messages = []`,`messages_omitted = true`,`messages_total_count = 0`(或 `None`)
- `is_ongoing = false`(D4 降级)

`Process` 的 `spawn_ts` / `metrics` 无 `serde(default)`,必须显式提供——骨架工厂统一兜住,避免散落构造点遗漏。

### D4:状态降级方案 a(零 IO 优先,展开即纠正)—— 已与用户确认

骨架 `is_ongoing=false` / `messages_total_count=0`。后果:展开**前**,一个真在运行的嵌套 subagent 在父执行链里短暂显示为已完成。

- 备选 b(已否决):合成骨架时读子文件首尾行补 `is_ongoing` + 计数。problem:每层 fan-out N 子就多 N 次轻量读,违背"零新 IO";且嵌套子多数已完成,收益低。
- 缓解:`SubagentCard` 对 `messagesOmitted=true` 的骨架首次展开 MUST 调 `getSubagentTrace` 懒拉,拉到真实 trace 后状态以懒拉结果为准。用测试把"骨架展开前 done / 展开后纠正"这一降级行为固定下来,使其是**已知契约**而非 bug。

### D4b:骨架 header 的 model badge 只读 `headerModel`,不从懒拉 messages 派生(apply 阶段发现)

apply 阶段真实数据验证发现 D4 降级有一个**布局跳动**副作用:`SubagentCard.modelName` 原逻辑是"`process.headerModel` 优先,缺省则从 `effectiveMessages` 派生"。骨架 `headerModel` 缺省 + `messages` 懒拉,导致**折叠时 header 无 model badge、展开懒拉到 messages 后 header 突然冒出 model badge**(用户反馈:"展开后突然多冒出来一个模型")。始终可见的 header 在展开瞬间发生宽度跳变。

- 修正:`SubagentCard` header(始终可见)的 model badge **只读** `process.headerModel`(后端候选转换阶段预算的稳定值);`modelName`(含 messages 派生 fallback)仅用于**展开后** body 的 `Model` 详情行。骨架 `headerModel` 缺省 → header 不显示 model(展开前后一致);真实 model 随展开 body 的详情行一并出现,属正常展开行为,非 header 跳变。
- 取舍:嵌套骨架 header 不显示 model(零 IO 下本就无法廉价获知子 agent 的 model);完整 resolve 的(非骨架)subagent 候选转换阶段已预算 `headerModel`,header 不受影响。代价是嵌套层折叠态少一个 model badge,换 header 不跳动。
- 验证:HTTP 页面端到端(playwright)——`Trace build_graph_profiled callers`(a179ec85)展开前后 header 均无 model badge,Model 仅出现在展开 body 的 `Type Task · Model opus4.6 · ID a179ec85` 详情行。

### D5:`description` 字符上限防 fan-out payload 膨胀

一层可能 fan-out ~100 个子骨架,每个携带 `description`。骨架 `description` SHALL 截断到固定字符上限(沿用既有 description 截断口径,如 200 字符),完整内容仍可由该 subagent 自身的 `input.description` / 展开后 trace 查看。

### D6:核心逻辑抽成 cdt-analyze 纯函数 `promote_result_agent_tasks(chunks)`

不在 API 层散写。纯函数输入 `Vec<Chunk>`、输出升级后的 `Vec<Chunk>`,便于单测直接喂构造的 AIChunk 验证:骨架生成、`parent_task_id` 回填、`SubagentSpawn` 紧随对应 `ToolExecution` 的顺序、`description` 截断、去重(不重复渲染)。复用 chunk-building 现有 `Attach subagents` / `Filter Task tool uses` / SubagentSpawn 插入顺序的语义。

### D7:`find_subagent_jsonl` 路径歧义 —— 范围外

`find_subagent_jsonl` 兜底遍历同 `project_dir` 下"任意 session 目录"的 `subagents/`,两个 session 都有 `agent-<id>.jsonl` 时按 `read_dir` 顺序取第一份,可能拿错文件。这是 main 既有隐患,嵌套展开会更频繁触发。

- 决定:本 change **不**修,单独记 GitHub issue / 独立 PR(精确路径优先 + 命中后读首行校验 parent/session id)。理由:与骨架升级正交,混入会放大 diff 与 review 面。

### D8:只接 `get_subagent_trace`,显式不接 `get_workflow_agent_trace`

`get_workflow_agent_trace`(WorkflowCard 渲染 workflow agent trace 用)同样走 `buildDisplayItemsFromChunks` → `ExecutionTrace`,表面看也该接 promote(codex 二审 warn)。但**刻意不接**:

- workflow agent 的子文件落在 `<root>/subagents/workflows/<runId>/agent-*.jsonl`,而 `SubagentCard` 递归展开统一调 `getSubagentTrace` → `find_subagent_jsonl`,后者只扫 `<root>/subagents/agent-*.jsonl`(**不含** `workflows/` 子目录)。
- 若给 `get_workflow_agent_trace` 接 promote,会产出一个"可展开但展开必为空"的假骨架——比现状(显示为工具)更糟。
- 完整支持 workflow agent 内嵌套 subagent 需要 `SubagentCard` 感知 workflow 上下文并改用 `getWorkflowAgentTrace` 懒拉,是独立的较大改动。
- 取舍:本 change 只覆盖普通 subagent 嵌套(Agent/Task 工具,落 `subagents/` 扁平目录,真实样本 7f59237e 即此类)。workflow agent 嵌套留待后续。

## Risks / Trade-offs

- [无 `result_agent_id` 的嵌套 subagent 不可展开] → 已知局限。零 IO 方案只升级 tool_result 已回填 `toolUseResult.agentId` 的嵌套调用;未完成 / 中断 / 未回填 agentId 的嵌套 `Agent` 调用保持工具显示（现状，不退化）。真实样本 7f59237e 验证:某 level-1 agent 内两个 Agent 调用，已完成的（有 agentId）正确升级、未回填的保持工具。绝大多数历史会话的嵌套已完成，覆盖足够。未来若需全覆盖，可引入 `meta.json.toolUseId` 父子键作 Phase0（codex 第一轮 finding 1），代价是每层读 N 个 meta.json——本 change 不做。
- [展开前状态短暂不准(D4)] → 接受为已知降级;首次展开懒拉纠正;测试固定行为。
- [无 trace 缓存,反复折叠重开重复 parse 同一文件] → 非阻塞 info;`get_subagent_trace` 单文件 parse 成本可控;留作后续可选短生命周期缓存(`rootSessionId+subSessionId+signature`)。
- [深层嵌套无限递归 / 环] → 由前端既有深度上限兜底;骨架不递归预解析,深度由用户点击驱动,天然有界。
- [路径歧义拿错文件(D7)] → 范围外,但需在 issue 留痕,避免被当作本 change 回归。

## Migration Plan

- 纯增量,无数据迁移。骨架升级作用于两处:`get_subagent_trace` 返回路径(懒拉)+ `get_session_detail` 经 `parse_subagent_candidate` 构建的内联 subagent `messages`(未裁剪时,见 D1b)。两处共用纯函数,均为已 build chunks 上的零 IO 线性扫描。
- 回滚:`promote_result_agent_tasks` 是两处各一次后处理调用,移除两处调用即回退到"嵌套显示为普通工具"的现状,无残留状态。

## Open Questions

- 无阻塞性未决项。`messages_total_count` 填 `0` 还是 `None` 在实现期按 `Process` 现有类型与 IPC 契约定(二者前端表现等价,以不破坏 `messagesTotalCount` round-trip 测试为准)。

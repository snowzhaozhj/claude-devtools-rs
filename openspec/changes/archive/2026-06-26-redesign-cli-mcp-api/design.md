## Context

cdt 的 CLI/MCP API 最初是为 UI 消费设计的：骨架 + 懒加载 + 按 `FileSignature` 缓存。AI agent 的需求不同——「一次拿到完整相关数据」。现有 `get_session_chunks` 用 `content_mode`（omit/overview/full）三档开关，默认 `omit` 只返回结构 + size 元数据，逼 AI 先探测、再按 range 取、可能猜错重来；多次调用的 system prompt 开销远超直接返回完整数据。

桌面端早已把 chunk 转成可视的「turn」结构：前端 `buildDisplayItems(chunk: AIChunk)` 用 `semantic_steps` 顺序做骨架、按 `tool_use_id` 从 `tool_executions` 填明细、识别 lastOutput（= answer）。但 CLI/MCP 的 view 层（`ChunkView`）没暴露这层，直接吐裸 chunk。这是「看不到工具调用链」的根因。

session 解析与查询走共享层 `cdt-query::Engine`（仅 `cdt-cli` 依赖），桌面端渲染会话走 Tauri IPC → `cdt-api::LocalDataApi` 直连，**不经过 `cdt-query`**。这条边界让本 change 的扩展能锁在 CLI/MCP 专属层而不碰桌面端性能。

**前置约束**：本设计的 turn 模型依赖 #540 修复——见 D0。

## Goals / Non-Goals

**Goals:**
- AI 一次调用拿到「完整相关数据」，把会话 514 那类 27 次调用降到 2-3 次。
- 工具 6 → 7、参数 31 → 23；CLI 与 MCP 数据参数完全一致。
- 引入 turn 模型暴露完整调用链（thinking / text / tool input-output / subagent）。
- 服务端内置截断替代参数化裁剪；API 自闭环（截断数据可取回全文）。
- 扩展锁在 CLI/MCP 专属层，桌面端零性能影响。

**Non-Goals:**
- 不改桌面端渲染（仍 chunk 层渲染）；turn 是 chunk 之上给 AI 的视图变换。
- 不在本 change 修 turn 锚定（属 #540，是本 change 的前置 blocker）。
- 不动 `cdt-core` / `cdt-parse` / `cdt-analyze` 及 `cdt-api::get_session_detail` hot path。
- 不覆盖 `cdt export`（CLI 独有的用户侧导出，与 AI API 无关）。

## Decisions

### D0：turn 模型依赖 #540（blocker，非本 change 修复）

turn = **一条真实用户消息 + 它的 AI 响应，锚在用户消息**（对齐 Claude Code UX turn 的 Stop-hook 界定；非 Agent SDK `max_turns` 的 per-inference）。turn 与 AIChunk 实践 1:1（语料验证：一句话 → 多 AIChunk 仅 13/9095 ≈ 0.14%，多由 Compact 切分）。

但现有 `cdt-analyze::context::session.rs` 的 `turn_index` 锚在 `AIChunk`（每个 `Chunk::Ai` 才 +1），导致被打断的用户消息丢 turn：响应被打断时 Claude Code 写 `model="<synthetic>"` 占位消息，被 `cdt-parse::noise.rs` 判 `HardNoise(SyntheticAssistant)` 过滤 → 这一轮不产 AIChunk → 用户消息的 `previous_user_chunk` 被下一条覆盖 → 从 turn 视图消失。语料实测 ~597/9095 ≈ 6.5% 真实对话消息丢 turn（集中在「反复催继续 / 打断」会话）。

**决策**：本 change 的 turn 模型建在「用户消息锚定」之上。turn 锚定的修复（含 `<synthetic>` 过滤是否过宽）走 #540 / 独立 change，先于本 change apply。诊断守卫 `crates/cdt-api/tests/corpus_turn_fidelity.rs`，修复后重跑「丢 turn」应趋近 0。

### D1：get_session（compact）与 get_turn（detail）拆两个工具

- **选择**：两个工具，各自返回稳定 schema。
- **拒绝**：单个 `get_session` 用 `turns` 参数有无切换 compact/full —— 同一接口因参数返回不同结构，AI 无法预测响应体积、难做 token 预算（codex 二审确认）。
- **代价**：工具数 +1。换 schema 稳定，值。

### D2：加 get_step_output 作为 escape hatch

- **选择**：独立工具取被截断 step 的完整 output 原文，API 自闭环。
- **拒绝**：让 AI 自己 `Read` 原文——Bash 输出 / MCP 返回 / WebFetch 结果没有文件路径可读，且历史文件可能已变；不通用。
- **拒绝**：`get_turn(session, turn, step)` 复用——会让 get_turn 因 `step` 参数返回不同 schema（同 D1 反模式）。

### D3：compact overview 的 tools 按工具类型聚合

- **形态**：`tools: [{name, count, errorCount}]`，按工具名去重。
- **拒绝**：per-call 列表 `[{name, isError}]`——一个 turn 可 200 次调用，列表无上限。聚合后条目数 = 工具种类（通常 5-15），天然有界。
- 保留 per-type `errorCount`（非只 turn 级），让 AI 不钻取即知哪种工具出错（用户否决了只给 turn 级 errorCount 的简化）。

### D4：分页用 total + nextCursor（对齐 MCP 协议）

- **选择**：`{total, nextCursor}`；`nextCursor` 存在即「还有下一页」，不存在即到底。
- **拒绝**：额外 `hasMore` / `returned`——`nextCursor` 存在即 hasMore，`items.length` 即 returned，冗余。
- **依据**：调研 Stripe / Anthropic / MCP 均用 flat 字段；MCP 协议本身就是 flat `nextCursor`。Slack 的嵌套 `response_metadata` 是少数派，不采。

### D5：分页字段直接写，不做泛型/组合抽象

- **选择**：`total` + `nextCursor` 两个字段直接写在各 response struct。
- **拒绝**：泛型 `PaginatedResponse<T>`——只 fit 纯列表（list_sessions / search），fit 不了 get_session / get_turn 这类「领域字段 + 子列表分页」的复合响应。
- **拒绝**：组合 `Pagination` + `#[serde(flatten)]`——flatten 有运行时开销且与 `rename_all` 有已知 edge case。两个字段重复 8 行 < 一个不 fit 的抽象。

### D6：question / answer 不截断

- 实测排除系统注入（skill body / task-notification 等 `is_meta`，chunk-building 已过滤）后，真实用户消息多 10-200B，answer 多 100-2000B。20 turns compact 总量 20-50KB，可控。
- 教训：早期用 JSONL 直接统计把 45KB 的 skill body 误当用户提问；必须用 `cdt-parse`/`cdt-analyze` 产出的 UserChunk 统计。

### D7：thinking 不截断

- 实测 thinking p50=1.5KB / p95=4.4KB / max=6.8KB。由 get_turn 的 step 分页（50/page）控制总量，不在字段级再截。

### D8：tool output 按 5KB 阈值截断

- `<5KB` 全量；`≥5KB` 截前 2000 字 + `outputTruncated: true` + `outputBytes`。大 output 主要是 Read 大文件，单个可达几十 KB。
- 全文走 D2 的 `get_step_output`。

### D9：get_turn 的 steps 从前往后

- answer 已在 get_turn 顶层，drill 的目的是看「因果推理链」，从头看更自然。
- 区别于 list_sessions / get_session 的「最近 20 在前」——turn 内 steps 是线性因果链，不是「最新最重要」。

### D10：search 返回 turn 级命中，平铺

- `results: [{sessionId, turnIndex, question, matchSnippet, timestamp, projectName}]`，同一 session 多 turn 命中各自独立一条。
- AI 拿 `turnIndex` 直接 `get_turn` 钻取，不需要中间「先 get_session 再找 turn」的跳转。
- **现实性约束**：现有 search 是纯子串匹配（`text_lower.find`），按 session mtime 倒序遍历，非全文索引/相关性排序。本 change 只把命中结果从 message 级映射到 turn 级，不改匹配算法本身。

### D11：grep 匹配完整 steps 内容 + matchedIn 标注

- get_session 的 `grep` 匹配整个 turn 的 chunks 内容（复用 `chunk_matches_grep`，覆盖 AI 响应文本 / tool name / tool input / tool output / error_message / 用户消息），命中返回整个 turn + `matchedIn` 标注命中位置。
- **matchedIn 归因规则**：在 chunk 级匹配成功后，二次扫描确定命中来源。取值规则：命中 tool_execution → `"tool:<toolName>"`（如 `"tool:Read"`）；命中 AI 响应文本 → `"answer"`；命中用户消息 → `"question"`；命中 thinking → `"thinking"`；命中 error_message → `"error"`。多处命中取第一个（按上述优先级）。
- **拒绝**：只匹配 question/answer——会漏「用户没提但工具调用里出现」的 turn。

### D12：filesTouched 放 list_sessions，userIntents 放 get_session

- `filesTouched`：~800B/session（≤20 条已去重，`session_metadata.rs` 用 HashSet）。放 list_sessions 让 AI 不钻取即可「按文件找 session」——这是 search 覆盖不了的语义（search 是「提到了文件名」，filesTouched 是「实际修改了」）。
- `userIntents`：~3KB/session 且与 list_sessions 已有的 `title` 重叠，放 get_session 即可。
- codex 二审确认：两字段性质不同（结构化小信号 vs 非结构化大文本），区别对待不是不一致。

### D13：删顶层 errors 列表

- 现有 get_session 顶层 `errors[{chunkIndex,...}]` 被 turn 模型覆盖：compact 的 `tools[].errorCount` + get_turn 的 step `isError`。且它用 `chunkIndex`，新模型已无 chunk 概念。

### D14：step 类型镜像桌面端 DisplayItem union

step 类型集合 = 前端 `buildDisplayItems` 的 DisplayItem union（已验证可用的 taxonomy）：`thinking` / `text` / `tool` / `subagent` / `teammate_spawn` / `workflow` / `interruption` / `user_message`（turn 内排队输入） / `slash`（用户 `/命令`，内联） / `teammate_message`。补齐桌面端 displayItemBuilder 当前漏处理的 `interruption`（SemanticStep 已有该变体）。

正交维度——`tool` step 的 output 三态（`ToolOutput` enum）：`{text}` / `{structured}`（Bash `{stdout,stderr,exitCode}` 等 JSON，**不扁平化**否则丢 exitCode）/ `{missing}`（orphan，tool 调了无 result）。

**工具升格独立 step 类型，当且仅当结构上非普通 input→output**：Task/Agent → `subagent`、SendMessage(派生队友) → `teammate_spawn`、Workflow → `workflow`。Read/Edit/Write/Skill/Bash/Grep/MCP 工具等都留 `tool` + `name`（开放字段），桌面端的 Read/Edit/Write/Bash 专用 Viewer 是视觉呈现层，不是数据子类型。step 类型是封闭集（结构轴），工具名是 tool step 上的开放字段（语义轴），不爆炸成几十个 step 类型。

### D15：subagent 当独立 session 暴露（A1）

- subagent step 放 `{type:"subagent", name, description, subagentSessionId, stepsCount}`；AI 用 `subagentSessionId` 复用 get_session / get_turn 递归钻取。
- **拒绝 B（递归内联）**：父 turn 体积爆炸 + 嵌套难解析。
- **拒绝 C（专用 get_subagent_trace 工具）**：破坏「7 工具统一」。
- subagent 本是有 session_id 的独立会话（`Process.session_id`）；其 chunks 在父 `get_session_detail` 时已随 `AIChunk.subagents[].messages` 加载。子目录 `<project>/<父session>/subagents/agent-*.jsonl` 不在普通 projects 目录，`get_session` 解析需建 lazy 反查索引（subagentSessionId → 父）。索引放 `cdt-query::Engine`（CLI/MCP 专属），桌面端不付代价。

### D16：扩展锁在 CLI/MCP 专属层（桌面端零性能影响）

| 层 | 桌面端共享 | 变化 |
|---|---|---|
| cdt-core / cdt-parse / cdt-analyze | 是 | 不变（semantic_steps + tool_executions + AIChunk.subagents 已有） |
| cdt-api::get_session_detail | 是 | 不改 hot path，只消费已有输出 |
| cdt-query::Engine | 否（仅 cdt-cli 依赖） | turn 构建 + subagent 反查索引主场 |
| cdt-cli view / mcp / main | 否 | TurnCompact / Step / StepOutput view + 7 工具 + turn/step-output 子命令 |

`buildDisplayItems`（AIChunk → steps + answer，~150 行纯数据变换、无 I/O）从前端 TS port 到 Rust 共享（落 `cdt-analyze` 或 `cdt-query`），消除「桌面端 TS 一份、CLI/MCP 没有」的真正重复。turn 配对（user 消息 + 其 AI 响应）是其上 ~30 行薄视图，不是第二套复杂逻辑。

## 数据模型（已废弃——以"更新后的数据模型"段为准）

> 原参考模型保留供审计。实际字段契约见下方 Revisions 段的"更新后的数据模型"和 specs delta。

完整带 JSON 示例 + 效果对比 + CLI 命令对照的可视化草稿见同目录 `reference-redesign.html`。

## Risks / Trade-offs

- [#540 未修则 turn 模型不忠实] → 本 change 标记 #540 为 apply 前置 blocker；诊断守卫 `corpus_turn_fidelity.rs` 把关。
- [BREAKING：删 get_session_chunks / content_mode 破坏现有 MCP 客户端 / CLI / skill] → 见 Migration Plan；CLI `--raw` 保留原 chunk 逃生舱。
- [subagent 反查索引冷扫成本] → 索引 lazy 化、CLI/MCP 专属；桌面端不 link `cdt-query` 不受影响。
- [search 仍是子串匹配，非全文/相关性] → 本 change 不改匹配算法，只改命中粒度；现实性已在 D10 标注，避免对 AI 承诺「智能搜索」。
- [turn 配对在「2 条真实 user 连发」边界与 #540 行为耦合] → 该边界本就是 #540 的丢 turn 场景，随 #540 一并定义。

## Migration Plan

1. 先落地 #540（turn 锚定改用户消息）+ 重跑 `corpus_turn_fidelity.rs` 确认丢 turn → 0。
2. port `buildDisplayItems` 到 Rust（`cdt-analyze`/`cdt-query`），桌面端 TS 暂保留（后续可改调 Rust，非本 change）。
3. `cdt-query::Engine` 加 turn 构建；`cdt-cli` 加 view structs + 7 工具 + 子命令。（subagent lazy 反查索引 deferred，见 D15b）
4. 删 `get_session_chunks`，迁移其 grep / context budget 行为到 get_turn。
5. 更新 setup / skill / 文档里对旧工具的引用。
- **回滚**：本 change 在 `cdt-cli` 层，回滚不影响桌面端与 `cdt-api`。

## Open Questions（已 resolved）

1. `session-turn-view` 作为新 capability → **保留**，turn 数据模型单一 owner 清晰。
2. `reference-redesign.html` 放 change 目录 → **保留**，archive 时冻结。
3. step 分页 + subagent 递归 → subagent 钻取 **defer**（见 D15b），step 分页在 apply 期细化。

## Revisions（grilling review, 2026-06-26）

以下修订块记录 propose → apply 之间的设计调整。**不删**原决策，保留审计。

### D0b：前置 blocker 已解决 + turn 三态

#540（turn 锚定）已 merge（commit 7db5c526）。#542（derive_turns 一等公民）已 merge（commit 0f30c9e4）。`cdt-analyze::derive_turns` 是 turn 边界**唯一权威**，`Turn{index, driver, member_chunk_ids}` + `TurnDriver{User, Teammate, Headless}` 三种 driver。

**API 层面不加 `driver` 字段**。`question` 填实际驱动输入：
- `User` → 用户消息文本
- `Teammate` → 队友消息文本
- `Headless` → `null`（退化前缀，无驱动输入）

`answer` 逻辑不变（AI 最终文本响应，被打断时 `null`）。

### D2b：get_step_output 改名 get_tool_output + 改用 toolUseId 寻址

**原 D2**：`get_step_output(session, turn, step)` 三参数按 step index 定位。

**修订**：改名为 **`get_tool_output`** + 改用 `(session, toolUseId)` 两参数。理由：
1. `toolUseId` 是 JSONL 持久化的全局唯一 ID；step index 是运行时构建的临时序号。持久化 ID 更稳定。
2. 已有 `DataApi::get_tool_output(session, tool_use_id)` 完整实现（parse → build_chunks → scan tool_use_id），直接复用零新代码。
3. 只有 tool step 会被截断（D6/D7 明确 question/answer/thinking 不截），`toolUseId` 完全覆盖。
4. **改名理由**：`get_step_output` 暗示"任何 step 类型都能取 output"，但实际只服务 tool step（只有 tool output ≥5KB 才截断）。`get_tool_output` 准确表达了"取被截断的工具输出全文"的语义，消除 step 泛化概念与 tool 专用逃生舱之间的命名矛盾。

### D4b：加 pageSize 参数

**原 D4**：删 `limit`，只留 `total + nextCursor`。

**修订**：加 `pageSize` 参数（替代 `limit`），适用于 `list_sessions` / `get_session` / `get_turn` / `search`。理由：AI 可能只需最近 5 条（`since="today"` + `pageSize=5`），也可能需全扫；固定 page size 会在少量场景浪费、全量场景强制多轮翻页。这不是 design 要删的反模式参数（`content_mode` / `group_by`），而是基本分页控制。

### D6b：project 参数确认删除

实测 336 个 project 目录：`find_session_project` 全扫 110-140ms，传 `project` 直接定位 120-140ms，**差异在噪声范围内**（进程启动 + tokio runtime ~100ms 固定开销淹没扫描成本）。删 `project` 无性能代价。`get_session` / `get_turn` / `get_tool_output` 统一只传 `session`（+ 各自特有参数），服务端自动解析 project。

### D14b：step 类型扩展 + Compact/System 保留

**原 D14**：step 类型 10 种（thinking / text / tool / subagent / teammate_spawn / workflow / interruption / user_message / slash / teammate_message）。

**修订**：加 `compaction` / `system` 两种，共 12 种。理由：作为解析工具不应丢信息。Compact chunk（上下文压缩摘要，4% session 含有）和 System chunk（系统提示，88% session 含有）在 `derive_turns` 中已被折叠进所属 turn 的 `member_chunk_ids`，作为 step 类型嵌入 turn 时序正确、不需新结构。

- `compaction` step 含 `summary`（压缩摘要文本）
- `system` step 含 `content`（系统提示内容）

### D15b：subagent 递归钻取 defer

**原 D15**：subagent step 暴露 `subagentSessionId`，AI 用 `get_session` / `get_turn` 递归钻取。

**修订**：本次 defer 实现。subagent step 照常返回 `{type: "subagent", name, description, subagentSessionId, stepsCount}`，但 `subagentSessionId` 暂不能传给 `get_session` / `get_turn`。理由：
1. subagent 路径发现（`find_subagent_jsonl` + 跨 project 已知坑）是独立基础设施。
2. API 契约里 `subagentSessionId` 字段已预留，后续补实现不 breaking。
3. 核心 turn/step 模型先落地验证。

### D17：metrics schema 定义

`metrics` 适用于 get_session compact 的 per-turn 和 get_turn 顶层。字段：

| 字段 | 类型 | 来源 |
|---|---|---|
| `inputTokens` | `u64` | AIChunk `usage.input_tokens` |
| `outputTokens` | `u64` | AIChunk `usage.output_tokens` |
| `cacheReadTokens` | `u64` | AIChunk `usage.cache_read_input_tokens` |
| `cacheCreationTokens` | `u64` | AIChunk `usage.cache_creation_input_tokens` |
| `cost` | `f64` | `cdt-query::cost::compute_turn_cost` |
| `durationMs` | `i64` | turn 首 chunk timestamp → 末 chunk endTs 差值 |
| `model` | `String` | AIChunk `model` |

全部可从已有 chunk 数据无额外 I/O 算出。

## 更新后的数据模型

- **get_session**：顶层 `sessionId / model / totalCost / durationMs / filesTouched / userIntents / total / nextCursor`；`turns: [{index, question, answer, tools:[{name,count,errorCount}], stepsCount, metrics, matchedIn?}]`。`question` nullable（Headless turn 为 null）。
- **get_turn**：顶层 `sessionId / turnIndex / question / answer / stepsTotal / nextCursor / metrics`；`steps: [{index, type, ...}]`，tool step 含 `toolUseId / name / input / output / outputTruncated / outputBytes`。
- **get_tool_output**：`{sessionId, toolUseId, toolName, outputBytes, output}`（全文不截断）。
- **search**：`{total, nextCursor, results:[{sessionId, turnIndex, question, matchSnippet, timestamp, projectName}]}`。
- **分页**：`list_sessions` / `get_session` / `get_turn` / `search` 统一支持 `pageSize` + `cursor` 参数。

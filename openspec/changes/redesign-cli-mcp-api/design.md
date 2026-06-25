## Context

cdt 的 CLI/MCP API 最初是为 UI 消费设计的：骨架 + 懒加载 + 按 `FileSignature` 缓存。AI agent 的需求不同——「一次拿到完整相关数据」。现有 `get_session_chunks` 用 `content_mode`（omit/overview/full）三档开关，默认 `omit` 只返回结构 + size 元数据，逼 AI 先探测、再按 range 取、可能猜错重来；多次调用的 system prompt 开销远超直接返回完整数据。

桌面端早已把 chunk 转成可视的「turn」结构：前端 `buildDisplayItems(chunk: AIChunk)` 用 `semantic_steps` 顺序做骨架、按 `tool_use_id` 从 `tool_executions` 填明细、识别 lastOutput（= answer）。但 CLI/MCP 的 view 层（`ChunkView`）没暴露这层，直接吐裸 chunk。这是「看不到工具调用链」的根因。

session 解析与查询走共享层 `cdt-query::Engine`（仅 `cdt-cli` 依赖），桌面端渲染会话走 Tauri IPC → `cdt-api::LocalDataApi` 直连，**不经过 `cdt-query`**。这条边界让本 change 的扩展能锁在 CLI/MCP 专属层而不碰桌面端性能。

**前置约束**：本设计的 turn 模型依赖 #540 修复——见 D0。

## Goals / Non-Goals

**Goals:**
- AI 一次调用拿到「完整相关数据」，把会话 514 那类 27 次调用降到 2-3 次。
- 工具 6 → 7、参数 31 → 21；CLI 与 MCP 数据参数完全一致。
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

- get_session 的 `grep` 匹配整个 turn 的 steps 内容（thinking / tool input / output / 文本），命中返回整个 turn + `matchedIn`（如 `"tool:Read"`）标注命中位置。
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

## 数据模型（参考，详细字段契约见 specs delta）

- **get_session**：顶层 `sessionId / model / totalCost / durationMs / filesTouched / userIntents / total / nextCursor`；`turns: [{index, question, answer, tools:[{name,count,errorCount}], stepsCount, metrics, matchedIn?}]`。
- **get_turn**：顶层 `sessionId / turnIndex / question / answer / stepsTotal / nextCursor / metrics`；`steps: [{index, type, ...}]`，tool step 含 `name / input / output / outputTruncated / outputBytes`。
- **get_step_output**：`{sessionId, turnIndex, stepIndex, toolName, outputBytes, output}`（全文不截断）。
- **search**：`{total, nextCursor, results:[{sessionId, turnIndex, question, matchSnippet, timestamp, projectName}]}`。

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
3. `cdt-query::Engine` 加 turn 构建 + subagent lazy 反查索引；`cdt-cli` 加 view structs + 7 工具 + 子命令。
4. 删 `get_session_chunks`，迁移其 grep / context budget 行为到 get_turn。
5. 更新 setup / skill / 文档里对旧工具的引用。
- **回滚**：本 change 在 `cdt-cli` 层，回滚不影响桌面端与 `cdt-api`。

## Open Questions

- `session-turn-view` 作为新 capability 是否合适，还是 turn 模型直接落 `mcp-server` 由 `cli-output` 引用？（倾向新 capability 单一 owner，reviewer 可挑战）
- `reference-redesign.html` 这类辅助文件放 change 目录是否符合 openspec 约定？不符则内容并入 design.md。
- get_turn 的 step 分页 cursor 与 subagent 递归钻取叠加时的寻址语义（subagent 内部也分页）——留待 apply 期细化。

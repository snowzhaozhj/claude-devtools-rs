## Context

cdt-devtools MCP server 当前暴露 8 个只读工具供 AI agent 查询 Claude Code 会话历史。在"回顾历史会话做了什么"场景下，agent 需要 4+ 次 MCP 调用 + 1 次 Bash workaround 才能定位到目标内容，根因是：(1) search 索引不覆盖 tool_use/tool_result 内容；(2) 无会话内搜索能力；(3) get_session_detail 无内容过滤；(4) summary 不包含工具活动信息。

Anthropic 认证材料建议 4-5 个工具是可靠选择的甜区，18+ 退化。本 MCP server 的 design.md D3 已明确选择"少而粗"策略。因此优先增加可组合参数而非新增工具。

## Goals / Non-Goals

**Goals:**

- Agent 回顾历史会话的 MCP 调用次数从 4+ 降到 1-2 次
- search_sessions 能命中 tool_use input 和 tool_result content 中的关键词
- agent 能在单次调用中按关键词过滤 session detail chunks
- summary 能告诉 agent "这个会话执行了哪些有副作用的操作"
- 工具数量保持 8 个不变

**Non-Goals:**

- 不做 regex 搜索（agent 不会可靠转义元字符）
- 不做 LLM 语义摘要（toolActivity 是确定性提取）
- 不新增 MCP 工具（如 `search_in_session`、`get_session_actions`）
- 不做倒排索引或 SQLite 持久化索引
- 不做 count-only / indices-only / match highlighting 等高级 grep 模式（v1 不需要）

## Decisions

### D1：grep 匹配策略——递归 JSON leaf visitor（统一 leaf-only）

对 `serde_json::Value` 的 grep 不使用 `serde_json::to_string` 全量序列化（会命中 JSON key、产生转义噪声），改用递归 visitor 只匹配 string leaf。所有大小的 Value 统一走 leaf visitor，不做 serialize fast path——保持 "JSON object key SHALL NOT 被匹配" 的承诺一致性。桌面端规模下（单 session 最大 ~10K 消息 / ~300 chunks），leaf 递归性能完全可接受。

**否决方案**：(A) 全量 `serde_json::to_string` + contains——误报高（JSON key/转义），大 output 时分配 5-25MB；(B) 按工具名白名单只索引 Bash/Edit/Write——couple 到 Claude Code 工具体系，新增工具必漏；(C) >100KB serialize fast path——与 leaf-only spec 承诺冲突（codex 二审 F4），为保一致性放弃。

### D2：grep × content_mode 交互——auto-promote matched chunks

grep 在完整数据上匹配（omit envelope 构建之前），命中的 chunks 自动 promote 到 full content mode，未命中的 context chunks 遵循用户设定的 content_mode。

**否决方案**：(A) 新增 excerpt content_mode——增加 API surface，需定义截断策略、边界表示、字段路径摘要，v1 不值得；(B) 保持 omit 让 agent 二次请求——多一轮 MCP 调用，违背"降低调用次数"的目标。

### D3：search 和 grep 的定位区分

`search_sessions(session=X)` = 索引查询，返回 lightweight hit list（session id、chunk index、snippet），用于发现；`get_session_detail(grep=Y)` = 浏览过滤，返回完整 chunk envelope + context window，用于验证。两者使用不同的匹配精度：search 索引有 8KB 截断，grep 扫完整 loaded detail。description 中明确说明此差异。

### D4：共用文本提取 helper

search_extract 索引、detail grep、summary toolActivity 三处共用同一个 `json_value_to_search_text(value, max_bytes)` / `json_value_contains(value, needle)` helper，统一大小限制、leaf-only 匹配规则、截断策略。避免三处各写一套字段规则导致行为不一致。

### D5：toolActivity 边界控制——bounded deterministic extraction

topCommands / topFiles 各取 top 20，gitOps 取 top 10，cliTools 去重不限数。命令截断到 200 chars 首行。附 `totalToolExecutions` + `omittedCount` 防 payload 膨胀。不做完整 shell parser，一次 Bash execution = 一条 command sample。

### D6：GrepMatcher 抽象层

定义 `GrepMatcher` enum（v1 只实现 `Literal` variant），隔离"匹配策略"与"遍历逻辑"。将来可扩展 regex / multi-keyword，遍历代码不改。

### D7：grep 与现有 filter/range/pagination 的组合顺序

处理管道顺序：`kind_filter → grep → context_expand → range/tail → pagination`。grep 在 kind_filter 之后（先缩小到 tool_calls / errors 再 grep）、range 之前（grep 可跨全 session 搜索，range 是最后的窗口裁剪）。grep 模式下 `totalChunks` 反映 grep + context 后的结果集大小，`cursor`/`hasMore` 基于该结果集分页。`grep` 与 `range`/`tail` 可同时使用（先 grep 再 range 裁剪），但 grep 的 context window 不会越过 range 边界。

### D8：MCP search_sessions 响应形状

当前 MCP `search_sessions` 把 `SearchSessionsResult` 包进 `PaginatedResponse`，丢失了 `sessionsSearched` / `isPartial` / `query` 字段。本 change 将 MCP search_sessions 改为直接返回完整 `SearchSessionsResult`（含 `sessionsSearched`、`isPartial`、`query`），在其 `results` 数组上做分页（`total` / `returned` / `hasMore` / `cursor`）。这保持了 pagination 语义的同时暴露搜索元数据。

## Risks / Trade-offs

- **[search vs grep 一致性]**：search 索引有 8KB 截断，grep 扫完整 output → 同一 query 可能 grep 命中但 search 找不到 → **缓解**：description 明确说明差异；两者定位不同（discovery vs verification）
- **[grep 命中 chunk 过大]**：auto-promote 时单个 matched chunk 含巨大 tool output → **缓解**：后续可加 per-output truncation，v1 不做
- **[搜索缓存膨胀]**：新增 tool content 索引后缓存条目增加 → **缓解**：单条 8KB 上限；427 消息会话增加 ~1-3MB 缓存，桌面端可接受
- **[大 Value leaf 递归性能]**：500KB 的 JSON output 递归遍历所有 leaf 可能有上万个 string 节点 → **缓解**：桌面端单 session 最大 ~300 chunks，大 Value 是少数；如果成为瓶颈后续可加 early-return 或 byte-budget 限制，v1 先保一致性

## Context

用户问"昨天做了什么"时，AI 需要理解每个会话的内容。当前 `SessionSummary` 只提供 `title`（首条用户消息），AI 被迫对每个会话做多次 `--content full --range M:N` 碎片化读取（实测 25 次 CLI 调用、$5、37 分钟）。

`session_metadata.rs` 的扫描循环已遍历每一行 JSONL，当前只提取 title / message_count / is_ongoing / git_branch。可在同一次遍历中零额外 I/O 成本提取更多字段。

`SessionSummary` 通过 serde 序列化后同时被 CLI `sessions list`、MCP `list_sessions`、SSE `SessionMetadataUpdate` 三路消费。新增 `#[serde(default, skip_serializing_if)]` 字段对三路自动透传，无需改消费端代码。

## Goals / Non-Goals

**Goals:**
- 在 `SessionMetadata` 扫描中提取会话活动摘要字段，让 AI 一次 `sessions list` 调用就能理解每个会话"做了什么"
- 保持 CLI / MCP / SKILL 三层字段一致
- 不增加 I/O 开销（复用已有扫描循环）

**Non-Goals:**
- 不做 LLM 摘要（成本高、依赖外部）
- 不做 topic extraction / keyword extraction（过度设计）
- 不提取 AI 回复文本（信噪比低，首行可能是 thinking / 工具说明）
- 不改 `sessions list` table 模式的默认列（避免 breaking change）

## Decisions

### D1：从 JSONL 扫描中提取 7 个新字段

在 `extract_session_metadata_with_ongoing` 的 `while let` 循环中，对每一行 `ParsedMessage` 顺带提取：

| 字段 | 类型 | 提取逻辑 | 上限 |
|---|---|---|---|
| `user_intents` | `Vec<String>` | 每条通过 `is_user_chunk_message` 过滤的 user 消息，取其文本首行（`\n` 前），截断 ≤100 chars | 30 条 |
| `last_active` | `i64` | 每条消息的 `timestamp`，取最大值（epoch ms） | — |
| `duration_ms` | `i64` | `last_active - first_timestamp` | — |
| `total_cost` | `Option<f64>` | 累加 assistant 消息的 `usage.input_tokens + output_tokens`，乘模型单价估算 | — |
| `tool_error_count` | `usize` | `ContentBlock::ToolResult { is_error: true }` 计数 | — |
| `files_touched` | `Vec<String>` | 从 `ContentBlock::ToolUse { name: "Edit"/"Write"/"MultiEdit" }` 的 `input.file_path` / `input.files[].file_path` 提取，去重 | 20 条 |
| `git_summary` | `Vec<String>` | 从 `ContentBlock::ToolUse { name: "Bash" }` 的 `input.command` 中匹配 `git commit -m "..."` 的 message 部分；从对应 `ToolResult` output 中正则提取 PR URL（`github.com/.+/pull/\d+`） | 10 条 |

**user_intents 噪声过滤**：跳过 ≤3 chars 的纯确认词（`ok`、`yes`、`嗯`、`好`、`继续`、`go`、`y`、`是`）。

**总结**：`user_intents` 回答"用户想做什么"，`files_touched` + `git_summary` 回答"实际做了什么"，`tool_error_count` 回答"成没成"。三者合起来就是一个会话的完整摘要。

**替代方案考虑**：
- ❌ 提取 AI 回复末尾文本：AI 回复结构不规则（text / tool_use 交替），"最后一段 text" 定义模糊，首行可能是 thinking 前缀，信噪比远低于 tool 执行中的结构化数据
- ❌ 只提取 user_intents 不提取 activity：codex 审查指出"用户说 `fix this bug` 但实际修了什么只有 tool 执行记录知道"

### D2：`user_intents` 提取复用 `is_user_chunk_message` 过滤

已有 `is_user_chunk_message` 精确过滤了 is_meta / teammate-message / system-output / tool-result-only 等非用户输入消息。`user_intents` 只从通过此过滤的消息中提取，与 `message_count` 计数语义一致。

提取位置：在 `is_user_chunk_message(&msg)` 判定为 true 的分支内，调 `extract_text(&msg.content)` 取首行。

### D3：`files_touched` / `git_summary` 从 assistant + user 消息分别提取

**ToolUse**（assistant 消息）和 **ToolResult**（user 消息）分布在不同消息中。codex 二审 Finding 2 指出只扫 assistant 会漏掉全部 tool_error_count 和 PR URL。

提取逻辑：
- **assistant 消息的 `ContentBlock::ToolUse`**：
  - `name == "Edit" || name == "Write"` → `input["file_path"]` 作为 `files_touched`
  - `name == "MultiEdit"` → `input["files"]` 数组中每个元素的 `["file_path"]`
  - `name == "Bash"` → `input["command"]` 匹配 `git commit -m` 提取 message；同时记录 `tool_use_id` 到 `pending_bash_ids: HashSet<String>` 备用
- **user 消息的 `ContentBlock::ToolResult`**：
  - `is_error == true` → `tool_error_count += 1`
  - `tool_use_id` 在 `pending_bash_ids` 中 → 对 result text 匹配 `https://github.com/[^\s]+/pull/\d+` 提取 PR URL
  - 处理后从 `pending_bash_ids` 删除该 id

**关键**：单 pass 中维护 `pending_bash_ids: HashSet<String>` 做 ToolUse→ToolResult 关联，只对 Bash 命令的输出提取 PR URL，避免假阳性。

### D4：`MetadataCacheEntry` 同步扩展

`MetadataCacheEntry` 是纯内存 Rust struct（无 serde 持久化）。codex 二审 Finding 3 指出：加字段后所有构造点编译失败直到显式填值，不存在"旧二进制反序列化成默认值"的路径。

**解法**：删除 schema_version 设计，直接扩展 struct 和所有构造/映射点。编译器保证不遗漏。

### D5：`total_cost` 改为存 token 计数，费用在查询层算

codex 二审 Finding 1 指出：`cdt-api` 引入 `cdt-query::cost` 会形成循环依赖（`cdt-query -> cdt-api -> cdt-query`），Cargo 直接拒编。

**修正方案**：metadata 扫描中只累加 token 原始计数和模型名，不在 `cdt-api` 层算费用：

- `SessionMetadata` 存 `total_input_tokens: u64`、`total_output_tokens: u64`、`model: Option<String>`
- `SessionSummary` 暴露 `totalCost: Option<f64>` 给消费端
- cost 估算在 CLI `cmd_sessions_list` / MCP `list_sessions` 层完成（它们已在 `cdt-cli` crate，可访问 `cdt-query::cost`）
- `cdt-cli` 在构造最终 JSON 时调 `cdt_query::cost::estimate_cost(input_tokens, output_tokens, model)` 计算

这样 `cdt-api` 不引入 `cdt-query` 依赖，费用估算只在消费端做一次。

### D6：CLI `--json` 字段列表更新

`list_available_fields()` 中 sessions list 的字段数组新增：`"projectId"`, `"projectName"`, `"userIntents"`, `"lastActive"`, `"durationMs"`, `"totalCost"`, `"toolErrorCount"`, `"filesTouched"`, `"gitSummary"`。

这些字段已通过 `SessionSummary` 的 serde 序列化自动存在于 JSON 输出中，`list_available_fields` 只是帮助文本。

### D7：SKILL 更新

`session-insights` SKILL.md 新增日报场景：

```
## Daily summary scenario
cdt sessions list --since 2026-06-08 --until 2026-06-09 --group-by project \
  --json=projectName,sessionId,title,userIntents,durationMs,totalCost,filesTouched,gitSummary
```

同时强调用 `--since YYYY-MM-DD --until YYYY-MM-DD` 精确日历日，避免 `--since 1d` 的滚动 24h 窗口语义。

### D8：`SessionMetadataUpdate` event 同步扩展

SSE/broadcast event `SessionMetadataUpdate` 新增对应字段。前端 Tauri IPC 消费端（`ui/src/lib/stores/`）如不需要这些字段可忽略（`#[serde(default)]` 保证向后兼容）。

## Risks / Trade-offs

- **[payload 膨胀]** `user_intents` 30 条 × 100 chars + `files_touched` 20 条 + `git_summary` 10 条 ≈ 每会话 ~5KB。100 个会话的 list ≈ 500KB。→ **缓解**：Vec 都有硬上限；MCP 的 `limit` 默认 20 已限制页大小；CLI `--json=field1,field2` 可投影裁剪
- **[cache 膨胀]** `MetadataCacheEntry` 从 ~200B 涨到 ~5KB per entry。2000 条 cache ≈ 10MB。→ **可接受**：桌面应用内存预算远大于此
- **[git_summary 正则噪声]** `git commit -m` 匹配可能误中用户讨论中的示例代码 → **缓解**：只从 `ContentBlock::ToolUse { name: "Bash" }` 的 `input.command` 中提取，不扫 AI text
- **[total_cost 精度]** metadata 扫描时 `model` 字段可能有未知值（新模型）→ **缓解**：未知模型的 token 按 0 计价，`total_cost` 标注为估算值

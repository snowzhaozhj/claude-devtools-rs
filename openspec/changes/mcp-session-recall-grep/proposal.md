## Why

MCP server 的 `search_sessions` 工具不索引 `tool_use` input 和 `tool_result` content（只索引 user/assistant 文本），导致 agent 搜索"mw switch"等工具执行内容时返回 0 结果。`get_session_detail` 缺乏内容过滤能力，agent 必须翻页猜 chunk range 才能定位目标内容。`get_session_summary` 只返回统计数据，无法告诉 agent "这个会话执行了哪些有副作用的操作"。这三个缺陷叠加导致 agent 完成一次"回顾历史会话做了什么"需要 4+ 次 MCP 调用 + Bash workaround，效率极低。

## What Changes

- **修复 search 索引**：`extract_searchable_entries` 将 `ContentBlock::ToolUse` 的 input 和 `ContentBlock::ToolResult` 的 content 纳入搜索索引，每个 tool block 上限 8KB 截断，使用递归 JSON leaf 提取（不索引 object key）
- **search_sessions 加 session 参数**：支持会话内搜索（intra-session search），复用已有的 `search_session_file` 能力
- **get_session_detail 加 grep 参数**：case-insensitive literal substring 匹配，借鉴 ripgrep 思路——GrepMatcher 抽象层、递归 JSON leaf visitor（大 Value >100KB 走 serialize fast path）、chunk-level context window（±N chunks）、grep 命中的 chunks 自动 promote 到 full content mode
- **get_session_summary 加 toolActivity 段**：从 tool_executions 确定性提取 bounded 结构化数据（topCommands / topFiles / gitOps / cliTools），不涉及 LLM 摘要
- **不新增 MCP 工具**——保持 8 个工具不变，只增加 3 个可选参数 + 1 个返回字段

## Capabilities

### New Capabilities

（无新增 capability）

### Modified Capabilities

- `session-search`：搜索索引范围从"user/assistant 文本"扩展到"含 tool_use input + tool_result content"，新增 session 参数支持会话内搜索
- `mcp-server`：`get_session_detail` 增加 grep / grep_context 参数 + auto-promote 行为；`get_session_summary` 返回新增 toolActivity 段；`search_sessions` 增加 session 参数

## Impact

- **cdt-discover**：`search_extract.rs` 索引逻辑变更 + 新增共用 helper（`json_value_to_search_text` / `json_value_contains`）
- **cdt-query**：`summary.rs` 新增 `ToolActivity` struct 和提取逻辑
- **cdt-cli**：`mcp/mod.rs` 参数定义 + grep 过滤 + auto-promote 逻辑
- **搜索缓存**：`SearchableEntry` 条目数增加（新增 tool_use/tool_result 类型），单条上限 8KB
- **IPC 兼容性**：所有新增字段使用 `#[serde(default)]`，不破坏现有前端

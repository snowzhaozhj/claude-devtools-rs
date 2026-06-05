## 1. 共用 Helper

- [x] 1.1 在 cdt-discover 新增 `search_text.rs` 模块，实现 `json_value_to_search_text(value, max_bytes) -> String`（递归提取 JSON string leaf，bounded）和 `json_value_contains(value, needle_lower) -> bool`（统一 leaf-only visitor，不做 serialize fast path）
- [x] 1.2 新增 `GrepMatcher` enum（Literal variant），实现 `matches(&self, haystack: &str) -> bool`（case-insensitive literal substring）
- [x] 1.3 为 helper 函数编写单元测试：基本匹配、JSON key 不匹配、8KB 截断、case-insensitive、嵌套 JSON 结构

## 2. Search 索引补全 tool content

- [x] 2.1 修改 `search_extract.rs::extract_searchable_entries`：在所有 `MessageContent::Blocks` 分支中统一遍历 `ContentBlock`，assistant 消息提取 `ToolUse` input，user 消息提取 `ToolResult` content（tool_result block 在 JSONL 中位于 user 消息），使用 `json_value_to_search_text` helper，message_type 标记为 "tool_use" / "tool_result"
- [x] 2.2 为 search_extract 新增测试：tool_use command 可搜索、tool_result content 可搜索、JSON key 不命中、大 tool_result 截断

## 3. search_sessions 加 session 参数

- [ ] 3.1 `QueryEngine::search` 新增 session_id 参数，有值时直接调 `search_session_file` 而非 `search_sessions`
- [ ] 3.2 MCP 层 `SearchParams` 新增 `session: Option<String>` 字段，传入 QueryEngine
- [ ] 3.3 MCP `search_sessions` 改为返回完整 `SearchSessionsResult`（含 `sessionsSearched` / `isPartial` / `query`），在 `results` 上做分页
- [ ] 3.4 更新 `search_sessions` tool description 说明 session 参数用途 + 搜索范围含 tool content
- [ ] 3.5 测试：带 session 参数的 intra-session search 返回正确结果；不带 session 参数行为不变

## 4. get_session_detail 加 grep

- [ ] 4.1 MCP 层 `SessionDetailParams` 新增 `grep: Option<String>` 和 `grep_context: Option<usize>` 字段
- [ ] 4.2 实现 `chunk_matches_grep(chunk, matcher) -> bool`，遍历 Ai/User/System/Compact chunk 内容，tool_execution 走 `json_value_contains` helper
- [ ] 4.3 在 `get_session_detail` handler 中，kind_filter 后加 grep filter + context window 扩展
- [ ] 4.4 实现 auto-promote：grep 命中的 chunks 用 full content mode build envelope，context chunks 遵循用户设定；envelope 加 `grepHit: bool` 字段
- [ ] 4.5 更新 `get_session_detail` tool description 说明 grep 参数
- [ ] 4.6 测试：grep 过滤返回正确 chunks、auto-promote 行为、context window、无匹配返空

## 5. CLI 参数同步

- [ ] 5.1 CLI `cdt search` 子命令新增 `--session` 参数，传入 QueryEngine::search 的 session 维度
- [ ] 5.2 CLI `cdt sessions detail` 子命令新增 `--grep` 和 `--grep-context` 参数
- [ ] 5.3 CLI handler 实现：search 带 session 时调用 intra-session search；detail 带 grep 时传入 QueryEngine

## 6. MCP server instructions 和 tool descriptions 更新

- [ ] 6.1 更新 `get_info()` 的 `instructions` USAGE PATTERN，加入 search(session param) / grep / toolActivity 的使用指引
- [ ] 6.2 更新 `search_sessions` tool description：说明搜索范围含 tool input/output + session 参数用途
- [ ] 6.3 更新 `get_session_detail` tool description：说明 grep + grep_context 参数 + auto-promote 行为
- [ ] 6.4 更新 `get_session_summary` tool description：说明新增 toolActivity 段

## 7. SKILL 更新

- [ ] 7.1 更新 `crates/cdt-cli/assets/skills/session-insights/SKILL.md` 的 Search workflow：加 `--session` 参数用法示例
- [ ] 7.2 更新 Diagnosis workflow：加 `--grep` 参数用法，说明 summary 现在包含 toolActivity
- [ ] 7.3 在 Workflow Selection 表格中新增"回顾历史操作"场景的推荐路径

## 8. summary 加 toolActivity

- [ ] 8.1 在 `cdt-query/summary.rs` 新增 `ToolActivity` struct（topCommands / topFiles / gitOps / cliTools / totalToolExecutions / omittedCount）
- [ ] 8.2 实现 `compute_tool_activity(chunks) -> ToolActivity`：遍历 Ai chunks 的 tool_executions，提取 Bash command 首行、Edit/Write/Read file_path、git 命令、CLI 工具名
- [ ] 8.3 在 `build_summary` 中调用并加入返回结构
- [ ] 8.4 测试：包含 Bash/Edit/git 操作的 session 返回正确 toolActivity

## 9. 发布

- [ ] 9.1 push 分支 + 开 PR
- [ ] 9.2 wait-ci 全绿
- [ ] 9.3 codex 二审通过
- [ ] 9.4 archive change

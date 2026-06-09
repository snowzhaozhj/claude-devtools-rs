## 1. 数据层：SessionMetadata 扩展

- [x] 1.1 `SessionMetadata` struct 新增字段：`user_intents`、`last_active`、`duration_ms`、`total_input_tokens`、`total_output_tokens`、`tool_error_count`、`files_touched`、`git_summary`
- [x] 1.2 `extract_session_metadata_with_ongoing` 扫描循环中新增提取逻辑：user_intents（首行 + 噪声过滤 + 上限 30）、last_active / duration_ms（时间戳追踪）、assistant ToolUse 提取 files_touched / git commit message + pending_bash_ids、user ToolResult 提取 tool_error_count + PR URL（关联 pending_bash_ids）
- [x] 1.3 token 计数：累加 assistant 消息 usage 的 input_tokens / output_tokens（不在 cdt-api 层算费用，避免循环依赖）
- [x] 1.4 `extract_session_metadata_from_parsed` 同步变体同步新增相同提取逻辑
- [x] 1.5 `MetadataCacheEntry` 新增对应字段（纯内存 struct，编译器保证构造点不遗漏，无需 schema_version）

## 2. 类型层：SessionSummary + SessionMetadataUpdate

- [x] 2.1 `SessionSummary` struct（types.rs）新增 7 个可选字段，`#[serde(default, skip_serializing_if)]`
- [x] 2.2 `SessionMetadataUpdate` event（events.rs）新增对应字段
- [x] 2.3 `local.rs` 中 metadata → SessionSummary 映射补充新字段赋值
- [x] 2.4 `local.rs` 中 `scan_metadata_for_page` 的 `SessionMetadataUpdate` 构造补充新字段

## 3. CLI 层

- [x] 3.1 `list_available_fields()` 中 sessions list 字段数组新增 `projectId`、`projectName`、`userIntents`、`lastActive`、`durationMs`、`totalCost`、`toolErrorCount`、`filesTouched`、`gitSummary`
- [x] 3.2 ~~CLI / MCP 消费层~~（design D5b 修订：改为 metadata 扫描中用内联定价函数直接算 `totalCost`，避免循环依赖）
- [x] 3.3 SKILL `session-insights` 新增日报场景路径文档

## 4. 测试

- [ ] 4.1 `session_metadata.rs` 单测：构造含多条 user/assistant/tool_use/tool_result 的 JSONL fixture，验证 user_intents 提取 + 噪声过滤 + 上限截断
- [ ] 4.2 `session_metadata.rs` 单测：验证 files_touched 去重 + git_summary commit message / PR URL 提取
- [ ] 4.3 `session_metadata.rs` 单测：验证 token 计数累加、tool_error_count 计数、last_active / duration_ms 计算
- [ ] 4.4 `session_metadata.rs` 单测：验证 pending_bash_ids 关联——PR URL 只从 Bash 命令的 ToolResult 提取
- [ ] 4.5 IPC contract test：验证 `SessionSummary` 新增字段的 JSON 序列化 camelCase 键名

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex + pr-review-toolkit 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

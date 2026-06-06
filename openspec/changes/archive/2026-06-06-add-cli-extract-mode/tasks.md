## 1. cdt-query extract 模块

- [x] 1.1 新增 `crates/cdt-query/src/extract.rs`：定义 `ChunkOverviewEntry`、`ToolExecEntry` 两个 struct（`#[serde(rename_all = "camelCase")]`）
- [x] 1.2 实现 `extract_error_summary(te: &ToolExecution) -> Option<String>`：按优先级 errorMessage → Structured output 的 stderr/error/exit_code → Text output 的 exit code regex → output 末尾 200 字符 → None
- [x] 1.3 实现 `extract_overview(indexed: &[(usize, &Chunk)]) -> Vec<ChunkOverviewEntry>`：每 chunk 一条，含 tool_count / error_count / tool_names（去重排序）
- [x] 1.4 实现 `extract_tool_executions(indexed: &[(usize, &Chunk)]) -> Vec<ToolExecEntry>`：接收携带绝对索引的 chunk slice，跨 chunk 展平所有 tool executions
- [x] 1.5 实现 `extract_errors(indexed: &[(usize, &Chunk)]) -> Vec<ToolExecEntry>`：仅 is_error=true 的子集
- [x] 1.6 将 `view.rs::summarize_input()` 下沉到 `cdt-query::extract::summarize_input()`，view.rs 改为调用 cdt-query 的版本
- [x] 1.7 在 `crates/cdt-query/src/lib.rs` 导出 extract 模块
- [x] 1.8 为 extract 模块写单元测试（覆盖：空 chunks、Bash Structured output 的 error summary、Bash Text output 的 error summary、errorMessage 优先于 output、overview 的 tool_names 去重排序、跨 chunk 展平绝对索引正确性、summarize_input 与 view.rs 一致性）

## 2. ErrorEntry 废弃迁移

- [x] 2.1 `engine.rs` 中 `get_session_errors()` 标 `#[deprecated]`，内部实现改为调 `extract_errors()` 后映射回 `ErrorEntry`
- [x] 2.2 `cmd_sessions_errors()` 中的 error message 显示改用 `extract_error_summary()` 替代直接取 `error_message` 字段

## 3. CLI --extract 参数

- [x] 3.1 `SessionsAction::Detail` 新增 `--extract` 参数（clap arg，`conflicts_with = "content"` 直接报错）
- [x] 3.2 `cmd_sessions_detail()` 新增 extract 分支：在现有管道末端，当 `--extract` 指定时调用 `extract::*` 函数
- [x] 3.3 实现 extract text 格式化输出（overview / errors / tools 三种 text 格式）
- [x] 3.4 实现 extract JSON 格式化输出（`--format json` 时输出扁平 JSON array）
- [x] 3.5 `list_available_fields()` 对 `--extract` 模式返回扁平字段名
- [x] 3.6 非法 `--extract` 值报错提示

## 4. session-insights skill 更新

- [x] 4.1 更新 `crates/cdt-cli/assets/skills/session-insights/SKILL.md`：在 Scenario quick reference 中加入 `--extract` 用法

## 5. 验证（含 codex 审查修正 Finding 4 的 MCP 契约测试）

- [x] 5.1 `cargo clippy --workspace --all-targets -- -D warnings` 全通过
- [x] 5.2 `cargo test -p cdt-query` 通过（含新增 extract 测试）
- [x] 5.3 `cargo test -p cdt-cli` 通过
- [x] 5.4 手动验证：用真实会话数据跑 `--extract overview`、`--extract errors`、`--extract tools` 三种模式 + `--format json` 组合
- [x] 5.5 验证 `sessions errors` 不再显示 `(no message)`（用含 Bash 错误的真实会话）

## N. 发布
- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex + pr-review-toolkit 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

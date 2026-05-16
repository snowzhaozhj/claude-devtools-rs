# Tasks

## 1. cdt-analyze: 加 IsOngoingStateMachine

- [x] 1.1 把现有 `build_activity_stack` + `is_ongoing_from_activities` + `Activity` enum + `INTERRUPT_PREFIX` + `is_shutdown_response` + `process_assistant` + `process_user` 整体下沉到 `#[cfg(test)] mod oracle` 子模块（保留所有现有逻辑做算法快照 / round-trip oracle）
- [x] 1.2 在 `crates/cdt-analyze/src/session_state.rs` 新增 `pub struct IsOngoingStateMachine { ongoing: bool, shutdown_tool_ids: HashSet<String> }` + `pub fn new()` / `pub fn feed(&mut self, msg: &ParsedMessage)` / `pub fn finalize(self) -> bool`
- [x] 1.3 SM 内复用现有判定语义：assistant blocks（Thinking / Text / ToolUse 含 ExitPlanMode / SendMessage shutdown_response）+ user blocks（Interruption category / ToolResult 含 shutdown match / rejection / `[Request interrupted by user` 文本前缀）
- [x] 1.4 重写 `pub fn check_messages_ongoing(&[ParsedMessage]) -> bool` 为 `let mut sm = IsOngoingStateMachine::new(); for msg in messages { sm.feed(msg); } sm.finalize()`，签名不变
- [x] 1.5 把 `IsOngoingStateMachine` 添加到 `crates/cdt-analyze/src/lib.rs` 的 `pub use` 导出
- [x] 1.6 `cargo clippy -p cdt-analyze --all-targets -- -D warnings` + `cargo fmt --all`
- [x] 1.7 `cargo test -p cdt-analyze`：既有 9 个 `check_messages_ongoing` 单测自动通过 SM 路径

## 2. round-trip property test

- [x] 2.1 在 `cdt-analyze/src/session_state.rs#tests` 新增 round-trip 测试：定义 6 类 fixture（normal completed / ongoing tool-use / interrupted / teammate-message / shutdown_response / resumed-after-interrupt）
- [x] 2.2 每个 fixture 对应一个 `Vec<ParsedMessage>`，分别用 SM 流式 + oracle 切片处理，断言 `assert_eq!(sm_result, oracle_result)`
- [x] 2.3 加边界 fixture：空 vec / 单 user / 单 assistant text / 单 assistant tool_use / 全 sidechain（应等同空，因 oracle 路径过滤）

## 3. cdt-api: extract_session_metadata 改流式

- [x] 3.1 改 `crates/cdt-api/src/ipc/session_metadata.rs::extract_session_metadata_with_ongoing`：构造 `let mut sm = IsOngoingStateMachine::new();`
- [x] 3.2 parse loop 内对每条解析出的 `msg` 调 `sm.feed(&msg)`（在原 `all_messages.push(msg)` 位置之前）
- [x] 3.3 删除 `let mut all_messages: Vec<cdt_core::ParsedMessage> = Vec::new();` 与 `all_messages.push(msg);`
- [x] 3.4 把 loop 后的 `let messages_ongoing = cdt_analyze::check_messages_ongoing(&all_messages);` 改为 `let messages_ongoing = sm.finalize();`
- [x] 3.5 `cargo clippy --workspace --all-targets -- -D warnings` + `cargo fmt --all`
- [x] 3.6 `cargo test -p cdt-api`：既有 metadata 单测全过

## 4. perf bench 验证无回归

- [x] 4.1 跑 `cargo test -p cdt-api --release --test perf_cold_scan -- --ignored --nocapture` 对比基线 89ms（实测 85ms，提升 ~4%，无回归）
- [x] 4.2 跑 `cargo test --release -p cdt-api --test perf_get_session_detail -- --ignored --nocapture` 看各阶段不退步（172/250/1221 msgs 三档分别 26/15/60ms，远低于 800ms budget）
- [x] 4.3 把 bench 输出贴到 PR 描述的 "Perf impact" 段

## 5. spec validate

- [x] 5.1 `openspec validate metadata-streaming-ongoing --strict` 通过
- [x] 5.2 spec delta 含 `ADDED Requirement` 块且首段含 SHALL/MUST

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

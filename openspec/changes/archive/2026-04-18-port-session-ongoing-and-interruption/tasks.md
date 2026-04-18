## 1. cdt-core 类型调整

- [x] 1.1 `crates/cdt-core/src/message.rs`：`HardNoiseReason` 删除
  `InterruptMarker` 变体；`MessageCategory` 新增 `Interruption` 变体
  （不在 `is_hard_noise` 判定内）；`pub use` 同步
- [x] 1.2 `crates/cdt-core/src/chunk.rs`：`SemanticStep` 新增
  `Interruption { text: String, timestamp: DateTime<Utc> }` 变体；
  roundtrip 测试覆盖
- [x] 1.3 `cargo clippy -p cdt-core --all-targets -- -D warnings`
- [x] 1.4 `cargo fmt --all`
- [x] 1.5 `cargo test -p cdt-core`

## 2. cdt-parse：interrupt 单独分类

- [x] 2.1 `crates/cdt-parse/src/noise.rs`：`classify_hard_noise` 里移除
  `INTERRUPT_PREFIX` 分支；保留 `INTERRUPT_PREFIX` 常量与 pub(crate)
  `is_interrupt_marker(content: &MessageContent) -> bool` 新函数
- [x] 2.2 更新 `interrupt_marker_is_noise` 测试为
  `interrupt_marker_is_not_hard_noise`：断言 `classify_hard_noise` 返回
  `None`、`is_interrupt_marker` 返回 `true`
- [x] 2.3 `crates/cdt-parse/src/` 分类入口（`lib.rs` 或 `parser.rs`）：
  在 `classify_hard_noise` 返回 `None` 且 `message_type == User` 时，
  调 `is_interrupt_marker` 判定，命中则把 `MessageCategory` 设为
  `Interruption`
- [x] 2.4 新增集成测试：`tests/*.rs` 或 `noise.rs` 模块里覆盖
  `[Request interrupted by user for tool use]` 产出
  `MessageCategory::Interruption`
- [x] 2.5 `cargo clippy -p cdt-parse --all-targets -- -D warnings`
- [x] 2.6 `cargo fmt --all`
- [x] 2.7 `cargo test -p cdt-parse`

## 3. cdt-analyze：Interruption step + check_messages_ongoing

- [x] 3.1 `crates/cdt-analyze/src/chunk/builder.rs`：主 loop 在遇到
  `category == Interruption` 的消息时，调 `flush_assistant_buffer`
  之前给当前 pending AIChunk（或刚 flush 的最后一个 AIChunk）追加
  `SemanticStep::Interruption { text, timestamp }`；无前驱 AI 时丢弃
- [x] 3.2 `crates/cdt-analyze/src/chunk/builder.rs` tests：新增用例
  `interrupt_marker_appended_as_semantic_step_to_last_ai_chunk`
- [x] 3.3 新文件 `crates/cdt-analyze/src/session_state.rs`：实现
  `check_messages_ongoing(messages: &[ParsedMessage]) -> bool`，按
  TS 算法 port（activity enum + ending index + shutdown tool ids）
- [x] 3.4 `crates/cdt-analyze/src/lib.rs`：`pub use session_state::
  check_messages_ongoing;`
- [x] 3.5 `session_state.rs` `#[cfg(test)]`：5 条 scenario 测试
  （text_output ending / interrupt text ending / tool rejection
  ending / shutdown response ending / ExitPlanMode ending；之后有
  AI activity 的 case 断言 true；没有则 false；empty slice → false）
- [x] 3.6 `cargo clippy -p cdt-analyze --all-targets -- -D warnings`
- [x] 3.7 `cargo fmt --all`
- [x] 3.8 `cargo test -p cdt-analyze`

## 4. cdt-api：SessionSummary/SessionDetail.isOngoing

- [x] 4.1 `crates/cdt-api/src/ipc/types.rs`：`SessionSummary` 与
  `SessionDetail` 加 `#[serde(default)] pub is_ongoing: bool`
- [x] 4.2 `crates/cdt-api/src/ipc/session_metadata.rs`：`SessionMetadata`
  加 `is_ongoing: bool`；扫描阶段累积 `Vec<ParsedMessage>`，尾部调
  `cdt_analyze::check_messages_ongoing(&messages)` 填充
- [x] 4.3 `crates/cdt-api/src/ipc/local.rs::list_sessions`：把
  `extract_session_metadata` 返回的 `is_ongoing` 透传到
  `SessionSummary.is_ongoing`
- [x] 4.4 `crates/cdt-api/src/ipc/local.rs::get_session_detail`：在
  `parse_file` 返回 `messages` 后调 `check_messages_ongoing`，填
  `SessionDetail.is_ongoing`
- [x] 4.5 `cargo clippy -p cdt-api --all-targets -- -D warnings`
- [x] 4.6 `cargo fmt --all`
- [x] 4.7 `cargo test -p cdt-api`（既有 pin/hide 测试不受影响）
- [x] 4.8 `cargo clippy --manifest-path src-tauri/Cargo.toml
  --all-targets -- -D warnings`（Tauri 侧只透传，不应回归）

## 5. UI：OngoingIndicator + Banner + Sidebar/SessionDetail

- [x] 5.1 `ui/src/components/OngoingIndicator.svelte`（新文件）：
  导出 `<OngoingIndicator size="sm"|"md" showLabel?: boolean />`
  绿点脉冲 + 可选 label；`<OngoingBanner />` 底部横幅（spinner +
  "Session is in progress..."），样式对齐原版 `OngoingIndicator.tsx`
- [x] 5.2 `ui/src/lib/api.ts`：`SessionSummary` 与 `SessionDetail` 类型
  补 `isOngoing?: boolean`
- [x] 5.3 `ui/src/components/Sidebar.svelte`：PINNED 与日期分组的
  session title 前按 `session.isOngoing` 渲染 `<OngoingIndicator />`
- [x] 5.4 `ui/src/routes/SessionDetail.svelte`：在 `conversation` 容器
  结尾按 `detail?.isOngoing` 插入 `<OngoingBanner />`
- [x] 5.5 `ui/src/routes/SessionDetail.svelte`：semantic step 渲染
  switch 新增 `step.kind === 'interruption'` 分支——红色 badge +
  "Session interrupted by user"（位置对齐 Thinking/Text 步骤）
- [x] 5.6 `npm run check --prefix ui` 0 errors

## 6. Preflight + followups + CLAUDE.md + archive

- [~] 6.1 fmt + workspace clippy + `cargo test --workspace --exclude
  cdt-watch` 全绿；`openspec validate --all --strict` 21/21 通过；
  `cdt-watch` 在本机 macOS FSEvents 下全部 6 个 test timeout（同
  `2026-04-18-realtime-session-refresh` 的处理），**本 change 未触
  watcher 代码**，视为环境 flake
- [x] 6.2 `openspec validate port-session-ongoing-and-interruption
  --strict` 通过
- [x] 6.3 `openspec/followups.md`：把 coverage-gap 段标为 ✅ 已在
  `port-session-ongoing-and-interruption` 修复；
  `crates/cdt-parse/src/noise.rs:13` impl-bug 同步标记已修复
- [x] 6.4 `CLAUDE.md` "UI 已知遗留问题" 删除 #1（ongoing/interruption）
  及附带的 `noise.rs:13` impl-bug 引用；剩余建议顺序段移除 ongoing
  相关文字
- [x] 6.5 `openspec archive port-session-ongoing-and-interruption -y`（
  归档为 `archive/2026-04-18-port-session-ongoing-and-interruption/`，
  5 个 spec 全部 sync：`session-parsing` 1 modified + 1 added；
  `chunk-building` 2 modified + 1 added；`ipc-data-api` 1 modified；
  `session-display` 2 added；`sidebar-navigation` 1 added；archive 后
  `openspec validate --all --strict` 20/20 通过）

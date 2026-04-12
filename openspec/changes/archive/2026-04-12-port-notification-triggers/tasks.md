## 1. 依赖 + 脚手架

- [x] 1.1 在 workspace `Cargo.toml` 添加 `uuid` 依赖；在 `cdt-config/Cargo.toml` 添加 `uuid`、`lru`
- [x] 1.2 在 `cdt-config/src/` 创建空 module 文件：`detected_error.rs`、`trigger_matcher.rs`、`error_trigger_checker.rs`、`error_detector.rs`、`notification_manager.rs`；在 `lib.rs` 注册
- [x] 1.3 `cargo build -p cdt-config` 确认编译通过

## 2. DetectedError 类型 + 构建

- [x] 2.1 在 `detected_error.rs` 定义 `DetectedError` struct（id/timestamp/session_id/project_id/file_path/source/message/line_number/tool_use_id/trigger_color/trigger_id/trigger_name/context）
- [x] 2.2 实现 `extract_error_message`：从 `ContentBlock` 提取文本或 "Unknown error"
- [x] 2.3 实现 `truncate_message`（max 500 字符）和 `create_detected_error` 构建函数
- [x] 2.4 单元测试：error message 提取、截断、构建

## 3. TriggerMatcher

- [x] 3.1 在 `trigger_matcher.rs` 实现 regex cache（`LruCache<String, Option<Regex>>`，容量 500）
- [x] 3.2 实现 `matches_pattern`：从 cache 取 regex → test（大小写不敏感）
- [x] 3.3 实现 `matches_ignore_patterns`：任一 ignore pattern 匹配则返回 true
- [x] 3.4 实现 `extract_tool_use_field`：按 `match_field` 从 tool input 取字段值
- [x] 3.5 实现 `get_content_blocks`：从 `ParsedMessage.content` 提取 `ContentBlock` 列表
- [x] 3.6 单元测试：pattern 匹配、ignore 匹配、field 提取、cache eviction

## 4. ErrorTriggerChecker

- [x] 4.1 在 `error_trigger_checker.rs` 实现 `check_tool_result_trigger`：`error_status` mode 检查 `is_error` flag + ignore patterns；`content_match` mode 匹配字段
- [x] 4.2 实现 `check_tool_use_trigger`：遍历 tool_use blocks，按 tool_name 过滤，按 match_field 提取，按 match_pattern 匹配
- [x] 4.3 实现 `check_token_threshold_trigger`：估算 tool call + result tokens，按 token_type（input/output/total）比较阈值
- [x] 4.4 实现 `matches_repository_scope` stub：无 `repository_ids` → true，有则 → false + `tracing::debug!`
- [x] 4.5 单元测试：`is_error` 检测、content match、token threshold、ignore pattern 抑制、tool name 过滤

## 5. ErrorDetector

- [x] 5.1 在 `error_detector.rs` 实现 `detect_errors`：接收 `&[ParsedMessage]` + enabled triggers → 遍历 messages × triggers → 路由到 checker → 收集 `DetectedError`
- [x] 5.2 实现 `detect_errors_with_trigger`（单 trigger 版本，用于历史预览）
- [x] 5.3 单元测试：多 trigger 多 message 组合、空 trigger 列表、混合 mode

## 6. NotificationManager

- [x] 6.1 在 `notification_manager.rs` 定义 `StoredNotification`（`DetectedError` + `is_read` + `created_at`）和 `GetNotificationsResult`（notifications/total/unread_count/has_more）
- [x] 6.2 实现 `NotificationManager`：`new` / `load`（从磁盘）/ `save`（到磁盘）
- [x] 6.3 实现 `add_notification`：追加 + auto-prune（max 100）+ save
- [x] 6.4 实现 `get_notifications`（分页：limit/offset）和 `get_unread_count`
- [x] 6.5 实现 `mark_as_read` / `mark_all_as_read` / `clear_all`
- [x] 6.6 单元测试：add + prune、分页、mark read、持久化 roundtrip

## 7. lib.rs 导出 + 集成

- [x] 7.1 在 `lib.rs` 通过 `pub use` 导出：`DetectedError`、`ErrorDetector`、`TriggerMatcher`、`NotificationManager`、`StoredNotification`
- [x] 7.2 `cargo clippy --workspace --all-targets -- -D warnings`
- [x] 7.3 `cargo fmt --all`
- [x] 7.4 `cargo test -p cdt-config` 全测试通过

## 8. 文档 + 收尾

- [x] 8.1 更新根 `CLAUDE.md` 的 Capability→crate map：`notification-triggers` → `done ✓`
- [x] 8.2 更新 `CLAUDE.md` 的 "Known TS impl-bugs" 段：标记 notification-triggers `is_error` 检测为 ✓
- [x] 8.3 更新 `openspec/followups.md`：标记相关条目
- [x] 8.4 `openspec validate port-notification-triggers --strict`

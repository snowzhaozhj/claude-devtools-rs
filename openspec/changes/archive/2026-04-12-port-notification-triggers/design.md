## Context

`cdt-config` 在 `port-configuration-management` 中已实现 trigger 数据类型（`NotificationTrigger`、`TriggerMode`、`TriggerContentType`）、CRUD（`TriggerManager`）和 regex 校验（`regex_safety.rs`）。本 port 在同一 crate 里增加 trigger **评估**层。

TS 侧分布在 6 个文件（~1300 行）：`ErrorDetector.ts`、`ErrorTriggerChecker.ts`、`TriggerMatcher.ts`、`ErrorMessageBuilder.ts`、`ErrorTriggerTester.ts`、`NotificationManager.ts`。

## Goals / Non-Goals

**Goals:**
- 实现 `DetectedError` 类型和 error message 构建
- 实现 `TriggerMatcher`：regex cache + pattern/ignore 匹配
- 实现 3 种 mode checker 纯函数（`error_status`/`content_match`/`token_threshold`）
- 实现 `ErrorDetector` orchestrator
- 实现 `NotificationManager`：通知 JSON 持久化、read/unread、分页、max 100 auto-prune
- 实现 trigger 历史预览（`testTrigger`）

**Non-Goals:**
- `repositoryIds` 范围过滤的完整实现（依赖 `cdt-discover::GitIdentityResolver`，本 port stub 为"无 repositoryIds 时全匹配"）
- Electron `Notification` API / 桌面推送（UI 层决策）
- IPC/HTTP handler 注册 → `port-ipc-data-api`
- `ToolResultExtractor` / `ToolSummaryFormatter` 的完整 port → 简化为内联 helper，因为 Rust 的 `ParsedMessage` 已经结构化

## Decisions

### D1: Module 结构

```
cdt-config/src/
├── detected_error.rs          # DetectedError 类型 + 构建 + 截断
├── trigger_matcher.rs         # regex cache + matchesPattern + matchesIgnorePatterns + extractToolUseField
├── error_trigger_checker.rs   # checkToolResultTrigger / checkToolUseTrigger / checkTokenThresholdTrigger
├── error_detector.rs          # ErrorDetector::detect_errors() orchestrator
├── notification_manager.rs    # NotificationManager: persist + read/unread + paging
└── (已有) trigger.rs          # TriggerManager CRUD（port-configuration-management）
```

### D2: Regex cache

TS 用 `Map` 做 LRU（500 条）。Rust 用 workspace 已有的 `lru` crate（`LruCache<String, Option<Regex>>`）。

### D3: `DetectedError` 的 id 生成

TS 用 `crypto.randomUUID()`。Rust 用 `uuid` crate 的 `Uuid::new_v4()`。需要加 workspace dep `uuid`。

### D4: `repositoryIds` 范围 stub

`matches_repository_scope` 在无 `repositoryIds` 时返回 `true`；有 `repositoryIds` 时暂返回 `false`（因为缺少 `GitIdentityResolver` 接入），记 `tracing::debug!`。后续 `port-team-coordination-metadata` 或单独 PR 接入。

### D5: Token 估算

复用 `cdt-core::estimate_tokens`（已有）或 `content.len() / 4` 简单估算，与 TS `estimateTokens` 一致。

### D6: `NotificationManager` 持久化

- 路径：`~/.claude/claude-devtools-notifications.json`
- 格式：`Vec<StoredNotification>`（`DetectedError` + `is_read` + `created_at`）
- Max 100 条，超出按 `created_at` 升序移除最老的
- 加载时 auto-prune

### D7: 历史预览（`test_trigger`）

TS 的 `ErrorTriggerTester` 扫描所有 project → session files → messages。Rust 可复用 `cdt-discover::ProjectScanner` 和 `cdt-parse::parse_file`，但这引入跨 crate 依赖。设计为 trait callback 模式：`ErrorDetector::test_trigger` 接受 `async Fn(project) -> Vec<(sessionId, Vec<ParsedMessage>)>` 参数，调用者提供 scanner 实现。

**简化方案**：本 port 只实现 `detect_errors_with_trigger`（单 session 版本）作为纯函数，完整的 multi-project scan 留给 `cdt-api` 层组装。

## Risks / Trade-offs

- **[Risk] `cdt-config` 依赖 `cdt-core::ParsedMessage`** → 已有依赖，无新风险。
- **[Trade-off] `repositoryIds` stub** → 功能完整性略有折扣，但避免 `cdt-config` → `cdt-discover` 的依赖。后续接入时只需实现 `matches_repository_scope` 的 cache + resolver。

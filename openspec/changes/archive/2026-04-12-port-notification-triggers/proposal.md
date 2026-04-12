## Why

`notification-triggers` 是 Remaining port order 中紧接 `configuration-management` 的下一步。trigger 数据类型 + CRUD + 持久化已在前一个 port 落地于 `cdt-config`，本 port 实现 trigger 的**评估引擎**：error 检测、pattern 匹配、token 阈值检查、历史预览，以及通知持久化（read/unread + 分页）。

TS 的 followup 指出 spec 要求检查 `is_error=true` flag，TS 实际在 `ErrorTriggerChecker.ts:170` 的 `requireError` 分支里做了检查，行为与 spec 一致。Rust port 延续此行为。

## What Changes

- 在 `cdt-config` crate 新增 trigger 评估模块：
  - `DetectedError` 类型 + error message 提取/截断
  - `TriggerMatcher`：regex cache（LRU）+ pattern/ignore 匹配（复用 `regex_safety.rs`）
  - `ErrorTriggerChecker`：3 种 mode 的纯函数 checker（`error_status` / `content_match` / `token_threshold`）
  - `ErrorDetector`：orchestrator，遍历 messages × triggers 产出 `DetectedError` 列表
  - `NotificationManager`：通知持久化到 `~/.claude/claude-devtools-notifications.json`，max 100 条，read/unread state，分页查询
- `repositoryIds` 范围过滤暂 stub（完整实现依赖 `cdt-discover` 的 `GitIdentityResolver`，留给后续 port 接入）

## Capabilities

### New Capabilities

（无新 capability）

### Modified Capabilities

- `notification-triggers`：Scenario "Tool result flagged `is_error`" 细化——MUST 检查 `is_error` flag（非 content-pattern matching），对齐 spec；增加 `NotificationManager` 持久化相关 scenario

## Impact

- **代码**：`crates/cdt-config/src/` 新增 `detected_error.rs`、`trigger_matcher.rs`、`error_trigger_checker.rs`、`error_detector.rs`、`notification_manager.rs`
- **依赖**：新增 `uuid`（`DetectedError.id` 生成）、`lru`（regex cache）；已有 `cdt-core`（`ParsedMessage` / `ContentBlock` / `ToolCall`）
- **下游**：`cdt-api` 的 IPC/HTTP handler 可直接消费 `ErrorDetector` + `NotificationManager`

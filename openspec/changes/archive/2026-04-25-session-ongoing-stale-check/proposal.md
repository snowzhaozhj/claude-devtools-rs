# proposal: session-ongoing-stale-check

## Why

`cdt-api` 计算 `SessionSummary.isOngoing` / `SessionDetail.isOngoing` 时仅调
`cdt_analyze::check_messages_ongoing` 做纯结构性活动栈判定，**没有**对齐
原版 `claude-devtools/src/main/services/discovery/ProjectScanner.ts:753-755`
的 `STALE_SESSION_THRESHOLD_MS = 5 * 60 * 1000` 兜底（issue #94）。

后果：用户 Ctrl+C / kill cli / 重启电脑导致 cli 异常退出时，session 末尾常停在
`tool_result` 之类 AI 活动而无 `text_output` / `interruption` / `ExitPlanMode`
等 ending 信号；活动栈算法将其判 ongoing，sidebar / SessionDetail 因此一直渲染
绿色脉冲圆点 + 蓝色横幅。用户实测：4 月 12 日的 session 在 4 月 25 日仍显示运行中。

## What Changes

`ipc-data-api`：在 `Expose project and session queries` Requirement 中明确
`isOngoing` 的计算包含**两路 AND**：
1. `cdt_analyze::check_messages_ongoing(messages)` 返回 `true`
2. session JSONL 文件 mtime 距 `now < 5 分钟`

任一条件不满足时 `isOngoing` SHALL 为 `false`。`list_sessions` 与
`get_session_detail` 两条 IPC 路径行为一致；HTTP 路径同样适用（list_sessions_sync
共用 `extract_session_metadata`）。

stale 阈值常量（5 分钟）定义在 `cdt-api`，对齐 TS `STALE_SESSION_THRESHOLD_MS`。

## Impact

- **Affected specs**：`ipc-data-api` MODIFIED `Expose project and session queries`
- **Affected code**：`crates/cdt-api/src/ipc/session_metadata.rs`（新增
  `STALE_SESSION_THRESHOLD` / `is_session_stale` / `is_file_stale`）、
  `crates/cdt-api/src/ipc/local.rs::get_session_detail`（叠加 stale check）
- **Risk**：低；纯收紧 ongoing 判定，不会把已结束 session 错判为 ongoing
  （唯一新增 false→true 路径不存在）。stat 失败保守保留 messages_ongoing
  判定，时钟回拨（mtime > now）保守判 not stale。
- **回滚**：删除 `is_file_stale` 调用即可退回 5min 前算法。

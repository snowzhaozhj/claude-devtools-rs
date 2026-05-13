## Why

ongoing 会话中 subagent 调用链不会自动刷新——用户反馈"subAgent 还在新增调用，UI 不更新，而且偶尔是 Task 状态能看明细，偶尔是 Agent 工具调用看不到明细"。

诊断定位到三处独立的契约 / 实现漏洞：

- **R1（file-watching）**：`cdt-watch::parse_project_event` 的路径解析硬要求 `components.len() == 2`，但 subagent JSONL 实际落盘是 4 层 `<projects_dir>/<project_id>/<session_id>/subagents/agent-*.jsonl`。子会话写入被 watcher 静默忽略，父 session 没有任何刷新通知。
- **R2（ipc-data-api + 前端 SubagentCard）**：lazy-load 模式下 `Process.messagesOmitted=true`，`process.messages` 永远是空数组；前端 `SubagentCard` 第一次 `getSubagentTrace` 后写入 `messagesLocal` 即永久 sticky，ongoing subagent 内部增量永远拿不到。后端缺乏一个对前端可见的"messages 总数版本指纹"让 UI 主动失效。
- **R3（tool-execution-linking）**：后端 `cdt-parse` 把 `Task` 和 `Agent` 两个工具名都识别为 task 调用并尝试关联 `SubagentProcess`，但前端 `displayItemBuilder.ts` 只对 `toolName === "Task"` 跳过 `ToolItem` 渲染让 `SubagentCard` 接管，导致 `Agent` 工具即使关联成功也被默认 `DefaultToolViewer` 渲染成无明细的一行。

三个根因联起来才让用户感受到"既不刷新又形态混乱"。需要在同一个 change 内一次性修齐，否则单修一处用户感受不到改善。

## What Changes

- **MODIFIED** `file-watching`：扩展 `parse_project_event` 路径识别能力，把 `<projects_dir>/<project_id>/<session_id>/subagents/agent-*.jsonl`（新结构 subagent JSONL）的写入归并到父 `(project_id, session_id)` 的 `FileChangeEvent`，让父 session 订阅者能在 subagent 内部追加时收到刷新通知。旧结构 `<project>/agent-*.jsonl` 不在本 change 范围（Claude Code 已淘汰；watcher 不允许做 IO 解析 parentUuid）。
- **MODIFIED** `ipc-data-api`：`Process` / `SubagentProcess` 新增 `messagesTotalCount: u32` 字段，在 `OMIT_SUBAGENT_MESSAGES=true` 裁剪 messages 前记录原 chunks 数量。`messages_total_count` MUST 在 `OMIT_SUBAGENT_MESSAGES=false` 回滚路径下仍正确填充（等于 `messages.len()`），让前端始终可用同一字段判断版本。
- **MODIFIED** `tool-execution-linking`：补一条 Requirement 说明前端"已关联 SubagentProcess 的 task tool 跳过 `ToolItem` 渲染由 SubagentCard 接管"的判定 MUST 与后端 `cdt-parse::is_task` 对齐（即同时识别 `Task` 与 `Agent`）。配套的 vitest / IPC contract 测试 SHALL 覆盖 `Agent` 工具关联场景。
- **前端 SubagentCard（无 spec 改动，归 ipc-data-api 的实现细节）**：`messagesLocal` 改为 version-keyed `$state`；当 `(isOngoing, endTs, messagesTotalCount)` 版本变化且 `isOngoing=true` 时主动调 `getSubagentTrace` 重拉。无展开状态时不主动拉（避免无谓 IPC）。

## Capabilities

### Modified Capabilities

- `file-watching`：新增 Requirement 描述 subagent 嵌套路径路由到父 session。
- `ipc-data-api`：在现有 "Trim subagent messages..." Requirement 上扩展 `messagesTotalCount` 字段契约；在 `Lazy load subagent trace` Requirement 上加"ongoing 主动重拉"Scenario。
- `tool-execution-linking`：新增 Requirement 描述前端"task tool 关联 subagent 后跳过 `ToolItem`"判定与 `is_task` 工具名集合对齐。

## Impact

- **后端**
  - `crates/cdt-watch/src/watcher.rs::parse_project_event`：新增嵌套路径分支
  - `crates/cdt-core/src/process.rs`（或 SubagentProcess 定义所在）：加 `messages_total_count: u32` 字段（`#[serde(rename = "messagesTotalCount")]`）
  - `crates/cdt-analyze/src/tool_linking/resolver.rs::candidate_to_process`：填充 `messages_total_count = cand.messages.len()`
  - `crates/cdt-api/src/ipc/local.rs`：裁剪 messages 前确保 `messages_total_count` 已填；contract test 覆盖
- **前端**
  - `ui/src/components/SubagentCard.svelte`：messagesLocal version-keyed + 主动重拉 effect
  - `ui/src/lib/displayItemBuilder.ts:167`：兼容 `toolName === "Agent"`
  - `ui/src/lib/api.ts`：`SubagentProcess` interface 加 `messagesTotalCount`
- **测试**
  - `crates/cdt-watch/src/watcher.rs::tests`：嵌套路径解析单测
  - `crates/cdt-api/tests/ipc_contract.rs`：`messagesTotalCount` 字段断言、`Agent` 工具关联 fixture
  - `ui/src/lib/__fixtures__/`：multi-project-rich fixture 加 `Agent` 工具 + subagent 关联 case
  - `ui/src/lib/displayItemBuilder.test.ts`（如有）：`Agent` 工具 + `taskIdsWithSubagents` 命中 → 跳过 ToolItem
- **风险**
  - R1 修复后 ongoing 大会话刷新频率上升（每条子 session 写入 → 父 refresh），需复测 `fileChangeStore` 250ms trailing 是否够。
  - R2 主动重拉每次 ≤ 1 次 `getSubagentTrace` IPC / refresh / 已展开卡片；需依赖 `dedupeRefresh` 同 key 合并避免短时重复拉。
  - 三个修复都属行为契约改动，IPC contract 必须同步更新；遗漏会让前后端解耦失败。

# align-subagent-ongoing-and-chat-rendering

## Why

本次改动起因是用户跑几个长时运行的后台 subagent 时观察到两类偏差：

1. **Subagent ongoing 状态误报** —— `Process.is_ongoing` 装载层走"末行 timestamp > 首行 → done"的简化判定，与主 session 走的 `check_messages_ongoing` 五信号活动栈算法不一致。subagent 中断后无 assistant 收尾（如 tool_use→tool_result 但无后续 assistant）时被误判 done，SubagentCard 右上角错显 ✓。
2. **聊天渲染与原版偏差** —— user 气泡丢 task-notification 卡片、AI header token 显示累加值异常大、工具行无 "tokens" 后缀、Bot 头像渲染歪、MetricsPill 带冗余 "Main"/"Ctx" 标签。

Bug 修复与 UI 对齐已落地（commits `5726c55` / `8fa538b` / `7fa7423` / `9b13b89` / `6d7bbe8`），但对应 spec 契约要么**模糊**（`tool-execution-linking` 的 `is_ongoing` Scenario 只写"不含终结标记"未定义是哪些信号），要么**缺失**（`session-display` 未覆盖 task-notification / AI header token snapshot 语义）。本 change 把这两块行为契约**精确化**或**补齐**到主 spec，避免未来被再次简化或回退。

## What Changes

### tool-execution-linking (MODIFIED)

- `Enrich subagent processes with team metadata` Requirement 的 `is_ongoing` 字段描述与 `is_ongoing 判定` Scenario 改写：明确 `Process.is_ongoing` MUST 走 `cdt_analyze::check_messages_ongoing(&parsed_messages)` 五信号活动栈算法（text / interrupt / ExitPlanMode / tool rejection / SendMessage shutdown_response），与主 session `get_session_detail.isOngoing` 一致；**禁止**仅用 `end_ts.is_none()` 或 "末行 timestamp > 首行" 之类的简化判定
- 新增 Scenario: `Orphan tool_result without assistant reply → is_ongoing=true`（复刻 `5a3a23b2.../agent-aee63780244f1f959.jsonl` 真实 case）
- 新增 Scenario: `装载层与主 session ongoing 判定一致`（断言同一 `ParsedMessage` 流跑两路判定结果一致）

### session-display (ADDED)

- 新增 Requirement `Render task notification cards in user bubble`：当 user chunk 的 `content` 含 `<task-notification>...</task-notification>` XML 块时，UI 层 MUST 用 `parseTaskNotifications` 抽出每个 block 的 `taskId` / `status` / `summary` / `outputFile` 四字段，渲染为独立卡片；文本经 `cleanDisplayText` 清洗后即使为空，只要卡片列表非空 user 气泡仍 MUST 渲染
- 新增 Requirement `AI header token summary uses last response usage snapshot`：AIChunk header 右侧的 token 数字 MUST 为该 chunk 最后一条 `responses[].usage` 的四项之和（`input_tokens + output_tokens + cache_creation_input_tokens + cache_read_input_tokens`），作为该 AI turn 结束时的 context window snapshot；**禁止**累加 chunk 内多条 responses（Anthropic usage 每次返回整段历史 cache size，累加会重复计数）。hover 时 MUST 显示 5 行 breakdown popover（Total / Input / Output / Cache create / Cache read），与 Info icon 前缀协同提示可 hover
- 新增 Requirement `Tool row displays approximate token count`：ExecutionTrace 内每个工具 row MUST 通过 `getToolContextTokens(exec) = estimate(input) + estimate(output)` 估算 token 总和（~4 字符/token 启发式），以 `~{compact} tokens` 格式显示在状态圆点前，与原版 `BaseItem.tsx:150` 一致

## Impact

- **Specs affected**: `tool-execution-linking` (MODIFIED 1 Requirement + ADDED 2 Scenarios), `session-display` (ADDED 3 Requirements)
- **Code**: 已全部落地，本 change 仅 sync spec；tasks.md 为已完成状态
- **Tests**: `crates/cdt-api/src/ipc/local.rs::tests` 新增 3 个 `#[tokio::test]` 覆盖装载层 ongoing 三 Scenario；前端纯视觉行为无新增单元测试（视觉回归靠人工 QA）
- **Performance**: `check_messages_ongoing` 在已 parse 的 `Vec<ParsedMessage>` 上 O(n) 单次遍历，perf bench 实测 IPC 时间与改前同档（46a25772 case TOTAL 55ms 不变）

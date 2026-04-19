# Tasks

本 change 的代码已先行落地（commits `5726c55` / `8fa538b` / `7fa7423` / `9b13b89` / `6d7bbe8`），本文件只做 spec 同步的 apply 记录。

## 1. tool-execution-linking 装载层 ongoing 判定

- [x] `parse_subagent_candidate` 在 `parse_file(path)` 后跑 `check_messages_ongoing(&msgs)`，删除 `is_ongoing = end_ts.is_none()` 简化判定（`crates/cdt-api/src/ipc/local.rs`）
- [x] resolver `compute_is_ongoing = cand.is_ongoing || cand.end_ts.is_none()` OR 兜底保留
- [x] 3 个 `#[tokio::test]` 覆盖：末尾 assistant tool_use 无收尾 / 末尾 text ending / orphan tool_result（`crates/cdt-api/src/ipc/local.rs::tests`）

## 2. session-display task-notification 卡片

- [x] `ui/src/lib/toolHelpers.ts` 新增 `parseTaskNotifications` + `TaskNotification` 类型
- [x] `ui/src/routes/SessionDetail.svelte` user chunk 分支：`@const taskNotifications`；渲染条件含 `taskNotifications.length > 0`；5 行卡片 UI + CSS
- [x] `cleanDisplayText` 的 `<task-notification>` 过滤保持原状

## 3. session-display AI header token snapshot

- [x] `ui/src/routes/SessionDetail.svelte` AI chunk 分支顶部 `@const lastUsage = [...chunk.responses].reverse().find(r => r.usage)?.usage`
- [x] `totalTokens` 基于 `lastUsage` 四项之和
- [x] Info icon（lucide Info 多 path SVG）+ ai-tokens-popover 自定义 hover 卡片

## 4. session-display 工具 row token 槽

- [x] `ui/src/lib/toolHelpers.ts` 新增 `estimateTokens` + `estimateContentTokens` + `getToolContextTokens`
- [x] `ExecutionTrace.svelte` + `SessionDetail.svelte` 的 tool BaseItem 传 `tokenCount={getToolContextTokens(exec)}`
- [x] `BaseItem.svelte` token 槽加 "tokens" 后缀

## 5. 非行为类视觉对齐（不入 spec，仅记录）

- [x] MetricsPill 去 label + `·`→`|`（`ui/src/components/MetricsPill.svelte`）
- [x] Claude 头像换多 path/rect inline SVG（`ui/src/routes/SessionDetail.svelte`）
- [x] 删除废弃 `BOT` 常量（`ui/src/lib/icons.ts`）

## 6. 验证

- [x] `cargo clippy --workspace --all-targets -- -D warnings` 0 warning
- [x] `cargo test -p cdt-api` 3 新 test 通过
- [x] `npm run check --prefix ui` 0 error
- [x] `just preflight` 全绿
- [x] `openspec validate align-subagent-ongoing-and-chat-rendering --strict` 通过
- [x] perf bench 三个 sample IPC 时间与改前同档（无回归）

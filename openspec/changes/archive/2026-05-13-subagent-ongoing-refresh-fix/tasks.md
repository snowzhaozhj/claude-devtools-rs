## 1. cdt-watch / file-watching（R1：嵌套 subagent 路径路由）

- [ ] 1.1 `crates/cdt-watch/src/watcher.rs::parse_project_event` 增加 4 层嵌套分支：`components.len() == 4 && components[2].as_os_str() == "subagents"` 且 `components[3]` 以 `agent-` 起头、`.jsonl` 结尾、非 `agent-acompact*.jsonl` 时，emit `FileChangeEvent { project_id: components[0], session_id: components[1], deleted, project_list_changed: false }`。**硬编码 `project_list_changed: false`**（codex 二审发现）—— 嵌套分支**不**走既有 2 层路径的 `!deleted && mark_project_seen(project_id)` 派生逻辑，避免极端 race 下父项目首次出现误触发 DashboardView / Sidebar 项目列表刷新
- [ ] 1.2 复用既有 `path.extension()?` 大小写不敏感比较；新分支命中后 SHALL 不再 fall through 到 2 层判定
- [ ] 1.3 单测 `crates/cdt-watch/src/watcher.rs::tests::parse_event_routes_nested_subagent_jsonl`：构造 4 层路径 → 期望 `project_id` 与 `session_id` 为父值
- [ ] 1.4 单测 `parse_event_ignores_agent_acompact_in_subagents_dir`：构造 `subagents/agent-acompact-x.jsonl` → 期望 `None`
- [ ] 1.5 单测 `parse_event_ignores_non_jsonl_under_subagents`：`subagents/notes.txt` → 期望 `None`
- [ ] 1.6 单测 `parse_event_ignores_non_agent_prefix_under_subagents`：`subagents/random.jsonl` → 期望 `None`
- [ ] 1.7 单测 `parse_event_keeps_legacy_two_level_behavior`：旧结构 `<project>/agent-x.jsonl`（2 层）行为不变（按 stem 当 sessionId）
- [ ] 1.8 单测 `parse_event_routes_nested_subagent_delete_event`：4 层路径 + `deleted=true` → 期望 `FileChangeEvent { deleted: true, project_list_changed: false }`，父 sessionId 正确
- [ ] 1.8b 单测 `parse_event_nested_subagent_forces_project_list_changed_false`：构造一个全新 `<project>/<session>/subagents/agent-x.jsonl` 路径（watcher 此前未见过该 project_id，`mark_project_seen` 若调用会返回 `true`）→ 期望 emit 的 `FileChangeEvent.project_list_changed === false`（codex 二审强制约束）
- [ ] 1.9 `crates/cdt-watch/tests/file_watching.rs`（集成 smoke）：在 tempdir 真造 `<project>/sess/subagents/agent-x.jsonl` 写入，订阅 `subscribe_files()` → 在 debounce 窗口结束后收到对应父 session 的 `FileChangeEvent`
- [ ] 1.10 `cargo test -p cdt-watch` 全绿；`just preflight` 不破坏既有 watcher 测试

## 2. cdt-core / cdt-analyze（R2 后端：messagesTotalCount 字段）

- [ ] 2.1 `crates/cdt-core/src/process.rs`（或 `Process` 定义所在）加字段 `pub messages_total_count: u32`，加 `#[serde(rename = "messagesTotalCount", default)]`
- [ ] 2.2 `grep -rn "Process {" crates --include="*.rs"` 列出所有 `Process { ... }` 构造点，一轮 Edit 全部补齐 `messages_total_count: <expr>`（按 CLAUDE.md "cdt-core 核心 struct 加字段先 grep 全构造点"）
- [ ] 2.3 `crates/cdt-analyze/src/tool_linking/resolver.rs::candidate_to_process`：填 `messages_total_count: cand.messages.len() as u32`，与 `header_model` / `last_isolated_tokens` / `is_shutdown_only` 同阶段
- [ ] 2.4 `crates/cdt-analyze/src/tool_linking/filter.rs::62` 附近的零值 Process 构造点（orphan 兜底）补 `messages_total_count: 0`
- [ ] 2.5 `cargo check --workspace` 与 `cargo clippy --workspace --all-targets -- -D warnings` 一次性绿
- [ ] 2.6 `cargo test -p cdt-analyze` 跑 resolver 测试，保证既有 Snapshot / Scenario 不破

## 3. cdt-api（R2 后端：IPC 裁剪保留 messagesTotalCount）

- [ ] 3.1 `crates/cdt-api/src/ipc/local.rs::apply_subagent_messages_omit`（或同等裁剪函数）：检查在 `messages = Vec::new()` 之前 `messages_total_count` 已被 resolver 填充；如有依赖 `messages.len()` 的旁路填充逻辑 SHALL 提前到裁剪前
- [ ] 3.2 `crates/cdt-api/tests/ipc_contract.rs`：在覆盖 `get_session_detail` 的现有 fixture 上断言 `SubagentProcess.messagesTotalCount` 是 `u32` 且 `≥ 0`、JSON key 拼写 camelCase
- [ ] 3.3 新增 contract test `subagent_messages_total_count_in_omit_path`：构造一个 ≥2 chunks 的 subagent JSONL，跑 `get_session_detail` 默认 OMIT 路径 → 断言 `messagesOmitted=true` 且 `messagesTotalCount = <真实 chunk 数>`、`messages = []`
- [ ] 3.4 新增 contract test `subagent_messages_total_count_in_rollback_path`：把 `OMIT_SUBAGENT_MESSAGES` 临时改 false 或绕过裁剪 → 断言 `messagesOmitted=false` 且 `messagesTotalCount = messages.len()`
- [ ] 3.5 `EXPECTED_TAURI_COMMANDS` 不变（无新 IPC 命令）；只是字段扩展，无需改 invoke_handler
- [ ] 3.6 `cargo test -p cdt-api --test ipc_contract` 全绿

## 4. tool-execution-linking 后端确认（R3 后端）

- [ ] 4.1 `crates/cdt-parse/src/parser.rs:187` 的 `is_task = name == "Task" || name == "Agent"` 保持不动；`resolve_subagents` 当前已对两类工具走同一关联流程
- [ ] 4.2 `crates/cdt-analyze/src/tool_linking/pair.rs` 已有 `Agent` 工具的关联测试（grep 已确认）；本 change 不新增后端逻辑，仅在前端补判定
- [ ] 4.3 IPC contract test 加 fixture：父 session 含一个 `Agent` 工具调用 + 对应 subagent JSONL（agentId 通过 `result_agent_id` 或 `agentId:` 文本暴露）；断言 `get_session_detail` 返回的 `AIChunk.subagents[i].parentTaskId === <agent tool_use_id>`、`AIChunk.toolExecutions[i].toolName === "Agent"`（两边能被前端关联起来）

## 5. ui / displayItemBuilder（R3 前端）

- [ ] 5.1 `ui/src/lib/displayItemBuilder.ts:167` 改判定：`if ((exec.toolName === "Task" || exec.toolName === "Agent") && taskIdsWithSubagents.has(exec.toolUseId)) break;`
- [ ] 5.2 `ui/src/lib/__fixtures__/multi-project-rich.ts`（或同等 vitest fixture）补 `Agent` 工具 + subagent 关联 case
- [ ] 5.3 vitest 单测 `ui/src/lib/displayItemBuilder.test.ts`（如缺新建）：
  - 5.3a `Agent tool with linked subagent should be filtered from display items`
  - 5.3b `Orphan Agent tool should remain as default ToolItem`
  - 5.3c `Task tool with linked subagent should still be filtered (regression)`
- [ ] 5.4 `npm run test:unit --prefix ui -- displayItemBuilder` 全绿

## 6. ui / SubagentCard（R2 前端：版本指纹 + 主动重拉）

- [ ] 6.1 `ui/src/lib/api.ts::SubagentProcess` interface 加 `messagesTotalCount: number`（与 IPC 字段名对齐，camelCase）
- [ ] 6.2 `ui/src/components/SubagentCard.svelte` 状态扩展：
  - 6.2a `const messagesVersion = $derived(\`${process.isOngoing ? '1' : '0'}|${process.endTs ?? '_'}|${process.messagesTotalCount ?? 0}\`)`
  - 6.2b 模块级 inflight Map：`Map<string /* sessionId|version */, Promise<Chunk[]>>`。**复用 key MUST 是 `${sessionId}|${messagesVersion}` 联合 key**（codex 二审发现，spec 约束）—— 仅按 sessionId 复用会让旧版本 Promise 在版本递增后被复用，把 stale trace 写入 `messagesLocal`
  - 6.2c **Promise settle 后必须 race-check**：fetch 时记录 `fetchedVersion`，settle 时若 `fetchedVersion !== process.messagesTotalCount` 拼出的当前版本 → 视为 stale，SHALL NOT 写入 `messagesLocal`（让新版本 fetch 接管）
- [ ] 6.3 `$effect` 监听 `messagesVersion`：
  - 已展开（`messagesLocal !== null`）且 `process.isOngoing === true` → 调 `getSubagentTrace` 重拉并替换 `messagesLocal`；inflight 命中 SHALL 复用 Promise
  - 已展开且 `process.isOngoing` 翻转到 `false` → 也触发一次 final 重拉（version 必然递增，自然命中）
  - 未展开（`messagesLocal === null`） → SHALL NOT 发 IPC；仅保持 null 等下次展开
- [ ] 6.4 注意 Svelte 5 `$effect` 顶层依赖：`messagesVersion` 通过 `$derived` 暴露成响应式值，`$effect` 内显式读它；其它 `process.*` 读取用 `untrack` 包裹避免无关重跑
- [ ] 6.5 vitest 单测 `ui/src/components/SubagentCard.test.ts`（如缺新建）：
  - 6.5a `expanded ongoing subagent re-fetches on messagesTotalCount increase`
  - 6.5b `collapsed subagent does NOT refetch when totalCount changes`
  - 6.5c `concurrent same-version bumps dedupe via inflight Map`
  - 6.5d `cross-version bumps issue separate IPC (stale Promise discarded)` —— **codex 二审强制 case**：版本 N fetch pending → 版本递增 N+1 → 期望两次独立 IPC，N+1 的结果写入 messagesLocal，N 的结果**不**写入（race-check 命中 stale）
  - 6.5e `missing messagesTotalCount (undefined) keeps version stable, no refetch`
- [ ] 6.6 `npm run test:unit --prefix ui -- SubagentCard` 全绿
- [ ] 6.7 `npm run check --prefix ui` 全绿（svelte-check 不报 state_referenced_locally）

## 7. 端到端验证 + 性能复测

- [ ] 7.1 `cargo tauri dev` 手动跑：选一个正在运行 Claude Code 子会话的项目，触发 Task / Agent subagent 启动 → 观察 SessionDetail 在子 JSONL 写入后**无需折叠重开**就能看到新增 messages（R1 + R2 联合验证）
- [ ] 7.2 同一个 session 含 `Agent` 工具调用 → 观察 UI 显示为 SubagentCard 可展开看明细（R3 验证）
- [ ] 7.3 `cargo test --release -p cdt-api --test perf_get_session_detail -- --ignored --nocapture` 跑性能基准；与本 change 前的最近一次基准 diff，确认 IPC payload / 后端时长无回归（messagesTotalCount 是单 u32 字段，预期可忽略）
- [ ] 7.4 复测 ongoing 大会话刷新频率：在一个 ≥ 100 chunks 的活跃 session 上跑 1 分钟，确认 `fileChangeStore` 250 ms trailing 合并正常工作（DevTools console 中 `[perf] SessionDetail` 日志频率 ≤ 4 Hz）

## 8. preflight + 异构二审

- [ ] 8.1 `just preflight`（fmt + lint + test + spec-validate）全绿
- [ ] 8.2 spec delta 已 archive 前再过一遍：`openspec validate subagent-ongoing-refresh-fix --strict`
- [x] 8.3 codex 异构二审 design（design 完成阶段就跑一次，按 CLAUDE.md `.claude/rules/codex-usage.md` 的 "design 阶段决策风险二审"段）：`Agent({ subagent_type: "codex:codex-rescue", prompt: "..." })` 让 codex 评 D1/D2/D3 的候选方案对比与风险点是否漏列 —— 已在 propose 阶段执行（agentId `a732d6793eb7217c3`），找到 2 个真问题已 fold 回 design + spec + tasks（嵌套分支 `project_list_changed=false` 强制；inflight 复用 key 绑定版本）
- [ ] 8.4 实现完成 push 之后再跑一次 codex 二审找 race / 边界 bug（`.claude/rules/codex-usage.md` "PR commit 之后二审"段），重点查：fileChangeStore 节流 + SubagentCard inflight 去重的并发安全；显式让 codex 验证 R1/R2/R3 三处修法是否真的覆盖原报告的所有症状

## 9. archive 与 PR

- [ ] 9.1 整个 change 实现 + 测试 + codex 二审通过后，本 PR 内同一个最后 commit `openspec archive subagent-ongoing-refresh-fix -y` 把 spec delta merge 回主 spec（按 `feedback_pr_archive_timing` 记忆约束："archive commit 作为 PR 最后一个 commit，reviewer 审时看主 spec 最终态"）
- [ ] 9.2 PR 描述中显式点名修复了 R1 / R2 / R3 三个根因；commit message 末尾标注 codex review 结论

## 1. cdt-analyze:骨架升级纯函数

- [x] 1.1 在 cdt-analyze 新增骨架 `Process` 工厂:由 `ToolExecution` 合成,按 design D3 填齐字段(`session_id=result_agent_id`、`parent_task_id=Some(tool_use_id)`、`spawn_ts=start_ts`、`metrics=Default`、`messages=[]`、`messages_omitted=true`、`is_ongoing=false`、`description` 截断到字节上限)
- [x] 1.2 实现纯函数 `promote_result_agent_tasks(chunks)`:遍历各 `AIChunk` 的 `ToolExecution`,对带 `result_agent_id` 的 `Agent`/`Task` 升级成骨架 subagent → push 进 `AIChunk.subagents`
- [x] 1.3 插入 `SubagentSpawn`:`placeholder_id=session_id`,按对应 `tool_use_id` 找到同 id 的 `ToolExecution` step 后相邻 insert(找不到则 append + `tracing::warn!`),复用既有插入顺序契约
- [x] 1.4 去重:骨架填 `parent_task_id` + 移除被升级的 Agent/Task `ToolExecution`(payload 瘦身),前端靠 `parent_task_id` 跳过不重复
- [x] 1.5 `pub use` 导出 `promote_result_agent_tasks`(`cdt-analyze::chunk` + `lib.rs`),保持 cdt-analyze sync / 无 runtime dep

## 2. cdt-analyze:单元测试

- [x] 2.1 喂含 `result_agent_id` 的 AIChunk → 验证骨架生成 + 字段(`parent_task_id`/`messages_omitted`/`is_ongoing`)
- [x] 2.2 验证 `SubagentSpawn` 紧随对应 `ToolExecution` 的插入顺序
- [x] 2.3 验证无 `result_agent_id` 的工具不升级、原样保留(普通工具 + 未完成的 Agent 调用两例)
- [x] 2.4 验证骨架不与原始 Agent 工具重复渲染(去重 + 已 resolve 不重复)
- [x] 2.5 验证 `description` 超长被截断到字节上限

## 3. cdt-api:接入 get_subagent_trace

- [x] 3.1 `local.rs::get_subagent_trace` 在 `build_chunks` 后调用 `promote_result_agent_tasks(chunks)` 再返回(`get_workflow_agent_trace` 走 WorkflowCard 不同渲染路径,spec 未覆盖,保守不接)
- [x] 3.2 确认不触碰 `get_session_detail` 主路径与 `scan_subagent_candidates_for_detail`,无新文件 IO

## 4. IPC 契约测试

- [x] 4.1 `cdt-api/tests/nested_subagent_trace.rs` 端到端:真 jsonl(sub-a spawn sub-b)→ `get_subagent_trace` 返回的 chunks 含 `messagesOmitted=true` + `parentTaskId` 的骨架 subagent,验证 camelCase 形态 + 真实 `result_agent_id` 提取链路

## 5. 前端验证与测试

- [x] 5.1 确认 `ExecutionTrace.svelte`(`item.type==="subagent"` 递归 + depth limit)/ `SubagentCard.svelte`(`ensureMessages` 对 `messagesOmitted=true` 懒拉)对骨架走既有路径,**前端零改**
- [x] 5.2 SubagentCard 懒拉覆盖:`messagesOmitted=true` 骨架展开触发 `getSubagentTrace`——`toggleExpanded`→`ensureMessages` 链路已就位;module 级 inflight 去重由现有 `SubagentCard.test.ts` 覆盖;组件级 lifecycle 本仓 vitest 不测(globals:false),由 5.3 真实数据端到端覆盖
- [x] 5.3 真实样本(7f59237e)HTTP 验证:`cdt serve` + `curl /api/sessions/<root>/subagents/<L1>/trace` → level-1 agent `a258f56fc2400949f` 内部 level-2 骨架 `ab60f11f3b1536ec3` 正确升级(messagesOmitted/parentTaskId/Explore/desc 齐);同 agent 内另一未回填 agentId 的 Agent 调用正确保持工具显示(边界已验)

## 6. 收尾

- [x] 6.1 CHANGELOG `## [Unreleased]` `### Added` 追加一行(英文,嵌套 subagent 可逐层展开 + 未完成边界说明)
- [x] 6.2 开 GitHub issue 记录范围外项:`find_subagent_jsonl` 路径歧义(#525,`bug` label)
- [x] 6.3 `just preflight` 全绿(fmt + lint + test + spec-validate,含 build ui/dist)

## N. 发布

- [ ] N.1 push 分支 + 开 PR(贴 Perf impact:升级步骤零新 IO,不碰主路径)
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex + pr-review-toolkit 二审通过(如发现 bug:修 → push → 回 N.2 重跑;可循环)
- [ ] N.4 archive change(archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿)

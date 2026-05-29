# 实现任务

> 注：runId 抽取实际落在 `tool-execution-linking` capability（issue #397 说的「chunk-building」是笔误，本 change 已对齐到真实 spec home）；运行态降级落 `ipc-data-api`；前端运行态渲染落 `session-display`。这是有意偏离 issue 表述、对齐实际 spec 归宿。

## 1. PR 6a — cdt-core: ToolExecution 加 workflow_script_path 字段

- [x] 1.1 在 `crates/cdt-core/src/tool_execution.rs` 的 `ToolExecution` struct 加 `workflow_script_path: Option<String>` + `#[serde(default, skip_serializing_if = "Option::is_none")]`（camelCase 序列化为 `workflowScriptPath`）
- [x] 1.2 grep 全部 `ToolExecution {` 构造点（20 处），一轮 Edit 补齐新字段再 `cargo check`，确认 Option + serde(default) 不破坏编译

## 2. PR 6a — cdt-analyze: pair.rs 抽取 scriptPath

- [x] 2.1 在 `crates/cdt-analyze/src/tool_linking/pair.rs` 配对完成时，当 `pu.tool_name == "Workflow"`：优先 `tool_use_result.get("scriptPath")`，缺失/非 string 时回退 `pu.input.get("scriptPath")`，取到 string 写入 `workflow_script_path`（与既有 `workflow_run_id` 抽取同处，output trim 前；双源 pair 点零额外 I/O）
- [x] 2.2 pair 单元测试加 fixture：toolUseResult 有 scriptPath / toolUseResult 无但 input 有（回退）/ 两处都无 / 非 string 四种 case
- [x] 2.3 `crates/cdt-api/tests/ipc_contract.rs` 加 round-trip test：Workflow ToolExecution 序列化含 `workflowScriptPath`；非 Workflow / 无值 不含该字段

## 3. PR 6a — cdt-api: 运行态降级解析（Tier 0）

- [x] 3.1 在 `workflow_manifest.rs` 新增 journal 解析（`parse_journal` + `extract_journal_agent_id`）：读 journal.jsonl，逐行区分 `type=="started"`/`type=="result"`，按 `agentId` 去重，合成匿名 agents（有 result→Completed，仅 started→Running）。**坑1**：合成 agent `failed=false`、`tokens/tool_calls=0`，绝不套 manifest 失败启发式
- [x] 3.2 新增 journal `FileSignature` 缓存（`WorkflowManifestCache.journal_entries`）；轻量行计数（行首判 type + 子串提 agentId），不做 JSON 全解析（result 行内嵌大输出）
- [x] 3.3 新增 `workflow_name_from_script_path`：精确 `strip_suffix(".js")` + `strip_suffix("-<runId>")`；缺失/不匹配→None（不模糊匹配）
- [x] 3.4 改 `resolve_single`：manifest `stat` 失败时进 `resolve_running_state` 降级分支——journal 存在→`Running`；journal 缺失→`Pending`
- [x] 3.5 改 `resolve_workflow_items` 调用链：`collect_workflow_candidates` 收 (run_id, script_path)，按 run_id 关联；journal_path = `session_dir/subagents/workflows/<run_id>/journal.jsonl`
- [x] 3.6 单元测试：journal 混合→Running+state；无 journal→Pending；race(全 result)→Running+全 Completed；name 剥取/runId 不一致/scriptPath None；已完成(manifest 存在)走 manifest 路径；journal 截断/垃圾/嵌套 escaped agentId/dedup 多 started

## 4. PR 6a — 前端 WorkflowCard 运行态渲染

- [x] 4.1 WorkflowCard 运行态：header `phaseSummary` 切换为 `N agents (M done)`，沿用既有 spinner + name 兜底（runId）
- [x] 4.2 `agentLabel()`：空 label → `"Agent <1-based全局序号>"`；chip status dot 沿用既有静态着色（done 绿/running 蓝），动画仅 header spinner
- [x] 4.3 重构 body：running 分支（静态 phase pill 列表 + 扁平匿名 chips，因合成 agent 无真实 phaseIndex）/ 完成态分支（phase 分组）；running+空 agents 兜底 "Running…"（修复旧逻辑 running+有agents+无phases 落空 `{#each phases}` 渲染空 body 的 gap）
- [x] 4.4 vitest 单测（`WorkflowCard.test.svelte.ts` 8 例）：计数 / 匿名 "Agent N" / 无假进度条 / dot 静态 / Tier1 静态 pill+扁平 / 完成态不回归；`pnpm --dir ui run check` 0 error
- [ ] 4.5 视觉验收：e2e 截图运行态 WorkflowCard（**deferred**——sandbox classifier 持续 outage 挡住 dev server 起 + 浏览器自动化；已用 8 个 DOM 级 vitest 断言精确锁定渲染（label/计数/dot/无进度条/无空白 body）兜底，infra 恢复后补截图 + impeccable critique）

## 5. PR 6b — cdt-api: script meta 静态解析（Tier 1，可选，引 json5）

- [x] 5.1 `Cargo.toml` workspace deps 加 `json5`（纯 Rust 微依赖 + justify 注释）+ cdt-api 引用
- [x] 5.2 `workflow_script.rs::extract_meta_block` 隔离 lexer：定位 `export const meta` 后第一个 `{`，平衡括号扫描（跟踪 `'`/`"`/`` ` `` 字符串、`//`/`/* */` 注释、`\` 转义、`{}` 深度）切出完整块。**坑2**：纯 Rust，无 oxc/tree-sitter
- [x] 5.3 `parse_script_meta`：切出的块喂 `json5::from_str` 取 `name` + `phases[].title`；任何失败返 `None`
- [x] 5.4 script `FileSignature` 缓存（`WorkflowManifestCache.script_entries`，缓存含解析失败结果免重复解析）
- [x] 5.5 接入 `resolve_running_state`：Tier 1 成功→`name` 优先 meta.name、`phases` 取静态列表；失败→保留 Tier 0（剥文件名 name + 空 phases）
- [x] 5.6 边界用例 fixture（`workflow_script.rs` 单测）：注释含括号 / 转义引号 / 双引号 / detail 在 title 前 / 无 phases / 单行 minified / 嵌套对象 / backtick 值降级 / 截断不配平 / 无 anchor。**有意偏离**：不拷贝 13 个真实 script（含用户 home 绝对路径 + prompt 内容，隐私 + ~180KB 噪声 + classifier 挡读），改用镜像真实 meta 形态的 2 个代表性 fixture（`assess-workflow-migration` 多行形态 + `explore-workflow-rendering` 含 whenToUse 形态）
- [x] 5.7 单元测试：每个边界用例稳健（每个 SHALL 句配测试，14 例 in workflow_script + 2 例 Tier1 in workflow_manifest）；前端 Tier 1 静态 phase pill 列表渲染（无当前 phase 高亮，vitest 覆盖）

## 6. PR 6c — script payload 泄漏核实（条件触发）

- [x] 6.1 核实结论：(a) cdt-core `WorkflowItem` **不含** script 字段（`scriptPreview` 仅 TS 侧定义，后端从不填充）——WorkflowItem 路径不泄漏；(b) `ToolExecution.input` **无 OMIT**（OMIT 仅覆盖 image data / response content / tool **output** / subagent messages，见 `local.rs::apply_all_payload_omissions` phase 3-5）——**inline 形态** Workflow（`{script, description}`）的 ~14KB script 经 `tool_use.input.script` 进入前端 payload
- [x] 6.2 决策：**不在本 PR 做**。理由：(1) 泄漏仅 inline 形态（13 个真实 workflow 全是 scriptPath 形态，0 命中）；(2) 该 input **不被渲染**（Workflow tool_use 走 WorkflowCard，用 `scriptPreview` 而非 `input.script`），纯 payload bloat 非正确性问题；(3) **pre-existing**，非本 change 引入；(4) 与运行态降级正交。**另开独立瘦身 PR / Issue 跟踪**（OMIT_WORKFLOW_INPUT_SCRIPT 模式：inline script 走 lazy IPC）。本 change PR 描述会点名此 finding

## 7. 验证

- [x] 7.1 `cargo clippy --workspace --all-targets -- -D warnings` 通过（0 warning）
- [x] 7.2 `cargo test --workspace` 全量通过（exit=0，61 个 test result: ok block，0 失败，含 cdt-query 构造点测试）
- [x] 7.3 `pnpm --dir ui run check` 0 error（1 个 pre-existing Connection.svelte 警告）；WorkflowCard vitest 8 例 + displayItem workflow 6 例全绿
- [x] 7.4 `openspec validate workflow-running-degradation --strict` 通过
- [x] 7.5 perf 实测：`CDT_PERF_USE_FIXTURE=1 bash scripts/run-perf-bench.sh --bench perf_get_session_detail`（min-of-5）→ **✓ 无回归**：wall=170ms（base 500ms，-66%）/ user=80ms（base 250ms，-68%）/ sys=30ms / max_rss=32MB（-76%）/ user/real=0.47。门控由 `resolve_single_prefers_manifest_when_present` 单测 + 早退（`candidates.is_empty()`）保证无 workflow/已完成 workflow 零增量

## N. 发布

- [x] N.1 push 分支 + 开 PR（PR #412，贴 Perf impact：门控说明 + 运行态降级路径）
- [x] N.2 wait-ci 全绿（15 项 SUCCESS）
- [x] N.3 codex 二审通过（**codex 额度耗尽至 5/31**，改用本仓 silent-failure-hunter + feature-dev:code-reviewer 异构替代二审；核实后修复 4 处本 PR 新增缺陷——manifest/journal stat 失败误判运行态 + read 失败静默吞 + Tier1 read 失败无日志 + 前端 agentLabel O(n²)——加固 1 测试；pre-existing `failed_by_heuristic` 误判记 Issue #413。用户批准接受替代二审）
- [x] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

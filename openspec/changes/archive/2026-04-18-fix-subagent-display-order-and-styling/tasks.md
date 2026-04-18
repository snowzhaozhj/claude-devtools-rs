## 1. 后端 `cdt-analyze` — SubagentSpawn 插入位置

- [x] 1.1 修改 `crates/cdt-analyze/src/chunk/builder.rs::attach_subagents_to_chunks`：用 `position()` 找 `tool_use_id == rt.task_use_id` 的 `SemanticStep::ToolExecution`，改 `push` 为 `insert(idx + 1, SubagentSpawn{..})`；未找到时 fallback append 并 `tracing::warn!`
- [x] 1.2 新增单测 `subagent_spawn_inserted_after_matching_task_step`（构造 Read → Task → Grep + 1 resolved subagent，断言顺序）
- [x] 1.3 新增单测 `multiple_tasks_each_get_spawn_inserted_after_own_task`（两个 Task 各自匹配 subagent）+ `orphan_task_emits_no_subagent_spawn`
- [x] 1.4 `cargo test -p cdt-analyze` 通过
- [x] 1.5 `cargo clippy -p cdt-analyze --all-targets -- -D warnings` 通过

## 2. 前端 `displayItemBuilder.ts` — Task 去重

- [x] 2.1 `ui/src/lib/displayItemBuilder.ts::buildDisplayItems`：构建 `taskIdsWithSubagents`（`chunk.subagents.map(s => s.parentTaskId).filter(Boolean)`）；遍历 `semantic_steps` 碰到 `tool_execution` 且 `exec.toolName === "Task"` 且集合包含 `exec.toolUseId` 时跳过
- [x] 2.2 对 `buildDisplayItemsFromChunks`（subagent 嵌套场景）自动继承（它逐个 AIChunk 调 `buildDisplayItems`）
- [x] 2.3 确认 `SubagentProcess.parentTaskId` 已经从 IPC 透传（`ui/src/lib/api.ts:147` 已存在）

## 3. 前端 `SubagentCard.svelte` — Badge + 模型名

- [x] 3.1 新建 `ui/src/lib/modelParser.ts`，实现 `parseModelString(raw): { family, name, majorVersion, minorVersion }`（对齐原版 `/Users/zhaohejie/RustroverProjects/claude-devtools/src/shared/utils/modelParser.ts` 行为）
- [x] 3.2 `SubagentCard.svelte::badgeLabel`：非 team 时固定返回 `"TASK"`；保留 `showBadgeDot` 逻辑（颜色 via `getSubagentTypeColorSet`）
- [x] 3.3 `SubagentCard.svelte::modelName`：改用 `parseModelString(r.model).name`，过滤 `<synthetic>`
- [x] 3.4 中性 badge 路径（无类型）也显示 `TASK`（`sa-badge-neutral` 从 `Task` 改 `TASK`）
- [x] 3.5 Execution Trace header 加 Terminal SVG 图标（复用 `lib/icons.ts::TERMINAL`）

## 4. 验证

- [x] 4.1 `npm run check --prefix ui` 通过（0 errors、5 条预先存在无关 warning）
- [x] 4.2 `cargo test --workspace` 通过（cdt-analyze 94 passed，全 workspace 绿）
- [x] 4.3 `cargo clippy --workspace --all-targets -- -D warnings` 通过
- [x] 4.4 `openspec validate fix-subagent-display-order-and-styling --strict` 通过
- [x] 4.5 `just dev` 启动桌面应用，打开含 Task + subagent 的会话，人工对照：顺序交错、TASK badge、`haiku4.5` 模型名；对比图 1 / 图 2 截图（用户 archive 时确认通过）

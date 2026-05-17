## 1. cdt-analyze（后端 aiGroupId 对齐 chunkId）

- [x] 1.1 改 `crates/cdt-analyze/src/context/session.rs::ai_chunk_id`：函数体直接 `ai.chunk_id.clone()`，删除 `responses[0].uuid` 与 `ai-<turn_index>` 两条 fallback；**只删 `ai_chunk_id` 自身的 `turn_index` 形参**（函数签名变为 `fn ai_chunk_id(ai: &AIChunk) -> String`），保留 caller 处的 `turn_index` 局部变量
- [x] 1.2 grep `ai_group_id` / `aiGroupId` 全 workspace：更新 `crates/cdt-analyze/tests/context_tracking.rs` 等所有断言点，改用 `chunk_id` 的新统一形态（`<base>:<n>`）
- [x] 1.3 `cargo test -p cdt-analyze` + `cargo test -p cdt-core` 全绿
- [x] 1.4 `crates/cdt-api/tests/ipc_contract.rs`：如有 round-trip 测断言 `aiGroupId` 形态，同步改；新增 `ai_group_id_equals_chunk_id` round-trip test 锁定字段语义

## 1b. cdt-api / Tauri（SessionDetail 新增 phaseInfo + injectionsByPhase）

- [x] 1b.1 `crates/cdt-api/src/ipc/types.rs::SessionDetail` 新增字段 `pub injections_by_phase: serde_json::Value`（默认 null，`#[serde(skip_serializing_if = "Value::is_null")]`）与 `pub phase_info: serde_json::Value`（同上），保持 camelCase 透传
- [x] 1b.2 `crates/cdt-api/src/ipc/local.rs::get_session_detail` 填充：`injections_by_phase` 通过遍历 `ctx_result.phase_info.phases` 取每 phase `last_ai_group_id` 在 `stats_map` 中的 `accumulated_injections`，构造 `Map<phaseNumber.to_string(), Vec<ContextInjection>>`；`phase_info` 直接 `serde_json::to_value(&ctx_result.phase_info)`；原 `context_injections` 字段保持等于 `injections_by_phase[最大 phaseNumber]`（即 latest phase）。`get_sessions_by_ids` 两处占位 fallback 也补齐新字段
- [x] 1b.3 `crates/cdt-api/tests/ipc_contract.rs` 新增 3 个 round-trip test：`session_detail_single_phase_injections_by_phase_equals_context_injections` / `session_detail_multi_phase_preserves_phase1_injections` / `chunk_id_format_is_unified_base_colon_n`
- [x] 1b.4 `ui/src/lib/api.ts::SessionDetail` 接口加 `phaseInfo?: ContextPhaseInfo` + `injectionsByPhase?: Record<string, ContextInjection[]>`（optional 兼容老后端），并新增 `ContextPhaseInfo` 类型定义
- [x] 1b.5 Tauri command 透传无需改（`SessionDetail` 已是 serde_json::Value 路径，新字段自动透传）；但 `src-tauri/src/lib.rs` 若有显式字段引用需 grep 同步

## 1c. chunk_id 形态统一（删 `ai:` 前缀 + 删 "首次裸 uuid" 特例，详见 design D1b）

- [x] 1c.1 `crates/cdt-analyze/src/chunk/builder.rs`：合并 `next_ai_chunk_id` + `next_non_ai_chunk_id` 为单一 `next_chunk_id(base, used_set) -> String`，永远返回 `format!("{base}:{n}")`
- [x] 1c.2 更新 builder.rs 内 3 个 chunk_id 形态断言测试（`duplicate_assistant_response_uuid_gets_stable_unique_chunk_ids` / `duplicate_user_uuid_gets_stable_unique_chunk_ids` / `user_uuid_collides_with_suffix_form_still_unique`）的 expected 值
- [x] 1c.3 `crates/cdt-analyze/src/context/session.rs`：`current_phase_compact_group_id = Some(compact.chunk_id.clone())`（原用 `compact.uuid`），让 compact_group_id 与 chunk_id 一致以供 ContextPanel 反查 delta
- [x] 1c.4 `crates/cdt-core/src/chunk.rs::tests` 硬编码 chunk_id 字面值：`"u1"` → `"u1:0"`、`"ai:a1:0"` → `"a1:0"`、`"s1"` → `"s1:0"`、`"c1"` → `"c1:0"`；JSON round-trip fixture 同步
- [x] 1c.5 `crates/cdt-core/src/context.rs` 测试中 `ai_group_id: "ai-0"` → `"ai-0:0"`
- [x] 1c.6 `cargo test --workspace` 全绿
- [x] 1c.7 spec delta `openspec/changes/context-panel-redesign/specs/ipc-data-api/spec.md` 写完（MODIFIED "Stable chunk identifiers in SessionDetail" + 6 scenario）

## 2. UI 数据层（contextExtractor 重写）

- [x] 2.1 `ui/src/lib/contextExtractor.ts`：删除 `ContextEntry` / `ContextCategory` 压扁类型；改为直接 `export type ContextInjection = ClaudeMd | MentionedFile | ToolOutput | ThinkingText | TaskCoordination | UserMessage` 的判别联合，保留所有后端字段（`turnIndex` / `aiGroupId` / `toolBreakdown`（Tool）/ `breakdown`（ThinkingText / TaskCoordination 共用此名）/ `textPreview` / `scope` / `displayName` / `path` / `firstSeenTurnIndex` / `exists` 等）
- [x] 2.2 `extractContext(detail)` 改为 `parseInjections(raw): ContextInjection[]`——只做类型 narrow，不做信息丢失变换
- [x] 2.3 保留 `CATEGORY_COLORS` 表（Ranked / chip 用）；新增 `groupClaudeMdByScope(injections): { global, project, directory }` 辅助函数

## 3. UI 组件层（拆 6 个 Section）

- [x] 3.1 新建目录 `ui/src/components/contextPanel/`，从 `ContextPanel.svelte` 抽出共享 `CollapsibleSection.svelte`（folded chevron + header + token 计数 + toggle）
- [x] 3.2 新建 `UserMessagesSection.svelte`：渲染 `UserMessageInjection[]`，每行 `Turn <turnIndex>` + `textPreview` + `~<tokens>`；按 `turnIndex` 升序
- [x] 3.3 新建 `ClaudeMdFilesSection.svelte`：按 `scope` 分 Global / Project / Directory 三组，每组内套现有 `DirectoryTree` 组件；空组不渲染
- [x] 3.4 新建 `MentionedFilesSection.svelte`：渲染 `MentionedFileInjection[]`，每行 displayName + path tooltip + tokens + `exists` 标记；按 tokens 降序
- [x] 3.5 新建 `ToolOutputsSection.svelte`：每条 `ToolOutputInjection` 显示 `Turn <turnIndex>` header + 展开后列 `toolBreakdown` 每个 tool 一行（toolName + tokenCount + isError chip），单条 tool 行可点击
- [x] 3.6 新建 `ThinkingTextSection.svelte`：每条 `ThinkingTextInjection` 显示 `Turn <turnIndex>` + 展开后两行 thinking / text 各自 token
- [x] 3.7 新建 `TaskCoordinationSection.svelte`：每条 `TaskCoordinationInjection` 显示 `Turn <turnIndex>` + 展开后列 `breakdown` 每个 item（label + kind chip + tokens）
- [x] 3.8 新建 `PhaseSelector.svelte`：接 `phases: ContextPhase[]` + `selected: number | null` + `onChange`，仅 `phases.length > 1` 渲染下拉
- [x] 3.9 重写 `ui/src/components/ContextPanel.svelte`：作为 orchestrator，按 phase 过滤、按 viewMode 切换、把 Section 串起来；新增 props `onNavigateToChunk(chunkId)` + `onNavigateToTool(chunkId, toolUseId)`

## 4. UI 锚点导航（SessionDetail 配合）

- [x] 4.1 `ui/src/routes/SessionDetail.svelte`：每个 chunk 容器（user / ai / system / compact）加 `data-chunk-id={chunk.chunkId}` 属性
- [x] 4.2 AIChunk 内每个 `ToolExecution` 渲染节点加 `data-tool-use-id={exec.toolUseId}` 属性
- [x] 4.3 实现 `handleNavigateToChunk(chunkId)`：`expandedChunks = new Set([...expandedChunks, chunkId])`（新建 Set 触发 Svelte 5 响应式）→ `await tick()` → `root.querySelector(\`[data-chunk-id="\${chunkId}"]\`)?.scrollIntoView({ block: "center", behavior: "smooth" })`
- [x] 4.4 实现 `handleNavigateToTool(chunkId, toolUseId)`：先调 `handleNavigateToChunk(chunkId)` → 再 `await tick()` → 再 querySelector `[data-tool-use-id="..."]` 滚到 tool（**不要**用 `setTimeout`，理由见 design D3）
- [x] 4.5 实现 `handleNavigateToUserGroup(aiGroupId)`：在 `detail.chunks` 找 `chunkId == aiGroupId` 的 AIChunk index，往前找最近的 `kind == "user"` 的 chunk，navigate 到它；找不到则 fallback navigate to AIChunk
- [x] 4.6 把三个 handler 当 props 传给 `<ContextPanel>`

## 5. Ranked 视图 + Phase 过滤

- [x] 5.1 `ContextPanel.svelte` 内 Ranked 视图加 Grouped/Flat 子切换（默认 Grouped），并接管现有 ranked 渲染
- [x] 5.2 实现 `selectActivePhaseInjections(detail, selectedPhase)`：`selectedPhase == null` 返回 `detail.injectionsByPhase?.[latestPhaseNumber] ?? detail.contextInjections ?? []`（latest fallback 链）；否则返回 `detail.injectionsByPhase?.[String(selectedPhase)] ?? []`
- [x] 5.3 选中 phase 但 `injectionsByPhase[N]` 为空或 undefined 时 SHALL 显示占位文案"本 phase 无 injection"；Header `Visible: ~Xk tokens` 按当前过滤后 injections 计算

## 6. Fixtures 与样式

- [x] 6.1 `ui/src/lib/__fixtures__/multi-project-rich.ts`：扩展 `contextInjections` 字段加完整 6 类样例（Tool 用 `toolBreakdown`，ThinkingText / TaskCoordination 用 `breakdown`，UserMessage 用 `textPreview` 等）；至少一个 fixture 含 2 phase + 同步填 `phaseInfo` + `injectionsByPhase`（key `"1"` 与 `"2"` 各一份）触发 Phase Selector
- [x] 6.2 复用 `app.css` CSS 变量（`--color-surface-raised` / `--color-text-muted` / `--color-border-subtle` 等），不引入新颜色 token；Section header 复用现有 `.cp-section-header` 视觉骨架但改用语义化 class 名

## 7. 测试

- [x] 7.1 Vitest 单测 `ui/src/lib/contextExtractor.test.ts`：覆盖 6 类 narrow + `groupClaudeMdByScope` + `selectActivePhaseInjections`（Latest fallback / 具体 phase / 空 phase 三种路径）
- [x] 7.2 Vitest 单测 `ui/src/components/contextPanel/__tests__/ContextPanel.test.ts`：mockIPC + 含完整 6 类的 fixture，断言 Section 渲染数 / Section 内行数 / Phase Selector 显隐
- [x] 7.3 Playwright e2e（`ui/playwright/context-panel.spec.ts`）：用 `?mock=1&fixture=multi-project-rich` 打开 panel，点击 ToolOutputs 内 tool 行，断言主视图滚到 AIChunk 且 tool 节点 visible
- [x] 7.4 `just preflight`（fmt + lint + test + spec-validate）全绿
- [x] 7.5 `pnpm --dir ui run check` 全绿
- [x] 7.6 `pnpm --dir ui run test:unit` + `just test-e2e` 全绿

## 8. 发布

- [ ] 8.1 push 分支 + 开 PR
- [ ] 8.2 wait-ci 全绿
- [ ] 8.3 codex 二审通过（如发现 bug：修 → push → 回到 8.2 重跑；可循环 M 次）
- [ ] 8.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

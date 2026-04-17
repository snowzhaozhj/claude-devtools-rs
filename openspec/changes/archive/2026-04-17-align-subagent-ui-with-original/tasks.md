## 1. 后端：cdt-core Process 扩字段

- [x] 1.1 在 `crates/cdt-core/src/process.rs` 新增 `MainSessionImpact` 结构体（`total_tokens: u64` + camelCase serde）
- [x] 1.2 在 `Process` 上增加字段 `subagent_type: Option<String>` / `messages: Vec<Chunk>` / `main_session_impact: Option<MainSessionImpact>` / `is_ongoing: bool` / `duration_ms: Option<u64>` / `parent_task_id: Option<String>` / `description: Option<String>`，所有新字段 `#[serde(default)]`
- [x] 1.3 更新 `process_roundtrip` 单测覆盖新字段（缺省值、完整值两组）
- [x] 1.4 `cargo test -p cdt-core` + `cargo clippy -p cdt-core --all-targets -- -D warnings`

## 2. 后端：cdt-analyze resolver 填充

- [x] 2.1 在 `crates/cdt-analyze/src/tool_linking/pair.rs`（或 resolver.rs）里新增 helper：`compute_duration_ms` / `compute_is_ongoing` / `extract_subagent_type_from_task_input` / `aggregate_main_session_impact`
- [x] 2.2 扩展 `resolve_subagents` 在返回 `Resolution::*` 的同时填充 `Process.subagent_type / messages / main_session_impact / is_ongoing / duration_ms / parent_task_id / description`
- [x] 2.3 `messages` 字段：调用 `cdt_analyze::chunk::build_chunks(&subagent_parsed_messages)` 生成（需要在 resolver 输入里拿到 subagent session 的 `ParsedMessage`）
- [x] 2.4 `parent_task_id` 回填：resolver 匹配成功时记录触发匹配的 Task/Agent tool_use_id，写入 `Process`
- [x] 2.5 新增 resolver 单测：`subagent_type_populated_from_task_input` / `parent_task_id_backfilled_on_match` / `duration_ms_computed_from_spawn_and_end` / `is_ongoing_true_when_no_end_ts` / `main_session_impact_aggregates_task_result_usage`
- [x] 2.6 `cargo test -p cdt-analyze` + `cargo clippy -p cdt-analyze --all-targets -- -D warnings`

## 3. 后端：agent-configs capability

- [x] 3.1 在 `crates/cdt-discover/src/agent_configs.rs` 新建模块；定义 `AgentConfig`（含 `name / color / description / scope / file_path`，camelCase serde）与 `AgentConfigScope { Global, Project(String) }`
- [x] 3.2 实现 `scan_global() -> Vec<AgentConfig>`：展开 `~/.claude/agents/*.md`，目录缺失返回空
- [x] 3.3 实现 `scan_project(project_id, cwd) -> Vec<AgentConfig>`：扫 `cwd/.claude/agents/*.md`
- [x] 3.4 实现手写 frontmatter 解析器 `parse_frontmatter(content: &str) -> (HashMap<String, String>, body)`：处理 `---...---`、`key: value`、去引号、跳过非法行；无 frontmatter 时 fallback 文件名
- [x] 3.5 实现聚合入口 `read_agent_configs(projects: &[(String, String)]) -> Vec<AgentConfig>`：汇总 global + 每个 project 级结果，按 `(scope_global_first, name)` 排序
- [x] 3.6 单测：`scans_global_only` / `scans_project_only` / `both_scopes_merged_and_sorted` / `parse_quoted_color` / `parse_missing_fields_defaults_to_none` / `parse_no_frontmatter_uses_filename` / `parse_invalid_line_skipped` / `missing_dir_returns_empty`
- [x] 3.7 `cargo test -p cdt-discover` + `cargo clippy -p cdt-discover --all-targets -- -D warnings`

## 4. 后端：cdt-api 对外暴露

- [x] 4.1 在 `crates/cdt-api/src/ipc/traits.rs` 的 `DataApi` trait 里**不加**此方法（保持原版风格：trigger CRUD 也是非 trait 方法）——改为在 `LocalDataApi` 的独立 `impl` 块直接加 `pub async fn read_agent_configs(&self) -> Result<Vec<AgentConfig>, ApiError>`
- [x] 4.2 实现：收集 `scanner.list_projects()` 中每个项目的 cwd（调 `ProjectPathResolver`），调用 `cdt_discover::agent_configs::read_agent_configs`
- [x] 4.3 在 `src-tauri/src/lib.rs` 新增 Tauri command `read_agent_configs`，透传结果为 `Vec<serde_json::Value>`
- [x] 4.4 在 `tauri::Builder` 的 `invoke_handler` 中注册新 command
- [x] 4.5 集成测试：`crates/cdt-api/tests/agent_configs.rs` 准备 tmp 目录 fixture，验证 LocalDataApi 聚合结果
- [x] 4.6 `cargo test -p cdt-api` + `cargo clippy -p cdt-api --all-targets -- -D warnings`

## 5. 前端：颜色与 agent configs store

- [x] 5.1 新建 `ui/src/lib/teamColors.ts`：移植原版 `shared/constants/teamColors.ts`，导出 `getTeamColorSet` 与 `getSubagentTypeColorSet`（两个函数合并在一个文件，对齐原版 8 色调色板）
- [x] 5.2 调色板改为 **8 色**（与原版一致），实现 hash 兜底（djb2 风格）
- [x] 5.3 新建 `ui/src/lib/agentConfigsStore.svelte.ts`：`$state` 数组 + `loadAgentConfigs()`（调 `invoke("read_agent_configs")`）+ 单例加载
- [x] 5.4 在 `ui/src/App.svelte` onMount 中调用 `loadAgentConfigs()` 一次
- [x] 5.5 新建 `ui/src/lib/formatters.ts`：`formatDuration(ms: number | null)` / `formatTokensCompact(n: number)` 对齐原版
- [x] 5.6 `npm run check --prefix ui`

## 6. 前端：MetricsPill 与 ExecutionTrace 组件

- [x] 6.1 新建 `ui/src/components/MetricsPill.svelte`：props `{ mainTokens?: number; isolatedTokens?: number; isolatedLabel?: string }`；根据入参渲染 0/1/2 个槽位
- [x] 6.2 扩展 `ui/src/lib/displayItemBuilder.ts`：新增 `buildDisplayItemsFromChunks(chunks: Chunk[])`，从 subagent `Process.messages`（已是 Chunk 流）产出 DisplayItem（复用现有 buildDisplayItems 逻辑，把多个 AI chunk 的步骤串联）
- [x] 6.3 新建 `ui/src/components/ExecutionTrace.svelte`：props `{ items: DisplayItem[]; depth?: number }`；遍历渲染 thinking/tool/output/slash/subagent 五种 item；`item.type === "subagent"` 时递归调用自身 + `<SubagentCard>`，限深度 ≤ 8
- [x] 6.4 ExecutionTrace 内嵌套 subagent 的展开状态用 Svelte 5 `$state` 独立维护，不污染外层
- [x] 6.5 `npm run check --prefix ui`

## 7. 前端：重写 SubagentCard

- [x] 7.1 删除 `SubagentCard.svelte` 中的 `navigateToSession()` 与 "Open Session" 按钮及相关 DOM
- [x] 7.2 从 `lib/api.ts` 的 `SubagentProcess` 类型扩展新字段（`subagentType / messages / mainSessionImpact / isOngoing / durationMs / parentTaskId / description`，camelCase）
- [x] 7.3 重写 Header：chevron + 彩色圆点（通过 teamColors/subagentTypeColors）+ badge（team memberName 或 subagentType，fallback "Task"）+ model 文本（如有）+ description（truncated 60 字符）+ status icon（isOngoing → Loader2 旋转 / 否则 CheckCircle2）+ `<MetricsPill>` + duration 文本
- [x] 7.4 Dashboard 区（仅 isExpanded 时渲染）：meta 行（Type / Duration / Model? / ID）+ Context Usage 列表（Main Context / Subagent Context 两条）
- [x] 7.5 Execution Trace 折叠块（仅 isExpanded 且 `messages.length > 0` 时渲染）：自身折叠头 + 展开后渲染 `<ExecutionTrace items={buildDisplayItemsFromChunks(process.messages)} />`
- [x] 7.6 特例处理：team 成员仅有 shutdown_response 时渲染极简内联行（border + 圆点 + badge + "Shutdown confirmed" + duration），不可展开
- [x] 7.7 所有 CSS 使用 var(--card-bg/border/...) token，对齐原版 Linear-style；圆角 6px、border 1px
- [x] 7.8 在 `ui/src/routes/SessionDetail.svelte` 的 subagent 渲染调用点移除对 `parentProjectId` prop 的依赖（不再需要打开新 tab，projectId 仅用于 ExecutionTrace 嵌套场景的引用透传）
- [x] 7.9 `npm run check --prefix ui`

## 8. 集成验证

- [x] 8.1 `cargo build --workspace` 确认全部 crate 编译
- [x] 8.2 `cargo test --workspace` 全量跑一遍
- [x] 8.3 `cargo clippy --workspace --all-targets -- -D warnings`
- [x] 8.4 `cargo fmt --all`
- [x] 8.5 `cargo tauri dev` 启动应用，打开一个含真实 subagent 的 session，逐项目视验证：卡片折叠/展开、彩色 badge、MetricsPill、Dashboard、ExecutionTrace 展开、嵌套 subagent 递归、team 成员 shutdown-only 特例
- [x] 8.6 `openspec validate align-subagent-ui-with-original --strict`
- [x] 8.7 在 `openspec/followups.md` 标注本 change 修复的关联条目（subagent UI 对齐），并更新 CLAUDE.md "UI 已知遗留问题" 章节

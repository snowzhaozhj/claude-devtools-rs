## Why

Rust 端当前 `SubagentCard.svelte` 自创了"点击打开新 tab"交互，但 subagent session 的 JSONL 常不在 parent project 下，点开后页面空白；并且原版 `claude-devtools/src/renderer/components/chat/items/SubagentItem.tsx` 根本没有打开新 tab 的设计——它是**当前 tab 内联展开 ExecutionTrace**，配合 MetricsPill、按 subagentType 的彩色 badge、agentConfigs 颜色查询等一套完整视觉。视觉与交互差距的根源是后端 `cdt-core::Process` 数据模型过于简化（缺 `messages / subagentType / mainSessionImpact / isOngoing / durationMs / parentTaskId`），让前端无法还原。

## What Changes

- **BREAKING** `cdt-core::Process` 新增字段：`subagent_type: Option<String>`、`messages: Vec<ParsedMessage>`（或轻量投影）、`main_session_impact: Option<MainSessionImpact>`、`is_ongoing: bool`、`duration_ms: Option<u64>`、`parent_task_id: Option<String>`、`description: Option<String>`（独立于 `root_task_description`）
- `cdt-analyze::tool_linking::resolver::resolve_subagents` 扩展填充新字段；新增辅助函数从 subagent session 的 `ParsedMessage` 流计算 `main_session_impact` / `duration_ms` / `is_ongoing`
- 新增 `cdt-discover::agent_configs` 模块：扫描 `~/.claude/agents/*.md` 与项目级 `.claude/agents/*.md`，解析 frontmatter 中的 `color`、`description`、`name`
- `cdt-api::LocalDataApi` 新增 `read_agent_configs() -> Vec<AgentConfig>` 方法；`src-tauri` 新增 `read_agent_configs` Tauri command
- 前端删除 `SubagentCard.navigateToSession()` 和"Open Session"按钮，改为**内联展开** ExecutionTrace（原版 Linear-style 三层结构：Header / Dashboard / Trace）
- 新增前端模块：`ui/src/lib/teamColors.ts`、`ui/src/lib/subagentTypeColors.ts`（含哈希兜底）、`ui/src/lib/agentConfigsStore.svelte.ts`、`ui/src/components/MetricsPill.svelte`、`ui/src/components/ExecutionTrace.svelte`
- 重写 `ui/src/components/SubagentCard.svelte`：Header（chevron/dot/badge/model/desc/status/MetricsPill/duration）+ Dashboard（meta + Context Usage）+ ExecutionTrace 可递归展开嵌套 subagent
- UI 行为变更：subagent 不再打开新 tab，改为在当前 AI 组内内联展开执行链

## Capabilities

### New Capabilities
- `agent-configs`: 扫描并解析 `.claude/agents/*.md` 文件，提供 subagent 类型 → 颜色/描述的查找服务

### Modified Capabilities
- `tool-execution-linking`: `Process` 结构体新增 `subagentType / messages / mainSessionImpact / isOngoing / durationMs / parentTaskId / description` 字段，由 `resolve_subagents` 填充
- `ipc-data-api`: 把已列出的 "Read agent configs" requirement 具化为 Rust 侧可执行的契约（新增 `read_agent_configs` Tauri command + 返回类型）
- `session-display`: 新增 "Subagent 内联展开 ExecutionTrace" Requirement 与关联 Scenarios（替代当前"打开新 tab"的隐式行为）
- `session-parsing`: 反转 "Deduplicate streaming entries by requestId" —— 在 Claude Code 新 JSONL 格式下，同 requestId 是同次 API response 的 grouping key，盲 dedupe 会丢含 `tool_use` 的 assistant 记录；改为不在 `parse_file` 主路径上自动去重

## Impact

- 代码：`crates/cdt-core/src/process.rs`（扩字段）、`crates/cdt-analyze/src/tool_linking/resolver.rs`（填充逻辑）、`crates/cdt-discover/src/`（新增 `agent_configs.rs`）、`crates/cdt-api/src/ipc/{traits,local,http}.rs`（新 API）、`src-tauri/src/lib.rs`（新 command）、`ui/src/lib/*`（新 store/颜色/工具）、`ui/src/components/{SubagentCard,MetricsPill,ExecutionTrace}.svelte`、`ui/src/routes/SessionDetail.svelte`（调用点）
- 依赖：可能新增 `serde_yaml`（解析 agent md frontmatter）或沿用 `gray_matter` 风格手写解析（优先手写，避免新依赖）
- 测试：`cdt-core::process` 新字段 roundtrip、`cdt-analyze` resolver 填充逻辑的新单测、`cdt-discover::agent_configs` 扫描解析单测、`cdt-api` 的新 API 集成测试
- UI：SubagentCard 旧测试需调整；ExecutionTrace 作为独立组件需新建测试场景
- 序列化契约：BREAKING — 前端缓存的 session detail 若含旧 Process shape 可能反序列化失败；新字段 `#[serde(default)]` 可保证向后兼容反序列化，但前端必须同步升级类型

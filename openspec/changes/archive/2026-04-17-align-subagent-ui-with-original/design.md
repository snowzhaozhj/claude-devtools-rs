## Context

当前 Rust 端 `SubagentCard.svelte` 的"打开新 tab"交互与空白页问题已在 proposal 中说明。原版 `claude-devtools/src/renderer/components/chat/items/SubagentItem.tsx`（约 570 行）的设计要点：

1. **三层结构**：Header（一行式卡片头，包含 chevron、彩色圆点、subagentType badge、model、description、status icon、MetricsPill、duration）→ 展开后的 Dashboard（meta 信息 + Context Usage 多维度列表）→ Execution Trace（可折叠的嵌套执行链）
2. **颜色体系**：team 成员用 `getTeamColorSet(memberColor)`；非 team subagent 用 `getSubagentTypeColorSet(subagentType, agentConfigs)`（优先查 `.claude/agents/*.md` 里 `color` frontmatter，未命中走 hash 哈希兜底）
3. **MetricsPill 多维度**：`mainSessionImpact.totalTokens`（父 session 开销，team 成员不显示）/ `lastUsage`（最新一次 usage 的 input+output+cache）/ `phaseBreakdown`（多阶段 compaction 时的 per-phase peak/post-compaction）
4. **ExecutionTrace**：从 `process.messages` 调用 `buildDisplayItemsFromMessages` 构建 DisplayItem 流，递归渲染；嵌套 subagent 递归展示
5. **特例**：team 成员若只有 shutdown_response（单次 SendMessage），渲染为极简内联行（无展开、无 metrics）

Rust 数据层 `Process` 当前只有 6 个字段，无法支持以上任何一条。前端 `SubagentCard` 的 "Open Session" 按钮实为补偿设计，不符合原版意图。

约束：
- 序列化契约必须兼容（新字段 `#[serde(default)]`）
- `cdt-core` 保持 sync；`cdt-discover` 的 agent config 扫描是文件 I/O，须 async
- 前端需递归渲染 ExecutionTrace，且嵌套 subagent 需能继续展开——组件内部自引用
- Svelte 5 runes 风格，不引入 React 残留

## Goals / Non-Goals

**Goals:**
- `Process` 结构承载原版 UI 需要的全部字段，能无损 serde camelCase 往返
- `resolve_subagents` 填充新字段，不破坏现有三阶段匹配逻辑
- `agent-configs` capability 独立，可被 UI 与其他未来 feature（例如 Settings agents 面板）复用
- Subagent 交互完全对齐原版：内联展开 → Dashboard → ExecutionTrace
- 颜色与 badge 视觉对齐原版（含 agent configs 查找、hash 兜底、team 颜色）
- 不引入新依赖（frontmatter 手写 YAML-lite 解析）

**Non-Goals:**
- 不做 ExecutionTrace 的错误高亮 / 搜索高亮 / notification color 传播（这些属于后续 `trigger-highlight-propagation` 能力，本次只预留接口）
- 不做 `aiGroupId` 相关的 Linked Tool 跨组 ID 体系（原版有但 Rust 端尚未需要）
- 不实现 `computeSubagentPhaseBreakdown` 的 compaction 阶段分解（延后到 context-tracking 补全后）——MetricsPill 本次只显示 Main + Isolated 两维
- 不改动"打开新 tab 查看任意 session" 的通用能力，仅移除 SubagentCard 里的调用点
- 不扩展原版已有但此次 port 不打算覆盖的字段：`mainSessionImpact.breakdown` 子项、`usage.cache_creation_5m` / `cache_creation_1h` 细分

## Decisions

### 1. `Process.messages` 字段类型：`Vec<Chunk>` 而非 `Vec<ParsedMessage>`

**选择**：使用已构建好的 `Vec<Chunk>`（来自 `cdt-analyze::chunk`），而非原始 `Vec<ParsedMessage>`。

**原因**：
- 前端已有完整的 `Chunk` 渲染管线（user/ai/system/compact），直接复用 ExecutionTrace 可以调同一个 `buildDisplayItems`
- 原版 TS 用 `ParsedMessage[]` 是因为其 displayItemBuilder 就是 message-based；Rust 端已经把这一步推进到 Chunk，下游复用成本更低
- ParsedMessage 体积大（含 raw content blocks），序列化到前端会放大 payload 2~3 倍

**代价**：需在 resolver 内部为每个 subagent session 跑一次 `build_chunks`，属于线性代价。

**替代方案**：
- A. 存 `Vec<ParsedMessage>`——放弃 chunk 构建复用，前端需新写一个 message→DisplayItem 管道
- B. 存空、加 `sessionId`，展开时前端再 `get_session_detail(subagent.sessionId)`——rejected，会退回"需要二次 IPC"的空白页老路

### 2. Agent config 扫描范围与格式

**路径**：
- 全局：`~/.claude/agents/*.md`
- 项目级：每个已发现 project 的 `cwd + /.claude/agents/*.md`（从 session JSONL 里 cwd 字段推导）

**解析**：手写 YAML frontmatter 解析器（`---\n...\n---` 之间读 `name:` / `color:` / `description:` 行），不引 `serde_yaml`。仅支持 `key: value` 简单形式；复杂嵌套忽略。

**返回类型**：
```rust
pub struct AgentConfig {
    pub name: String,
    pub color: Option<String>,
    pub description: Option<String>,
    pub scope: AgentConfigScope, // Global | Project(project_id)
    pub file_path: PathBuf,
}
```

**替代方案**：引 `gray_matter` crate。rejected——依赖约 15k LOC 只为 3 个字段，违背 CLAUDE.md 的"deps 需要 justification"。

### 3. 颜色哈希兜底算法

**选择**：移植原版 `shared/constants/teamColors.ts` 的 14 色调色板 + djb2 hash。

**原因**：必须与原版产出一致的颜色映射，否则同一 subagentType 在两端会显示不同颜色，破坏"原版对齐"目标。

**风险**：原版调色板若后续变动，需同步；本次把调色板常量锁进 `ui/src/lib/subagentTypeColors.ts`，加注释引用原版文件路径。

### 4. MetricsPill 本次只实现两维

**显示**：
- `mainSessionImpact.totalTokens`（非 team 时）
- `lastUsage.input + output + cache_read + cache_creation`（总上下文）

**省略**：`phaseBreakdown` / `cumulativeMetrics.turnCount` / Team 成员的 "Total Output"。保留字段占位，值为 `null`。

**原因**：phaseBreakdown 依赖 `computeSubagentPhaseBreakdown`，该函数依赖 compaction 边界检测，当前 context-tracking 实现中 subagent 粒度的 phase 数据尚未暴露。做 M1 minimal viable 版，M2 再补。

### 5. 前端 ExecutionTrace 递归实现

Svelte 5 组件可以自引用（import 自身），用于嵌套 subagent。递归深度理论无上限，但原版也不加限制。

**组件签名**：
```ts
<ExecutionTrace items={DisplayItem[]} projectId={string} />
```

`item.type === "subagent"` 时内部 render 一个新的 `<SubagentCard process={item.process} nested={true} />`，该 `SubagentCard` 内部再调 `<ExecutionTrace />`。`nested` prop 用于略微减小 padding 与字号。

**环检测**：实际数据不会产生环（subagent 的 messages 里再 spawn 的子 subagent 是不同的 session），但加一个 depth 上限 8，超过则只渲染 header 不递归（防御）。

### 6. `read_agent_configs` 缓存策略

**选择**：前端 store 单例缓存，mount 时加载一次，不做 mtime 重载。用户修改 agent md 文件需重启应用。

**原因**：agent configs 极少变更，原版也没实现 reload；mtime watch 会增加 file-watching 负担。后续可加手动 "Reload agent configs" 按钮或接 file-watching capability。

## Risks / Trade-offs

- [**向后兼容 risk**] 旧版 Rust 端写出的 SessionDetail JSON 不含新字段 → serde `#[serde(default)]` + `Option<T>` + `Vec` 默认空即可反序列化，不会 panic
- [**payload 膨胀 risk**] `Process.messages: Vec<Chunk>` 会让 session detail JSON 显著变大（尤其 subagent 嵌套多层时） → 可在未来加按需 lazy 加载，但本次先不优化；用户反馈再做
- [**agent config 格式兼容 risk**] 手写 frontmatter 解析器遇到 `color: "#ff0000"` 与 `color: red` 要都处理；不支持多行 YAML → 仅覆盖 `key: value` 单行是原版既有行为，对齐即可
- [**颜色哈希漂移 risk**] 原版哈希算法若后续调整，Rust 端不会感知 → 在 `subagentTypeColors.ts` 注释里锁原版文件 commit hash，CI 可加一条快照对比测试
- [**递归渲染性能 risk**] 深度嵌套 subagent 会一次性渲染所有层 → Svelte 的 `{#if isExpanded}` 默认关闭，只在用户展开时才渲染子层；性能可控
- [**breaking serde change**] 若任何下游消费者（如 HTTP API 客户端）基于旧 shape 反序列化且没有宽容字段处理，会失败 → 本仓库内 HTTP client 不存在；外部无消费者

## Migration Plan

1. **后端扩 Process**：加字段 + `#[serde(default)]` + 单测。既有数据读取全部保持兼容（新字段默认值）。
2. **后端 resolver 填充**：实现新字段计算，单独单测。
3. **后端 agent-configs capability**：`cdt-discover::agent_configs` + `LocalDataApi::read_agent_configs` + Tauri command，独立单测与 IPC roundtrip 测试。
4. **前端颜色/store/MetricsPill**：无副作用的纯前端模块，单独类型检查通过即可。
5. **前端 ExecutionTrace**：作为通用组件先落地，用 mock data 验证递归。
6. **前端 SubagentCard 重写**：删旧行为 + 接新字段 + 接 store + 接 ExecutionTrace。
7. **集成验证**：`cargo tauri dev` 跑通真实 subagent session，检查颜色/metrics/trace 与原版截图一致。

**回滚**：任一步失败可单独回滚；后端改动是严格加字段，不影响既有功能；前端改动局限于 SubagentCard + 新增文件。

## Open Questions

- Team 成员 `cumulativeMetrics.turnCount` 的计算口径（assistant 消息且有 usage 的数量）是否在本次实现？—— 暂定不实现，Dashboard 里先用 `null` 占位。
- `parentTaskId` 是否在 resolver 可直接取到？原版把 Task 的 `tool_use.id` 挂到 Process 上；Rust 端 resolver 目前只返回 `Resolution`，需要在 `resolve_subagents` 返回值里记录 Task tool_use_id。—— 设计为：扩 `AIChunk.subagents` 的回填逻辑，把匹配到的 Task 的 `tool_use_id` 写入 `Process.parent_task_id`。
- Agent config 扫描是否需要 SSH 远程支持？—— 本次只扫本机路径；SSH 下 agent configs 暂不读取（degrade gracefully，颜色走 hash 兜底）。

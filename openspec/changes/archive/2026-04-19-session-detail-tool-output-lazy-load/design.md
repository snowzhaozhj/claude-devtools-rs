## Context

Phase 2/3/4 已把 subagent.messages / image.data / responses[].content 三大字段裁掉。phase 4 实测 IPC time 没按 payload 线性下降——`payload 砍 40% IPC 只砍 13%`。原因是 V8 `JSON.parse` 大对象 + Tauri webview baseline 都是固定/非线性开销，**chunk 数量 96 没变**，object graph 拓扑没动。

剩余 1668 KB 内 `tool_exec.output` 占 26%（436 KB）。`output` 是 OMIT 性价比最高的剩余字段——因为：
1. ExecutionTrace 默认折叠（`expandedKeys = $state(new Set())`，每个 tool 用户点击才展开）
2. 折叠状态下 `getToolSummary(toolName, input)` 只读 input 不读 output
3. 5 个 ToolViewer（Read/Edit/Write/Bash/Default）只在展开后渲染，才需要 output

## Goals / Non-Goals

**Goals:**

- 把所有 `tool_executions[].output` 内 text/value 字段从 `get_session_detail` 默认 IPC payload 中裁掉
- 提供按需懒拉的 IPC `get_tool_output(rootSessionId, sessionId, toolUseId)`
- 前端 ExecutionTrace 在用户点击展开时按需拉，与 SubagentCard 同模式
- 保留 `output` enum variant 形状（`Text` / `Structured` / `Missing` 不变），只清空 inner 字段

**Non-Goals:**

- 不裁 `tool_exec.input`：header summary 需要 `file_path` / `command` 等小字段。要 OMIT input 必须按 toolName 字段级裁剪（per-tool whitelist），复杂度高，留下下轮。
- 不改 ExecutionTrace 默认折叠状态——它本来就折叠，懒拉自然贴合。
- 不做主动预加载（不在 chunk 进视口时预 prefetch tool output）——观察用户实际点击行为后再优化。
- 不改 HTTP path（与 phase 2/3/4 同分叉）。

## Decisions

### 决策 1：`output` 内 text/value 清空，不改 enum 形状

**选择**：保留 `ToolOutput::Text { text }` / `Structured { value }` / `Missing` 三 variant；OMIT 时清空 `text = ""` / `value = Null`，不改 enum 结构。

**替代方案 A：加 `Omitted { kind: String }` 第四 variant**
- 优点：语义清晰
- 缺点：5 个 ToolViewer + tool routing 都要兼容新 variant；破坏 cdt-core 公共 API

**替代方案 B：直接把 output 改成 `Option<ToolOutput>`**
- 缺点：序列化/反序列化语义变化太大；大量构造点要改

**理由**：phase 4 同款保守选择——derived flag + 内部清空，不动数据形状。前端 viewer 完全兼容（拿到空字符串就显示"加载中..."等懒拉返回）。

### 决策 2：用 `tool_use_id` 定位，不用 chunk uuid + index

**选择**：`get_tool_output(rootSessionId, sessionId, toolUseId)`。后端按 sessionId 找 jsonl，parse → 在 `tool_executions` 流中线性 scan 找 `tool_use_id` 匹配的 ToolExecution，返回其 `output`。

**替代方案**：`(chunkUuid, toolIndex)` 复合 key（与 image phase 3 同模式）。
- 缺点：tool_use_id 已经是全 session 唯一稳定 ID，没必要绕弯。

**理由**：`tool_use_id` 在 build_chunks 后由 ToolExecution 直接持有，前端 `exec.toolUseId` 可直接拿到，最简单。

### 决策 3：失败 fallback 返回 `ToolOutput::Missing` 不报错

**选择**：jsonl 不存在 / id 找不到 / parse 失败 → 返回 `Missing`。

**理由**：UI 拿到 `Missing` 已有显示分支（"orphan tool_use 无 result"），可用性 > 报错。

### 决策 4：OMIT 顺序 image → response.content → tool_output → subagent

**理由**：
- image / response.content / tool_output 三个递归覆盖 subagent.messages 嵌套层（保证 `OMIT_SUBAGENT_MESSAGES=false` 回滚路径下嵌套层也被裁）
- subagent OMIT 最后跑（直接清空 messages，前面三个对 cleared messages 是 no-op）

### 决策 5：前端 outputCache 用本地 Map 不放 store

**选择**：`ExecutionTrace.svelte` 内 `let outputCache = $state(new Map<string, ToolOutput>())` per-component 状态；展开 → 检查 cache → 缺失则 IPC 拉 → 写入 cache。

**替代方案**：放 tabStore 全局 cache 跨 chunk 复用。
- 缺点：tool_use_id 全 session 唯一，跨 ExecutionTrace 实例理论上能复用，但实际同 trace 内不会重复，跨 trace 复用收益小。
- 复杂度：要 store 设计 + invalidate 逻辑

**理由**：YAGNI。先 per-component 简单实现，未来观察是否真的有跨实例复用场景再升级。

## Risks / Trade-offs

- **[trade-off] 收益按非线性折扣只有 -12% 到 -16% IPC time**：明确告知用户这是渐进改良，不是 phase 4 那种 -40% 的爽感。根治需要后端虚拟分页 / Tauri Channel binary 序列化。
- **[风险] tool 展开时首次拉 output 有延迟**：单条 output 一般 < 100 KB，IPC 单跑 < 30 ms，用户感知应可接受。如果出现明显卡顿，加 ExecutionTrace 滚动到视口时预加载（IntersectionObserver + prefetch）即可，本期不做。
- **[风险] 老前端 build + 新后端**：前端没拿到 outputOmitted 字段（默认 false）→ 直接渲染空字符串/null value——broken viewer。但 phase 4 / 3 / 2 同款风险，实际部署不会同时跨这种 build 边界。
- **[风险] `get_tool_output` 后端要重新 parse 整条 jsonl**：单次几十 ms 量级，连续展开 50 个 tool → 可能累积。优化方案：LocalDataApi 加 `LruCache<sessionId, Vec<ToolExecution>>` 短期缓存（**本期不做**，留 follow-up）。

## Migration Plan

### 部署步骤

1. **Rust 侧无破坏性 schema 变更**：`ToolExecution.output_omitted` 加 `#[serde(default)]`，老 JSON 反序列化为 `false`。
2. **前端按 `outputOmitted` 分支**：true → toggle 时调 `getToolOutput` 走新路径；false 或缺失 → 走原 `exec.output` 路径。
3. **回滚**：`OMIT_TOOL_OUTPUT: bool = false` 即恢复完整 payload。

## Open Questions

- 是否需要 in-memory `LruCache<sessionId, parsed>` 提速重复展开？— **不在本期**：先实测真实使用频率。
- 是否需要 IntersectionObserver 预加载？— **不在本期**：观察用户点击模式再定。
- `tool_exec.input` 字段级裁剪？— **下下轮**：要 per-tool whitelist 设计，复杂度上去。

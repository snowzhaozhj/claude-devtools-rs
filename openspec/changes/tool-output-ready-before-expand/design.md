## Context

`get_session_detail` IPC 默认对 `tool_executions[].output` 做 `OMIT_TOOL_OUTPUT` 裁剪（`crates/cdt-api/src/ipc/local.rs::apply_tool_output_omit`），首屏 payload 不带 output 内容。前端 `outputCache` LRU(200) 在用户展开工具时按需 `getToolOutput` IPC 拉取。

现有展开路径有两类：
1. **Read 工具**：`SessionDetail.toggle` / `ExecutionTrace.toggle` 中走 `if (exec && isReadTool(exec) && !isOutputReady(exec)) { await ensureToolOutput(exec); ... }` —— 等 output 拉到才把 key 加入 expanded set。
2. **其他工具**：直接 `expandedItems.add(key)`，再 `void ensureToolOutput(exec)` 异步注入。

Read 走第 1 类路径是因为 #69 修复"omitted Read 展开后内容跳变"——其实 Bash / DefaultToolViewer 同样依赖 `exec.output`，同样会闪：
- `BashToolViewer.svelte`：`{#if outputStr}` 包裹 OUTPUT 块，omit 时 `outputStr=""`，等回填后突然出现。
- `DefaultToolViewer.svelte`：同样 `{#if outputStr}` 包裹 OUTPUT 块。
- `EditToolViewer.svelte`：仅渲染 input 字段，与 output 无关。
- `WriteToolViewer.svelte`：仅渲染 input.content，与 output 无关。

约束：**不能引入"展开工具列表本身的卡顿"**。具体表现是若展开 AIChunk 时一次性对所有 Bash 走 prefetch，会触发并发 IPC 把响应队列挤掉首屏交互。

## Goals / Non-Goals

**Goals:**

- 把"omitted output 拉到再展开"的契约从 Read 扩展到所有 viewer 用 output 渲染的工具（Bash / Default）。
- 主 SessionDetail 与嵌套 SubagentCard ExecutionTrace 行为对齐。
- vitest 覆盖 Bash / Default 路径。

**Non-Goals:**

- **不**改 Edit / Write 的展开行为（它们不依赖 output；改了反而让用户感觉按钮无响应）。
- **不**扩展 prefetch 范围（保持 `prefetchReadOutputs` 仅 prefetch Read，避免批量 IPC）。
- **不**改 `OMIT_TOOL_OUTPUT` 后端常量与 `getToolOutput` IPC 协议。
- **不**改 `outputCache` 容量或 LRU 策略。

## Decisions

### D1：用"viewer 用 output"作为 await 判定，不用"非 Edit/Write"反向枚举

**采用方案**：定义一个 `viewerUsesOutput(exec): boolean`：
- Read / Bash / 默认查看器（DefaultToolViewer 路径）：`true`
- Edit / Write：`false`

调用点：
```ts
if (exec && viewerUsesOutput(exec) && !isOutputReady(exec)) {
  await ensureToolOutput(exec);
  if (!isOutputReady(exec)) return; // IPC 失败或 missing
}
```

**候选方案**：
- A：`if (exec.outputOmitted && !isOutputReady(exec))` —— 简单但会让 Edit / Write 在 outputOmitted=true 时也等 IPC，按钮看起来卡（实际不需要 output）。
- B：`if (!isEditTool(exec) && !isWriteTool(exec))` —— 反向枚举，未来加新 viewer 时容易漏。

**取舍**：选 D1（正向白名单）。`viewerUsesOutput` 与 `ExecutionTrace` 里的 `isReadTool` / `isEditTool` / `isWriteTool` / `isBashTool` 判定函数同层；新增 viewer 时显式声明是否依赖 output，与 ToolViewer 路由表一一对应。

### D1b（apply 阶段修订）：`viewerUsesOutput` 抽到 `toolHelpers.ts` 共享 export

D1 原计划在 `SessionDetail.svelte` / `ExecutionTrace.svelte` 各自局部声明 `viewerUsesOutput`。apply 时考虑到 vitest 只能 import 模块级 export（svelte 组件内部函数无法直接覆盖），把 helper 抽到 `ui/src/lib/toolHelpers.ts::viewerUsesOutput` —— 两个调用点 `import` 即可，新增的 vitest case 直接覆盖判定逻辑。spec 行为契约不变；不算反转决策，仅是实现层落地选择。

### D2：单点延迟语义，不动 prefetch

**采用方案**：仅修改 toggle 内 await 范围；**不**改 `prefetchReadOutputs(chunk)` 的过滤条件（保持仅 Read）。

**理由**：

- prefetch Read 的语义是"展开 chunk 时尽量减少首次点 Read 的等待"——Read 文件常被多次反复看。
- Bash / Default 输出多为命令日志 / 结构化结果，单次查看居多；prefetch 没有 Read 那么强的命中收益，但批量 IPC 风险大。
- 用户反馈定位是"Bash 展开闪"，单点延迟即可消除闪烁；prefetch 是优化点，不在本 change 范围。

### D3：toggle 失败 fallback 路径保持当前行为

**采用方案**：`ensureToolOutput` 失败（IPC 错 / output kind=missing）后，与 Read 一样静默 return —— 工具保持折叠状态、用户可再次点击重试。

**候选方案**：失败后强制展开显示空内容 + 错误提示。

**取舍**：保持 D3 与 Read 一致，避免引入新的失败 UI；错误已经写入 `console.warn`。

### D4：嵌套 ExecutionTrace 同步对齐

**采用方案**：`ExecutionTrace.svelte::toggle` 与 `SessionDetail.svelte::toggle` 用同名 helper（局部声明各自的），保证嵌套层（SubagentCard 内）行为一致。

**理由**：spec 已有 `Scenario: 嵌套 ExecutionTrace 的 omitted Read 输出 ready 后再展开`，本 change 同步扩展即可，不引入新设计层。

## Risks / Trade-offs

- **[微延迟感知]** Bash / Default 首次展开按钮按下到内容出现会比当前版本多一次 IPC 往返（本机 Tauri 通常 5–30 ms）→ Read 路径已验证可接受；按钮 hover/active 状态保持不变，不会被感知为"卡死"。
- **[并发 IPC 卡顿]** 若用户在短时间内连点多个 Bash —— `outputLoads` Map dedupe 同一 toolUseId，且每个 IPC 都是独立调用，Tauri webview 按队列处理，不会触发"列表展开卡顿" → 与 Read 现状对称无新增风险。
- **[IPC 失败]** 网络/文件系统瞬时错误 → 与 Read 一致：toggle 静默 return，工具保持折叠；用户可重点。
- **[未来扩展]** 新增 ToolViewer 时需要在 `viewerUsesOutput` 显式登记。补救：在 `ExecutionTrace.svelte` 紧邻 viewer 路由表声明，code review 会注意到。

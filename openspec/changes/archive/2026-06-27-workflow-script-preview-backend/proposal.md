## Why

`WorkflowCard` 前端有一段渲染 workflow 编排脚本的 "View script" 折叠 UI，消费 `WorkflowItem.scriptPreview` 字段（PR #562 已保留并补齐 a11y），但后端发出的 `WorkflowItem` 从未填充该字段——真实 app 里 `{#if effectiveWorkflow.scriptPreview}` 永远 false，"View script" 仅在 mock fixture 下渲染。`PRODUCT.md` Design Principle 1「审计优先」要求用户能快速理解 Claude 刚做了什么；workflow 实际跑的编排脚本是高价值可审计信息。本 change 补上后端填充链路，UI 即自动生效（GitHub issue #561）。

## What Changes

- `WorkflowItem` 新增 `scriptPreview: Option<String>`（camelCase `scriptPreview`，`skip_serializing_if = Option::is_none`）——与前端已就绪字段同名。
- workflow manifest 解析处填充 `scriptPreview`，两种 Workflow tool 形态：
  - **inline `{script}` 形态**：取 `tool_use.input.script`（内容已驻 `ToolExecution.input` 内存，零额外 I/O）
  - **`scriptPath` 形态**：读脚本文件，按 `FileSignature` 缓存（script immutable → 每进程读一次），读取即截断
- **payload 瘦身**：preview 截断到 32 KB 上限，截断时尾部追加可见 marker（非静默截断）；bounds get_session_detail payload 与缓存内存。
- preview 进 `get_session_detail` 的 list payload（completed workflow 无 lazy 路径——`effectiveWorkflow = fullDetail ?? workflow`，completed 时 `fullDetail` 恒 null）；`get_workflow_detail` 走同一解析函数自动一致。
- **前端零改动**：`scriptPreview` 字段、`api.ts` interface、fixture、`WorkflowCard` disclosure 均已就绪。

## Capabilities

### New Capabilities
<!-- 无新增 capability -->

### Modified Capabilities
- `ipc-data-api`: `WorkflowItem` 新增 `scriptPreview` 字段及其填充契约（inline / scriptPath 两形态来源、32 KB 截断策略、性能门控）。

## Impact

- `crates/cdt-core/src/workflow.rs`：`WorkflowItem` 加字段 + `pending()` + 测试。
- `crates/cdt-api/src/ipc/workflow_manifest.rs`：candidate 携带 inline script、script 读缓存合并 meta+preview、resolve 路径填充 preview。
- `crates/cdt-api/tests/ipc_contract.rs`：`scriptPreview` 序列化 round-trip + 两形态 fixture 用例。
- 前端：无改动（消费侧已就绪）。
- 性能：inline 形态零 I/O；scriptPath 形态首次 +1 stat+read（小文件、异步、按 FileSignature 缓存复用），在 `get_session_detail` < 800ms 预算内；无 Workflow 的 session 零增量。

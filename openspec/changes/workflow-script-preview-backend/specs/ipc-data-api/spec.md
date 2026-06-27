## ADDED Requirements

### Requirement: WorkflowItem scriptPreview 填充

`WorkflowItem` SHALL 携带可选字段 `scriptPreview: Option<String>`（序列化为 camelCase `scriptPreview`，`None` 时 SHALL 从 JSON 省略），承载该 workflow run 实际执行的编排脚本预览，供前端 "View script" disclosure 审计渲染。该字段 SHALL 在 `get_session_detail` 的 list payload 路径填充——该路径从 chunks 的 `ToolExecution` 携带脚本来源（inline `script` / `workflow_script_path`），是 completed workflow（前端不经轮询）唯一可见的 preview 来源。`get_workflow_detail` 轮询路径复用同一解析函数，但其调用上下文不携带脚本来源（与该路径当前不重建 `name`/`phases` 同源限制），故 `scriptPreview` 为 `None`；completed workflow 不触发该轮询，不受影响。

填充来源按 Workflow tool 调用形态分流：

- **inline `{script}` 形态**（`tool_use.input` 含非空 `script` 字段）：`scriptPreview` SHALL 取 `tool_use.input.script` 的内容（该内容已驻 `ToolExecution.input` 内存，SHALL NOT 触发额外文件 I/O）。
- **`scriptPath` 形态**（`workflow_script_path` 非空）：`scriptPreview` SHALL 取该脚本文件内容，且该读 SHALL 按 `FileSignature` 缓存（脚本 immutable，稳态命中缓存复用、不重读盘）。
- **优先级**：当 inline `script` 与 `workflow_script_path` 对同一 run 同时存在时，SHALL 优先用 inline 内容（零 I/O），SHALL NOT 读文件。
- inline 与 scriptPath 来源皆缺失时 `scriptPreview` SHALL 为 `None`。
- scriptPath 文件 stat/读取失败时 `scriptPreview` SHALL 为 `None`（不 panic、不重试）。

预览长度 SHALL 截断到 32 KB 上限以约束 IPC payload 与缓存内存：超限时 SHALL 在 UTF-8 字符边界截断，并在尾部追加可见的截断 marker（含原始总字节数），SHALL NOT 静默丢弃尾部。截断后的预览串（含 marker）SHALL 即为缓存与 IPC 发出的内容（缓存 SHALL NOT 驻留全量未截断文件内容）。scriptPath 文件大小超过读取上限（远高于 inline 512 KB 合法上限）时 SHALL NOT 全量读入内存，preview SHALL 仅为一行标注总字节数的 oversize marker。

性能门控 SHALL 严格：无 Workflow tool call 的 session SHALL 零增量（候选为空时早返回）；inline 形态 SHALL 零文件 I/O；`scriptPath` 形态的文件读 SHALL 仅在该形态下触发且按 `FileSignature` 缓存复用。

#### Scenario: inline script 形态填充 preview（零文件 I/O）

- **WHEN** 某 Workflow tool execution 的 `input` 含 `script` 字段（inline 形态），脚本内容为 `"export const meta = {...}\nphase('A')"`
- **THEN** 对应 `WorkflowItem.scriptPreview` SHALL 为 `Some` 且内容等于该 inline 脚本（≤ 32 KB 时原样、不截断）
- **AND** 填充该 preview SHALL NOT 读取任何文件

#### Scenario: scriptPath 形态读文件填充 preview

- **WHEN** 某 Workflow tool execution 的 `input`/`result` 不含 inline `script` 但 `workflow_script_path` 指向一个 ≤ 32 KB 的存在脚本文件
- **THEN** 对应 `WorkflowItem.scriptPreview` SHALL 为 `Some` 且内容等于该文件内容
- **AND** 同一文件在同进程内重复解析 SHALL 复用 `FileSignature` 缓存而非重复读盘

#### Scenario: 超 32 KB 脚本截断并追加可见 marker

- **WHEN** 脚本内容超过 32 KB 上限
- **THEN** `WorkflowItem.scriptPreview` 长度 SHALL 不超过 32 KB 主体加截断 marker
- **AND** 截断 SHALL 落在 UTF-8 字符边界（不产生半个多字节字符）
- **AND** 预览尾部 SHALL 含明示截断且标注原始总字节数的可见 marker

#### Scenario: inline 与 scriptPath 同时存在时 inline 优先（零 I/O）

- **WHEN** 同一 run 的 tool execution 既有非空 inline `input.script` 又有 `workflow_script_path`
- **THEN** `WorkflowItem.scriptPreview` SHALL 取 inline 内容
- **AND** SHALL NOT 读取 `workflow_script_path` 指向的文件

#### Scenario: scriptPath 文件读取失败时 scriptPreview 省略

- **WHEN** 仅有 `workflow_script_path` 但该文件 stat 或读取失败（缺失 / 权限 / IO）
- **THEN** `WorkflowItem.scriptPreview` SHALL 为 `None`
- **AND** 解析 SHALL NOT panic，SHALL NOT 阻塞其它 workflow 解析

#### Scenario: 无脚本来源时 scriptPreview 省略

- **WHEN** 某 `WorkflowItem` 既无 inline `script` 也无可读的 `workflow_script_path`
- **THEN** `scriptPreview` SHALL 为 `None`
- **AND** 序列化 JSON SHALL 不含 `scriptPreview` 键

#### Scenario: 无 Workflow 的 session 零增量

- **WHEN** session 不含任何 Workflow tool call
- **THEN** 解析 SHALL 不进入任何 script preview 读取/截断逻辑（候选为空早返回）

## ADDED Requirements

### Requirement: Workflow 运行态降级解析（manifest 缺失）

当某 `workflow_run_id` 对应的 manifest 文件 `workflows/wf_<runId>.json` **不存在**（workflow 运行中，manifest 完成后才一次性原子写入）时，系统 SHALL **不**返回空白 `Pending` 占位，而是用运行中确实存在的磁盘信号（`journal.jsonl` + script 文件名）合成诚实的运行态 `WorkflowItem`。

运行态状态判定 SHALL **完全独立于** manifest 完成态路径的失败启发式（`tokens == 0 && tool_calls == 0 → failed`）——刚启动的 agent `tokens=0` 是正常的，套用失败启发式会把运行中 agent 误判为 failed。运行态 SHALL 仅依据 journal 事件判定：

- **per-agent**：同一 `agentId` 在 journal 中出现过 `type == "result"` 事件 → `WorkflowAgentState::Completed`；仅出现 `type == "started"` → `WorkflowAgentState::Running`。
- **整体 status**：
  - manifest 存在 → 走原完成态解析路径（不受本 Requirement 影响）。
  - manifest 缺失 **AND** journal 存在且含 ≥1 `started` 事件 → `WorkflowStatus::Running`。
  - manifest 缺失 **AND** journal 缺失或为空 → `WorkflowStatus::Pending`。

合成 agent SHALL：按 `agentId` 去重计数；`label` 留空（由前端补 `"Agent N"`）；`tokens`/`tool_calls`/`duration_ms` 填 0、`failed` 填 false（journal 无此数据）。运行态合成 agent 的 `Completed` 语义为「已结束」而非「已成功」——journal `result` 对失败 agent 也 append，运行态不区分成败（成败裁定是 manifest 完成态职责）。

workflow `name` SHALL 从 `workflow_script_path` 的 basename 用精确 `strip_suffix` 剥取：先剥 `.js`，再剥 `-<runId>`（runId 为完整 `wf_` 前缀字符串）。任一 `strip_suffix` 不匹配（runId 与文件名后缀不一致，如 resume 场景）→ `name` 为 `None`；SHALL NOT 用模糊 `find`/`replace` 剥出半截 name。script 路径缺失时 `name` 同为 `None`（前端兜底显示 "Workflow"）。

性能门控 SHALL 严格：本降级路径仅在 `workflow_run_id` 存在 **且** manifest `stat` 失败时触发；journal SHALL 按 `FileSignature` 缓存，journal 未变化时复用缓存结果，变化时只重读 journal 做廉价行计数（不做 JSON 全解析，仅区分 `started`/`result` 与提取 `agentId`）。无 Workflow / 已完成 Workflow 的 session SHALL 走原 manifest 快路径，零增量。

#### Scenario: manifest 缺失但 journal 有 started 与 result

- **WHEN** `wf_<runId>.json` manifest 不存在，且 `subagents/workflows/wf_<runId>/journal.jsonl` 含 3 条 `started`（agentId a1/a2/a3）+ 1 条 `result`（agentId a1）
- **THEN** 产出的 `WorkflowItem.status` SHALL 为 `WorkflowStatus::Running`
- **AND** `agents` SHALL 含 3 个合成 agent（按 agentId 去重）
- **AND** a1 对应 agent SHALL 为 `WorkflowAgentState::Completed`，a2/a3 SHALL 为 `WorkflowAgentState::Running`
- **AND** 所有合成 agent 的 `failed` SHALL 为 false（即使 tokens/tool_calls 为 0）

#### Scenario: manifest 缺失且 journal 不存在

- **WHEN** `wf_<runId>.json` manifest 不存在，且 `journal.jsonl` 不存在（agent 刚启动 journal 尚未 append）
- **THEN** 产出的 `WorkflowItem.status` SHALL 为 `WorkflowStatus::Pending`
- **AND** `agents` SHALL 为空

#### Scenario: 运行态 name 从 scriptPath 剥 runId 后缀

- **WHEN** manifest 缺失，`workflow_script_path` 为 `/x/workflows/scripts/explore-workflow-rendering-wf_a3fbf671-153.js`，runId 为 `wf_a3fbf671-153`
- **THEN** 产出的 `WorkflowItem.name` SHALL 为 `Some("explore-workflow-rendering")`

#### Scenario: 运行态 scriptPath 缺失时 name 为 None

- **WHEN** manifest 缺失，journal 存在，但 `workflow_script_path` 为 `None`（inline script 调用形态或跨 project_dir 找不到）
- **THEN** 产出的 `WorkflowItem.name` SHALL 为 `None`
- **AND** `status` SHALL 仍按 journal 判定为 `Running`

#### Scenario: runId 与 scriptPath 后缀不一致时 name 为 None

- **WHEN** manifest 缺失，`workflow_script_path` basename 的 `-<runId>` 后缀与当前 runId 不精确匹配（如 resume 场景 `input.resumeFromRunId ≠ result.runId`，或文件名异形）
- **THEN** `strip_suffix` 失败，`WorkflowItem.name` SHALL 为 `None`
- **AND** SHALL NOT 产出剥取错误的半截 name

#### Scenario: race 窗口——journal 全 result 但 manifest 未写

- **WHEN** manifest 缺失，journal 中每个 agentId 都有 `result` 事件
- **THEN** `WorkflowItem.status` SHALL 仍为 `WorkflowStatus::Running`（manifest 是完成态唯一权威源）
- **AND** 所有合成 agent SHALL 为 `Completed`（即显示 `N agents (N done)`，下次 manifest 出现后由 watcher 触发切全量）

#### Scenario: 已完成 workflow 不触发降级路径

- **WHEN** `wf_<runId>.json` manifest 存在且可解析
- **THEN** 系统 SHALL 走原 manifest 完成态解析路径产出 `WorkflowItem`
- **AND** SHALL NOT 读取 journal.jsonl（零增量）

### Requirement: Workflow script meta 静态解析（Tier 1）

作为运行态降级的可选增强，系统 MAY 解析 workflow script 文件的 `export const meta = { ... }` 块，提取 `name` 与 `phases` 静态列表补充到运行态 `WorkflowItem`。该解析失败时 SHALL **静默降回 Tier 0**（不影响 status / agents 判定，不 panic，不显示半截内容）。

解析 SHALL 用窄职责隔离 lexer 切出 `meta` 对象字面量块，再把切出的块整体交 `json5` 库做结构解析——SHALL NOT 手搓对象结构提取。lexer 做括号深度平衡扫描时 SHALL 跟踪三种字符串分隔符（`'` / `"` / `` ` `` backtick）+ 转义（`\`）+ 注释（`//` / `/* */`），确保字符串与注释内的 `{` `}` 不计入深度。script 文件 immutable，解析结果 SHALL 按 script `FileSignature` 缓存，命中缓存时不重新解析。

解析出的 `phases` 仅作**静态列表**展示，SHALL NOT 标注「当前第几 phase」（journal 无 phase 标记，运行态无权威「当前 phase」来源）。

#### Scenario: 解析 meta 得 name 与 phases

- **WHEN** script 含 `export const meta = { name: 'foo', phases: [{ title: 'Build', detail: '...' }, { title: 'Verify' }] }`
- **THEN** 解析 SHALL 返回 `name == "foo"` 与含 2 个 `WorkflowPhase`（title `Build` / `Verify`）的列表

#### Scenario: meta 含注释 / 转义引号 / detail 在 title 前仍稳健

- **WHEN** script 的 meta 块含 `//` 行注释或 `/* */` 块注释、含 `\'` 转义引号、或 phase 对象把 `detail` 字段写在 `title` 之前
- **THEN** 隔离 lexer SHALL 正确配平括号切出完整 meta 块（不被注释中的 `}` 或字符串中的引号干扰）
- **AND** `json5` SHALL 正确解析字段顺序无关的对象

#### Scenario: meta 值含 backtick 模板串——lexer 配平但 json5 降级

- **WHEN** script 的 meta 某字符串值用 backtick（`` ` ``）分隔且内含 `{` 或 `}` 字符
- **THEN** 隔离 lexer SHALL 把 backtick 串当字符串跳过、不把串内 `{` `}` 计入括号深度，仍切出完整且配平的 meta 块
- **AND** 由于 `json5` 不支持 backtick 分隔的值，`json5::from_str` SHALL 报错 → `parse_script_meta` 返回 `None` 静默降回 Tier 0（不 panic、不显示半截内容）

#### Scenario: meta 无 phases 字段

- **WHEN** script 的 meta 仅含 `name` 与 `description`，无 `phases` 字段
- **THEN** 解析 SHALL 返回 `phases` 为空列表
- **AND** 运行态 `WorkflowItem` SHALL 正常退化为 Tier 0 显示（仅 name + agent 计数）

#### Scenario: 解析失败静默降级

- **WHEN** script 文件不存在、meta 块无法配平、或 `json5` 解析报错（例如 backtick 分隔的值）
- **THEN** 解析 SHALL 返回 `None`
- **AND** 运行态 `WorkflowItem` SHALL 保留 Tier 0 的 status / agents 判定，`phases` 为空
- **AND** SHALL NOT panic 或显示半截解析内容

#### Scenario: script 按文件签名缓存

- **WHEN** 同一 immutable script 在多次 `get_session_detail` 调用中被解析
- **THEN** 首次解析后 SHALL 按 `FileSignature` 缓存结果
- **AND** 后续调用 SHALL 命中缓存不重新解析（script immutable）

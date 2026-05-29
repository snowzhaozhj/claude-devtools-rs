## Why

Workflow manifest `workflows/wf_<runId>.json` 是 workflow **完成后**才一次性原子写入的——运行中该文件根本不存在。当前 `resolve_single` 在 manifest 缺失时一律返回 `WorkflowItem::pending`，导致用户在 workflow 运行期间（最有信息需求的时刻）只看到一张空白卡片，无法判断「编排器是否在跑、起了多少 agent、跑完几个」。

运行态是瞬态但高频可见的：一个 fan-out workflow 可能 spawn 几十个 agent 跑数分钟。本 change 在 manifest 缺失时，用运行中**确实存在**的两个磁盘信号（`journal.jsonl` + script 文件）诚实降级出 Running 态，让运行期间的卡片传达真实进度，而非空白。

## What Changes

- **Tier 0（核心，零新依赖）**：当 `workflow_run_id` 存在但 manifest 缺失时：
  - 从 `toolUseResult.scriptPath` 抽取 `workflow_script_path` 存入 `ToolExecution`（新字段）。
  - 用 scriptPath basename 剥已知 runId 字面后缀得 workflow `name`。
  - 读 `subagents/workflows/wf_<runId>/journal.jsonl`，**按 agentId** 数 `started`/`result`，合成匿名 agents（有 `result` → `Completed`，仅 `started` → `Running`），组装 `WorkflowItem{status: Running, phases: []}`。
  - journal 按 `FileSignature` 缓存（journal 变化只重读廉价行计数）。
  - 前端 WorkflowCard 在运行态对空 label 的合成 agent 显示 `"Agent N"`，并展示 `N agents (M done)` 计数。
- **Tier 1（可选增强，引入 `json5` crate）**：`parse_script_meta` 用窄职责隔离 lexer 切出 `meta = {...}` 块（跟踪字符串/注释/转义 + 括号深度）→ 喂 `json5` 取 `name` + `phases`；解析失败返回 `None` **静默降回 Tier 0**；按 script `FileSignature` 缓存。
- **运行态状态判定独立于 manifest 的失败启发式**：运行态绝不套用 manifest 路径的 `tokens==0 && toolCalls==0 → failed` 判定（刚启动 agent tokens=0 是正常的）。

非目标：不重建运行态 per-agent 真实 label（journal 启动顺序 ≠ script 声明顺序，`parallel()` 并发启动，位置对应法不可靠）；不标「当前第几 phase」（journal 无 phase 标记）。

## Capabilities

### New Capabilities
<!-- 无新 capability -->

### Modified Capabilities
- `tool-execution-linking`: 新增「从 `toolUseResult.scriptPath` 抽取 `workflow_script_path`」Requirement（与既有 runId 抽取并列）。
- `ipc-data-api`: 新增「Workflow 运行态降级解析（manifest 缺失）」Requirement（Tier 0：journal 合成 + scriptPath 取名 + 状态判定独立于失败启发式）+「Workflow script meta 静态解析」Requirement（Tier 1：隔离 lexer + json5 取 phases）。
- `session-display`: 修改既有「WorkflowCard 渲染」Requirement 的 Running 场景——运行态存在合成 agents 时展开显示匿名 `"Agent N"` chips + `N agents (M done)` 计数。

## Impact

- 代码：
  - `cdt-core::tool_execution::ToolExecution` 新增 `workflow_script_path: Option<String>` 字段（`#[serde(default, skip_serializing_if = "Option::is_none")]`）。
  - `cdt-analyze::tool_linking::pair` 在 Workflow tool_result 配对处抽取 scriptPath。
  - `cdt-api::ipc::workflow_manifest`：`resolve_single` 增加 manifest-missing 降级分支；新增 journal 解析 + 缓存；（Tier 1）新增 `parse_script_meta` + script 缓存。
  - `ui/` WorkflowCard 组件：运行态匿名 agent 渲染。
- 依赖：Tier 1 引入 `json5`（纯 Rust 微依赖）；Tier 0 零新依赖。
- 性能：门控严格——仅 `run_id` 存在**且** manifest 缺失才触发；script 按文件签名只解析一次缓存；无 workflow / 已完成 workflow 走原 manifest 快路径，**零增量**。

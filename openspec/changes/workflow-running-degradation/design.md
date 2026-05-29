## Context

实证调研（监控真实 workflow 从启动到完成的磁盘生命周期 + 13 个真实 script + 6 个边界用例解析验证，结论见 issue #397 评论「PR 6 运行态渲染修正设计」）确认：

| 文件 | 写入时机 | 含 phase/label？ | 运行中可用 |
|---|---|---|---|
| `workflows/wf_<id>.json`（manifest） | 完成后一次性原子写 | 全量 | ❌ 运行中不存在 |
| `subagents/workflows/wf_<id>/journal.jsonl` | 每个 agent 启停 append | **无**（仅 `started`/`result` + `key`(内容哈希) + `agentId`） | ✅ |
| `workflows/scripts/<name>-wf_<id>.js` | 启动瞬间 | meta 有 phases；agent() 有 label | ✅ |

关键结论：
1. **运行中无任何「中间状态文件」**（已遍历 sessions/transcripts/tasks/jobs/daemon 确认）。`/workflows` TUI 的实时 label 来自执行进程内存，不落盘。
2. **journal 启动顺序 ≠ script 声明顺序**（`parallel()` 并发启动）→ 位置对应法不可靠，**放弃运行态 per-agent label 重建**。
3. **运行中 phase 只有 script 一个来源**（journal 无 phase 标记）。

当前 `cdt-api::ipc::workflow_manifest::resolve_single` 在 `fs.stat(manifest_path)` 失败时直接返回 `WorkflowItem::pending`，运行态 UI 空白。

## Goals / Non-Goals

**Goals:**
- manifest 缺失但 workflow 在跑时，用 journal + scriptPath 合成诚实的 Running 态（编排器名 + agent 计数 + 已完成数）。
- 运行态状态判定**独立于** manifest 路径的失败启发式。
- 严格门控 + 双层缓存，无 workflow / 已完成 workflow 零增量。
- 全程 graceful degradation：绝不 panic、不显示半截垃圾。

**Non-Goals:**
- 不重建运行态 per-agent 真实 label（用匿名 `"Agent N"` 替代）。
- 不标「当前第几 phase」（journal 无 phase 标记，Tier 1 只能给静态 phase 列表）。
- 不改 manifest 完成态解析路径（manifest 出现后走原全量路径）。
- 不读 `agent-*.jsonl`（贵；journal 行计数已够 Tier 0）。

## Decisions

### D1: Tier 0 / Tier 1 分级策略

把运行态降级拆成两档，按「健壮性 + 性能 + UX 增量」分层：

| 档 | 显示 | 来源 | 解析 JS | 健壮性 |
|---|---|---|---|---|
| **Tier 0**（必做，PR 6a） | `name · Running · N agents (M done)` | scriptPath basename 剥已知 runId 字面后缀 + journal 按 **agentId** 数 started/result | **零解析** | 不可能失败 |
| **Tier 1**（可选，PR 6b） | + phases 静态列表 | `json5` + 隔离 lexer 解析 script `meta` | 按文件签名解析一次缓存 | 失败静默降回 Tier 0 |

候选方案：
- A. 只做 Tier 0（选作 PR 6a 下限）：零依赖、不可能失败，已覆盖核心 UX（编排器名 + 进度计数）。
- B. Tier 0 + Tier 1（选作完整目标）：Tier 1 是 Tier 0 之上的纯增量，失败回落 Tier 0，无新失败面。
- C. 直接解析 script 全量重建 phases + 当前 phase（否决）：journal 无 phase 标记，无法标当前 phase；徒增解析复杂度。

**Tier 1 固有天花板（诚实声明）**：journal 无 phase 标记 → 即使解析出 phases 也只能显示**静态列表**，标不出「当前第几 phase」。运行态瞬态、完成后 manifest 自动接管全量，故 Tier 1 增量价值有限，列为可选。

### D2: 解析选型 `json5` + 隔离 lexer，而非 `oxc` / `tree-sitter`

需求本质：从一个 immutable、瞬态、已缓存的 script 里提取 **2 个字段**（`name` + `phases`）。对此投资重型 JS parser 不成比例。

候选方案：
- A. **`json5` + 窄职责隔离 lexer**（选此）：
  - 隔离 lexer 只做**平衡括号扫描**切出 `meta = { ... }` 块，结构解析交 `json5` 库——**非**被否决的「手搓结构提取」。
  - lexer 做平衡扫描时 SHALL 跟踪**三种字符串分隔符**（`'` 单引号、`"` 双引号、`` ` `` backtick）+ 转义（`\`）+ 注释（`//` 行 / `/* */` 块），确保字符串/注释内的 `{` `}` **不计入**括号深度。meta 是纯数据对象字面量（值为 string / 数组 / 嵌套对象），**无裸 regex 字面量、无裸除法 `/`**（`/` 只可能出现在注释或字符串内，均已被状态机吸收）→ 平衡扫描无歧义。
  - **关键区分**：lexer 跟踪 backtick 是为了**正确配平括号**（backtick 串里的 `}` 不能误计）；而 `json5` 库本身**不支持 backtick 分隔的值**——若某 meta 值用 backtick（13 真实 script 0 次命中），lexer 仍能切出完整块，但 `json5::from_str` 解析报错 → 走 graceful `None` 降级。两件事不矛盾：切块靠 lexer，解析值靠 json5。
  - `json5` 是纯 Rust 微依赖，无 build script、无 C 依赖、无平台风险。
- B. `oxc`（否决）：churn 重 + arena 生命周期侵入调用方 + 体积大；为提 2 字段引入完整 JS AST 过度投资。
- C. `tree-sitter`（否决）：C-grammar 依赖触发 **Windows 跨平台构建风险**（本仓 Windows 兼容是硬约束）+ 体积大。
- D. 纯手搓结构提取（否决）：`detail` 写在 `title` 前、detail 含 `]`/`}`/`title:` 时必崩；健壮性差。

**为什么隔离 lexer 不算手搓结构提取**：lexer 只回答「`meta = {` 之后到哪个 `}` 配平」这一个词法问题（括号深度 + 字符串/注释状态机），不解析对象结构；切出的块整体交 `json5`。结构语义完全由成熟库承担。

**script 是 async-function 体**（顶层 `return` + `await` + `export`），不是合法 ES module —— 隔离 lexer 只扫 `meta` 块不受影响，而整文件喂任何 module parser 都会报错（这也反向支持否决 A 之外的全量 parse 方案）。

### D3: 运行态状态判定**独立于** manifest 失败启发式

manifest 完成态路径用 `tokens == 0 && tool_calls == 0 && result_preview.is_none() → failed` 判定失败 agent。**运行态绝不能套用此启发式**：刚启动的 agent `tokens=0 && toolCalls=0` 是**正常**的（还没干活），套失败启发式会把运行中 agent 全判成 failed，UI 显示一片红，是严重误导。

运行态状态判定**只看 journal 事件**：

- **per-agent**：同一 `agentId` 出现过 `result` → `Completed`；仅出现 `started` → `Running`。（journal 无 `failed` 事件——失败也走 `result`，运行态不区分成败，留给 manifest 完成态裁定。）

**「Completed」在运行态的语义是「已结束」而非「已成功」**：journal 的 `result` 事件对失败 agent 也会 append，运行态无法（也不该用 `agent-*.jsonl` 重活）区分成败。spec 与 UI 文案 SHALL 把运行态的 completed 表述为「finished/done」语义，**不**等同于「成功」——真正的成败裁定是 manifest 完成态的职责。运行态绿点表示「这个 agent 跑完了」，最终成败由后续 manifest 出现后接管。这是 Tier 0 的诚实边界，不是 bug。
- **整体**：
  - 有 manifest → 走原完成态路径（含失败启发式，此时合理）。
  - 无 manifest + 有 journal（≥1 `started`） → `WorkflowStatus::Running`。
  - 无 manifest + 无 journal（刚启动 journal 还没 append） → `WorkflowStatus::Pending`。

合成 agent 的 `tokens`/`tool_calls`/`failed` 字段全填 0/false（journal 无此数据），`label` 留空由前端补 `"Agent N"`。

**name 后缀剥取 SHALL 用精确 `strip_suffix` 而非 `find`/`replace`**：从 basename 先 `strip_suffix(".js")`，再 `strip_suffix(&format!("-{run_id}"))`。任一步不匹配（runId 与文件名后缀不一致，如 resume 场景 `input.resumeFromRunId ≠ result.runId`、或跨 project_dir 文件名异形）→ 返回 `None`，**绝不**用模糊匹配剥出半截垃圾 name。runId 三处一致性（manifest 文件名 stem == journal 目录名 == script 文件 `-wf_<id>` 后缀 == `toolUseResult.runId`）由实证确认，但 strip 失败仍 graceful 降级。

### D-V1: 运行态匿名 agent 用 `"Agent N"` chip + 计数，不伪造 per-agent 身份

journal 启动顺序 ≠ script 声明顺序，无法把合成 agent 对回 script 里的具名 `agent(..., {label})`。强行编号到具名 label 会**误导**（顺序错位）。

候选方案：
- A. 空 label → 前端渲染 `"Agent 1".."Agent N"` 纯序号 chip + header `N agents (M done)` 计数（选此）：诚实传达「起了 N 个、完成 M 个」，不声称知道每个是谁。
- B. 尝试按 journal 顺序映射 script label（否决）：顺序不可靠，错位误导比匿名更糟。
- C. 运行态完全不显示 agent，只显示 spinner（否决）：丢失「起了多少 / 完成多少」这一运行态最有价值的进度信号。

## Visual Contract

### Surface Decision
复用既有 `WorkflowCard`（session detail 对话流内，AIChunk 命中 `workflows` 字段时实例化）的 **Running 分支**，不新增 surface。运行态是同一卡片的一个状态，不该另起入口（对齐 `PRODUCT.md` 的「状态归一到同一组件」倾向）。

### Visual Layer
- 运行态 header：`spinner（旋转）· name（或 "Workflow" 兜底）· "Running" · N agents (M done)`。spinner 是**唯一**带动画的元素（沿用既有 `DESIGN.md::The Status Owns the Color Rule`——running 用中性/进行色，不滥用红绿）。
- 展开区：匿名 agent chip 横排，每个 chip status dot 静态着色（completed 绿 / running 中性），**chip 不带动画**（动画只在 header spinner，对齐既有 Scenario「WorkflowCard 仅 header 有动画」）。
- chip label：`"Agent 1"`、`"Agent 2"`…（合成 agent 空 label 时前端补齐）。
- **绝不**渲染假进度条 / 百分比（无总数权威源，伪造进度是欺骗）。

### State Coverage
- Running + 有合成 agents（≥1 started）：header 计数 + 展开 agent chips（本 change 新增渲染）。
- Running + Tier 1 解出 phases：agent chips 之上额外显示 phase 静态列表（无「当前 phase」高亮）。
- Pending（无 journal）：沿用既有 pending/最小态（spinner + 无 agent）。
- 完成态 / 部分失败态 / Empty / Launch error：均走既有路径，不变。

### DESIGN.md delta plan
本 change 不引入新 token / 新组件，仅复用既有 WorkflowCard running 态视觉语言（spinner + status dot + chip）。无 `DESIGN.md` 沉淀项。

## Risks / Trade-offs

- **race 窗口**（journal 全 `result` 但 manifest 未写）：显示 `Running + 全 done`（`N agents (N done)`）；下次 poll manifest 出现后 watcher（F1）触发重渲染自动切全量。可接受的短暂态。
- **跨 project_dir**（dev / worktree 场景 script 写到另一 project_dir）：scriptPath 用绝对路径定位；找不到 script → name 取不到 → 降级显示 `"Workflow · Running · N agents"`（兜底 name）。
- **无 phases 字段**（spec 里 `phases` 可选）：Tier 1 解析出 `phases: []`，正常退化为 Tier 0 显示。
- **刚启动无 journal**：`status: Pending`，下次 poll journal 出现后转 Running。
- **journal 巨大**（极端 fan-out 上千 agent）：行计数 O(行数) 但按 FileSignature 缓存，仅 journal 变化时重读；append-only 文件签名变化频繁但行计数廉价（无 JSON 全解析，只数 `started`/`result` + agentId 去重）。
- **`json5` 解析慢**（Tier 1）：按 script FileSignature 一辈子只解析一次；script immutable，命中缓存后零成本。
- **ToolExecution 加字段破坏构造点**：`workflow_script_path` 用 `Option<String> + serde(default, skip_serializing_if)`，对老 fixture / 老前端无破坏（同 `workflow_run_id` 模式）。
- **scriptPath 来源**：优先从 `toolUseResult.scriptPath` 抽取（与 runId 同处，PR 6 实证设计指明此处有 scriptPath）；若该处缺失，SHALL 回退到配对 `tool_use.input.scriptPath`（Epic 实证：input 的 scriptPath 调用形态确含此字段）。两处都无（inline `{script}` 调用形态）→ `None`，name 降级。pair.rs 在配对点同时持有 `tool_use_result` 与 `pu.input`，双源抽取零额外 I/O。
- **dead workflow 永久 Running**：workflow 进程被 kill 且 manifest 永不写入时，卡片会一直显示 Running（无任何磁盘信号能区分「还在跑」与「已死」——manifest 完成态路径同样无「abandoned」检测）。这是诚实降级的固有边界：只显示已知信息，不臆测死亡。下次有 manifest 出现即由 watcher（F1）切全量。

## Context

Session 导出（Markdown / HTML 桌面端 + CLI Markdown）的 render switch 对 `slash` / `teammate_message` / `teammate_spawn` / `workflow` 四类 DisplayItem 与 subagent 内部对话静默 `return ""`。这是 change `fix-export-tool-order-and-output` 的 scope-out 遗留（issue #534）。

现状关键事实（已实证）：

- **四类内容数据现成**：`slash` ← `AIChunk.slashCommands`；`teammate_message` ← `AIChunk.teammateMessages`；`teammate_spawn` ← `ToolExecution.teammateSpawn`（由 `buildDisplayItems` 从带 `teammateSpawn` 的 tool 转化）；`workflow` ← `SessionDetail.workflowItems`（通过 `ToolExecution.workflowRunId` 关联，对齐视图 `SessionDetail.svelte::workflowMap`）。这四类在导出 payload 中**不被裁剪**。
- **`buildDisplayItems` 不产 `WorkflowDisplayItem`**：视图层是「带 `workflowRunId` 的 tool 命中 `workflowMap` 时渲染 `WorkflowCard` 替代 tool」，DisplayItem union 里的 `WorkflowDisplayItem` case 永不触发。
- **subagent 内部对话被后端裁剪**：`crates/cdt-api/src/ipc/local.rs::apply_omissions_impl` 在 `OMIT_SUBAGENT_MESSAGES=true` 时把顶层 `ai.subagents[].messages` 清空 + `messages_omitted=true`。导出路径 `apply_export_omissions` 同样命中此裁剪。
- **递归膨胀风险已证实**：`apply_image_omit` 注释明确「subagent 裁剪之前嵌套层 messages 可能非空」，即 `build_chunks` 产出的 `subagent.messages` 内嵌套子代理的 messages **完整递归填充**。直接 `OMIT_SUBAGENT_MESSAGES=false` 会让整个子代理递归树进 payload，大会话可达数 MB，触碰 `perf.md` 「IPC payload > 1 MB 须瘦身」红线。
- **前端已有递归渲染工具**：`displayItemBuilder.ts::buildDisplayItemsFromChunks(Chunk[])` 把 subagent messages 平铺成 DisplayItem 流（`WorkflowCard` agent trace 已复用）。
- **CLI 数据流**：`cdt-cli/src/main.rs::export` 经 `engine.get_session_detail`（in-process）取 detail，构造 `filtered_detail` 时显式 `workflow_items: vec![]` 丢弃 workflow，`render_subagent_md` 仅渲染元数据不递归 messages。

## Goals / Non-Goals

**Goals:**
- Markdown / HTML / CLI 导出补齐 `slash` / `teammate_message` / `teammate_spawn` / `workflow` 四类渲染，语义对齐 SessionDetail 视图。
- 导出 subagent 内部对话（递归 messages），施加 byte cap + 嵌套深度上限封顶 payload，超限标注省略。
- 桌面端 / CLI / 浏览器 HTTP 三路导出行为一致。

**Non-Goals:**
- 不改 JSON 导出的渲染逻辑（JSON 是 `JSON.stringify(projected)`，subagent messages 封顶后自然包含；不另写 JSON 专属渲染）。
- 不改视图层 SessionDetail.svelte 的渲染（视图已正确渲染五类，本 change 只补导出器）。
- 不实现 workflow agent trace 的内部对话导出（agent trace 是 `getWorkflowAgentTrace` 懒拉取，静态导出无法触发；只导出 WorkflowItem 静态摘要 + agents 列表）。
- 不改 `OMIT_IMAGE_DATA`（image data 在导出路径仍裁剪，与现 spec 一致）。

## Decisions

### D1：四类纯前端渲染补 render switch（markdown + html）

`markdownExporter.ts::renderDisplayItem` / `htmlExporter.ts::renderDisplayItemHtml` 的 `case "slash" / "teammate_message" / "teammate_spawn"` 各补实际渲染，对齐视图语义：
- `slash` → `### Slash: /{name}` + args/message + instructions（有则作 blockquote/折叠）。
- `teammate_message` → `### Teammate: {teammateId}` + body markdown（`isNoise` / `isResend` 的处理对齐视图：视图照常渲染，导出同样保留）。
- `teammate_spawn` → 单行 `*[teammate spawned] {name}*`（对齐视图极简单行）。

**备选**：在 `buildDisplayItems` 阶段就把这些转成文本——否决，会污染视图共用的 builder，违反 spec「导出复用 buildDisplayItems 时序」的边界（builder 只排序不渲染）。

### D2：workflow 渲染在 tool case 内关联，按 runId 去重，不造 WorkflowDisplayItem

exporter 接收 `workflowItems` 并构建 `Map<runId, WorkflowItem>` + 一个 `seenRunIds: Set<string>`；`renderDisplayItem` 的 `tool` case 中若 `exec.workflowRunId` 命中且该 runId 未渲染过，渲染 workflow 摘要（name + status + phases + agents 列表 + tokens/duration）替代普通 tool 并记入 seen；**同一 runId 的后续 tool 调用 SHALL 跳过（既不重复渲染 workflow，也不降级为普通 tool）**，对齐视图 `buildSummary::seenWorkflowIds` 的去重语义（codex F4：视图去重在 summary 层，exporter 直接照搬 tool case 会把重复固化）。

数据传递：`projection.ts::ProjectedSessionDetail` 增加 `workflowItems` 字段透传；`exportAsMarkdown` / `exportAsHtml` 把它构建成 map 后传入 `renderAIChunk` → `renderDisplayItem`，seen set 在单次导出内贯穿。

**备选**：让 `buildDisplayItems` 产 `WorkflowDisplayItem`——否决，会改动视图共用的 builder 行为（视图当前依赖 tool+workflowMap 路径），扩大爆炸半径且与视图渲染路径不一致。保留 union 里的 `WorkflowDisplayItem` case 返回空（dead 但类型完整）。

### D3：后端导出路径 subagent messages 封顶填充——depth-cap + per-subagent byte-cap + global byte-cap

新增 pub 函数 `cap_subagent_messages(chunks)`（内部参数化版 `cap_subagent_messages_with_limits` 便于小阈值单测），替代 `apply_export_omissions` 里的顶层整体清空。三闸门**顺序明确**（codex F3 + 复核 (a)）：

1. **先 depth-cap**：递归清空嵌套深度 > `MAX_SUBAGENT_DEPTH` 的 subagent.messages（顶层 subagent.messages 内嵌套子代理的 messages 全清空 + `messages_omitted=true`），砍掉递归爆炸最大来源。
2. **再 per-subagent byte-cap**：对每个**保留**的 subagent，按 depth-cap **清空后形态**真实计量序列化字节（`serde_json::to_vec(&sub.messages).map(|v| v.len())`，非 text 长度近似——codex F2），超 `MAX_BYTES_PER_SUBAGENT` 则该 subagent.messages 清空 + `messages_omitted=true`。**单个 subagent 独立判定**（codex F8：低全局预算会让早期巨型 subagent 吃光、后续高价值 subagent 全省略，故 per-subagent 闸门不参与全局累计）。
3. **最后 global byte-cap 兜底**：按 chunks 顺序累计「未被前两步清空的」subagent messages 序列化字节，累计超 `MAX_EXPORT_SUBAGENT_TOTAL_BYTES` 后，后续 subagent.messages 清空 + `messages_omitted=true`（codex 复核 (a)：per-subagent 有界但 N 个之和无界——agent team fan-out N 个各 1.8MB teammate 可累计超 50MB，单次 IPC 仍需硬上界）。全局上限设得**宽松**，仅防极端 fan-out，正常会话不触发。

**常量取值**：`MAX_SUBAGENT_DEPTH = 1`（只展开顶层 subagent 直接对话；嵌套子代理仍作卡片摘要——爆炸风险高、导出价值低）；`MAX_BYTES_PER_SUBAGENT = 2_097_152`（2 MiB，控单个 debug-log 型病态 subagent）；`MAX_EXPORT_SUBAGENT_TOTAL_BYTES = 52_428_800`（50 MiB，控极端 fan-out 的 IPC 总量硬上界）。两层 byte cap 取值悬殊——per-subagent 控单个、global 控总量，正常会话两者都不触发。值写成 `const` + 注释 justify。

**三路同参数**（codex F5/F7）：cap 函数对桌面 IPC（`get_session_detail_for_export`）、浏览器 HTTP 导出（见 D5）、CLI 用**同一 depth + 同 per-subagent + 同 global byte cap**。两层 byte cap 都足够宽松，CLI/浏览器"完整文件导出"日常不被截断，仅病态/极端 fan-out 触发，三路行为一致。

`OMIT_SUBAGENT_MESSAGES` 常量保留给 display 路径（`apply_display_omissions` 仍全清空，首屏零影响）。

**备选**：(a) `OMIT_SUBAGENT_MESSAGES=false` 无上限——否决，递归爆炸。(b) 低全局累计预算（如 1 MB 单闸门）——否决（F8 早期偏向）；改用 per-subagent（病态保护）+ 宽松 global（总量兜底）双层。(c) text 长度近似计量——否决（F2 漏算 tool output，量纲失真）。

### D4：前端 / CLI 递归渲染 subagent 内部对话（递归前先 project）

- **前端**（markdown + html）：`renderSubagent` / `renderSubagentHtml` 在元数据 header 后，若 `sub.messages` 非空则**先对 `sub.messages` 应用与外层同一的 `projectChunk(options)`**（codex F1：递归层不 project 会让 `--no-thinking` / `name-only` / 截断 / `includeSubagents=false` 的 Task 去重在内部对话失效），再 `buildDisplayItemsFromChunks` 平铺并逐项 `renderDisplayItem`（缩进/嵌套样式标识层级）；`sub.messagesOmitted` 为 true 时追加 `*[内部对话已省略：超出导出上限]*`。
- **去重 key 独立性**（codex 复核 (b)）：投影 MUST 先于 `buildDisplayItemsFromChunks`；`includeSubagents=false` 时投影先移除该层 `AIChunk.subagents`，使 builder 的 `taskIdsWithSubagents` 为空、不按 `parentTaskId` 吞掉 Task/Agent 工具。depth-cap 截断后嵌套子代理卡片仍在（卡片本身即内容代表 + `messagesOmitted` 标注），不会丢工具。subagent 去重 key 为 `parentTaskId` / `toolUseId`（工具调用粒度），与 workflow `seenRunIds`（runId 粒度）独立，不跨层误 skip 父层 Task 调用。
- **CLI**：`render_subagent_md` 后递归渲染 `sub.messages`（CLI 已有 `render_chunk_md`，对 messages 内 Chunk 复用，复用前同样按 CLI `ExportOptions` 过滤 thinking/detail）；`messages_omitted` 同样标注。

**复用边界**：前端 `buildDisplayItemsFromChunks` + `projectChunk` 已存在且经测试，递归层复用零新增逻辑。

### D5：三路导出路径一致——桌面 IPC / CLI / 浏览器 HTTP 共用 cap

- **桌面 IPC**：`apply_export_omissions` 调 `cap_subagent_messages`。
- **CLI**：移除 `main.rs::export` 构造 `filtered_detail` 时的 `workflow_items: vec![]`，透传 `session_detail.workflow_items`（apply 阶段先验证 `engine.get_session_detail` 是否填充；未填充则 workflow 降级为普通 tool + tasks.md 记 deferred）；export 前对 `filtered_detail.chunks` 调同一 `cap_subagent_messages`。
- **浏览器 HTTP**（codex F7）：浏览器 `getSessionDetailForExport` 经 transport 映射到 `GET /api/sessions/{id}`，与首屏共用且 HTTP route 当前**不调任何 omission**（返回完整数据、无 cap）。新增 `?export=1` query 分支：HTTP `get_session_detail` route 见 `export=1` 时对结果调 `apply_export_omissions`（含 cap），否则保持现状（首屏完整不变）。`transport.ts` 把 `get_session_detail_for_export` 映射到 `/api/sessions/{id}?export=1`，与桌面 IPC `get_session_detail_for_export` 对称。

### D6：`messagesOmitted` 语义扩展不新增 IPC 字段

`messagesOmitted=true` 在 display 路径表示"全清空、可 `getSubagentTrace` 懒拉"，在 export 路径表示"封顶省略、静态导出无法补取"。两语境渲染层**统一标注"内部对话已省略"**，文案按导出语境措辞（无懒拉按钮）。**不新增 `messagesOmissionReason` 字段**（codex F6）——渲染层与导出消费方只需"是否省略"布尔，区分 byte/depth/display 原因无实际消费方需求；新增 IPC 字段会扩大 contract 面且需同步 ipc_contract test，收益不抵成本。该决策在 spec 注明 `messagesOmitted` 的双语境含义。

## Risks / Trade-offs

- **[payload 膨胀超预期]** → depth=1 砍递归爆炸 + per-subagent 2MB byte cap 截病态 subagent（真实 serde 计量）；导出非 hot path，单 subagent 2MB 量级可接受。
- **[嵌套子代理对话丢失]** → depth=1 下嵌套子代理只剩卡片摘要 + "已省略"标注。权衡：嵌套对话价值低、爆炸风险高；spec 明确此限制。
- **[BREAKING：导出数据策略反转]** → 修改 spec「导出数据完整性」subagent messages 条款（从"裁剪"改"封顶填充"）；display 路径不变，仅 export 路径行为变更。`messagesOmitted` 消费方（渲染层）兼容——从"恒 true 需懒拉"变为"可能 false（有内容）或 true（省略）"，渲染按布尔分支无破坏。
- **[CLI/HTTP engine 未填 workflow_items / 递归 messages]** → apply 阶段首先验证；未填充则 workflow 降级 + tasks.md 记 deferred，不阻塞四类渲染主交付。
- **[per-subagent serde 计量成本]** → 仅对导出路径保留的 subagent 各算一次 `to_vec().len()`，导出非 hot path，N 个 subagent 各一次序列化可接受；不在首屏/列表路径触发。
- **[CLI markdown subagent / teammate-message 不按 timestamp 穿插]**（codex PR 二审 W1）→ CLI `render_ai_body` 手写顺序：slash 最前 → semantic_steps 时序工具 → teammate messages 追加 → subagent 卡片末尾，**故意不严格 timestamp 穿插**（与既有 CLI subagent 堆末尾策略一致；CLI 不复用前端 `buildDisplayItems`）。spec「复用 buildDisplayItems 时序」约束**仅适用于前端 markdown/html**；CLI 是已知简化偏差，导出内容完整不缺失，仅相对顺序与视图不同。前端路径无此偏差。
- **[深链栈安全：`apply_image_omit` 在 depth-cap 前递归完整树]**（codex PR 二审 W3，**pre-existing**）→ `apply_export_omissions` 先跑 `apply_image_omit`（递归完整原始树）再跑 `cap_subagent_messages` 的 depth=1 闸门，故 depth-cap 不保护 image-omit 的栈深度。真实 Claude 会话子代理嵌套仅 2-3 层，数千层为对抗构造；该递归在本 PR 前已存在（非本次引入）。后续硬化（image-omit 也加 depth guard）记 GitHub Issue，不阻塞本交付。

## Migration Plan

- 纯增量：display 路径（首屏）与浏览器首屏（无 `export=1`）行为完全不变。
- 导出路径变更对用户是「导出件内容更完整」，无破坏回滚需求；如需回滚，`cap_subagent_messages` 退化为清空（`MAX_BYTES_PER_SUBAGENT=0` 或 `MAX_SUBAGENT_DEPTH=0`）即恢复旧行为。

## Open Questions

- `engine.get_session_detail`（CLI in-process）与 HTTP route 返回的 `subagent.messages` 递归层 / `workflow_items` 是否已完整填充——apply 阶段首先验证（tasks 1.4），决定 CLI/HTTP 是否需额外取数或仅渲染降级。

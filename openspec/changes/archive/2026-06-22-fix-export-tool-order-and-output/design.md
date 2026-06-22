## Context

会话导出有两个独立 bug，根因已通过通读源码 + codex 二审 100% 确认：

- **Bug 1（时序反转）**：`markdownExporter.ts::renderAIChunk`(69-89) 与 `htmlExporter.ts::renderAIHtml`(74-101) 先遍历 `semanticSteps`（其 step renderer 对 `tool_execution` / `subagent_spawn` 直接 `return ""`），再把**全部** `chunk.toolExecutions`、`chunk.subagents` 追加到末尾。于是工具调用 / subagent 卡片永远排在最终文本之后。而 `displayItemBuilder.ts::buildDisplayItems`（SessionDetail 视图所用）早已实现正确的时序合并：以 `toolUseId → ToolExecution`、`sessionId → SubagentProcess` 两张表解析 `semanticSteps` 内的引用 step，落入 `pool` 后按 `timestamp` 稳定排序。导出器只是没复用它。

- **Bug 2（output 全空）**：桌面导出 `SessionMetaMenu.svelte::doExport`(191) 复用 `get_session_detail` IPC，该 command(`lib.rs:78`) 调 `apply_omissions()` → `apply_display_omissions` → `OMIT_TOOL_OUTPUT=true` 把 `tool_executions[].output` 的 inner 字段 trim 空（保留 variant kind）+ 设 `output_omitted=true`。导出复用此瘦身 payload，故 output 全空。浏览器 `?http=1` 路径不调 omit，故不空——与"桌面全空"现象吻合。

约束：本仓核心动机是性能，首屏 `get_session_detail` 热路径 SHALL 零回归（`.claude/rules/perf.md`）。IPC 改动 SHALL 走字段改动 checklist（`src-tauri/CLAUDE.md`）。

## Goals / Non-Goals

**Goals:**
- 导出 Markdown / HTML 内容按时间顺序穿插 thinking / text / tool / subagent，与 SessionDetail 视图一致。
- 三种格式（Markdown / JSON / HTML）的工具调用都带真实 output。
- 首屏 `get_session_detail` 行为与性能零改动。
- 导出排序逻辑与视图**单一真相源**（不再前端各写一套）。

**Non-Goals:**
- 不做按需懒拉取 tool output（Plan A 否决）。
- 不修 teammate / slash / workflow 内容在导出中缺失（另开 issue）。
- 不改 toolOutputMode 的 truncated / name-only 既有语义。
- 不在导出中引入 subagent 内部消息 / 图片渲染（导出器本就不渲染）。

## Decisions

### D1：Bug 1 复用 `buildDisplayItems` 而非在导出器内重写排序

导出器改为 `const { items, lastOutput } = buildDisplayItems(projectedChunk)`，按 `items` 顺序渲染各 DisplayItem，末尾再渲染 `lastOutput`（最终文本）。删除原"末尾统一堆 toolExecutions / subagents"两个 loop。

- **为什么**：`buildDisplayItems` 已是 spec-backed + 测试覆盖的时序合并真相源（`session-display` "Subagent 卡片与 Task tool 就地交错渲染"）。导出器自写一套排序必然与视图漂移——本 bug 正是漂移的结果。复用后导出与视图**永远一致**。
- **拒绝方案**：在导出器内用 `startTs` / `spawnTs` 重写一遍合并排序。被拒：重复实现、漏掉 Task/Agent 去重（`taskIdsWithSubagents`）与 teammate_spawn 替换等边角，未来视图改了导出又漂。
- **lastOutput 拼回规则（codex warning 回应）**：`buildDisplayItems` 把"最后一个 text step"抽出为 `lastOutput`、不放进 `items`。导出**先渲染 `items` 再追加 `lastOutput`**——这恰好镜像 SessionDetail 视图布局（`items` 在滚动区按 ts 排序、`lastOutput` 作为末尾常驻最终消息，`SessionDetail.svelte`）。spec 的目标是"导出顺序与视图一致"，故 items+lastOutput 追加即正确。理论上"最后一段文本之后还有 tool"在真实 JSONL 不发生（一个 turn 以 assistant 文本收尾，tool_use→tool_result→assistant 续写），测试用"工具夹在两段文本之间"锁定顺序。

### D2：用**非缓存** `buildDisplayItems`，不用 `buildDisplayItemsCached`

导出调 `buildDisplayItems`（纯函数），**不**调 `buildDisplayItemsCached`。

- **为什么**：导出喂入的是 `projectSessionDetail` **投影后**的 chunk（toolOutputMode 截断 / 过滤 thinking）。`chunkDigest` 只编码 `toolUseId / endTs / output.kind / outputOmitted` 等，**不**编码 output 文本是否被截断——投影后 chunk 与原始 chunk 会算出**相同 digest**，若走 cached 版会与 SessionDetail 视图的缓存项**撞键互污**（视图渲染到截断数据，或导出读到视图数据）。非缓存版是纯函数、一次性调用、O(steps)，无此风险。
- **代价**：导出时不复用视图暖缓存，但导出是一次性动作，可忽略。

### D3：Bug 2 走后端导出专用全量 IPC command（Plan B），否决前端懒拉取（Plan A）

新增 Tauri command `get_session_detail_for_export(projectId, sessionId) -> SessionDetailResponse`：内部调同一 `api.get_session_detail`（始终传 `None` fingerprint，强制 `Full` variant），返回前调**导出专用裁剪** `apply_export_omissions`（见 D4），再序列化。前端 `doExport` 改调此 command。

- **为什么否决 Plan A（前端对每个 omitted output 调 `get_tool_output` 懒拉取）**：大会话有成百上千 tool execution，逐个懒拉取 = N 次 IPC 往返，正是 perf.md 明令禁止的"串行 await / 重复 IPC"反模式。导出一次性动作，单次全量往返更优。
- **为什么新增独立 command 而非给 `get_session_detail` 加 bool 参数**：首屏 command 带 fingerprint 缓存语义（`Unchanged` 分支），把"裁剪策略"耦合进去会污染热路径协议且易误用。独立 command 职责单一，首屏协议零改动。

### D4（修订，回应 codex critical + Q1 perf）：导出保留 tool-output + response-content，仍裁剪 image + subagent-messages

> **原 D4 被否**：原计划"导出 omit 只跳过 tool-output、保留 image/response-content/subagent-messages 裁剪"。codex critical 指出：JSON 导出是 `JSON.stringify(整个 projected detail)`（`jsonExporter.ts`），不是只渲染 semanticSteps —— 保留 response-content 裁剪会让 JSON 出现 `responses[].content: ""` + `contentOmitted: true`，违背本 change 修"导出完整性"的目标。
>
> **"完全不裁剪"亦被否**：一度考虑导出返回完整 detail（含 image base64）。但 codex Q1 指出 JSON 在主线程 `JSON.stringify`，image-heavy 会话会 OOM / 冻 UI；且**完整 image base64 inline 正是 perf.md 列明的"IPC 整页 base64 inline"反模式**，与用户"不要引入性能问题"的硬要求冲突。

**修订决策**：新增 `apply_export_omissions(chunks)` = 仅 `apply_image_omit` + 清空 subagent `messages`（保留各自 `*Omitted` 标志），**不**裁剪 tool-output、**不**裁剪 response-content。`SessionDetailResponse` 新增 `apply_export_omissions(&mut self)` 方法，导出 command 调它。

- **保留 tool-output + response-content**：这两项是导出器实际消费的内容（tool output 是 Bug 2、response-content 是 JSON critical）；都是文本/结构化、体积有界。
- **裁剪 image + subagent-messages**：image base64 是 payload 大头（反模式）；subagent 内部 messages 体积可观且**任何导出器都不渲染**（MD/HTML 只渲染 subagent 单行 header，JSON 消费者也不依赖嵌套 conversation——其渲染已移入范围外 issue）。裁掉这两项把导出 payload 控在有界、规避 Q1 的 OOM/卡顿。
- **JSON 自描述**：image / subagent messages 在 JSON 中以 `dataOmitted:true` / `messagesOmitted:true` 自描述，非"丢数据"。

### D5：IPC 字段改动 checklist 四处同步

新 command SHALL 同 PR 同步：(a) `ipc_contract.rs::EXPECTED_TAURI_COMMANDS` + contract test；(b) `api.ts` 新增 `getSessionDetailForExport`；(c) `tauriMock.ts::KNOWN_TAURI_COMMANDS`；(d) `src-tauri/src/lib.rs::invoke_handler!`。

### D6（新增，回应 codex warning #5）：浏览器/HTTP transport 复用既有 `get_session_detail`，不新增 HTTP route

前端 `getSessionDetailForExport(projectId, sessionId)` 按运行时分叉：

- **Tauri 桌面**：`getTransport().invoke("get_session_detail_for_export", ...)`（新 command，保留 tool-output + response-content、裁剪 image + subagent-messages）。
- **浏览器/HTTP**：直接复用既有 `getSessionDetail(projectId, sessionId, null)` —— HTTP `get_session_detail` 路由本就不裁剪、返回完整 detail（已 grep 确认 `routes.rs` 不调 omissions），**无需新增 HTTP route**。null fingerprint 强制 `Full`，无 `Unchanged` 空壳风险（codex Q3 确认）。

**一致性边界**：Markdown / HTML 导出 output 桌面与浏览器**完全一致**（两端都不渲染 image / subagent messages）。JSON 导出仅在 image data / subagent messages 上有差异（桌面裁剪、浏览器保留），二者 tool-output + response-content **一致非空**——spec 的"桌面与浏览器一致" Scenario 即按 tool-output + response-content 这一范围约束。`transport.ts` 需为 Tauri 分支登记 `get_session_detail_for_export`。

### D7（新增，回应 codex warning #3）：`includeSubagents` 与 Task tool 去重的等价性

`buildDisplayItems` 在有 subagent 关联时会按 `taskIdsWithSubagents` **跳过** Task/Agent 工具调用（subagent 卡片就是该调用的可视代表）。导出复用它后须保证 `includeSubagents` 行为不丢内容：

- **`includeSubagents = true`（默认）**：subagent 保留在 chunk → 渲染 subagent 卡片（单行 header 摘要）、Task 工具被去重不重复渲染（镜像视图）。
- **`includeSubagents = false`**：`projectSubagents` 改为**整体丢弃 subagents（返回 `[]`）**，而非现状"仅清空 messages"。这样 `buildDisplayItems` 看不到 subagent → `taskIdsWithSubagents` 为空 → **Task 工具正常渲染为普通工具**、无 subagent 卡片。否则（保留 subagent 元数据但导出层再过滤 SubagentItem）Task 与 subagent 会**双双消失**。
- **既有 spec 修订**：`session-export` "子代理内容导出" 的两个 Scenario 据此 MODIFIED。导出渲染 subagent **内部对话消息**（嵌套 conversation）仍未实现 —— 与 teammate / slash / workflow 渲染缺失一并移入范围外 issue。

## Risks / Trade-offs

- **[导出 payload / JSON.stringify 卡顿（codex Q1）]** → 裁剪 image + subagent-messages 后，导出 payload 主要由 tool-output + response-content 构成（文本/结构化、有界），规避 image base64 inline 反模式与超大 JSON 主线程 stringify 的 OOM/冻 UI。`doExport` 既有 `try/catch → setFeedback("export-fail")` 兜底残余失败。不设懒拉取（N 往返更糟）。
- **[buildDisplayItems 渲染了导出器未覆盖的 DisplayItem 类型（slash / teammate / workflow / subagent 内部消息）]** → 导出器 render switch 对未覆盖类型走 default 跳过（与当前行为一致，无回归），缺失项另开 issue。
- **[投影后 chunk 喂 buildDisplayItems 的字段完整性]** → 投影只动 `semanticSteps` / `toolExecutions` / `subagents` 三字段且保持 shape，`buildDisplayItems` 所需 `timestamp` / `toolUseId` / `sessionId` 均保留；D2 已规避缓存撞键。
- **[误改首屏裁剪]** → `apply_display_omissions` / `get_session_detail` command 零改动；导出走**独立** command + **独立** `apply_export_omissions`，物理隔离，首屏行为不变（contract test 加一条断言首屏仍裁剪 tool-output）。

## Migration Plan

纯增量：新增 command + 新增 `apply_export_omissions`（local.rs）+ `SessionDetailResponse::apply_export_omissions`（types.rs）+ 重写两个导出器函数 + 调整 `projectSubagents` false 分支 + 前端 transport 分叉。无数据迁移、无持久化格式变更。回滚 = 还原前端导出器 + 删新 command/函数（首屏不依赖任何新增物）。

## Open Questions

无。teammate / slash / workflow / subagent 内部消息的导出渲染已明确移出本 change 范围，另开 issue。

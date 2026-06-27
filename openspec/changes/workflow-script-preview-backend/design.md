## Context

后端 tool linking 已提取 `ToolExecution.workflow_script_path`（`pair.rs:107`，优先 `toolUseResult.scriptPath`，缺失回退 `tool_use.input.scriptPath`；inline `{script}` 形态两处皆无 → `None`），但该值只停在 `ToolExecution`，从未流进 IPC 发出的 `WorkflowItem`。`WorkflowCard.svelte` 的 "View script" disclosure 消费 `WorkflowItem.scriptPreview`，前端契约已就绪（`api.ts` interface、fixture、a11y 化 disclosure，PR #562），唯独后端不发该字段。

现状解析链路（`crates/cdt-api/src/ipc/workflow_manifest.rs`）：
- `collect_workflow_candidates(chunks)` → `Vec<(run_id, Option<script_path>)>`，按 run_id 去重。
- `resolve_workflow_items` 对每个 candidate 调 `resolve_single`。
- `resolve_single`：manifest 存在 → `parse_manifest`（**不读 script**）；manifest 缺失（NotFound）→ `resolve_running_state`。
- `resolve_running_state` 已调 `read_script_meta` 读 script 文件解析 `ScriptMeta`（name + phases），按 `FileSignature` 缓存——但读完即丢弃文件内容。
- `get_workflow_detail` IPC 走 `resolve_single_detail` → 同一 `resolve_single`。

前端 `effectiveWorkflow = fullDetail ?? workflow`，`fullDetail` 仅在 `startPoll()`（running/pending workflow 展开时）由 `getWorkflowDetail` 填充；**completed workflow `fullDetail` 恒 null** → 用 list payload 的 `workflow` prop。因此 preview 必须进 `get_session_detail` 的 list payload，无 lazy 路径可依赖。

## Goals / Non-Goals

**Goals:**
- `WorkflowItem` 填充 `scriptPreview`，inline 与 scriptPath 两形态都能显示。
- 大脚本不撑爆 IPC payload（截断 + bounded 缓存内存）。
- 前端零改动，满足 `session-display`「Script disclosure 默认折叠」scenario。
- 不在 `get_session_detail` 热路径引入无界 I/O 或重复读。

**Non-Goals:**
- 不做 lazy-load IPC（`get_xxx_lazy`）——completed workflow 无 lazy 触发点，且截断后 payload 已 bounded，lazy 反而增加一次 round-trip。
- 不改前端（disclosure、字段、fixture 已就绪）。
- 不在 `WorkflowItem` 加单独的 `scriptPreviewTruncated` bool——见 D3。
- 不解析脚本语义/语法高亮——preview 是原文预格式化块。

## Decisions

### D1: preview 在 `resolve_single` 统一填充（单插入点）

`resolve_single` 拆出 `resolve_single_inner`（产出不含 preview 的 item）；`resolve_single` 在 inner 返回后调一次 `resolve_script_preview(script_path, inline_script, fs, cache)` 设 `item.script_preview`。理由：`resolve_single` 有多个早返回分支（stat io-err / read fail / running-state / manifest 成功），逐分支设 preview 易漏；wrapper 单点设置覆盖所有正常产出路径。`resolve_single_detail`（`get_workflow_detail`）走同一 `resolve_single`，list 与 detail 自动一致。

### D2: 两形态来源分流，inline 零 I/O

`collect_workflow_candidates` 扩展为携带 `inline_script`（从 `exec.input.get("script")` 取，`ToolExecution.input` 已驻内存）。`resolve_script_preview`：
- `inline_script` 非空 → 直接截断该字符串，**零文件 I/O**。
- 否则 `script_path` 非空 → `read_script_data(path)` 读文件（缓存）取 preview。
- 两者皆无 → `None`。

inline 优先于 scriptPath：inline 形态 input 必带 `script`，scriptPath 形态 input/result 带路径，二者互斥。

### D3: 截断到 32 KB + 串内可见 marker，不加结构化 bool 字段

`const SCRIPT_PREVIEW_MAX_BYTES: usize = 32 * 1024`。超限时按 **UTF-8 char 边界**截断（脚本含中文，见 `workflow_script.rs` 测试），尾部追加 `\n\n/* … script truncated, <total> bytes total … */`。

读取侧 bounded（codex #7）：scriptPath 文件读前先看 `fs_meta.size`，超 `const MAX_SCRIPT_READ_BYTES: usize = 1 MB`（远高于 Workflow inline script 512 KB 上限，覆盖一切合法脚本）则**不**全量 `read_to_string` 进内存，preview 仅置一行 oversize marker（标注总字节数）、meta 置 `None`（降回 Tier 0 name）。这把异常大文件的瞬态内存 bound 在 1 MB，正常脚本（≪ 32 KB）完全不受影响。

不加 `scriptPreviewTruncated: bool` 的理由：仓内 `xxxOmitted` flag 模式存在是因为**前端消费它走 fallback 链**；本场景前端只渲染 `<pre>{scriptPreview}</pre>`，不消费 truncated 标志。串内 marker 让用户在 `<pre>` 块直接看到截断（满足 `perf.md`「no silent caps」honesty），且零前端改动、零额外契约同步面。32 KB 足以审计脚本结构（meta + phase/agent 编排骨架），多 workflow 场景 N × 32 KB 仍远低于 1 MB IPC 预算。

### D4: script 读缓存合并 meta + preview（避免双读）

`read_script_meta` 改名 `read_script_data`，返回 `ScriptData { meta: Option<ScriptMeta>, preview: Option<String> }`，从**同一次**文件读同时派生 meta（`parse_script_meta`）与 preview（截断），按 `FileSignature` 缓存整个 `ScriptData`（含读/解析失败的负缓存）。`resolve_running_state` 取 `.meta`（name+phases）；`resolve_script_preview` 取 `.preview`。

并发语义（codex #6）：沿用既有 `read_script_meta` 的「短锁查缓存 → 释放锁 → await 读盘 → 重新锁 insert」模式（`std::sync::Mutex` 本就不能跨 await 持有）。**稳态**下同一 script 命中缓存复用、零重读；**并发 miss 窗口**（两个 `get_session_detail` 同时首扫同 session）可能短暂双读，但读盘幂等、结果一致，无害。故声明为「稳态命中复用」而非「全局仅一次读」。

### D5: 热路径增量与门控

- 无 Workflow 的 session：`collect_workflow_candidates` 返回空 → `resolve_workflow_items` 早返回，零增量。
- completed workflow（manifest 存在）：新增对 scriptPath 文件的读——但按 `FileSignature` 缓存（script immutable，写后不变），每进程首次读、后续仅 stat。小文件异步读，几个 workflow 累计 < 几 ms，在 `get_session_detail` < 800ms 预算内。
- inline workflow：零文件 I/O。
- 错误分支与 preview 的独立性（codex #5 澄清）：manifest stat/read 失败 → item 是 `pending` placeholder（**status** 维度的降级）。但 `script_preview` 来源（inline `input.script` 在内存 / scriptPath 是**另一个**文件）与 manifest 读取**相互独立**——wrapper 仍按 D1 统一调 `resolve_script_preview` 填充 preview。即「manifest 读不到」不蕴含「脚本读不到」，pending placeholder 仍可带 preview（脚本仍可审计）。仅当 script 来源本身也缺失/读失败时 preview 才为 `None`。

### D6: detail 路径 preview = None（已知限制，非本 change 引入）

`get_workflow_detail`（前端轮询 running/pending workflow 用）的调用上下文不携带 chunks，固定传 `script_path = None` 且无 inline script——故 detail payload 的 `scriptPreview` 为 `None`。这与该路径**当前已有**的「不重建 `name`/`phases`」同源限制一致（`local.rs:4459` 传 None）。影响面：running workflow 展开后轮询返回时，前端 `effectiveWorkflow = fullDetail ?? workflow` 用 detail 替换 list，preview 短暂消失。**不影响主用例**——completed workflow `isTerminal=true` 不触发轮询，恒用 list payload 的 preview。改进（让 detail 也带 preview）需 read_dir 重建 script_path，是每 3s 轮询的 I/O 增量，与「不引入性能问题」冲突，故记为后续 issue 候选，不在本 change。

## Risks / Trade-offs

- **热路径 I/O 增量**：completed workflow 首次打开 session 读 script 文件。缓解：FileSignature 缓存 + 截断读 + 仅 scriptPath 形态触发。回滚成本低（preview 设置是单点 wrapper）。
- **32 KB 截断丢尾部**：超大脚本尾部不可见。可接受——审计价值集中在 meta + 编排骨架（脚本头部）；marker 明示截断，用户知情。若日后需全量可加 lazy IPC（Non-Goal）。
- **inline script 占 input 内存**：inline 形态 `ToolExecution.input` 本已持有全量 script（pre-existing，非本 change 引入）；preview 截断后另存 ≤ 32 KB。无新增显著内存。
- **路径越权**：scriptPath 来自 session JSONL（Claude Code 自身写入的可信数据），且 `read_script_data` 复用既有 `read_script_meta` 的读路径（现状已无条件读该路径）——本 change 不扩大攻击面，不新增越权读。

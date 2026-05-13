## Context

仓库现状：

- `cdt-watch::FileWatcher` 递归监视 `~/.claude/projects/`，但 `parse_project_event` 只识别 2 层路径 `<projects>/<project_id>/<session_id>.jsonl`（`crates/cdt-watch/src/watcher.rs:165`）。
- 主会话 JSONL 是 2 层；subagent JSONL 在新结构下是 4 层 `<projects>/<project_id>/<session_id>/subagents/agent-<sub_session_id>.jsonl`（`crates/cdt-api/src/ipc/local.rs:1539`）。watch 阶段直接 drop 这类事件。
- IPC 层 `OMIT_SUBAGENT_MESSAGES=true`（默认）把每个 subagent `messages` 裁成空 Vec、`messagesOmitted=true`，前端 `SubagentCard` 用 `getSubagentTrace` 懒拉一次后写入 `messagesLocal` 永久 sticky（`ui/src/components/SubagentCard.svelte:29-52`）。
- 后端 `cdt-parse::is_task` 同时识别 `Task` / `Agent` 两个工具名为 task 调用（`crates/cdt-parse/src/parser.rs:187`）并尝试匹配 `SubagentProcess`；前端 `displayItemBuilder.ts:167` 跳过 `ToolItem` 渲染的判定只对 `toolName === "Task"` 成立。

ongoing 会话中三处问题叠加，让用户感觉 subagent 调用链完全冻结。本 change 一次性修齐契约 + 实现 + 测试。

## Goals / Non-Goals

**Goals**

- ongoing subagent 内部 JSONL 追加 SHALL 触发父 session 的 `file-change` 事件，订阅者无需扫描磁盘。
- ongoing subagent 卡片在用户展开后 SHALL 在每次父 session refresh 后看到最新 messages（无需折叠重开）。
- `Agent` 工具调用关联到 SubagentProcess 时 SHALL 与 `Task` 工具享受同等的"跳过 ToolItem，由 SubagentCard 承担明细渲染"待遇。
- 三处修复共同覆盖一个 IPC contract test 用例（`Agent` 工具 + ongoing subagent + 嵌套追加），让回归一眼可见。

**Non-Goals**

- 不改 `OMIT_SUBAGENT_MESSAGES` 默认值（性能上是 phase 1-5 砍 88% 的关键开关，保持 true）。
- 不重写 SubagentCard 的 trace 拉取协议；仅扩展 invalidate 与重拉触发。
- 不解决旧结构 `<project>/agent-<sid>.jsonl` 的 watch 路由（Claude Code 已淘汰；watch 阶段不允许做 IO 解析 parentUuid）。如未来确实需要，开新 change。
- 不改 fileChangeStore 的 250 ms trailing 节流（先复测够不够，不够再单独提改动）。

## Decisions

### D1：cdt-watch 在 `parse_project_event` 内识别 subagent 嵌套路径

**问题**：4 层 subagent JSONL 路径被 `components.len() != 2` 直接拒绝。

**候选方案**

- **A.1（采纳）**：在 `parse_project_event` 内追加分支——若 `components.len() == 4 && components[2] == "subagents"` 且 `components[3]` 形如 `agent-*.jsonl`（排除 `agent-acompact*.jsonl`），则解析出 `project_id = components[0]`、`session_id = components[1]`（父 session UUID），照常 emit `FileChangeEvent`（与父 session JSONL 同 channel、同 payload schema）。`mark_project_seen` 也对父 `project_id` 调一次（理论上父 JSONL 已注册过，这里幂等）。
- **A.2**：把 subagent 文件路由到独立的新 broadcast channel + 新 Tauri event 名。前端单独订阅。
- **A.3**：放弃 watch，让前端 ongoing subagent 卡片自行 polling。

**取舍**

- A.1 复用既有 `file-change` channel，**前端订阅链路零改动**，`SessionDetail::refreshDetail` 自然把新 subagent 内容拉进来；缺点是父 session refresh 频率上升（见 R1 风险）。
- A.2 信号更精细但前端要写两套订阅 + 合并逻辑，且 IPC contract 多一类事件——投入大、收益小。
- A.3 polling 是反 spec 行为（file-watching capability 的存在意义就是去 polling），且新 polling 会和已有 file-change 互相打架。

**结论**：A.1 是最小契约改动。新增 Requirement 写在 file-watching spec，明确"嵌套 subagent JSONL 路由到父 (project_id, session_id)"。

**强制约束（codex 二审发现）**：嵌套分支 emit 的 `FileChangeEvent.project_list_changed` MUST 固定为 `false`，**不**走既有 2 层路径的 `!deleted && mark_project_seen(project_id)` 派生逻辑。理由：若 `mark_project_seen` 返回 `true`（父项目首次出现于 watcher 记账——极端 race 下父 session JSONL 还没触发过事件、但子 session 已开始写），`project_list_changed=true` 会让前端 `DashboardView` / `Sidebar` 误以为新项目出现而刷新整个项目列表。嵌套分支只应当是"父 session 内部增量"信号，**不**应当影响项目列表 UI。spec delta 与 tasks 1.1 SHALL 显式 hard-code 嵌套分支 `project_list_changed: false`。

### D2：在 `Process`（`SubagentProcess`）上加 `messagesTotalCount` 字段 + 前端 version-keyed 重拉

**问题**：`messagesOmitted=true` 时 `process.messages.length === 0`，前端拿不到任何"messages 数量变化"的信号；`endTs` 在 ongoing 期间一直是 None；`lastIsolatedTokens` 不一定每条消息都涨；都无法独立做版本指纹。

**候选方案**

- **B.1（采纳）**：后端新增 `messagesTotalCount: u32` 字段，记录裁剪前 `cand.messages.len()`。`OMIT_SUBAGENT_MESSAGES=false` 回滚路径也填同一字段（等于 `messages.len()`），让前端始终用同一字段做版本判断。前端 SubagentCard 派生 `messagesVersion = "${isOngoing|0/1}|${endTs ?? '_'}|${messagesTotalCount}"`，在 `$effect` 中检测版本变化：若用户**已展开**该卡片且 `isOngoing=true`，主动调 `getSubagentTrace` 重拉并刷新 `messagesLocal`。未展开时仅清空 stale cache，等下次展开再走原 lazy 路径。
- **B.2**：ongoing 期间后端不裁剪 ongoing subagent 的 messages（仅裁剪已完成的）。
- **B.3**：前端每次父 detail 刷新时无条件清空 `messagesLocal`，让 lazy 路径每次展开重拉。

**取舍**

- B.1 后端只多一个 `u32`，前端只加一个 `$effect`，能精准捕捉 "messages 有新增" 这个唯一关键事件，不引入冗余 IPC（未展开就不拉）。
- B.2 改后端 omit 策略，性能回退面大——subagent 在跑过程中 messages 增长可观，OMIT phase 节省的 ~40% payload 失效；而且 ongoing 翻转后到下一次 refresh 之间窗口期 messages 又会被裁，行为不一致。
- B.3 简单粗暴，但 ongoing 会话中每次父 refresh（哪怕只是父 JSONL 自己写 1 条 user 消息）都会让所有展开过的卡片陷入 loading，UX 倒退。

**结论**：B.1。Spec 在 ipc-data-api 现有 "Trim subagent messages..." Requirement 上扩展 `messagesTotalCount` 字段契约；在 `Lazy load subagent trace` Requirement 上加一条 Scenario 描述"ongoing + 版本递增 + 已展开 → 主动重拉"。

**细节约束**

- `messagesVersion` MUST 包含 `isOngoing`——`isOngoing` 翻转到 false 时同时 `endTs` 出现，触发最后一次重拉以同步 final 状态。
- 主动重拉走 `getSubagentTrace` IPC，复用既有 lazy 拉取协议；前端 `inflight` 复用 key MUST 是 `${sessionId}|${messagesVersion}`（即 sessionId + 版本指纹联合 key），**不**仅按 sessionId 复用。理由（codex 二审发现）：仅按 sessionId 复用时，第一次 `getSubagentTrace` 在版本 N 时发起、pending 期间版本递增到 N+1，新触发的重拉若复用旧 Promise 会把版本 N 的旧 trace 写入 `messagesLocal`，且因 effect 已认为"已在拉取中"而不再排第二轮——版本 N+1 的新 chunks 永远拿不到。Promise settle 后 SHALL 再检查"当前版本 == fetch 时版本"，不等则视为 stale 并立即触发新一轮（或采用版本绑定 key 自然让两次成为两个 Promise）。
- 未展开（`isExpanded === false`）时不发 IPC——避免大会话 ongoing 期间 N 个未展开卡片每次 refresh 都狂拉。effect 内的"已展开"判定 MUST 用 `isExpanded` 而非 `messagesLocal !== null`（implementation 阶段 codex 二审 C1 反转）：首次展开 `ensureMessages` 的 `await` pending 期间 `messagesLocal` 仍为 `null`，若 effect 拿 `messagesLocal` 判 guard 会短路掉新版本的接管 fetch，旧 fetch settle 后 stale trace 被固化。

### D2b：`ensureMessages` 严格版本匹配 + IPC 失败保持 null（implementation 阶段 codex 二审反转）

D2 初版 `ensureMessages` 的 race-check 写成 `currentVersion === fetchedVersion || messagesLocal == null`——"版本不匹配但 messagesLocal 还没值时也兜底写入"。这条 `|| null` 是 C1 race 的根本机制：首次展开 fetch 期间 version 跳变，effect 因 D2a 的旧 guard 短路、不发新 fetch，旧 fetch settle 时 `messagesLocal` 仍是 `null`，`|| null` 命中把 stale trace 固化。

修正后 `ensureMessages` 严格判 `currentVersion === fetchedVersion`，不匹配时 SHALL NOT 写入。由 D2a 修正后的 effect（用 `isExpanded` 作 guard）发起新版本 fetch 接管显示。

IPC 失败处理同样反转（C3）：早期把 `messagesLocal = []` 当作"loading 兜底"，但 `[]` 命中 `if (messagesLocal != null) return;` guard，下次用户折叠重开**不会**再调 IPC，永久卡死。修正后 catch 内**不**改动 `messagesLocal`（保持 `null`），让 guard 在下次展开时落到 fetch 路径重试。视觉上 `null` 与 `[]` 的 effectiveMessages 都是空数组（fallback 到 `process.messages = []`），用户体感一致但有重试入口。

### D3：前端 `displayItemBuilder` 与后端 `is_task` 工具名集合对齐

**问题**：后端 `cdt-parse::is_task` 已经把 `Task` / `Agent` 都视为 task 调用并尝试关联 SubagentProcess，但前端只跳 `toolName === "Task"`，让 `Agent` 工具即使被后端关联也仍走默认 ToolItem 渲染。

**候选方案**

- **C.1（采纳）**：`displayItemBuilder.ts:167` 改判 `(exec.toolName === "Task" || exec.toolName === "Agent") && taskIdsWithSubagents.has(exec.toolUseId)`。集合 `taskIdsWithSubagents` 已由 `chunk.subagents[*].parentTaskId` 自动构造，与后端 `is_task` 决定的 tool 集合天然一致。
- **C.2**：在后端额外暴露一个 `isTaskTool: bool` 字段挂到每个 `ToolExecution` 上，前端只信这个 flag。
- **C.3**：把 `is_task` 工具名清单提到 `cdt-core` 常量并通过 IPC 暴露给前端，前端运行时构建集合判定。

**取舍**

- C.1 改 1 行代码，spec 增 1 条 Requirement 明确"前端 task tool 关联跳过判定 MUST 与后端 `is_task` 集合对齐"。约束清晰但**未来若后端再加一个工具名（比如 `SpawnAgent`）容易漏改前端**。
- C.2 增加 IPC payload 一字段，前后端解耦更彻底，但 contract test 多覆盖一字段，且 `isTaskTool` 在 ToolExecution 上语义略偏（这是关联结果而非工具属性）。
- C.3 最干净（动态 alignment），但 `cdt-core` 暴露常量给 webview 需要新 IPC `get_task_tool_names()` 或在 session detail 里 inline 一份，复杂度过大。

**结论**：C.1 + spec Requirement 显式锁定"工具名集合 SHALL 与 `cdt-parse::is_task` 完全一致"。后续若再加工具名，spec 改动会强制提醒前后端同步。tasks.md 加 IPC contract test 覆盖 `Agent` 工具关联 case，作为回归保险。

## Risks / Trade-offs

- **R1 修复后大会话 ongoing 期间刷新频率上升**：父 session refresh 频率从"只跟父 JSONL 写入对齐"变为"跟父 + 所有子 session 写入对齐"。Mitigation：`fileChangeStore` 现有 250 ms trailing 节流（`ui/src/lib/fileChangeStore.svelte.ts:135-159`）合并所有相同 key 的刷新；新增 file-change 仍走同 `${projectId}|${sessionId}` key，trailing 自动合并。验证手段：复测 `cdt-api/tests/perf_get_session_detail.rs` 在 ongoing 大会话场景下的 IPC 时长是否回归。
- **R2 主动重拉的 IPC 风暴**：单次 refresh 内 N 个展开卡片各拉一次 `getSubagentTrace`。Mitigation：B.1 用 sessionId-keyed 简单 inflight（`Promise` map）合并短时重复；未展开卡片不拉。Stretch：若仍有问题，让 `get_session_detail` 把 ongoing subagent 的 `messages` 直接附带（短期回到 OMIT=false 路径），但要看 payload 增量，不在本 change 范围。
- **R3 工具名集合后续扩张容易漏改前端**：若 `cdt-parse::is_task` 再加新名字、`displayItemBuilder.ts` 没同步，行为契约会再次裂开。Mitigation：spec Requirement 强制约束 + IPC contract test 用 fixture 覆盖每个工具名；rust-conventions-reviewer 与 ui-reviewer 在合并前过一遍。
- **跨 crate 字段加固阻塞 PostToolUse clippy hook**：`messagesTotalCount` 加在 `Process` / `SubagentProcess` 等 `cdt-core` 结构上时按 CLAUDE.md `cdt-core 核心 struct 加字段先 grep 全构造点` 原则，所有构造点同一轮 Edit 补齐；新字段写成 `u32 + #[serde(default)]` 让老 fixture 与旧前端无破坏。

## Migration Plan

- 不涉及数据迁移；纯代码 + 契约改动。
- 上线后旧版前端读到新后端：`messagesTotalCount` 字段被忽略（无 `serde(default)` 也没关系，前端 JSON.parse 不读就行），sticky bug 仍在但不会崩。
- 新版前端连旧后端：`messagesTotalCount` 缺失视为 `undefined`，版本指纹永远是 `'_|_|undefined'`，主动重拉 effect 永不触发，行为退化为旧版本，安全。
- file-watching 新增分支对旧消费者 100% 向后兼容（FileChangeEvent schema 不变，只是多了一类来源）。

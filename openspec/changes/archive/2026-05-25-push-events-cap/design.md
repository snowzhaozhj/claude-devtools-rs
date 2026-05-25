## Context

PR #306（archive `spec-overhaul-file-watching-pilot`）的 `design.md::D-1/D-2/D-3/D-4` 已论证：跨进程 push event payload 字段形态契约**应**新建 `push-events` capability 作为单一 owner，让 `ipc-data-api`（当前 2493 行 / 41 Requirement，是仓库内最大 spec）与 `http-data-api` 各自简化为 transport 桥契约。本 PR 是该决策的落地（issue #303 9-PR 序列的 PR 2）。

当前现状（`grep -n -E "file-change|FileChange|PushEvent|sessionListChanged|session_list_changed|SseLagged|sse-lagged|SessionMetadataUpdate|session-metadata-update|DetectedError|detected-error" openspec/specs/<cap>/spec.md`）：

- `ipc-data-api`：file-change schema 整段 SHALL（含 `sessionListChanged` / camelCase / serde tag / `SseLagged` 形态）+ 6 处 Scenario（`Tauri 转发 file-change 事件` / `file-change payload 是 camelCase` / `已知 project 下新 session 首次出现 sessionListChanged 为 true` / `普通 JSONL append sessionListChanged 为 false` / `HTTP/SSE PushEvent::FileChange 携带 session_list_changed 字段` / `PushEvent::SseLagged 序列化形态与 sentinel 兼容`），是事实上的 schema owner。
- `http-data-api`：第 95 / 277 / 308-324 行（SSE PushEvent 桥接 + sse_lagged sentinel + ensureSseReady race）。
- `frontend-test-pyramid`：第 31 / 39 行硬编码 mockIPC 的 4 条事件名 SHALL（`notification-update` / `notification-added` / `file-change` / `session-metadata-update`）。
- `notification-triggers`：第 118 行 `FileSignature` 自然恢复机制提到 file-change。
- `sidebar-navigation`：50+ 处 file-change / session-metadata-update / sse-lagged 引用，含字段语义直接复制（如 `payload.sessionListChanged` 第 736 行）。
- `session-display`：10+ 处 file-change 引用，含字段定义直接复制。

约束：

- 不动后端代码 / 测试（spec 文档纯文本重写）。
- 不改任何 push event 字段名 / 字段语义 / serde tag（行为契约语义不变；仅协议 owner 切换）。
- 同 commit 刷 `scripts/spec-purity-baseline.txt`（propose 期 + archive 期两次，详 `D-3`）。
- 沿用 PR #306 D-3 先例：主 spec Purpose 段直 edit（OpenSpec 的 spec delta 架构不解析 Purpose section，无走流程的可选项）。

## Goals / Non-Goals

**Goals**：

- 新建 `push-events` 主 spec（ADDED capability），按 `openspec/SPEC_GUIDE.md` 4 层骨架（Purpose / FR / NFR / Cross-references）落地，含 4 个 PushEvent variant 的 payload 形态契约 + 通用 enum 形态 Requirement。
- 6 个 MODIFIED spec delta 按 PR #306 D-2 表格替换：移走字段形态 SHALL，保留 transport 层 / 消费行为契约。
- 同 PR baseline ratchet 一致性：propose 期加 `change/push-events-cap/*` 行，archive 期删除 + 加 `spec/push-events` 行，`SPEC_PURITY_STRICT=1` 双向 ratchet 通过。
- 主 spec Purpose 段直 edit 的架构例外说明（D-4 节），延续 PR #306 D-3 先例审计可追。

**Non-Goals**：

- 不改任何 PushEvent 字段名 / 字段语义 / serde tag / variant 枚举（pure spec 文档重写）。
- 不动 ipc-data-api / http-data-api / frontend-test-pyramid / notification-triggers / sidebar-navigation / session-display 内未涉及 push event payload 字段定义的其它 Requirement。
- 不动 `cdt-watch` / `cdt-api` 等 Rust 代码 / 测试 / IPC contract test。
- 不改 `cdt-api::http::routes` / `cdt-api::http::bridge` 等内部 module 名（实现细节，spec 不归属）。
- 不引入新 hard-fail lint。
- 不在主 spec Cross-references 段引用未来 PR 才会落地的 capability（如 PR 9 计划的 ipc-data-api 拆分产物）。
- 不立即重排 ipc-data-api 内未涉及 push event 的 Requirement 顺序（避免 reviewer diff 噪音；PR 9 拆分时再统一处理）。

## Decisions

### D-1：复盘 PR 1 推荐 b 是否仍成立（结论：成立）

**问题**：PR 1 archive 在 PR 2 落地前论证「push-events 持有所有跨进程 push event payload schema」（推荐 b）。本 PR propose 阶段 SHALL 重新核对 PR 1 D-1 的四条理由是否仍 hold。

**核对 1：当前 PushEvent variant 实际清单**

`grep -n "PushEvent::" openspec/specs/ipc-data-api/spec.md` + `openspec/specs/http-data-api/spec.md`：

- `PushEvent::FileChange`（含 `session_list_changed` 字段）
- `PushEvent::SessionMetadataUpdate`
- `PushEvent::DetectedError`（spec 内仅出现 `DetectedError` 类型名 + `notification-added` 事件名；HTTP SSE 与 IPC 双桥）
- `PushEvent::SseLagged`（含 `source` / `missed` 字段）
- `PushEvent::SshStatusChange`（http-data-api spec line 273）

≥ 4 个 variant，与 PR 1 D-1 的"4 条核心事件"基本一致；新加 `SshStatusChange` 仍属"跨进程 push event"范畴，归属 push-events 自然。验收 grep 第 3 条要求 `≥ 4 Requirement`，落地时一个 enum 形态 Requirement + 4 个 variant 形态 Requirement = 5 个，满足。

**核对 2：ipc-data-api 现状 schema SHALL 句数**

`wc -l openspec/specs/ipc-data-api/spec.md` = 2493 行（PR 1 archive 时是 2428 行；中间合并的 PR #305 file-change `session_list_changed` 增益 + PR #306 file-watching pilot 不动 ipc-data-api，主要增量来自其它 PR）。仍是仓库最大 spec，`Emit file-change events` Requirement 含 8+ 处 SHALL 涉及 payload 字段名 / camelCase / serde tag。减负前提仍成立。

**核对 3：命名 vs 范围对齐 / 减负 / PR 9 拆分清晰三条理由**

- 命名 vs 范围对齐：`push-events` 命名本身涵盖所有跨进程 push event（不只 file-change）。仍 hold。
- 减负：移走后 ipc-data-api 减少约 80-100 行（含 1 个 Requirement Body 整段 + 6 个 Scenario），让 `Emit file-change events` Requirement 简化为 transport 桥契约一句话引用。仍 hold。
- PR 9 拆分清晰：PR 9 拆 ipc-data-api 时 push 部分自然走 push-events，业务部分回各 cap，结构干净，避免二次拆。仍 hold。

**结论**：PR 1 D-1 推荐 b 仍成立；按推荐 b 落地。

### D-2：实际迁移过程中的 owner 边界细化决策

PR 1 D-1 给的 owner 边界粒度是"payload 字段形态"，实际迁移时仍有几处需要 PR 2 内决策。

#### D-2.1：`SseLagged.source` 字段取值清单是否 SHALL 化？

**问题**：现状 `ipc-data-api::Scenario PushEvent::SseLagged 序列化形态与 sentinel 兼容` 仅列举 `source: "file-change"` 一条；实际代码可能扩展到 `"session-metadata-update"` / 其它。`source` 字段当前没有 SHALL 限定取值清单。

**候选**：

- (a) push-events 主 spec 为 `SseLagged.source` 列举固定取值 SHALL 清单（要求实现枚举式扩展时同步改 spec）。
- (b) 不限定取值，只 SHALL 字段类型为 string + 含义为"丢失事件来源标识符"，由产生 lag 的具体 broadcast bridge 决定取值。

**决策**：选 **(b)**——不限定 `source` 取值清单。

**理由**：

1. `source` 字段语义本就是"标识哪条 broadcast 上游 lagged"，每个 broadcast 桥（file_tx / metadata_tx / error_tx 等）有自己的 source 标识，未来增减 broadcast 不应当改协议 spec。
2. 前端 silent refresh handler 不依赖具体 source 字符串值（仅判 `type === "sse_lagged"` 走 silent refresh），取值是 best-effort 诊断信息。
3. 与现有 `sse_lagged` sentinel 行为一致：旧 sentinel `'{"type":"sse_lagged"}'` 不含 `source` 字段时前端读 undefined 不报错（已在 ipc-data-api 现有 SHALL 中确认）。
4. 限定取值清单会让"加新 broadcast bridge 顺手增 source 取值"也变成 BREAKING change candidate，过度刚性。

#### D-2.2：`SessionMetadataUpdate.groupId` 字段归 push-events 还是仍留 ipc-data-api？

**问题**：`SessionMetadataUpdate.groupId` 是 PR `port-metadata-event-group-id-fix` 加的字段，用于多 worktree group 场景下前端 filter 准确性。该字段：

- 字段形态契约（camelCase / snake_case / serde 输出位置）：归 push-events
- 字段语义契约（"groupId 取自 worktree git common dir，与 projectId 在多 worktree 场景下不等值"）：归 push-events，因为这是字段语义；任何消费方（IPC 路径 / SSE 路径）都按同一语义解析

**决策**：`groupId` 字段形态 + 字段语义 SHALL 全部归 push-events。

**理由**：

1. `groupId` 是 push event payload 字段，按 D-1 推荐 b 的"payload 字段形态契约"原则归 push-events。
2. 字段语义不属于"transport 层细节"——transport 层（IPC `app.emit` / SSE serialize）不感知 groupId 取值规则；ipc-data-api 与 http-data-api 都是消费 push-events 的形态契约，不应当各自重复字段语义 SHALL。
3. 前端 `transport.ts::normalizePushPayload` 把 SSE snake_case `group_id` 映射到 camelCase `groupId` 是 transport 归一化（仍归各自 transport spec），但"groupId 字段含义"是 payload 形态契约的一部分。

**实施细节**：

- push-events 主 spec `SessionMetadataUpdate payload` Requirement 内 SHALL 含 `groupId` 字段语义说明（与 `projectId` 在多 worktree 场景下取值差异）。
- ipc-data-api / http-data-api 现有 `groupId` 引用改为引用 `[[push-events::session-metadata-update]]`。
- 主 spec 字段名遵循"IPC payload 字段 camelCase / SSE wire snake_case"双形态，与现有 `sessionListChanged` 处理一致。

#### D-2.3：SSH polling watcher 字段填写规则归 push-events 还是 file-watching？

**问题**：file-watching 主 spec 已有 Requirement「Watch SSH remote project directory via SFTP polling」（line 185-）含 `session_list_changed` 字段对称填写规则、断连重连 baseline diff、跨 polling 起停首见性继承等。这些规则属于"watcher 怎么发 FileChangeEvent"还是"FileChangeEvent payload 字段语义"？

**决策**：SSH polling watcher 字段填写规则**保留在 file-watching**。

**理由**：

1. SSH polling 是 watcher 实现选择（轮询 vs OS 通知），其填写规则描述的是 watcher 行为契约——"watcher 在何种内部状态下填字段什么值"。
2. push-events 只关心 payload 形态（含字段语义"`session_list_changed: bool` 标记是否会改变 group 内 session 集合"），不关心"watcher 怎么决定填 true/false"。
3. file-watching 现有 SSH polling Requirement 已精细规约 baseline diff / 跨重连首见性 / 跨 context 切换 etc，搬到 push-events 反而模糊"watcher 视角 vs 跨进程 transport 视角"边界。

**owner 边界更新**（PR 1 D-1 owner 边界精化）：

- **push-events**：跨进程 push event 的 payload 字段形态契约（字段名 / camelCase / snake_case / serde tag / 字段语义"该字段标记什么"）。
- **file-watching**：FileChangeEvent 内字段填写规则（watcher 视角"在何种内部状态下应填 true/false"）。
- **ipc-data-api**：Tauri host 在 setup 阶段订阅哪个内部 broadcast、用哪个 webview event name `app.emit(...)`、`Lagged` 时的 fallback 行为；具体 payload 形态见 `[[push-events]]`。
- **http-data-api**：HTTP `/events` SSE transport 协议（路径 / Content-Type / `lastEventId` 重连 / `sse_lagged` sentinel 何时发出）；具体 PushEvent 序列化形态见 `[[push-events]]`。

#### D-2.4：sidebar-navigation / session-display 内字段语义直接复制处的处理粒度

**问题**：sidebar-navigation 第 735-769 行有 `payload.sessionListChanged` / `payload.deleted` 字段语义直接复制（"WHEN unified invalidator 检测到... enrich `FileChangeEvent` 时把 `session_list_changed` 置为 `true`...AND emit `file-change` payload `{ ..., sessionListChanged: true }`"）。这些 Scenario 兼具消费契约（"前端收到该 payload 时调 loadProjects"）与字段定义复制（"watcher 检测到 X 时填 sessionListChanged=true"）。

**候选**：

- (a) 整段保留（消费 + 字段定义不分），违反单 owner。
- (b) 整段移走（含消费断言），sidebar-navigation 失去消费契约。
- (c) 拆分：消费契约 SHALL 句留 sidebar-navigation；字段定义部分（"watcher 检测到 X 时填 sessionListChanged=true"）改为引用 `[[push-events::file-change]]`，让 Scenario 简化为"WHEN 收到 payload 含 `sessionListChanged: true`（语义见 [[push-events::file-change]]）→ THEN sidebar 调 loadProjects"。

**决策**：选 **(c)**——保留消费断言句子、字段定义改为引用。

**实施**：

- sidebar-navigation Scenario `silent 刷新 sessionListChanged 时 scopeTotal 同步刷新`（line 734）等 Scenario 的 WHEN 段从「unified invalidator 检测到... 把 `session_list_changed` 置为 true」改为「Tauri host emit `file-change` payload 含 `sessionListChanged: true`（字段语义见 `[[push-events::file-change]]`）」。
- session-display 同类 Scenario 同步处理。
- 消费断言（"sidebar 调 loadProjects(refresh: true)" / "session detail 调 refreshDetail()"）原样保留——这些是各自 cap 的业务行为契约。

### D-3：archive 顺序坑预防 + baseline 二次刷新

**风险**：本 PR 改 1 ADDED + 6 MODIFIED spec delta，是 archive 顺序坑高风险 PR（`openspec archive` 用 delta 的 `MODIFIED Requirement` 完整 body **替换**主 spec 对应 Requirement，不做三方合并；详 `openspec/CLAUDE.md::硬约束 4`）。

**对冲措施 1：本 PR 单 PR 串行 archive**

- 本 PR archive 与 issue #303 9-PR 序列后续 PR（PR 3+ 涉及 ssh-remote-context / configuration-management / sidebar-navigation / session-display 改动）必 SHALL **错开 archive 时间窗**——PR 2 archive 完成后，PR 3+ 才进入 propose 阶段。
- 同期不允许其它 PR 同时 archive 含 ipc-data-api / http-data-api / frontend-test-pyramid / notification-triggers / sidebar-navigation / session-display 任一 cap 的 MODIFIED Requirement 改动。
- 同期 PR 列表当前快照（fetch origin/main 后）：本 PR 之外**没有**进行中的 active change 改这 6 cap 的 MODIFIED Requirement——风险窗口窄。

**对冲措施 2：baseline 二次刷新（PR #306 已踩过坑）**

- **propose 期**（本 commit）：`scripts/spec-purity-baseline.txt` 加 7 行：
  - `change/push-events-cap/push-events <count>`
  - `change/push-events-cap/ipc-data-api <count>`
  - `change/push-events-cap/http-data-api <count>`
  - `change/push-events-cap/frontend-test-pyramid <count>`
  - `change/push-events-cap/notification-triggers <count>`
  - `change/push-events-cap/sidebar-navigation <count>`
  - `change/push-events-cap/session-display <count>`
- **archive 期**（archive commit）：把 7 行 `change/push-events-cap/*` 删除 + 加 1 行 `spec/push-events <count>`；同时其它 6 个被 MODIFIED 的 spec 对应 `spec/<cap>` baseline 数字会因移走 schema SHALL 而下降，需重新 `bash scripts/check-spec-purity.sh --baseline > scripts/spec-purity-baseline.txt`。
- ratchet 模式：`SPEC_PURITY_STRICT=1` 双向 ratchet（默认单向，但 archive commit 上 CI 走双向校验防 silent degradation——`scripts/check-openspec-archives.sh` 间接守护）。

**对冲措施 3：CI archive 拦截窗口规避**

- `scripts/check-openspec-archives.sh` 拦"已完成但未 archive"的 change 仅在 `(change 在 changes/<slug>/ 下 active) AND (tasks 全勾)` 同时成立时触发。tasks.md 末尾固定预留 N.1-N.4 不勾，从首次 push 到 archive 之间每次 CI run 都至少有一个条件不成立（详 `.claude/rules/opsx-apply-cadence.md::archive 时机 vs CI 拦截`）。

### D-4：主 spec Purpose 段直 edit 的架构例外（继承 PR #306 D-3，校正 OpenSpec 行为）

**继承先例**：PR #306 archive `design.md::D-3` 已确立"OpenSpec 的 spec delta 架构（`buildUpdatedSpec`）只解析 `## ADDED/MODIFIED/REMOVED/RENAMED Requirements` 头并对 Requirement 做替换，主 spec `before` 段（`# <cap> Specification` 与 `## Requirements` 之间的 Purpose / 顶层背景）在 archive 时**整段保留**——delta 没有任何机制能改 Purpose"作为架构例外，主 spec Purpose 段直 edit 同 commit 落地与 spec delta 配套。

**本 PR 适用范围（校正初稿误判）**：

- **新建 push-events 主 spec**：propose 阶段曾预设"delta 文件顶部写 Purpose 即可被 archive sync 写到主 spec"。**实测 OpenSpec 源码（`specs-apply.js::buildSpecSkeleton`）**：新 spec 路径下 archive sync 用 hard-coded skeleton（`# <cap> Specification\n\n## Purpose\nTBD - created by archiving change <slug>. Update Purpose after archive.\n\n## Requirements\n`）+ 把 ADDED Requirements 拼进去；**delta 文件 `before` 段（`## ADDED Requirements` header 之前的所有内容）整段被丢弃**——`extractRequirementsSection` 只提取 Requirement blocks，preamble 不参与拼接。
- **结论**：新建 cap 也命中 PR #306 D-3 例外路径——archive 完成后 SHALL 在同一 archive commit 直接 Edit `openspec/specs/push-events/spec.md` Purpose 段把 `TBD - created by archiving change ...` 替换为本 PR 设计的真实 Purpose 文案；与 spec delta 配套同 commit 落地（diff 可见）。
- **6 MODIFIED spec 的 Purpose 段**：本 PR 不改这 6 个 spec 的 Purpose（仅删除某些 Requirement Body 内字段定义 + Scenario 改为引用）。无 Purpose 直 edit 需要。

**例外正当性**：

1. **架构限制**：spec delta 不解析 Purpose section（`buildUpdatedSpec` 对新 spec 用固定 skeleton，对既有 spec 整段保留 `before`），没有"走流程"的可选项
2. **范围限定**：仅修改 `## Purpose` 段下方一段散文（用户价值描述），不触碰任何 Requirement / Scenario / SHALL / MUST 行为契约
3. **审计可追**：本 D-4 显式记录例外；archive commit diff 能看见 Purpose 改动伴随新 spec sync
4. **同 commit 落地**：Purpose 直 edit 与 archive 同 commit，不是 stealth 修改

## Risks / Trade-offs

- **风险 1：archive 顺序坑**：与同期其它 PR 撞同 cap 的 MODIFIED Requirement → **缓解**：D-3 对冲措施 1（独立串行 archive） + D-3 对冲措施 2（baseline 二次刷新）+ D-3 对冲措施 3（CI 拦截窗口规避）。
- **风险 2：spec-guide-reviewer 第二次实战 + 跨 7 spec 大 diff**：reviewer 可能漏抓字段语义复制 vs 消费断言的边界差异（D-2.4 拆分粒度） → **缓解**：codex 二审作为正交视角兜底；spec-guide-reviewer 报告的 hard / soft finding 全部本 PR 内修。
- **风险 3：D-2.3 owner 边界精化（SSH polling 留 file-watching）reviewer 可能挑战「`session_list_changed` 字段语义跨 push-events / file-watching 双 owner」**：实际上 push-events 拥有"字段名 + 字段类型 + 字段语义抽象描述"，file-watching 拥有"watcher 视角下何时填 true / 何时填 false 的具体规则"，两者不重叠。push-events Cross-references 段 SHALL 显式注明该精化决策。
- **trade-off 1：D-2.1 不限定 `SseLagged.source` 取值清单**：换 best-effort 诊断信息扩展自由 → 接受（前端不依赖具体取值，未来扩展不需改 spec）。
- **trade-off 2：D-2.4 Scenario 拆分（消费断言留 / 字段定义引用）让 sidebar-navigation 内 Scenario 略增"语义见 [[push-events::file-change]]"补语**：换跨 spec 单 owner → 接受（reviewer 重点查"消费 vs 字段定义"边界）。

## Migration Plan

1. 本 PR 走 1 个 ADDED + 6 个 MODIFIED Requirement delta 重写，不需要 capability migration / 数据迁移 / 实现代码改动。
2. archive 后 push-events 主 spec 自动 sync；6 MODIFIED spec 主 spec 同步 sync。
3. baseline 二次刷新（D-3 对冲措施 2）随 archive commit 落地。
4. 后续 PR 3-9 按 issue #303 顺序推进，对 push-events 主 spec 视为只读引用 owner，跨 cap 协议字段不再重复定义。

## Open Questions

无（D-1 复盘 PR 1 推荐 b 仍成立；D-2.1-D-2.4 owner 边界细化决策已闭合；D-3 archive 顺序坑预防 + baseline 二次刷新落地；D-4 主 spec Purpose 段架构例外本 PR 不触发）。

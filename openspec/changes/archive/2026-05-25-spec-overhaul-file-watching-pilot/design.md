## Context

PR #302 把 `openspec/SPEC_GUIDE.md` 落进 main，定义了 spec 该写什么 / 不该写什么 + 反例对照表 + 4 层骨架（Purpose / FR / NFR / Cross-references）。issue #303 把后续 spec 重写工作拆成 9 个 PR，本 PR（PR 1）做 file-watching 试点，目的有三：

1. **跑通"按 SPEC_GUIDE 重写主 spec"工艺**——验证下笔顺序、validate 链路、baseline ratchet 同 commit 刷新流程能否在最简单的 capability（file-watching：仅 9 Requirement / 37 Scenario / 反模式 8 处全 p4）上闭合。
2. **沉淀 spec-guide-reviewer subagent**——后续 PR 2-9 都要靠它审增量 spec PR diff。本 PR 让 reviewer 自身首次实战（审本 PR 自己的 spec 改动），找到 prompt bug 同 PR 内修。
3. **论证 push-events cap 决策草案**——`grep "file-change\|FileChangeEvent" openspec/specs/`（不含 file-watching/）发现 5 个 spec 引用 file-change，其中 ipc-data-api 持有 8 处 payload schema SHALL 句，是真正的 schema owner。这跟最初猜的「file-watching 是源头」相反。PR 2 真新建 push-events cap 时需先有决策草案，本 PR 落 D-1（push-events 范围）/ D-2（6 引用 spec 替换策略）/ D-3（Purpose 直 edit 架构例外）/ D-4（archive 顺序坑预防）四个决策给 PR 2 reviewer 评估。

约束：
- 不动其它 capability 主 spec（archive 顺序坑预防：本 PR 单 capability 改动，PR 2 propose 时 file-watching 已 archive 主 spec 稳定，无互覆盖风险）
- 不改 file-change payload 字段名 / 字段语义 / IPC 行为契约（本 PR 仅 spec 文档重写，不动后端代码 / 测试）
- 同 commit 刷 `scripts/spec-purity-baseline.txt::spec/file-watching`（双向 ratchet：超 baseline 拒、低于 baseline 也拒）

## Goals / Non-Goals

**Goals**：
- 把 file-watching 主 spec 按 SPEC_GUIDE 4 层骨架重写：Scenario 全简体中文、Purpose 改用户价值视角、NFR 数字独立成 Requirement、删 FR Body 内嵌的实现机制术语（broadcast / debounce / pipeline / 通道 / in-process）
- 落地 spec-guide-reviewer subagent：只读、分级 finding、参考既有 reviewer 框架（windows-compat-reviewer / rust-conventions-reviewer）
- 论证 push-events cap 范围 / 引用替换策略 / archive 顺序坑预防三条决策

**Pilot scope 折中**（明确不做的清理）：

本 PR 是 SPEC_GUIDE 重写工艺试点，scope 锁定在 **Scenario 中文化 + Purpose 改写 + NFR 抽出 + reviewer subagent 落地** 四件，**不**顺手清现有 Requirement Body 内残留的 16 处内部符号引用（`mark_project_seen` / `known_projects` / `cdt-watch::FileWatcher` 等）。理由：

1. 这 16 处分散在 8 个 MODIFIED Requirement Body 内，多数描述"实现 race / 内部状态机"行为契约的派生原因，去掉需要重写 Body 段落，风险显著高于本 PR 的 4 件主线工作
2. SPEC_GUIDE 第 132 行明确「遇到一个修一个，PR 改动到含历史污染的 Requirement 时顺手按反例对照表清理；这次没改的 Requirement 不强制清」——本 PR scope 不包括清这些。pilot 的目标是**跑通工艺**，不是把 file-watching 变成 SPEC_GUIDE 满分样板
3. 后续 PR 2-9 各自负责自己改的 spec 时仍可按"遇到一个修一个"清；如这 16 处仍保留至 9-PR 序列结束，issue #303 完成后开 follow-up cleanup PR 单独处理（lint baseline 仍由 ratchet 守门防恶化）

**Non-Goals**：
- 不新建 push-events 主 spec（PR 2 落地）
- 不动 ipc-data-api / sidebar-navigation / session-display / http-data-api / frontend-test-pyramid / notification-triggers 主 spec（PR 2-9 各自负责）
- 不改 cdt-watch / cdt-api 等代码 / 测试（spec 文档 + reviewer subagent 文档双重纯文档改动）
- 不引入新 hard-fail lint（SPEC_GUIDE 第 136 行明确禁止）
- 不重排 Requirement 顺序（保留现有 9 个 Requirement 的相对顺序，避免 reviewer diff 噪音）

## Decisions

### D-1：push-events cap 范围（PR 2 立项依据）

**问题**：cross-spec file-change 引用现状（`grep -rn "file-change\|FileChangeEvent" openspec/specs/`）：

| spec | 行号 | 角色 |
|---|---|---|
| ipc-data-api | 5, 189-200, 211-281, 1066, 1419-1436 | **Payload schema owner**（含 `sessionListChanged` / camelCase / serde / `PushEvent::FileChange` / `sse-lagged` 等 8+ 处 SHALL）|
| sidebar-navigation | 263, 270-298, 359, 392-440, 609, 701, 735-769, 802, 816, 849, 908-957 | listen 消费契约（`silent=true` 刷新 / 滚动位置保持 / `sessionListChanged` 字段消费）|
| session-display | 390-410 | listen 消费契约（命中 `(projectId, sessionId)` 时刷新当前会话）|
| http-data-api | 95, 277 | SSE 推送层映射（`PushEvent::FileChange` 转 SSE）|
| frontend-test-pyramid | （PR 1 未直接 grep 命中，brief 第 3.1 节列 2 处） | 测试覆盖契约 |
| notification-triggers | 118 | FileSignature 自然恢复机制 |

候选：

- **(a) push-events 仅持 file-change schema** —— 其它 push event（`session-metadata-update` / `detected-error` / `sse-lagged`）留 ipc-data-api。最小变更，但 push-events 命名冗余（既然叫 push-events 却只装一种 push event）。
- **(b) push-events 持有所有跨进程 push event payload schema** —— file-change / session-metadata-update / detected-error / sse-lagged 的 **payload 形态**（字段名 / camelCase / serde tag 约定 / 字段语义）全归 push-events；ipc-data-api 与 http-data-api 各自简化为 **transport 层**（"Tauri host SHALL `app.emit(X)` bridge Y"、"HTTP `/events` SSE SHALL serialize Z 为 sentinel"），具体 payload schema 引用 `[[push-events]]`。长期协议清晰、ipc-data-api 减负（当前 2428 行 / 41 Req 已超载）、PR 9 拆分时 push 部分自然走 push-events。

  **owner 边界（避免 sse-lagged / detected-error 等多通道事件归属模糊）**：
  - push-events：跨进程 push event 的 **payload 字段形态契约**（含 file-change / session-metadata-update / detected-error / sse-lagged / cdt-error 等）
  - ipc-data-api：Tauri host 在 `setup` 阶段订阅哪个内部 broadcast、用哪个 webview event name `app.emit(...)`、`Lagged` 时的 fallback 行为；具体 payload 形态见 `[[push-events]]`
  - http-data-api：HTTP `/events` SSE transport 协议（路径 / Content-Type / `lastEventId` 重连 / `sse_lagged` sentinel 何时发出）；具体 PushEvent 序列化形态见 `[[push-events]]`
- **(c) 不新建 push-events，把 file-change schema 集中到 file-watching** —— 跟 SPEC_GUIDE 跨 spec 唯一 owner 原则一致但 file-watching 变重，且 sse-lagged / session-metadata-update 等非文件事件归 file-watching 语义牵强。

**决策**：推荐 **(b) push-events 持有所有跨进程 push event payload schema**。

**理由**：
1. push-events 命名本身就涵盖所有跨进程 push event，不该只装 file-change（命名 vs 范围对齐）。
2. ipc-data-api 已超载（2428 行 / 41 Req 是仓库内最大 spec），抽 push payload 是减负的天然路径，与 SPEC_GUIDE「外部协议单一 owner」原则一致。
3. PR 9 ipc-data-api 拆分时 push 部分已自然走 push-events，业务部分回各 cap，结构干净；avoid one-shot push-events-only 拆分后又要二次拆。
4. 风险可控：本 PR 仅论证（design.md），不动 ipc-data-api 主 spec；PR 2 reviewer 可改主意（候选 a/c 仍开放），design.md 留三候选。

### D-2：现有 6 引用 spec 的替换策略（PR 2 落地，PR 1 仅论证）

PR 2 propose 时按下表替换；PR 1 不动。

| spec / 行号 | 当前内容 | PR 2 改法 |
|---|---|---|
| frontend-test-pyramid::31 / ::39 | listen event 覆盖范围 SHALL + 4 条核心事件逐项对齐断言 | "事件名清单见 `[[push-events]]`"，本 spec 仅断言 mockIPC 与之对齐 |
| http-data-api::95 / ::277 | file-change SSE 推送 SHALL + ensureSseReady race 描述 | "SSE PushEvent payload 见 `[[push-events]]`"；race 描述保留（HTTP transport 层细节） |
| notification-triggers::118 | FileSignature 自然恢复机制 | 引用 `[[push-events::file-change]]` |
| ipc-data-api::189-200 / Scenarios | file-change payload 整段 SHALL（camelCase / sessionListChanged / SseLagged） | **整段移到 push-events**；ipc-data-api 改为"Tauri host SHALL bridge file-change push event（payload 见 `[[push-events]]`）" |
| ipc-data-api::Scenarios `Tauri 转发 file-change 事件` / `file-change payload 是 camelCase` / `enriched session_list_changed` | 整段移到 push-events |
| sidebar-navigation / session-display | file-change listen 消费契约（含 `payload.session_list_changed` 等字段名引用，如 `sidebar-navigation::spec.md::735-736 / 744-754 / 762`）| **保留消费行为断言**（"收到 X 时 → 调 Y / 命中当前选中 → 拉新数据"等业务行为契约属各自 cap）；遇到 spec 直接复制字段名 / 字段语义的句子，改为引用 `[[push-events::file-change]]`，避免协议字段在多个 spec 双 owner |

`PushEvent::SessionMetadataUpdate` / `PushEvent::DetectedError` / `PushEvent::SseLagged` 同步走相同迁移（PR 2 一并落地）。

### D-3：Purpose 段直 edit 主 spec 的架构例外

**问题**：`openspec/CLAUDE.md::硬约束 1` 明令"任何对主 spec 的改动必须走 spec delta"，但 OpenSpec 的 spec delta 架构（`/opt/homebrew/lib/node_modules/@fission-ai/openspec/dist/core/specs-apply.js::buildUpdatedSpec`）只解析 `## ADDED/MODIFIED/REMOVED/RENAMED Requirements` 头并对 Requirement 做替换，主 spec `before` 段（`# <cap> Specification` 与 `## Requirements` 之间的 Purpose / 顶层背景）在 archive 时**整段保留**——delta 没有任何机制能改 Purpose。

**候选**：

- (a) 不改 Purpose，等 OpenSpec 上游加 Purpose delta 支持后再改：违背 SPEC_GUIDE 第 1 步「先回答为什么存在这个能力」原则；本 PR 试点价值打折；后续 PR 2-9 都会撞同一墙
- (b) 直接 Edit 主 spec 的 Purpose 段（与 spec delta 同 commit 落地）：违反硬约束 1 的字面，但符合硬约束 1 的意图（"主 spec 是产出物不是输入源"针对的是 Requirement 行为契约改动；Purpose 是描述性散文，不属行为契约）
- (c) 把 Purpose 写到 design.md，主 spec 留 stub：spec.md 失去入口段，reviewer / 新人无法快速理解能力；与 OpenSpec validate 强制要求 Purpose 段冲突

**决策**：选 **(b)**，直接 Edit 主 spec Purpose 段。

**例外正当性**：
1. **架构限制**：spec delta 不解析 Purpose section，没有"走流程"的可选项
2. **范围限定**：仅修改 `## Purpose` 段下方一段散文（用户价值描述），不触碰任何 Requirement / Scenario / SHALL / MUST 行为契约
3. **审计可追**：本 D-4 显式记录例外；archive 后该例外作为后续 cleanup PR 的参考前例
4. **同 commit 落地**：Purpose 直 edit 与 spec delta 在同一 commit，diff 可见 Purpose 改动伴随 9 个 Requirement MODIFY，不是 stealth 修改

**后续动作**：把"OpenSpec 上游 Purpose delta 支持"作为 cross-cap workflow 改进项考虑；本 PR 内不立 GitHub Issue（issue #303 9-PR 序列消化掉所有 Purpose 重写后再评估是否值得上游 PR）。

### D-4：archive 顺序坑预防

`openspec archive <slug>` 用 delta 的 `MODIFIED Requirement` 完整 body **替换**主 spec 对应 Requirement，不做三方合并（参见 `openspec/CLAUDE.md::硬约束 4`）。

**PR 1 风险评估**：本 PR 仅 MODIFY file-watching 主 spec，**不**触发其它 cap 主 spec。即使 PR 2 同时进行（不会，因 PR 2 等 PR 1 archive 后 propose），双 PR 之间无 Requirement 互覆盖：
- PR 1 archive 时 file-watching 主 spec 被 sync（其它 cap 不动）
- PR 2 propose 时 file-watching 主 spec 已稳定，PR 2 改 ipc-data-api / sidebar-navigation / 等 cap 与 file-watching 无 delta 交集

**PR 2 后续注意（不影响 PR 1）**：PR 2 propose 时 ipc-data-api 跨 6 个 Requirement 改动，若同期还有别的 ipc-data-api 改动（如 PR 3 / PR 4 拆分计划）需按创建顺序 archive 先老后新；判断不准用 `(b)` 兜底（已 archive 倒序时手工 diff 主 spec merge 回去）。本 PR archive 时间点单一，不涉及。

## Risks / Trade-offs

- **风险 1**：spec-guide-reviewer 自身首次实战，prompt 设计可能漏抓常见反模式或误报合法 SHALL → **缓解**：本 PR 内运行 reviewer 自审 spec delta，发现 prompt bug 同 PR 内修；codex 二审作为正交视角兜底
- **风险 2**：NFR 抽出后 file-watching p4 metric baseline 重计算结果可能不为 0（"30s catch-up" / "1s 内退出" 等数字仍会被词法 lint 计 p4）→ **缓解**：实际值由 `bash scripts/check-spec-purity.sh --baseline` 输出 + 同 commit 刷新；不强求 0，只要 ratchet 一致即可
- **风险 3**：push-events 决策 (b) 后续 PR 2 reviewer 改投 (a) 或 (c) → **缓解**：design.md 留三候选 + 推荐理由，不锁死；PR 2 propose 时再 D-1b/D-1c 修订块
- **trade-off**：本 PR 不重排 Requirement 顺序（保现状 9 个 Requirement 相对位置）→ 视觉上 NFR 可能不在末尾「不靠近 Cross-references 段」，但避免 reviewer diff 噪音。SPEC_GUIDE 4 层骨架是下笔顺序约束，不强制最终位置

## Migration Plan

1. 本 PR 走 `MODIFIED Requirement` delta 重写，不需要 capability migration / 数据迁移
2. archive 后主 spec 自动 sync；后续 PR 2-9 按 issue #303 顺序推进

## Open Questions

无（本 PR 范围闭合；push-events 范围最终决定权在 PR 2 reviewer）。

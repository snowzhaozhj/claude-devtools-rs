## Why

PR #302 把 `openspec/SPEC_GUIDE.md` 落进 main 后，`openspec/specs/` 下 24 份 spec 仍含历史污染（实现概要型 Purpose、英文 Scenario 标题、FR/NFR 混写、跨 spec 重复定义同一 IPC 字段等）。9-PR 重写计划（issue #303）需要先用一个最简单的 capability 试点：跑通"按 SPEC_GUIDE 重写主 spec"的工艺，沉淀 reviewer subagent，并为后续 PR 2 真新建 push-events capability 做决策准备。

`file-watching` 现状：251 行 / 9 Requirement / 37 Scenario / 反模式 baseline 8（全 p4 metric）/ 24 个英文 Scenario 标题违反 `config.yaml::rules.specs::第 5 条`。它跨 5 spec 引用 file-change 协议，但真正持有 payload schema SHALL 的是 ipc-data-api（8 处），不是 file-watching——这一发现需在 design.md 落成 push-events cap 决策草案，给 PR 2 立项依据。

## What Changes

- **重写 file-watching 主 spec**：Scenario 标题 24 个英文 → 简体中文（对齐 `config.yaml::rules.specs::第 5 条`）；Purpose 段从"用 debounce 后的 broadcast 通道把 file-change / todo-change 事件分发"改写为用户价值视角（Purpose 段经 design.md `D-3` 决策走"直 edit 主 spec"架构例外路径——OpenSpec spec delta 不解析 Purpose section，无可选项；详 design.md）；把"100ms debounce / 30s catch-up / 3s polling / 1s 停止"等 NFR 数字从 FR Body 抽出，独立成一个 NFR Requirement——按 `SPEC_GUIDE.md::4 层骨架::第 3 条`「FR 与 NFR 分开」。
- **沉淀 push-events cap 决策草案**：在 `design.md::D1-D3` 论证 push-events capability 的范围（候选 a/b/c）、PR 2 落地步骤、archive 顺序坑预防。本 PR **不**新建 push-events cap，**不**改其它 cap 主 spec——决策仅供 PR 2 reviewer 评估。
- **新增 spec-guide-reviewer subagent**（apply 阶段落地 `.claude/agents/spec-guide-reviewer.md`）：按 `openspec/SPEC_GUIDE.md` 4 层骨架 + 反例对照表 + reviewer checklist 审 spec PR diff，分级 finding（hard / warn / info），不 hard-fail。后续所有 spec PR 都可调它做增量审查；本 PR 让 reviewer 自身首次实战（审本 PR 自己的 spec 改动），发现 prompt bug 同 PR 修。
- **同步刷新 spec-purity-baseline**：file-watching p4 metric 数字从 FR 抽到 NFR Requirement 后，词法 lint 仍计 p4 命中——baseline 文件按 `scripts/check-spec-purity.sh::双向 ratchet` 同 commit 落地（不刷会被 CI 拒）。

## Capabilities

### New Capabilities

无。push-events 仅作设计草案讨论，不在本 PR 新建主 spec（按 design.md `D-4` archive 顺序坑预防）。

### Modified Capabilities

- `file-watching`: Scenario 命名简体中文统一 / Purpose 段重写为用户价值视角 / NFR 独立成 Requirement（"事件投递时延、远端 polling 频率与停止时延"）/ 删除 FR Body 内嵌的 NFR 数字描述。本 PR **不**改 file-change payload schema 字段名 / 字段语义 / 新增 / 删除任何 FR Scenario 行为契约。

## Impact

- Affected specs：file-watching（1 ADDED + 8 MODIFIED + 1 REMOVED；REMOVED「Debounce rapid file events」与 ADDED「事件投递时延、远端 polling 频率与停止时延」是等价迁移——无行为契约语义变更）
- Affected code：0（仅 spec + reviewer subagent 文档；无 Rust / TS / Tauri 改动）
- Affected baseline：`scripts/spec-purity-baseline.txt::spec/file-watching` 8 → 期望随 NFR 抽出后实际命中重计算（同 commit 刷新）
- 风险：行为句被误删 → codex design 二审 + spec-fidelity-reviewer + spec-guide-reviewer 三道审查覆盖（spec-guide-reviewer 自身首次实战，发现 prompt bug 同 PR 修）
- 后续依赖：push-events 决策结论沉淀进 issue #304 评论，作为 PR 2 立项依据；`file-watching` 主 spec 稳定后 PR 2 可独立 propose 新建 push-events cap 而无 archive 顺序坑风险

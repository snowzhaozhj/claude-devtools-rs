## 1. 准备 / Audit

- [x] 1.1 grep 18 个 S+XS cap spec.md 定位 6 条尺子命中点（fn 名 / 源码路径 / `tracing::xxx!(target:...)` / IPC contract test scenario / 颗粒过细 / TS 路径），按 cap 列出
- [x] 1.2 标记 2 个**已干净**的 cap（`session-search` / `notification-ui`），不写 delta

## 2. 写 propose 工件

- [x] 2.1 写 `openspec/changes/openspec-slim-S-tier/proposal.md`（Why + What Changes + Capabilities）
- [x] 2.2 写 `openspec/changes/openspec-slim-S-tier/design.md`（Context + Goals + 6 条尺子 D1-D7 + Risks）
- [x] 2.3 写 `openspec/changes/openspec-slim-S-tier/tasks.md`（本文件）
- [x] 2.4 写 16 个 cap 的 spec delta（每个 cap 一个 `openspec/changes/openspec-slim-S-tier/specs/<cap>/spec.md`，全部 `MODIFIED Requirement` 全文重写形式）

## 3. validate

- [x] 3.1 跑 `openspec validate openspec-slim-S-tier --strict` 一次过

## 4. design 二审（codex）

- [x] 4.1 调 `Agent({ subagent_type: "codex:codex-rescue", prompt: ... })` 对 design.md + 16 cap delta 跑 design 二审；prompt 模板见 `.claude/templates/codex-prompt-design-review.md`
- [x] 4.2 codex 提的 design 问题 → 修 design.md / proposal.md / 各 cap delta 三处一致 → re-validate

## 5. apply：每 cap 一 commit

每 cap 一个 commit `chore(spec): slim <cap>`。文件改动落到 `openspec/changes/openspec-slim-S-tier/specs/<cap>/spec.md`（archive 时 sync 回主 spec）。无 spec.md 改动的 cap（已干净）不出 commit。

- [x] 5.1 `chore(spec): slim settings-ui`
- [x] 5.2 `chore(spec): slim app-auto-update`
- [x] 5.3 `chore(spec): slim team-coordination-metadata`
- [x] 5.4 `chore(spec): slim notification-triggers`
- [x] 5.5 `chore(spec): slim file-watching`
- [x] 5.6 `chore(spec): slim session-parsing`
- [x] 5.7 `chore(spec): slim memory-viewer`
- [x] 5.8 `chore(spec): slim wsl-distro-discovery`
- [x] 5.9 `chore(spec): slim tab-management`
- [x] 5.10 `chore(spec): slim application-telemetry`
- [x] 5.11 `chore(spec): slim server-mode`
- [x] 5.12 `chore(spec): slim context-tracking`
- [x] 5.13 `chore(spec): slim frontend-test-pyramid`
- [x] 5.14 `chore(spec): slim ui-search`
- [x] 5.15 `chore(spec): slim agent-configs`
- [x] 5.16 `chore(spec): slim app-chrome`

## 6. 验收

- [x] 6.1 跑 `openspec validate openspec-slim-S-tier --strict` 仍过
- [x] 6.2 行数 / 反引号下降——18 个 cap 行数 + 反引号统计 vs baseline 对比写入 PR 描述
- [x] 6.3 IPC 字段名 / Tauri command 名 / `xxxOmitted` 命名 / SSE event 名等 byte-equal 校验：grep delta 文件确认未改这些 surface

## N. 发布

- [x] N.1 push 分支 + 开 PR
- [x] N.2 wait-ci 全绿
- [x] N.3 codex 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [x] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

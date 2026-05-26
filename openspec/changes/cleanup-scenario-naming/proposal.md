## Why

按 issue #303 9-PR plan 阶段 3 推进 PR 6 —— 跨 capability spec 的 Scenario 标题命名清理。前序 PR 5（change cleanup-sidebar-navigation）+ follow-up（change cleanup-spec-dangling-design-refs，issue #323）已合，sidebar-navigation 主 spec 干净。本 change 把所有 active capability spec 内"明显内部 symbol 视角"的 Scenario 标题改为"用户 / 系统可观察行为视角"，对齐 `openspec/SPEC_GUIDE.md::反例 1` + reviewer checklist 末两条。

排除：`session-display`（PR 7 拆分）/ `sidebar-navigation`（PR 8 重组）/ `ipc-data-api`（PR 9 拆分）—— 这三个 cap 的 Scenario 标题清理在后续拆分 PR 内一起做，避免 churn。

## What Changes

- 重命名 19 个 Scenario 标题，跨 8 个 capability / 15 个 Requirement
- 每条改名都属"标题里有内部 fn 名 / mod 路径 / Rust 类型签名 / 内部 const / lib 名 / 内部 channel 名"中至少一项的明显 case；微妙边界（cap 内部协议术语、文档级风格用语）按 case-by-case 保留
- 行为契约 100% 不变：所有 SHALL / MUST / WHEN / THEN / AND 句保持原样，仅替换 Scenario 标题用语；Requirement / Scenario 数量不增不减
- 用 `MODIFIED Requirement` 全文重写每个含改名 Scenario 的 Requirement（即把 Requirement body 与所有 Scenario 一起拷过来，仅替换需改的 Scenario 标题），便于 reviewer 对照

## Impact

- Affected specs: `app-chrome` (MODIFIED 1) + `application-telemetry` (MODIFIED 1) + `chunk-building` (MODIFIED 2) + `context-tracking` (MODIFIED 1) + `fs-abstraction` (MODIFIED 3) + `project-discovery` (MODIFIED 3) + `session-parsing` (MODIFIED 3) + `tool-execution-linking` (MODIFIED 1) = 15 Requirement 跨 8 cap
- Affected code: 无（纯 spec 文档清理）
- Risk: 极低 —— 行为契约 0 变更；只改标题用词；姊妹清理工艺已落地（PR #309 / #312 / #319 / #322），反例分类规则收敛

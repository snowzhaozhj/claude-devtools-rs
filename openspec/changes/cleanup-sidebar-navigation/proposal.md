## Why

按 issue #303 9-PR plan 批次 PR 5 推进 `sidebar-navigation` capability spec 反例清理（44 hits → 目标 ~11）。当前 spec 把内部 src 路径（`Sidebar.svelte` / `cdt-api/src/...` / `ui/src/lib/...` / `sessionListStore.svelte.ts` 等 17 处）+ 历史 PR 引用（`PR #183` 3 处）+ 一组 store 内部 debounce / cache / race window tuning 数字混进 Requirement body，让行为契约 vs 实现 tuning 边界模糊。复用 ssh-remote-context-cleanup（PR #312, D-1b 数字三分）+ cleanup-config-and-context-menu（PR #319, D-2b 数字三分）已验证工艺继续做 sidebar-navigation。

## What Changes

- 重写 14 个 Requirement body 抽象掉 src 路径 + PR 引用 + 内部模块名 + 库 path（`broadcast::Sender::send`），保留 SHALL / MUST 句的语义对等
- store / 滚动 debounce / SWR race window 数字按 SPEC_GUIDE 反例 4 三分：用户感知阈值（toast / 渐显 / hover / debounce 100 ms 滚动停顿）保留具体值在 Scenario WHEN/THEN；实现 tuning（race 触发窗口 200-500 ms / "20 ms 间隔 5 次"测试场景常数）移到 design.md
- 同 commit 刷 `scripts/spec-purity-baseline.txt`：`spec/sidebar-navigation 44 → 11`（保留约 11 处用户感知阈值数字命中：1 处 600ms toast + 10 处 100/150/200/250/1500 ms 渐显与 debounce 阈值）
- 行为契约语义 100% 不变 —— Requirement / Scenario 数量与 SHALL / MUST 句覆盖均不增不减

## Impact

- Affected specs: `sidebar-navigation` (MODIFIED 14 Requirement)
- Affected code: 无（纯 spec 文档清理）
- Risk: 低 —— 行为契约不动，仅文档抽象级调整；姊妹工艺已落地两 PR，反例分类规则收敛

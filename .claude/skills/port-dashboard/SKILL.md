---
name: port-dashboard
description: 扫描 CLAUDE.md 的 Capability→crate 进度表、openspec/changes/archive/、openspec/followups.md 三处真相源，打印统一 port 看板，并高亮不一致（例如表里写 done 但 archive 里没记录；followups 里某条 TS impl-bug 在 CLAUDE.md 已标 ✓ 但 followups 本身仍是 pending）。
---

# port-dashboard

当用户说 `/port-dashboard`、"看一下当前 port 进度"、"我们做到哪了" 时触发。新会话启动后若要判断接下来做哪个 port，也应该先跑一次这个 skill。

## 输入

无参数。

## 工作步骤

1. **读三处真相源**（只读，不改）：
   - `CLAUDE.md` 里的 "## Capability → crate map" 表——每行一个 capability + `Port status` 列（done ✓ / done ✓ † / in progress / not started）。
   - `openspec/changes/archive/` 目录下的子目录名，形如 `YYYY-MM-DD-port-<capability>`。每个子目录下 `proposal.md` 头几行即可获取完成日期与范围。
   - `openspec/followups.md` 里所有 `[impl-bug?]` / `[coverage-gap]` / `[spec-gap]` 条目。每条已修的在原文会有 "✅ 已在 …修正" 标记；未修的没有。

2. **构建看板**：按 CLAUDE.md 列出的 13 个 capability 顺序，每行聚合：
   - Capability 名 + owning crate
   - CLAUDE.md 表里的状态
   - archive 里是否有对应 port 目录（有则给归档日期）
   - 该 capability 对应的 followups 条目数（已修 / 剩余）

3. **跨源一致性检查**（高亮不一致）：
   - CLAUDE.md 写 `done` 但 archive 里没有对应 port 目录 → ⚠️ stale status
   - archive 里有 port 目录但 CLAUDE.md 表里仍写 `not started` → ⚠️ missed update
   - followups.md 某条涉及已 done 的 capability，但自身没有 "✅ 已修正" 标记 → ⚠️ followups gc 欠账

4. **输出**（markdown，不超过 40 行）：
   ```
   # Port Dashboard
   _scanned: <今天日期>_

   | # | Capability | Crate | CLAUDE.md | Archive | Followups |
   |---|---|---|---|---|---|
   | 1 | session-parsing | cdt-parse | ✓ done | 2026-04-11 | 2✅ / 0 pending |
   | 2 | chunk-building | cdt-analyze | ✓ done | 2026-04-11 | 1✅ / 0 pending |
   | 3 | tool-execution-linking | cdt-analyze | ✓ done † | 2026-04-11 | 1✅ / 1 pending (Req 4 team enrichment) |
   | 4 | project-discovery | cdt-discover | – | – | 1 pending (spec-gap path decode) |
   ...

   ## 进度
   - 3/13 done, 0 in progress, 10 remaining
   - 下一个推荐：<根据 CLAUDE.md "Remaining port order" 顶行>

   ## 不一致警报
   - ⚠️ <若有>
   - ✅ 三处真相源一致
   ```

## 硬性约束

- 只读。不改 CLAUDE.md、不改 followups.md、不改 archive。
- 如果发现不一致，**提议**修改（列出 diff），但不要自动应用——等用户说"修一下"再动手。
- 输出严格 ≤ 40 行 markdown，不要展开每个 followup 的完整描述。
- 不要运行 `cargo` 或 `openspec` CLI——三处真相源都是纯文本读取。

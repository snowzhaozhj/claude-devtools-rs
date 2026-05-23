---
name: port-dashboard
description: ⚠️ **已过期**（2026-05-23）：原依赖 `openspec/followups.md` 按 capability 切章节，已迁移到 GitHub Issues + `openspec/TS_BASELINE_DEVIATIONS.md` + `openspec/README.md::路线图`，本 skill 不再有承载场景。看进度用 `gh issue list` / `gh issue list --milestone vX.Y`；TS 偏差预警直接 Read `openspec/TS_BASELINE_DEVIATIONS.md`。仅显式 `/port-dashboard` 时才调用。
disable-model-invocation: true
---

# port-dashboard ⚠️ 已过期

claude-devtools-rs 的 port 阶段（13 个 capability）已全部归档（截至 2026-04-12）。原依赖的 `openspec/followups.md` 已于 2026-05-23 改造为 `TS_BASELINE_DEVIATIONS.md`——只装 TS 偏差预警 + UI 隐式契约 + 少量 backlog，**不再按 capability 切章节**。本 skill 的扫描/对照逻辑已无目标。

新流程：
- "看进度" → `gh issue list --state open` + `gh issue list --milestone vX.Y`
- "TS 偏差预警" → `Read openspec/TS_BASELINE_DEVIATIONS.md`
- "路线图" → `Read openspec/README.md`（路线图段）

下面是历史逻辑，仅供需要时手动跑一次参考：

## 输入

无参数。

## 工作步骤

1. **读两处真相源**（只读，不改）：
   - `openspec/changes/archive/` 子目录列表——形如 `YYYY-MM-DD-port-<capability>` 或 `YYYY-MM-DD-<slug>`（后期非 port 的 change 也走这里）。识别其中 `port-<capability>` 前缀的归档日期即为该 capability 的 port 完成日。
   - `openspec/followups.md`——按 `^## <capability>` 切章节；每章节里每个 `^### \[<tag>\]` 条目（tag 为 `impl-bug?` / `coverage-gap` / `spec-gap` / `deviation` / `implicit`）。条目标题里出现"✅ 已在 ... 修正"或正文含"已修复"为"已修"；否则为"pending"。

2. **构建看板**（不要硬编码"13 个 capability"——按 followups.md 实际章节列）：

   - 行 = followups.md 里的 `## <capability>` 章节（13 个 capability 之外可能还有 UI 实时刷新 / 性能 / Subagent / Windows / Implicit 等汇总章节）
   - 列：
     - Capability 名
     - 对应 archive 里有几个 port-* 目录（0 / 1 / N）+ 最新日期
     - followups 条目数（已修 ✅ / pending）

3. **跨源一致性检查**（高亮不一致）：
   - 某 capability 有 archive 但 followups 章节里还有 pending → ⚠️ 落地不全
   - 某 followup 标了"✅ 已在 ... 修正"但 `... ` 引用的 change slug 不在 archive 里 → ⚠️ 引用漂移
   - followups 章节名拼写与 capability 不匹配（如 `team-coordination-metadata` vs `team-metadata`） → ⚠️ 命名漂移

4. **输出**（markdown，≤ 50 行）：

   ```
   # Port Dashboard
   _scanned: <今天日期>_

   | Capability | Archive | Followups |
   |---|---|---|
   | session-parsing | 2026-04-11 | 3✅ / 0 pending |
   | chunk-building | 2026-04-11 | 2✅ / 1 pending（[implicit] SemanticStepGrouper 粒度）|
   | tool-execution-linking | 2026-04-11 | 1✅ / 1 pending（subagent 跨 project_dir）|
   | ...

   ## 汇总
   - 13/13 capability 已 port（archive 全覆盖）
   - followups: N✅ / M pending
   - pending 集中在：<列前 3 个 capability>

   ## 不一致警报
   - ⚠️ <若有>
   - ✅ 两处真相源一致
   ```

## 硬性约束

- 只读。不改 followups.md、不改 archive。
- 发现不一致只**提议**修改（列出 diff），不自动应用——等用户说"修一下"再动手。
- 输出严格 ≤ 50 行 markdown，不要展开每个 followup 的完整描述（最多带条目标题）。
- 不要运行 `cargo` 或 `openspec` CLI——两处真相源都是纯文本读取（archive 用 `ls`，followups 用 `Read` + `Grep`）。
- 不要硬编码 capability 数量——按 followups.md 章节实际数走。如果 followups.md 章节比 archive 少，那是 followups 没全覆盖；反之是 followups 多记了汇总章节。两种情况都在"不一致警报"里报。

## Context

Issue #256（"WebView idle CPU 13.4% 来源未定位"）经 2026-05-24 现场 sample 诊断 + codex 二轮二审定位，主因是 `ui/src/components/Sidebar.svelte::.metadata-pending` 的 `metadata-pending-shimmer 1500ms linear infinite` 动画。诊断数据：

- WebContent 主线程 idle 1.3% → peak 9.5%（用户切 group 制造多 session 同时 pending）
- WebKit `SharedTimer` fires 9.5/s → 44/s
- Paint 路径 `paintReachableBackingStoreContents` → `RenderLayer::paintLayerWithEffects` 占 peak 主线程活跃 **52%**
- `TextBoxPainter::paintForeground` 出现次数从 ~6 暴涨到 **67**（8.6×），CoreText `CTTypesetterCreateWithUniCharProviderAndOptions` 反复重排

shimmer 的存在违反三处既有真相源（详 `proposal.md::Why`）：`PRODUCT.md::Design Principle 5`（"实时但不闪烁"）、`PRODUCT.md::Anti-references` + `Accessibility`（"避免夸张动效" / "动效控制在 150-250ms"）、`DESIGN.md:198`（"Skeleton placeholder 必须静态 opacity 占位，禁用 shimmer"）。

引入路径：

- **PR #177**（`e1a0118`，2026-05-20）："feat(perf): unify session-list loading + SWR cache + visual fade-in" 引入 shimmer 视觉 —— 漏读 `DESIGN.md:198`，codex 二审通过但未交叉检查视觉契约
- **PR #270**（`c159a7a`，2026-05-24）："perf(ui): sidebar shimmer 收紧到 metadata 请求 > 1500ms 才触发" 治标 —— 加 `SvelteMap<sessionId, requestedAt>` + 250 ms `setInterval` ticker + 14 个测试，把触发条件从"骨架"收紧到"骨架 + > 1500 ms"，但代码与 `spec.md::Metadata 占位字段视觉渐显`（"骨架行 SHALL 携带 `.metadata-pending` class 触发 shimmer 动画"，**无阈值**）出现新的 spec drift；同样未对照 `DESIGN.md:198`

stakeholders：sidebar 重度用户（1k+ session 列表、远端 SSH session 元数据高 lag 场景）；perf 责任域（`.claude/rules/perf.md::辅助工具系统 CPU 阈值`）；视觉契约责任域（`DESIGN.md` Named Rules 维护方）。

## Goals / Non-Goals

**Goals:**

1. 让 `PRODUCT.md` / `DESIGN.md` / `spec.md::sidebar-navigation` / `Sidebar.svelte` 实现四方对齐到「**静态 opacity 占位 + 真值 CSS `transition` fade-in**」契约
2. 根除 issue #256 的 paint 路径反模式：删除 `background-position` 动画 + `infinite` 重绘循环
3. 撤销 PR #270 引入的 `metadataRequestedAt` SvelteMap + 250 ms ticker + 相关测试 —— 这些代码仅为 shimmer 服务，shimmer 删除后变为死代码
4. 修复 process 漏洞：`.claude/templates/codex-prompt-{pr,design}-review.md` 补 UI 改动 SHALL 对照 `DESIGN.md` / `PRODUCT.md` 的硬约束，避免下一个视觉契约违规再绕过 codex

**Non-Goals:**

- 不审计或修改其他 5 处 `infinite` CSS 动画（`OngoingBanner ongoing-spin`、`ProjectSwitcher pulse-text`、`SubagentCard sa-status-spin`、`ConnectionStatusBadge spin`、`ContextSwitchOverlay spin`）—— 它们按 `DESIGN.md::The One Live Signal Rule` 是合法 live signal（issue #256 评论已确认），不在本 change 范围
- 不改 IPC / 后端算法 / 数据流：`session-metadata-update` 推送链路、in-place patch 语义、SSE bridge、`sessionListStore` LRU 缓存全部保留不变
- 不改 `.metadata-pending` class 的挂载 / 移除条件：回归 `class:metadata-pending={!session.title && session.messageCount === 0 && !session.isOngoing}`（与 PR #177 之前一致），不再保留 PR #270 的 1500 ms 阈值逻辑

## Decisions

### D1：彻底删除 shimmer 视觉，不走"保留 shimmer + 优化"路径

shimmer 视觉本身违反 `DESIGN.md:198` 硬约束。可考虑的"保留 + 优化"替代：

- **替代 A（伪元素 + transform: translateX 改 compositor-only）**：保留 shimmer 视觉但避开 paint-only 属性 → 仍违反 `DESIGN.md:198` 的"必须静态" + `PRODUCT.md::Principle 5` 的"避免 loading 中间态"，把规则当装饰
- **替代 B（保留 PR #270 的 1500 ms 阈值 + 加 `prefers-reduced-motion: reduce` 静态降级）**：常态不挂 shimmer 但 lag 时仍挂 → 与替代 A 同样把硬约束改成"超阈值时允许"，是开后门；且 `prefers-reduced-motion` 是 a11y 兜底而非规则例外
- **替代 C（保留 shimmer 改成 1 次性 ≤ 2200 ms 短动画）**：依然违反 `DESIGN.md:198`（明令 skeleton placeholder 必须 **静态**），且 1 次后再无视觉提示反而更困惑

**选 D1（删除）**：唯一与三处真相源全部一致的方案，同时根除 perf 反模式，且实现复杂度最低（删代码净减 ~217 行）。

### D2：同步撤销 PR #270 引入的 `metadataRequestedAt` SvelteMap + ticker，不保留为"未来扩展点"

PR #270 引入的代码（`metadataRequestedAt: SvelteMap`、`metadataNow: $state`、`shimmerTickHandle`、`SHIMMER_TICK_INTERVAL_MS`、`lib/metadataShimmer.ts` 整个模块、`tauriMock.ts::pendingMetadataDelayMs` 钩子、`metadataShimmer.test.ts`、`sidebar-shimmer-debounce.spec.ts`）**仅** 服务于 shimmer 阈值判定。shimmer 删除后这些全部变为死代码。

可考虑替代：保留 `metadataRequestedAt` 跟踪机制供未来"如果 lag > N ms 触发其他视觉提示"扩展。

**选 D2（撤销）**：YAGNI（you aren't gonna need it）。死代码长期维护负担、增加 bundle 体积、`SvelteMap` + `$effect` + `setInterval` 三段反应式联动是 svelte 5 陷阱高发区（`ui/CLAUDE.md::Svelte 5 陷阱` 已列）；未来若需新视觉提示再按届时需求重新设计，不预设抽象。

### D3：`spec.md::sidebar-navigation::Metadata 占位字段视觉渐显` 修订而非删除整个 Requirement

Requirement 当前包含两段语义：

- **(a) shimmer 占位动画**（违规，删除）
- **(b) 真值到达后 CSS `transition: opacity 150ms ease-out` fade-in**（合规，保留）

可考虑替代：直接删除整个 Requirement。

**选 D3（修订）**：fade-in transition 是真值到达后对抗"内容跳变"的视觉契约，是合规且有用的行为契约——它不引入 loading 中间态（fade-in 在真值字段已存在的情况下进行），符合 `PRODUCT.md::Principle 5` 的"原地更新"语义。删除整个 Requirement 会丢失这个契约，让"渐显时长 100-200 ms"等已落地的实现失去 spec 锚点。

修订范围：

- 删除「class 上 SHALL 挂统一的 shimmer 占位动画...」整句
- 删除 Scenario「骨架行渲染时显示 shimmer + 占位文字」中 `THEN ... SHALL 携带 .metadata-pending class 触发 shimmer 动画` → 改为 `THEN ... SHALL 携带 .metadata-pending class 应用静态 opacity 占位样式`
- 删除 Scenario「Metadata patch 到达后字段渐显」中 `AND 渐显完成后 shimmer 动画 SHALL 已停止` 一行（无 shimmer 后此 AND 无意义）
- 保留 fade-in transition 的全部 SHALL 与时长约束

### D4：同 PR 补 codex prompt 模板的 DESIGN.md / PRODUCT.md 必读条款（process 修复）

PR #177 / #270 连续绕过 codex 二审引入 / 维护视觉契约违规，根因是 `.claude/templates/codex-prompt-{pr,design}-review.md` 模板**不要求** codex 对照 `DESIGN.md` / `PRODUCT.md`。codex 默认只看代码与 prompt 中明示的怀疑点，视觉契约违规不会被发现。

可考虑替代：单独立 follow-up issue 跟踪 process 修复。

**选 D4（同 PR）**：上下文连贯（issue #256 诊断 + 本 change 的根因分析正是发现 process 漏洞）；改动极小（两个模板各加 5-10 行）；拆分为独立 PR 反而增加 codex / CI / review 总开销而无独立价值（违反 `.claude/rules/parallelism-modes.md::4 ✓ 全满足才拆 PR`）。

### D-V1：骨架行视觉契约 = 静态 `opacity: 0.55` + 静态 `linear-gradient` 占位背景

可考虑替代：纯 `--color-surface-overlay` 背景（删除 `linear-gradient`）。

**选 D-V1（保留 gradient 静态）**：原 `linear-gradient(90deg, transparent 0%, --color-surface-overlay 50%, transparent 100%)` 提供 horizontal 视觉层次，比纯灰块更接近"未加载文本占位"语义且与既有 sidebar 视觉调性一致；不动画 `background-position` = 不触发 paint loop = 不违反 `DESIGN.md:198`；删 gradient 改纯色对视觉收益有限但需评估其他 token 变更，超出本 change 的"删除 shimmer"边界。

## Visual Contract

### Surface Decision

不新增 surface。`Sidebar.svelte` 既有 session list 容器、骨架行 `.metadata-pending` 子元素结构均不变；仅删除 shimmer 动画 class 与 PR #270 引入的运行时状态机。

链回 `PRODUCT.md::Anti-references`「不...夸张动效」+ `PRODUCT.md::Design Principle 5`「实时但不闪烁」。

### Visual Layer

引用 Named Rules：

- `DESIGN.md::The One Live Signal Rule` 边界条款（DESIGN.md:198）：「Skeleton placeholder 必须**静态** opacity 占位，**禁用** shimmer，避免与真 live signal 竞争注意力」—— 本 change 直接对齐这条
- `DESIGN.md::The Static-vs-Live Shape Rule`（DESIGN.md:174）：动态 live = circular spinner（OngoingBanner / SubagentCard）；静态识别 = outline 空心圆。**shimmer 不属于任何一类形态**，是 form-impurity——删除还原到二元对立框架
- `DESIGN.md::The Persistent Selection Is Quiet Rule`（DESIGN.md:178）：sidebar 长期持有视觉权重应让位给当前焦点。骨架行作为列表中的瞬时态，更不应竞争注意力

### State Coverage

| 状态 | class | 视觉 | 实现位置 |
|---|---|---|---|
| 骨架态（pending） | `.metadata-pending` | 静态 `opacity: 0.55` + 静态 `linear-gradient` 背景 + 占位回退文本（**完整 sessionId**，由 CSS `text-overflow: ellipsis` 自然截断——与主 spec `Requirement: 会话项展示::Scenario: 无标题的会话` 对齐，禁 JS 侧手动 `slice(0, 8) + "…"`）| `Sidebar.svelte::.session-item.metadata-pending .session-title-text/.session-meta` 静态 CSS |
| fade-in 过渡 | 移除 `.metadata-pending` 同帧 | CSS `transition: opacity 150ms ease-out` 从 0.55 → 1 | `Sidebar.svelte::.session-item .session-title-text/.session-meta { transition: opacity 150ms ease-out }`（既有，不变） |
| 真值态 | 无 class | `opacity: 1`，正常文本 | 同上 |

不存在的状态（删除）：

- ~~shimmer 横移动画态~~：删除
- ~~`metadataNow` ticker 驱动的 1500 ms 阈值判定态~~：删除（PR #270 撤销）

### DESIGN.md delta plan

无 delta。本 change 是 `DESIGN.md:198` 既有规则的代码对齐，不新增 / 不修改 token、不新增 / 不修改 Named Rules。`DESIGN.md` 文件本身不需要改动。

archive 阶段不跑 `/impeccable extract`（无可提取新规约）。

## Risks / Trade-offs

| Risk | Mitigation |
|---|---|
| 撤销 PR #270 让最近改过这块的开发者感觉重复劳动 | `proposal.md::Why` + 本 design.md `Context` 显式归档 PR #177 → #270 → 本 change 的链路与决策依据；本 change 的 commit message 引用 PR #270 commit hash 让 git blame 可追溯 |
| 删除 e2e `sidebar-shimmer-debounce.spec.ts` 后骨架行视觉无 e2e coverage | `tasks.md` 加新 e2e（或扩既有 e2e）：assert 骨架行 `getComputedStyle(...).animation === 'none'` + assert opacity ~ 0.55；行为契约从"shimmer 出现/消失时机"切换到"静态视觉 + fade-in 平滑性" |
| `lib/metadataShimmer.ts` 被删除后，仍被 import 的代码会导致 svelte-check / vitest fail | `tasks.md` 显式列删除顺序：先删 import / 调用方（Sidebar.svelte、相关测试），再删模块文件；本地跑 `pnpm --dir ui run check` 确认 0 error 后再 push |
| `tauriMock.ts::pendingMetadataDelayMs` 钩子可能被其他 e2e 引用 | grep 全仓 `pendingMetadataDelayMs`，确认仅 `sidebar-shimmer-debounce.spec.ts` 使用；删除后批量验证 `pnpm --dir ui exec playwright test` 全过 |
| codex prompt 模板补 DESIGN.md / PRODUCT.md 必读条款可能让 codex 二审 token 消耗增加 | 控制条款长度（每个模板 +5-10 行）；只要求 codex "若改动涉及 UI 组件 / `.svelte` / 视觉行为，对照 `DESIGN.md` Named Rules 与 `PRODUCT.md` Design Principles 检查"，不强制每次都跑完整契约扫描 |
| spec 修订删除 shimmer SHALL 句后，未来若有人再次提议 skeleton 加动效会缺少历史决策上下文 | 主 spec 的 Requirement 描述里加一条「skeleton placeholder 视觉 SHALL **不**包含 `infinite` 动画或 paint-only / `background-position` 类周期重绘」防回归 SHALL 句，引用 `DESIGN.md::The One Live Signal Rule` |
| 主 spec `Requirement: Metadata 占位字段视觉渐显::Scenario: Metadata patch 到达后字段渐显` 自 PR #177 起就要求 `title 文本 SHALL 通过 CSS transition: opacity 150ms ease-out` fade-in，但实现仅给 `.session-item` 容器层加了 `transition: opacity 0.15s`，**没**给子元素 `.session-title-text` / `.session-meta` 加 transition——子元素 opacity 从 `0.55` 突变回 `1` 是瞬时切换、未真生效 fade-in（spec 与实现旧 bug） | `tasks.md` 加修复任务：给 `.session-title-text` / `.session-meta` 增加 `transition: opacity 150ms ease-out`，让 spec SHALL 句真生效；不改 spec（spec 描述的行为是合理的，仅实现漏） |
| 用户反馈"没 shimmer 反而看不出加载中"想回滚 | **merge 前**：直接 `git revert` 本 PR 的所有 commit + 重写 PR；**merge 后但 archive 前**（PR active 状态）：CI 会拦"已完成但未 archive"，所以 merge 后必须立即 archive 才不挂 CI；**archive 后**：开新 OpenSpec change（slug 类似 `restore-sidebar-skeleton-cue`），重新对照 `DESIGN.md:198`/`PRODUCT.md::Principle 5` 论证替代视觉契约（如静态非动画形态的 lag 提示），走完整 propose / apply / archive 流程——不能直接撤本 change 的 archive commit（`openspec/changes/archive/<日期>-<slug>/` 是冻结快照，撤销会破坏其他 change 的 archive 顺序假设）；预计回滚成本：merge 前 < 1 h、archive 后 ≥ 半天 |

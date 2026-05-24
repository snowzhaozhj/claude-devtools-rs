## Why

Issue #256 性能诊断（2026-05-24）实测 sidebar `metadata-pending-shimmer` 在 metadata 慢到达 / 用户切 group 触发多 session 同时 pending 时，`peak` 主线程 9.5% 单核（Activity Monitor 13.4% 进程级），其中 paint 路径占主线程活跃 52%、`TextBoxPainter::paintForeground` 暴涨 8.6×、CoreText `CTTypesetterCreateWithUniCharProviderAndOptions` 在每帧反复重排——根因是 shimmer 通过 `background-position` 动画（paint-only 属性）持续标脏覆盖文本的 backing store。

更深层问题：shimmer 的存在本身违反三处既有真相源（issue #256 评论已闭环坐实）：

1. **`PRODUCT.md::Design Principle 5`**：「实时但不闪烁。会话刷新、metadata patch 和通知更新应保持原地更新，**避免 loading 中间态打断阅读**。」
2. **`PRODUCT.md::Anti-references` + `Accessibility`**：「避免...夸张动效」+「动效控制在 150-250ms」——`metadata-pending-shimmer` 是 1500 ms `infinite` 装饰性循环，6 倍超动效预算。
3. **`DESIGN.md:198`**（One Live Signal Rule 边界）：「Skeleton placeholder（**必须静态** opacity 占位，**禁用** shimmer，避免与真 live signal 竞争注意力）」。

PR #177（2026-05-20，merge `e1a0118`）引入 shimmer 时漏读 `DESIGN.md:198`；PR #270（2026-05-24，merge `c159a7a`）随后从触发条件层面收紧（仅 metadata 请求 > 1500 ms 才挂 class）——此次治标的副作用是引入 `SvelteMap<sessionId, requestedAt>` + 250 ms `setInterval` ticker + 14 个测试用例，并让代码（"骨架 + > 1500 ms"）与 `spec.md:921-943` 既有 SHALL 句（"骨架行 SHALL 携带 `.metadata-pending` class 触发 shimmer 动画"，**未规定阈值**）出现新的 spec drift。

本 change 选择**根除路径**：彻底删除 shimmer 视觉 + 撤销 PR #270 引入的运行时 + 修订 `sidebar-navigation` spec 让三方（`PRODUCT.md` / `DESIGN.md` / `spec.md` / 实现）一致对齐到「静态 opacity 占位 + 真值到达后 CSS `transition` fade-in」。

## What Changes

- **BREAKING (视觉契约)**：sidebar 骨架行视觉从「shimmer 1500 ms `infinite` + opacity 0.55」改为「静态 opacity 0.55 + `linear-gradient` 占位背景」；真值到达后保留既有 `transition: opacity 150ms ease-out` fade-in
- 删除 `ui/src/components/Sidebar.svelte` 中的 `animation: metadata-pending-shimmer 1500ms linear infinite` + `@keyframes metadata-pending-shimmer`
- 撤销 PR #270 (`c159a7a`) 引入的代码：`metadataRequestedAt: SvelteMap`、`metadataNow` `$state`、`shimmerTickHandle` 250 ms ticker、相关 `$effect` 块、`onDestroy` 清理、`SHIMMER_TICK_INTERVAL_MS` 常量
- 删除 `ui/src/lib/metadataShimmer.ts` 与 `ui/src/lib/metadataShimmer.test.ts`
- 删除 `ui/src/lib/tauriMock.ts` 中 `pendingMetadataDelayMs` URL 钩子相关代码
- 删除 e2e 测试 `ui/tests/e2e/sidebar-shimmer-debounce.spec.ts`
- 修订 `openspec/specs/sidebar-navigation/spec.md` Requirement「Metadata 占位字段视觉渐显」：移除"`SHALL` 挂统一的 shimmer 占位动画"句、对应 Scenario 中 shimmer 相关 `THEN` / `AND`、保留"骨架 → 真值 fade-in via CSS transition"行为契约
- 同步更新 `ui/CLAUDE.md::数据流（前端侧）` 段落：「shimmer + fade-in 承载」→「静态 opacity 占位 + fade-in 承载」
- 补丁 `.claude/templates/codex-prompt-pr-review.md` 与 `.claude/templates/codex-prompt-design-review.md`：当改动涉及 UI 组件时，prompt 模板 SHALL 显式要求 codex 对照 `DESIGN.md` / `PRODUCT.md` 的视觉契约 / 设计原则进行交叉检查（防止本次 PR #177 / #270 类视觉契约违规再次绕过 codex 二审）

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `sidebar-navigation`：修订 Requirement「Metadata 占位字段视觉渐显」，移除 shimmer SHALL 要求、保留 fade-in transition 行为契约；删除 PR #259 / #270 引入的「> 1500 ms 阈值」语义（spec 此前未含此阈值，但代码已偏离）

## Impact

- **代码删除**：`ui/src/lib/metadataShimmer.ts`、`ui/src/lib/metadataShimmer.test.ts`、`ui/tests/e2e/sidebar-shimmer-debounce.spec.ts` 三个新文件整体回收（共 ~217 行）
- **代码修改**：
  - `ui/src/components/Sidebar.svelte`（删 shimmer CSS + 撤销 PR #270 的 SvelteMap / ticker / `$effect` 块；预估净减 ~80 行）
  - `ui/src/lib/tauriMock.ts`（删 `pendingMetadataDelayMs` 钩子）
- **文档同步**：`ui/CLAUDE.md`、`.claude/templates/codex-prompt-{pr,design}-review.md`
- **Spec 变更**：`openspec/specs/sidebar-navigation/spec.md`（archive 时 sync）
- **不影响**：IPC 字段 / 后端算法 / 状态判定 / 数据流语义；`session-metadata-update` 推送链路与 in-place patch 不变；`.metadata-pending` class 的挂载 / 移除条件回归 spec 原始语义（`!session.title && session.messageCount === 0 && !session.isOngoing` 判定，与 PR #177 之前一致）
- **关闭 issue**：#256（idle WebView CPU 13.4% 来源未定位）；副作用关闭 #259（shimmer 收紧——根除取代收紧）
- **性能预期**：基于 issue #256 peak 数据，删除 shimmer 后预估 peak 主线程 9.5% → < 2%、Activity Monitor 13.4% → < 5%（paint 路径整段消失，CoreText 重排归零）

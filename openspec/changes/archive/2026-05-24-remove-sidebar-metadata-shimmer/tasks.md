## 1. 删除 PR #270 引入的运行时与测试

- [x] 1.1 删除 `ui/src/components/Sidebar.svelte` 中 `metadataRequestedAt: SvelteMap`、`metadataNow: $state`、`shimmerTickHandle`、`SHIMMER_TICK_INTERVAL_MS` 常量、`SvelteMap` import、`metadataShimmer` 模块 import 与所有 `$effect` 块（sessions 数组同步、ticker 启停）、`onDestroy` 内 `shimmerTickHandle` 清理代码（撤销 PR #270 / commit `c159a7a` 在 `Sidebar.svelte` 的全部新增）
- [x] 1.2 删除文件 `ui/src/lib/metadataShimmer.ts`
- [x] 1.3 删除文件 `ui/src/lib/metadataShimmer.test.ts`
- [x] 1.4 删除 `ui/src/lib/tauriMock.ts` 中 `pendingMetadataDelayMs` URL 钩子相关代码（query string 解析、fixture metadata emit 延迟分支、相关注释）
- [x] 1.5 删除文件 `ui/tests/e2e/sidebar-shimmer-debounce.spec.ts`
- [x] 1.6 grep 全仓 `metadata-pending-shimmer` / `metadataShimmer` / `pendingMetadataDelayMs` / `shimmerTickHandle` / `metadataRequestedAt` 确认无遗漏引用

## 2. 删除 Sidebar.svelte shimmer CSS + 还原 metadata-pending class 判定

- [x] 2.1 删除 `ui/src/components/Sidebar.svelte` CSS 中 `animation: metadata-pending-shimmer 1500ms linear infinite;` 一行
- [x] 2.2 删除 `ui/src/components/Sidebar.svelte` CSS 中 `@keyframes metadata-pending-shimmer { ... }` 整段
- [x] 2.3 把模板中 `class:metadata-pending={...}` 的判定还原为 `!session.title && session.messageCount === 0 && !session.isOngoing`（撤销 PR #270 引入的 `shouldShowMetadataShimmer` 调用），与 spec.md 当前 SHALL 句一致
- [x] 2.4 更新 `ui/src/components/Sidebar.svelte` 中 `.session-item.metadata-pending` 上方的多行 CSS 注释（line 1596-1602）：移除"shimmer 横移 + 内容半透明 + 加载中语义视觉化"叙述，改为对齐 spec 修订后的"静态 opacity 0.55 占位 + linear-gradient 静态背景层次 + 真值到达后 CSS transition fade-in"语义；引用 `DESIGN.md::The One Live Signal Rule` 边界条款（DESIGN.md:198）作为锚点
- [x] 2.5 修复 spec 与实现旧 bug：`Sidebar.svelte` CSS 给 `.session-title-text` 与 `.session-meta` 子元素增加 `transition: opacity 150ms ease-out`，让主 spec `Requirement: Metadata 占位字段视觉渐显::Scenario: Metadata patch 到达后字段渐显` 中"title 文本 SHALL 通过 CSS `transition: opacity 150ms ease-out` 从骨架占位的 `opacity: 0.55` 渐升到正常的 `opacity: 1`"真生效（之前 PR #177 仅在 `.session-item` 容器层加 transition，子元素 opacity `0.55 → 1` 是瞬时切换、未真生效 fade-in；本 change 同时修订 spec Scenario 措辞从"透明渐变到不透明"对齐到"0.55 → 1"——codex 三审 finding #2 / 第二轮验证残留修正）
- [x] 2.6 占位回退文本 SHALL 显示**完整 sessionId**（CSS `text-overflow: ellipsis` 自然截断），与主 spec `Requirement: 会话项展示::Scenario: 无标题的会话` 一致——若 PR #177 / #270 路径上 `Sidebar.svelte` 的 fallback 文本曾被改为 `slice(0, 8) + "…"`，本 task SHALL 还原为完整 sessionId；grep `Sidebar.svelte` 内 `slice(0, 8)` / `substring(0, 8)` 等手动截断模式确认无遗漏

## 3. 子目录文档与注释同步

- [x] 3.1 更新 `ui/CLAUDE.md::数据流（前端侧）` 段落：「冷启视觉过渡由 `.metadata-pending` shimmer + `transition: opacity 150ms` fade-in 承载」→「冷启视觉过渡由 `.metadata-pending` 静态 opacity 占位 + `transition: opacity 150ms` fade-in 承载」
- [x] 3.2 grep 仓库内其它 markdown / 注释提到 "shimmer" 的地方（CLAUDE.md / followups.md / 注释等），逐一确认是否需要同步措辞，删除会误导的"加载中 shimmer"描述

## 4. 测试覆盖调整

- [x] 4.1 新增或扩展 `ui/tests/e2e/` 下的 e2e spec 覆盖"骨架行视觉为静态"行为：assert 骨架 `.session-item.metadata-pending` 子元素 `getComputedStyle(el).animationName === 'none'` 与 `opacity` 在 `[0.5, 0.6]` 区间；assert metadata patch 到达后 `.metadata-pending` 同帧移除并触发 opacity transition；命名建议 `sidebar-skeleton-static.spec.ts`
- [x] 4.2 删除 `ui/src/components/Sidebar.test.svelte.ts` 中 PR #270 引入的 shimmer 阈值判定测试（如存在），保留 PR #270 之前已有的骨架行渲染回退文本测试
- [x] 4.3 跑 `pnpm --dir ui run check`（svelte-check 0 errors）
- [x] 4.4 跑 `pnpm --dir ui exec vitest run`（含新加 / 调整后的单测）
- [x] 4.5 跑 `pnpm --dir ui exec playwright test`（含新加 e2e）

## 5. codex prompt 模板补 DESIGN.md / PRODUCT.md 必读条款（process 修复）

- [x] 5.1 修改 `.claude/templates/codex-prompt-pr-review.md`：在 prompt 模板的「我希望你重点查的问题」段前或「约束」段中加入新条款，要求 codex 在改动涉及 UI / `.svelte` / CSS / 视觉行为时**主动**对照 `DESIGN.md` Named Rules 与 `PRODUCT.md` Design Principles / Anti-references / Accessibility 进行交叉检查（防止 PR #177 / #270 类视觉契约违规再次绕过 codex 二审）；条款 ≤ 10 行，不强制每次完整契约扫描
- [x] 5.2 同步修改 `.claude/templates/codex-prompt-design-review.md`：在「我的具体怀疑点」段前或「约束」段加入对应条款（design 阶段二审同样需要在 propose 完成前对照视觉契约）；条款 ≤ 10 行
- [x] 5.3 ~~同步修改 `.claude/templates/codex-prompt-progressive-diagnosis.md`~~ —— 该文件目前不存在（`.claude/rules/codex-usage.md` 第 5 行是前向引用，等其他 change 创建该模板）；本 change 范围**不**含创建新模板，**仅**补丁现有的 2 个；未来创建该第三个模板时 SHALL 同步包含本 change 引入的视觉契约交叉检查条款

## 6. 本地 preflight 与 spec 验证

- [x] 6.1 跑 `just preflight`（fmt + lint + test + spec validate 全过）
- [x] 6.2 跑 `openspec validate remove-sidebar-metadata-shimmer --strict` 通过
- [x] 6.3 propose 阶段调 codex design 二审（按 `.claude/rules/codex-usage.md` 第 3 节：UI 重构 + 性能关键路径双命中，必须 codex 二审 design.md），按 `.claude/templates/codex-prompt-design-review.md` 模板；codex 报问题先修 design / spec / tasks 三处文档再 re-validate 才进 apply
- [x] 6.4 perf 实测对比（替代估算）：在同一复现场景（用户切 group 制造多 session 同时 metadata-pending）下用 `/perf` skill `idle-cpu-diagnose` 子模式跑 30s sample × 2（main + WebContent），分别取 baseline（origin/main 含 PR #270）与 patch（本 change 落代码后）；按 `.claude/rules/perf.md::PR Perf impact 模板` 整理四维数据（wall / user / sys / max RSS / user/real ratio）+ 主线程活跃 % + WebKit `SharedTimer` fires/30s + paint 路径 `paintReachableBackingStoreContents` 占比 + `TextBoxPainter::paintForeground` 出现次数

## 7. 发布

- [x] 7.1 push 分支 + 开 PR（PR 描述含本 change 链路：issue #256 诊断 + codex 二轮二审 + impeccable 加载发现 spec/DESIGN.md 三方矛盾 + 撤销 PR #270 决策；**Perf impact** 段贴 6.4 实测对比表（baseline vs patch），**禁止**仅写"预期 13.4% → < 5%"估算——按 `.claude/rules/perf.md::PR Perf impact 模板（强制）` 四维齐贴；如果实测未达 < 5% 目标，PR 描述 SHALL 显式标注实测值 + 解释剩余 CPU 来源（避免"修了但没修干净"被合并）
- [x] 7.2 wait-ci 全绿（`scripts/check-openspec-archives.sh` 等 CI-only check）
- [x] 7.3 codex 二审通过（PR push 后调 `Agent({ subagent_type: "codex:codex-rescue", ... })`；如发现 bug：修 → push → 回到 7.2 重跑；可循环 M 次）
- [x] 7.4 archive change（`openspec archive remove-sidebar-metadata-shimmer -y` 一步原子完成 mv + sync；archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）；PR merge 后 issue #256 自动关闭（PR 描述含 `Closes #256`）；issue #259 已由 PR #270 关闭，本 PR **不**重复 close（codex 三审 finding #3）；PR 描述含 `Refs #259` / `supersedes #270 的实现路径` 标注关系

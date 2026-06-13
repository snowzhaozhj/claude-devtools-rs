## 1. menu-items factory

- [x] 1.1 在 `ui/src/lib/contextMenu/menu-items.ts` 新增 `buildMarkdownBlockItems(text: string, ctx: MenuItemContext)`：空 text 返回 `[]`；非空时 `appendSelectionCopyIfAny` → 「复制纯文本」(`stripMarkdownFormatting(text)`) → 「复制为 Markdown」(原文 `text`) → `finalizeWithSeparators`
- [x] 1.2 从 `markdown.ts` 导出 `stripMarkdownFormatting`（当前为内部函数）供 factory 复用，或在 factory 内复用现有导出路径

## 2. SessionDetail 挂载

挂载点统一为**展开后的 `<div class="prose lazy-md">` 原生元素本身**（`use:` action 不能挂 Svelte 组件 `BaseItem`；prose div 是 `BaseItem` children snippet 内的原生 DOM）。命中区域 = 展开内容区，折叠态 prose 不渲染故无菜单（见 design D3）。一个元素可同时持有 `{@attach attachMarkdown(...)}` 与 `use:contextMenu`。

- [x] 2.1 Output 块（`item.type==="output"`，`:1276`）的 prose div 加 `use:contextMenu={() => buildMarkdownBlockItems(item.text, buildMenuCtx())}`（`buildMenuCtx` 已在 oncontextmenu 瞬间预读 `selectionText`）
- [x] 2.2 Thinking 块（`item.type==="thinking"`，`:1262`）prose div 加同款 provider（`item.text`）
- [x] 2.3 User message 块（`item.type==="user_message"`，`:1289`）prose div 加同款 provider（`item.text`）
- [x] 2.4 slash/SKILL 块（`item.type==="slash"`，`:1209`）prose div 加 provider（`item.slash.instructions`）；`instructions` 为空时 prose 不渲染、自然不挂
- [x] 2.5 确认 `buildMenuCtx()` 返回的 `selectionText` 在右键瞬间读取（复用现有 chunk 容器的 ctx 构造路径），factory 不读 DOM

## 3. 测试

- [x] 3.1 `menu-items.test.ts` 新增 `buildMarkdownBlockItems` 单测：纯文本/Markdown 两项内容正确、空 text 返回 `[]`、有选区融合「复制选中文本」、纯函数无 DOM 依赖
- [x] 3.2 e2e（Playwright）验证四类块各自：右键仅弹该块菜单（含两项）、**不**弹整条 AI 消息菜单、「复制为 Markdown」写入该块自身文本（Output/Thinking/User message 用 `item.text`，slash 用 `instructions`）；slash 块另测 `instructions` 为空时不弹菜单
- [x] 3.3 `pnpm --dir ui run check` + `vitest --run` 受影响测试全绿

## 4. 验证与文档

- [x] 4.1 `just dev` 或 `?mock=1` 浏览器入口手动验：右键四类块各弹该块菜单、复制内容正确、有选区时「复制选中文本」优先、不影响代码块/OutputBlock 现有复制按钮
- [x] 4.2 CHANGELOG `## [Unreleased]` `### Fixed` 追加一行（用户可感知：右键工具展开块可直接复制该块内容）
- [x] 4.3 开 GitHub issue 跟踪 `CopyButton` `mode` prop 的 spec/实现漂移（既有技术债，不在本 change 范围）

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex + pr-review-toolkit 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

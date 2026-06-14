## 1. ExecutionTrace 挂载

挂载点统一为**展开后的 `<div class="prose ...">` 原生元素本身**（`use:` action 不能挂 Svelte 组件 `BaseItem`；prose div 是 `BaseItem` children snippet 内的原生 DOM）。命中区域 = 展开内容区，折叠态 prose 不渲染故无菜单（见 design D4）。

- [x] 1.1 `ui/src/components/ExecutionTrace.svelte` import：`contextMenu`（`../lib/contextMenu.svelte`）、`buildMarkdownBlockItems` + `type MenuItemContext`（`../lib/contextMenu/menu-items`）、`getMenuSettings`（`../lib/contextMenu/settings.svelte`）、`getMenuItemDispatch`（`../lib/contextMenu/dispatch`）
- [x] 1.2 新增 `buildBlockMenuCtx(): MenuItemContext`：`sessionId: traceSessionId`、`projectId`、`settings: getMenuSettings()`、`selectionText: window.getSelection()?.toString() ?? ""`、`dispatch: getMenuItemDispatch()`
- [x] 1.3 Thinking 块（`:173`）prose div 加 `use:contextMenu={() => buildMarkdownBlockItems(item.text, buildBlockMenuCtx())}`
- [x] 1.4 Output 块（`:188`）prose div 加同款 provider（`item.text`）
- [x] 1.5 User message 块（`:202`）prose div 加同款 provider（`item.text`）
- [x] 1.6 确认 slash 块（无展开 body）与 tool 块（ToolViewer 自带复制）不动；嵌套 SubagentCard 透传 `projectId`（如缺失）

## 2. 测试

- [x] 2.1 `buildMarkdownBlockItems` 单测已由 PR #516（`menu-items.test.ts`）覆盖纯文本/Markdown/空文本/选区融合——确认无需重复，不行则补
- [x] 2.2 e2e（Playwright）验证：展开 subagent trace 后右键 Output/Thinking/User 块各弹该块菜单（含两项）、**不**冒泡到父 AI 消息菜单、「复制为 Markdown」写入该块自身 `item.text`（非整条消息）；至少一个 case 用内容断言（非 count 断言）保证判别力
- [x] 2.3 e2e 覆盖 workflow agent trace 展开块右键路径（与 subagent 共用 ExecutionTrace，验证一致行为）
- [x] 2.4 `pnpm --dir ui run check` + `vitest --run` 受影响测试全绿

## 3. 验证与文档

- [x] 3.1 `?http=1` 或 `?mock=1` 浏览器入口真数据手动验：展开真实 subagent + workflow agent trace，右键三类块各弹该块菜单、复制内容正确、有选区时「复制选中文本」优先、0 console error
- [x] 3.2 CHANGELOG `## [Unreleased]` `### Fixed` 追加一行（用户可感知：subagent / workflow 执行链内工具展开块也可右键复制该块内容）

## N. 发布

- [x] N.1 push 分支 + 开 PR
- [x] N.2 wait-ci 全绿
- [x] N.3 codex + pr-review-toolkit 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [x] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

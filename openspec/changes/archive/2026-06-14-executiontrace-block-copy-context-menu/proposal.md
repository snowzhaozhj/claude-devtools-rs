## Why

PR #516（change `tool-block-copy-context-menu`）给会话详情流里的工具展开块（slash/SKILL、Output、Thinking、User message）加了右键复制——但只挂在 `SessionDetail.svelte` 的块上。subagent 与 workflow 的执行链**都经 `ExecutionTrace.svelte` 渲染**（`SubagentCard` 与 `WorkflowCard` 都把 trace 交给它），而 ExecutionTrace 内的 thinking / output / user_message 三块 `.prose` 容器**完全没挂 `use:contextMenu`**。结果：展开一个 subagent（或 workflow agent）的执行链后，右键里面的 Thinking / Output / User prompt 块复制不到该块内容，复现了 PR #516 想消除的同一个 friction。

PR #516 的 `session-display::消息 chunk 右键菜单` Requirement 把块级复制 Scenario 全部限定在"AI 消息组内"（SessionDetail），spec line 991 仅把 "subagent ExecutionTrace" 当作 stopPropagation 的子元素提及，**没有**断言 ExecutionTrace 自己的块有复制菜单——这是一个 spec 覆盖缺口，本 change 补齐。

## What Changes

- 给 `ExecutionTrace.svelte` 内承载单段 markdown 源文本的三类块——Thinking（`item.type==="thinking"`）、Output（`item.type==="output"`）、User message（`item.type==="user_message"`）——的展开 `.prose` 容器各挂 `use:contextMenu={() => buildMarkdownBlockItems(item.text, ctx)}`，复用 PR #516 已落地的 `buildMarkdownBlockItems` factory，输出「复制纯文本 / 复制为 Markdown」（有选区时融合「复制选中文本」）。
- 因 ExecutionTrace 同时被 `SubagentCard` 与 `WorkflowCard` 复用，单点修复**同时覆盖 subagent 执行链与 workflow agent 执行链**两个场景。
- 零新增 factory / 组件 / 依赖；复制源直接用块的 `item.text`，无新数据提取路径。
- ExecutionTrace 内的 slash 块为 `collapsible={false}` 无展开 body、tool 块由各 ToolViewer（Read/Edit/Write/Bash 已挂 contextMenu，Default 用 OutputBlock 自带复制按钮 + stopPropagation）覆盖，**不在**本 change 改动范围。

## Capabilities

### New Capabilities

（无）

### Modified Capabilities

- `session-display`: `Subagent 内联展开 ExecutionTrace` Requirement 扩展——明确 ExecutionTrace 内 thinking/output/user_message 块各挂自己的 `buildMarkdownBlockItems` 右键复制菜单，覆盖 SubagentCard 与 WorkflowCard 两条渲染路径，右键弹该块菜单且复制该块自身文本。

## Impact

- `ui/src/components/ExecutionTrace.svelte`：import `contextMenu` action + `buildMarkdownBlockItems` factory + dispatch/settings helper；新增 `buildBlockMenuCtx()` 构造 `MenuItemContext`；thinking/output/user_message 三块 prose div 挂 `use:contextMenu`。
- 测试：`ExecutionTrace` 块复制的 e2e（subagent + workflow trace 展开块右键弹该块菜单、不冒泡、复制内容为块自身文本）；`buildMarkdownBlockItems` 单测已由 PR #516 覆盖。
- 无 IPC / 后端 / Rust 改动；无新增依赖；无视觉改动（仅新增右键交互）。

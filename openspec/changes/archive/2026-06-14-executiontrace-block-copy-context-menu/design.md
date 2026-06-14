## Context

PR #516 给 `SessionDetail.svelte` AI 组内的四类工具展开块挂了 `use:contextMenu={() => buildMarkdownBlockItems(...)}`。但 subagent 与 workflow 的执行链不经 SessionDetail 那段模板——它们走 `ExecutionTrace.svelte`：

- `SubagentCard.svelte:401`：`<ExecutionTrace items={traceItems} {rootSessionId} sessionId={process.sessionId} {depth} />`
- `WorkflowCard.svelte:211`：`<ExecutionTrace items={agentDisplayItems} rootSessionId={sessionId} />`

`ExecutionTrace.svelte` 内三类块的展开 body 是裸 `<div class="prose ...">{@html renderMarkdown(item.text)}</div>`，**没挂任何 `use:contextMenu`**：

- Thinking（`item.type==="thinking"`，`:173`，`<div class="prose prose-thinking">`，复制源 `item.text`）
- Output（`item.type==="output"`，`:188`，`<div class="prose">`，复制源 `item.text`）
- User message（`item.type==="user_message"`，`:202`，`<div class="prose">`，复制源 `item.text`）

右键这三块时，事件不被任何块级菜单拦截——冒泡到全局兜底（`preventDefault` 阻止系统菜单但不弹复制菜单）或 Layer 2 选区菜单（仅在有选区时）。结果：用户右键 Thinking/Output/User 块复制不到该块内容，正是 PR #516 想消除的 friction 在执行链场景的残留。

基础设施已齐备（PR #516 落地）：`buildMarkdownBlockItems(text, ctx)` factory 已存在并被单测覆盖；`use:contextMenu` action 的 `handleContextMenu` 已 `e.stopPropagation()`；选区融合走 `appendSelectionCopyIfAny`。本 change 只需把现成模式复制到 ExecutionTrace 这个新 surface。

## Goals / Non-Goals

**Goals:**

- 右键 ExecutionTrace 内 Thinking / Output / User message 块时弹**该块**的复制菜单（复制纯文本 / 复制为 Markdown / 有选区时复制选中文本）。
- 单点修复 ExecutionTrace 同时覆盖 subagent 执行链与 workflow agent 执行链。
- 复用现有 factory 与视觉契约，零新增 factory / 组件 / 依赖 / IPC。

**Non-Goals:**

- **不**改 ExecutionTrace 内 slash 块（`collapsible={false}` 无展开 body，无内容可复制）与 tool 块（各 ToolViewer 已自带复制路径）。
- **不**改 `TeammateMessageItem` 整条消息体复制（属不同 surface，复制源是整条 teammate 消息而非工具展开块，应走 `buildUserMessageItems` 类 factory，另开 change）。
- **不**改 compact / system chunk 块（不同 chunk 类型，另议）。
- **不**改任何后端 / IPC / 数据结构。

## Decisions

### D1：单点改 ExecutionTrace 同时覆盖 subagent + workflow

subagent 执行链（SubagentCard）与 workflow agent 执行链（WorkflowCard）都把 trace 委托给同一个 `ExecutionTrace.svelte`。在 ExecutionTrace 内挂菜单是唯一无重复的修复点——既不需要改 SubagentCard / WorkflowCard，也不会漏掉任一场景。

**Alternative considered**：在 SubagentCard / WorkflowCard 各自包裹一层菜单。否决：二者都只是 trace 的容器，真正渲染块的是 ExecutionTrace；在外层包裹既重复又命中区域错位（会扩到 chip / header）。

### D2：复用 `buildMarkdownBlockItems(text, ctx)`，不新增 factory

三块复制语义与 SessionDetail 的 output/thinking/user_message 块完全相同（一段 markdown 源 → 纯文本 + Markdown 两项），直接复用 PR #516 的 `buildMarkdownBlockItems`。复制源用块的 `item.text`（已是 markdown 源），「复制纯文本」走 `stripMarkdownFormatting`。

### D3：ExecutionTrace 内自建 `buildBlockMenuCtx()` 构造 `MenuItemContext`

ExecutionTrace 没有 SessionDetail 的 `buildMenuCtx()`，需自建等价构造：`sessionId` 用 `traceSessionId`（`sessionId ?? rootSessionId`，trace 所属会话）、`projectId` 用 props `projectId`（嵌套场景为 `""`）、`selectionText` 右键瞬间读 `window.getSelection()?.toString() ?? ""`、`settings` 走 `getMenuSettings()`、`dispatch` 走 `getMenuItemDispatch()`。

**关键不变量**：`buildMarkdownBlockItems` 仅消费 `ctx.selectionText`（选区融合）与 `ctx.dispatch.copyToClipboard`（写剪贴板）；`sessionId` / `projectId` / `settings` 对纯 markdown 块复制**不参与**计算（它们服务于 openInEditor / openInTerminal 等文件类 item，本 factory 不产出）。故 projectId 为 `""` 不影响复制正确性。

### D4：挂在展开后的 `.prose` 原生 div 上，`stopPropagation` 由 action 内置兜底

`use:` action 只能落原生 DOM——挂在 `BaseItem` children snippet 内的 `<div class="prose ...">` 上（与 PR #516 在 SessionDetail 的做法一致）。命中区域 = 展开后的内容区；折叠态 prose 不渲染（`BaseItem` 仅 `isExpanded` 时渲染 children），自然无菜单——合理，内容不可见时复制无意义。`use:contextMenu` 的 `handleContextMenu` 已内置 `e.stopPropagation()`，阻止冒泡到 ExecutionTrace 外层 / 父 AI chunk 菜单，无需额外手写。

## Visual Contract

### Surface Decision

复制入口落在**既有右键菜单 surface**（`AppContextMenu` portal 浮层），不引入新 surface、不新增按钮——与 PR #516 完全同源。链回 `PRODUCT.md::Design Principle 2`（熟悉即效率，沿用右键菜单成熟模式）：用户在 SessionDetail 工具展开块已建立"右键这块就复制这块"预期，执行链内同类块行为一致是补齐而非新增预期。

### Visual Layer

无新增视觉元素。三块右键菜单完全复用 `frontend-context-menu` 既有视觉规格——引用 `DESIGN.md::The App Owns the Right-Click Rule`（自定义菜单统一走 `use:contextMenu`，禁止裸 `oncontextmenu`）与 Context menu 视觉契约（单 column 文字菜单 / `--color-surface` bg / `--color-border-emphasis` border / 8px radius / `0 4px 16px` shadow / `⌘C` muted 右对齐 shortcut hint）。

### State Coverage

| 状态 | 表现 |
|---|---|
| 右键块（无选区） | 弹「复制纯文本」+「复制为 Markdown」两项 |
| 右键块（有选区） | 首项插入「复制选中文本」(`⌘C`)，下接两项 |
| 复制成功 | 沿用菜单项 `feedback: { label: "已复制!" }` → 600ms 后关闭 |
| 复制失败 | 沿用 `copyToClipboard` 静默降级 |
| 折叠态 | prose 不渲染，无菜单（内容不可见，复制无意义） |
| 空文本块 | `buildMarkdownBlockItems("", ctx)` 返回 `[]` → `openMenu` 收到空 items 不弹菜单 |
| 键盘 ContextMenu 键 / Shift+F10 | 沿用 `use:contextMenu` action 的 `handleKeyDown` 定位 |
| `prefers-reduced-motion` | 菜单本身无 infinite 动画，无需额外处理 |

### DESIGN.md delta plan

无新增 token / 组件，无需 `/impeccable extract`。本 change 纯属在既有 surface 复用既有视觉契约。

## Risks / Trade-offs

- **可发现性依赖右键** → 缓解：与 PR #516 同源，本产品全面采用右键菜单（`DESIGN.md::The App Owns the Right-Click Rule`），用户为工程师群体，VS Code 式右键复制是肌肉记忆。
- **嵌套 ExecutionTrace 的事件冒泡** → 子块 action 的 `stopPropagation` 已阻止冒泡到外层 trace / 父 chunk（action 内置行为）；嵌套 subagent（depth>0）场景同样适用，风险低。e2e SHALL 验证右键 trace 块只弹该块菜单、不弹整条父消息菜单。
- **`projectId` 为空** → 仅当未来给 ExecutionTrace 块加文件类 item 时才相关；当前 `buildMarkdownBlockItems` 不消费 projectId，无影响。已在 D3 不变量记录，防回归。

## Open Questions

无。

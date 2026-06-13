## Why

会话详情流里，AI 消息内的工具展开块（slash/SKILL 指令、Output、Thinking、User message）右键时，事件冒泡到外层「整条 AI 消息」的菜单，其复制项只取 AI 文字正文、明确排除这些块的内容——用户右键想复制某个工具展开块，菜单根本复制不到该块。用户被迫先精确选中文本（WKWebView 选择体验差）才能复制，很不方便。右键这个块就该复制这个块，是用户直觉。

## What Changes

- 给 SessionDetail AI 组内的四类工具展开块（slash/SKILL、output、thinking、user_message）各挂自己的 `use:contextMenu`，右键它们时弹**该块**的复制菜单，而非冒泡到整条 AI 消息。
- 新增 menu-items factory（`buildMarkdownBlockItems`），菜单复用现有语言：「复制纯文本」（strip markdown）+「复制为 Markdown」（原始 markdown 源）；有选区时「复制选中文本」优先（沿用 `appendSelectionCopyIfAny`）。
- 复制源直接用块的 `item.text` / `item.slash.instructions`（本就是 markdown 源），不新增数据提取路径。
- **零新增按钮 / 零新增组件**：代码块（`.code-block-copy`）与 OutputBlock（`.copy-float`）现有 hover 复制按钮保留不动——它们是更细粒度的子元素复制，与右键整块互补。
- 不触及 `CopyButton` / `mode` prop（既有 spec 漂移另开 issue 跟踪，不混入本 change）。

## Capabilities

### New Capabilities

（无）

### Modified Capabilities

- `frontend-context-menu`: `menu-items 函数库` Requirement 新增 markdown 块 factory（`buildMarkdownBlockItems`），输出「复制纯文本 / 复制为 Markdown」并支持选区融合。
- `session-display`: `消息 chunk 右键菜单` Requirement 的覆盖范围从 chunk 容器粒度扩展到 AI 组内工具展开子块粒度——右键 slash/output/thinking/user_message 子块时弹该块自己的复制菜单，且阻止冒泡到父 AI chunk 菜单。

## Impact

- `ui/src/lib/contextMenu/menu-items.ts`：新增 `buildMarkdownBlockItems` factory。
- `ui/src/routes/SessionDetail.svelte`：slash/output/thinking/user_message 四处工具展开块挂 `use:contextMenu`。
- `ui/src/lib/contextMenu/markdown.ts`：复用 `stripMarkdownFormatting`（纯文本）+ 原文（Markdown），按需补 block 级 helper。
- 测试：`menu-items.test.ts` 新增 factory 单测；e2e 验证右键子块弹该块菜单且不冒泡到整条消息。
- 无 IPC / 后端 / Rust 改动；无新增依赖。

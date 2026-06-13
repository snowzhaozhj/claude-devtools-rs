## MODIFIED Requirements

### Requirement: menu-items 函数库

应用 SHALL 按 surface 拆分提供右键菜单 items factory 函数（用户消息 / 助手消息 / Bash 工具 / 文件类工具 / worktree chip / project card / 选区 / **markdown 工具展开块** 等），每个返回 items 数组。所有 factory 接受统一上下文（含 sessionId / projectId / settings / 5 个 IPC 调用闭包：copyToClipboard / openInEditor / openInTerminal / revealInDir / openUrl + selectionText 当前选区文本快照），让 item.action 自包含——factory 内**不**直接 import IPC 模块、**不**直接读 DOM（含 `getSelection` / `activeElement`），所有 IPC 走 ctx.dispatch 间接调用以便单测 mock，所有运行时浏览器状态 SHALL 通过 ctx 字段传入。Factory SHALL 是纯函数：给定输入 → 确定输出，不持有外部状态、不读 DOM。

调用方 SHALL 在 oncontextmenu 触发瞬间预先读选区文本后通过 ctx.selectionText 传入 factory，统一 selection 快照源避免 factory 内部读 DOM 的 jsdom / SSR / 测试稳定性问题。

`buildMarkdownBlockItems(text, ctx)` factory 服务于会话详情流内承载单段 markdown 源文本的工具展开块（slash 指令 / Output / Thinking / User message）。它 SHALL 输出「复制纯文本」（`stripMarkdownFormatting(text)`）与「复制为 Markdown」（原文 `text`）两项 copy item，并遵循选区融合与 separator 规则；`text` 为空时 SHALL 返回空数组（调用方据此不弹空菜单）。

#### Scenario: factory 返回纯数据

- **WHEN** 单测调用某 factory 传入 mock ctx 含 mock dispatch
- **THEN** 返回值 SHALL 是 items 数组
- **AND** items 内的 action 闭包仅引用 ctx.dispatch 与传入的数据，不调真 IPC
- **AND** mock dispatch 后调用 action SHALL 只触发 mock 函数，不发真 IPC
- **AND** 单测 SHALL **不**需要 jsdom 的 getSelection polyfill

#### Scenario: separator 自动插入按 kind 分组

- **WHEN** factory 返回的 items 含相邻 kind 不同的 item（典型 copy 后跟 navigate）
- **THEN** factory 内部 SHALL 在 kind 切换处插入 separator
- **AND** factory SHALL trim 首尾孤立 separator

#### Scenario: 有选区时融合"复制选中文本"

- **WHEN** 调用方在 oncontextmenu 触发瞬间读选区文本长度 > 0 后调 factory（含 selectionText）
- **THEN** factory SHALL 在首段（kind=copy）首项前动态插入"复制选中文本" item（含快捷键 hint）
- **AND** 该 item 的 action 调 ctx.dispatch.copyToClipboard

#### Scenario: 无选区时不插入选区项

- **WHEN** 调用方传入 selectionText 为空字符串
- **THEN** factory SHALL **不**插入"复制选中文本" item
- **AND** 返回 items 与 selectionText 为空字符串时调用结果一致（确定性纯函数）

#### Scenario: buildMarkdownBlockItems 输出纯文本与 Markdown 两项

- **WHEN** 以一段 markdown 源文本（如 `"# 标题\n正文 **粗体**"`）与含 mock dispatch 的 ctx（selectionText 为空）调用 `buildMarkdownBlockItems(text, ctx)`
- **THEN** SHALL 返回含「复制纯文本」与「复制为 Markdown」两个 copy item 的数组
- **AND** 「复制为 Markdown」item 的 action 调 `ctx.dispatch.copyToClipboard(text)` 写入原始 markdown 源
- **AND** 「复制纯文本」item 的 action 写入 `stripMarkdownFormatting(text)`（去掉 heading hash、加粗星号等标记）

#### Scenario: buildMarkdownBlockItems 在有选区时融合"复制选中文本"

- **WHEN** ctx.selectionText 非空时调用 `buildMarkdownBlockItems(text, ctx)`
- **THEN** SHALL 在首项前插入「复制选中文本」item（`shortcut: "⌘C"`）
- **AND** 其 action 调 `ctx.dispatch.copyToClipboard(ctx.selectionText)`

#### Scenario: buildMarkdownBlockItems 空文本返回空数组

- **WHEN** 以空字符串 text 调用 `buildMarkdownBlockItems("", ctx)`
- **THEN** SHALL 返回空数组
- **AND** 调用方据此 SHALL 不弹出菜单

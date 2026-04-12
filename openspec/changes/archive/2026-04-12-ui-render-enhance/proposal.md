## Why

SessionDetail 页面当前用纯文本 `<pre>` 渲染所有内容：AI 输出的 markdown 格式（标题、列表、代码块）原样显示为文本，tool input/output 的 JSON 没有语法高亮。可读性差，尤其对长回复和大 JSON payload。

## What Changes

- AI text 输出（`SemanticStep::Text`）改用 Markdown 渲染（标题、列表、行内代码、代码块）
- AI thinking 输出（`SemanticStep::Thinking`）也用 Markdown 渲染
- Tool input/output 的 JSON 用语法高亮渲染
- 引入前端依赖：markdown 解析库 + 代码高亮库

## Capabilities

### New Capabilities

（无）

### Modified Capabilities

（无）

## Impact

- **前端文件**：`ui/src/routes/SessionDetail.svelte` 修改渲染逻辑
- **依赖**：`ui/package.json` 新增 markdown 解析和代码高亮库
- **后端**：无改动

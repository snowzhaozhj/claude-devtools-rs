## Why

Rust 端口的工具查看器（Read / Write / Bash）和内容区域（AI 输出、用户消息、代码块、Thinking）目前没有一键复制按钮。原版 TS 实现中这些区域都有 copy 按钮（`CopyButton` 组件 + `CopyablePath` 组件），是用户复制代码片段、文件路径、命令输出的高频操作入口。缺失导致用户必须手动框选 + Ctrl/Cmd+C，体验割裂。

## What Changes

- 新增通用 `CopyButton` Svelte 组件（overlay 模式 + inline 模式）
- 新增 `CopyablePath` Svelte 组件（点击路径即复制完整绝对路径）
- `CodeBlockViewer` 添加 header 内 inline copy 按钮（复制文件完整内容）
- `MarkdownViewer` 添加 overlay copy 按钮（复制原始 markdown 内容）
- AI 文本输出块 / 用户消息气泡 / Thinking 块 / 代码块（`<pre><code>`）添加 overlay copy 按钮
- 所有 copy 操作统一视觉反馈：Copy 图标 → Check 图标，1.5-2s 后恢复

## Capabilities

### New Capabilities
- `copy-to-clipboard`: 覆盖一键复制按钮的组件设计、触发位置、视觉反馈、错误处理

### Modified Capabilities
- `tool-viewer-routing`: Write/Bash 工具查看器新增 copy 按钮

## Impact

- 新增 UI 组件：`ui/src/lib/components/common/CopyButton.svelte`、`ui/src/lib/components/common/CopyablePath.svelte`
- 修改组件：`CodeBlockViewer.svelte`、`MarkdownViewer.svelte`、AI/User chunk 渲染组件
- 依赖：`navigator.clipboard.writeText()` API（Tauri webview 环境 + HTTP mode 浏览器环境均支持）
- 无后端 / IPC 改动

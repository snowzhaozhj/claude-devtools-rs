## Why

用户需要了解一个 session 注入了哪些上下文（CLAUDE.md 文件、system prompts、mentioned files 等），以理解 Claude 的行为依据。原版有完整的 Context Panel 右侧边栏，Rust 版缺失此功能。

## What Changes

- **Context Panel 右侧边栏**：320px 宽度，展示当前 session 的上下文注入信息，按类别分组（System Messages、CLAUDE.md、Tool Outputs、Thinking）
- **Toggle 按钮**：在 top-bar 的 Context badge 上点击打开/关闭 Context Panel
- **数据提取**：从已有 chunks 中前端提取上下文信息（system chunks → CLAUDE.md / system prompts，AI chunks → tool executions / thinking text），不需要后端改动

## Capabilities

### New Capabilities

（无——纯前端 UI，数据从已有 chunks 提取）

### Modified Capabilities

（无）

## Impact

- `ui/src/components/ContextPanel.svelte`：新建，右侧边栏组件
- `ui/src/lib/contextExtractor.ts`：新建，从 chunks 提取上下文分类
- `ui/src/routes/SessionDetail.svelte`：集成 toggle + ContextPanel
- `ui/src/App.svelte`：布局调整支持右侧边栏
- 无后端改动、无 Tauri IPC 变更

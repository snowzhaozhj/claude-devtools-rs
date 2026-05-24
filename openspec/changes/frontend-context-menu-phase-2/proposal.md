## Why

`frontend-context-menu` Phase 1 已落地基础组件（`ContextMenu.svelte` + `useContextMenu.ts` action + `ContextMenuItem` 类型 + 5 surface 接入手段），但实际接入的 surface 仅 1 个（worktree-create 操作 demo），用户在用户消息 / AI 消息 / 工具块 / worktree chip / 项目卡 / 任意文本选区上右键暂时只能看到浏览器原生菜单，缺失"复制为 markdown"、"在编辑器打开"、"在终端打开"、"在浏览器搜索"等核心能力。issue #239 要求把 5 个 surface 全部接入并落地横向基础设施（IPC + Settings 字段 + 函数库）一次性解决。

## What Changes

- 新增**横向基础设施**：
  - `ui/src/lib/contextMenu/menu-items.ts` 函数库：`buildUserMessageItems` / `buildAssistantMessageItems` / `buildBashToolItems` / `buildFileToolItems` / `buildWorktreeChipItems` / `buildSelectionItems` 等 surface-specific factory，统一返回 `ContextMenuItem[]` 给 `useContextMenu` action 消费
  - `ui/src/lib/contextMenu/markdown.ts` Chunk → Markdown 反序 helper（前端反向，对应 Q5 决策）
  - `ui/src/lib/deeplink.ts` deeplink 设计（in-app hash route `#/session/<sessionId>/chunk/<chunkId>`，对应 Q1 决策）
  - 新 IPC `open_in_terminal`：跨平台（macOS osascript / Windows wt fallback cmd / Linux x-terminal-emulator）只 cd 不执行命令（对应 Q3 决策避免 RCE）
  - 新 IPC `open_in_editor`：按 Settings 走 VS Code/Cursor/Zed/Sublime，**支持 `code -g path:line` 跳行号**（对应 Q4 决策）
  - Settings 新字段：`external_editor` / `search_engine` / `terminal_app`（对应 Q2 + 平台终端选择）
- **5 surface 接入**：
  - 用户消息 chunk + AI 消息 chunk（`SessionDetail.svelte`）
  - Bash 工具块（`BashToolViewer.svelte`）
  - Read/Edit/Write 工具块（3 个 ToolViewer）
  - Worktree chip + 项目卡（`WorktreeChipCluster` + `Sidebar` 项目卡）
  - 任意文本选区菜单（window-level 接管 `contextmenu` 事件）
- **UI 整体优化**（设计师 teammate 在 visual contract 内决策）：
  - icon 加不加（Phase 1 D3 不加，Phase 2 文件 action / external action 重审）
  - shortcut hint 右侧灰色 ⌘C
  - separator 分组语义（复制类 / 导航类 / 外部应用类）
  - submenu 引入（"在终端打开" → iTerm/Terminal/Warp 二级；"在编辑器打开" → VS Code/Cursor/Zed/Sublime 二级）
  - active state 对比度提升、min-width / max-width（路径类 item ellipsis）、暗色模式验菜单层级
  - **不引入 danger item**（对应 Q7 决策，phase 2 暂无 destructive 动作）
- **跳过**（留 followup）：
  - 折叠 chunk 右键菜单项（已有 toggle 入口，对应 Q6 决策）

## Capabilities

### New Capabilities
<!-- 无新 capability -->

### Modified Capabilities
- `session-display`：加"消息 chunk 右键菜单" Requirement（用户消息 + AI 消息 surface）
- `tool-execution-linking`：加"工具块右键菜单" Requirement（Bash + Read/Edit/Write 三子类）
- `sidebar-navigation`：加"worktree chip / 项目卡右键菜单" Requirement
- `frontend-context-menu`：加"文本选区菜单" + "menu-items 函数库 + UI 视觉规范" Requirement
- `configuration-management`：加 `external_editor` / `search_engine` / `terminal_app` 字段 Requirement

## Impact

- **后端**：
  - 新增 `cdt-api` 内 `OpenInTerminalCommand` / `OpenInEditorCommand` IPC + Tauri capabilities allow-list；spawn 子进程逻辑封装到 `cdt-config` 或 `cdt-api/src/ipc/external_app.rs`
  - `cdt-config` Settings 加三字段（`external_editor` / `search_engine` / `terminal_app`），含 schema 校验 + 默认值
- **前端**：
  - 新建 `ui/src/lib/contextMenu/` 子目录承载 menu-items + markdown helper；6 个 surface 组件接入 `use:contextMenu`
  - 全局 window-level `contextmenu` 接管（任意选区菜单）
  - Settings 页面加 3 个新字段输入控件
- **Tauri 配置**：`src-tauri/capabilities/default.json` 加新 IPC 白名单
- **测试**：
  - Rust IPC contract test（cdt-api ipc_contract）覆盖新 IPC 字段
  - Vitest 单测覆盖 menu-items 函数库 + Chunk → MD 反序
  - Playwright e2e 覆盖 5 surface 右键菜单触发 + item 调度
  - QA teammate 跑 `e2e-http-verify` skill + Tauri dev 桌面端 smoke 真验证（避免 mockIPC 伪覆盖）
- **文档**：`PRODUCT.md` / `DESIGN.md` 按 visual contract delta plan 落地新 token / 组件
- **风险**：
  - 进程 spawn capabilities allow-list 写错可能 RCE
  - 文本选区菜单 window 级接管要避免与 Phase 1 surface-level `use:contextMenu` 冲突
  - menu-items 接口稳定性（5 surface 共享，phase A 定型后不能轻易破）
  - 伪覆盖（QA 主抓）

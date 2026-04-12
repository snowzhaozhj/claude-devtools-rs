## 1. Tauri 项目初始化

- [x] 1.1 使用 `npm create tauri-app@latest` 在项目根目录初始化 Tauri 2 + Svelte 5 + Vite + TypeScript 项目
- [x] 1.2 调整目录结构：前端放 `ui/`，Tauri 后端放 `src-tauri/`
- [x] 1.3 在 `src-tauri/Cargo.toml` 添加 data layer 依赖（`cdt-api`、`cdt-discover`、`cdt-config`、`cdt-ssh`）
- [x] 1.4 `cargo tauri dev` 确认空白窗口能启动

## 2. Tauri IPC Commands

- [x] 2.1 在 `src-tauri/src/lib.rs` 实现 `list_projects` command：构造 `LocalDataApi` → 调用 `list_projects()` → 返回 JSON
- [x] 2.2 实现 `list_sessions` command：接收 `project_id` → 返回 sessions
- [x] 2.3 实现 `get_session_detail` command：接收 `project_id` + `session_id` → 返回 chunks + metrics
- [x] 2.4 `cargo build` src-tauri 确认编译通过

## 3. Svelte 前端

- [x] 3.1 在 `ui/src/lib/api.ts` 封装 Tauri invoke 调用（`listProjects`、`listSessions`、`getSessionDetail`）
- [x] 3.2 实现 `ProjectList.svelte`：调用 `listProjects()` → 渲染项目卡片列表（名称 + 路径 + session 数）
- [x] 3.3 实现 `SessionList.svelte`：接收 `projectId` → 调用 `listSessions()` → 渲染 session 列表（ID + 时间 + 大小）
- [x] 3.4 在 `App.svelte` 实现简单路由：项目列表 → 点击 → 会话列表 → 返回
- [x] 3.5 基础暗色主题样式

## 4. 验证

- [x] 4.1 `cargo tauri dev` 启动，确认能看到本机 Claude Code 项目列表
- [x] 4.2 点击项目能看到会话列表
- [x] 4.3 `openspec validate tauri-svelte-scaffold --strict`

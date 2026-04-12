## Why

13 个 data layer capability 已完成，HTTP API 可用，但没有 UI。原版 Electron 应用在大会话 / 展开 tool call 时严重卡顿。需要一个性能好、UI 好看的桌面客户端。

选 Tauri 2 + Svelte 5：
- Tauri 用系统 WebView，内存占用低（Electron 的 1/5）
- Svelte 是编译时框架，无虚拟 DOM diff，大列表渲染快
- Rust 后端通过 Tauri IPC 直接调 `LocalDataApi`，零 HTTP 开销

本 change 只搭骨架：Tauri 项目初始化 + Svelte 前端 + Tauri IPC 绑定 + 基础页面（项目列表 + 会话列表）。

## What Changes

- 项目根目录新增 `ui/` 目录：Svelte 5 + Vite 前端
- 新增 `src-tauri/` 目录（或复用 `crates/cdt-cli`）：Tauri 后端
- Tauri IPC commands 绑定 `LocalDataApi`
- 基础两页面：项目列表 → 会话列表

## Capabilities

### New Capabilities
- `desktop-ui`：Tauri 桌面应用骨架（不在 openspec/specs 中，属 UI 层）

### Modified Capabilities
（无）

## Impact

- **目录结构**：新增 `ui/` + Tauri 配置
- **依赖**：新增 `tauri`、`tauri-build` Rust 依赖；`@tauri-apps/cli`、`svelte`、`vite` npm 依赖
- **构建**：`cargo tauri dev` 启动开发模式

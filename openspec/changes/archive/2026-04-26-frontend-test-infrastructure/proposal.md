## Why

当前仓库 13 个数据 capability 都有 Rust 单元/集成测覆盖，但 **UI 层（Tauri commands + Svelte 组件 + 跨组件交互）零自动化测试**——任何 UI 改动都依赖人手 `just dev` 起桌面窗口手动点。这有三个问题：

1. **回归慢**：改一个 Sidebar 样式要重启 Tauri、登录、点到对应路径才能看到，单次 30s+
2. **IPC 字段漂移无人守护**：最近 5 轮 perf 瘦身陆续加 `xxx_omitted` flag、`get_xxx_lazy` IPC，每次都靠手动 grep 验证 camelCase / 字段名一致；一旦后端字段名笔误，编译过 + 真后端跑炸 + 前端 fallback 链兜底——bug 只在 release 后用户报
3. **UI 行为 spec 的 6 个 scenario 没有可执行测试**：sidebar-navigation 的「骨架列表快速加载」、teammate-message 的渲染契约、session-detail 的「贴底滚动反闪烁三原则」等——目前只在 spec.md 里以散文形式存在

引入分层测试金字塔解决这三个痛点，同时为后续 UI 迭代建立长期回归网。

## What Changes

- **新增 `ui/src/lib/tauriMock.ts` + fixture 体系**：Tauri 官方 `mockIPC()` + `mockWindows()` dev-only 注入，覆盖全部 22 个 Tauri IPC command + 4 个 listen event；URL `?mock=1` 或浏览器无 Tauri runtime 时自动启用，真桌面窗口完全旁路
- **新增 `ui/tests/e2e/`**：Playwright + Chromium 跑 5-10 个 user story 用例（启动 / 选项目 / 看 session / Cmd+K 搜索 / 切主题 / Settings / Notifications / Sidebar 拖拽 / 右键菜单），baseline screenshot 仅 CI 跑
- **新增 `ui/src/**/*.test.ts`**：Vitest + jsdom 给纯函数和 store 加单测（theme / tabStore / sidebarStore / fileChangeStore / 路径编码 / icons），不测 dumb 组件
- **新增 `src-tauri/tests/ipc_contract.rs`**：每个 Tauri command 走「调 LocalDataApi → serialize → 断言 camelCase 字段名 + omitted flag 形状」的契约测，IPC 字段漂移立即 CI 红
- **新增 `.github/workflows/test.yml` 的 frontend-test job**：Vitest + Playwright + ipc_contract 三件一并跑
- **新增 `just test-ui` / `just test-e2e` recipe**：本地一键跑前端测试栈
- **更新 `ui/package.json`**：加 `vitest` / `@vitest/ui` / `jsdom` / `@playwright/test` / `@testing-library/svelte` 等 devDeps

## Capabilities

### New Capabilities

- `frontend-test-pyramid`：定义测试基础设施的契约——mockIPC 必须覆盖哪些 command、Playwright 必须覆盖哪些 user story、Rust IPC contract test 必须断言哪些字段形状、CI 必须在哪些 job 红线，以及四层之间的职责边界（谁守护什么、谁不守护什么）

### Modified Capabilities

无。本 change 只新增测试基础设施，不修改任何已有数据 / IPC / UI 行为契约。`ipc-data-api` 现有 22 个 Tauri command 形状不变，只是新增**断言其形状**的测试。

## Impact

- **新代码**：`ui/src/lib/tauriMock.ts`、`ui/src/lib/__fixtures__/*.ts`、`ui/tests/e2e/*.spec.ts`、`ui/src/**/*.test.ts`、`src-tauri/tests/ipc_contract.rs`、`playwright.config.ts`、`vitest.config.ts`
- **依赖新增**（前端 devDeps）：`@playwright/test`、`vitest`、`@vitest/ui`、`jsdom`、`@testing-library/svelte`、`@testing-library/jest-dom`
- **依赖新增**（Rust dev-deps，src-tauri）：`tempfile`、`tokio` with `test-util`、`serde_json`（已有）
- **CI 时长**：估计 +2-3 分钟（vitest <30s + playwright 5 用例 ~60s + ipc_contract <30s + npm install 缓存 ~30s）
- **修改文件**：`ui/package.json`（devDeps + scripts）、`src-tauri/Cargo.toml`（dev-dependencies）、`justfile`（新 recipe）、`.github/workflows/`（新 job 或扩现有 job）
- **不影响**：现有 13 capability 实现、Tauri command 形状、release bundle 体积、生产用户体验
- **风险**：mockIPC fixture 与真后端形状漂移 → 由 Rust IPC contract test 兜底捕获；Playwright baseline screenshot 在不同 OS / Chromium 版本可能 diff → 决定不 commit baseline，只 CI 跑（详见 design.md D5）

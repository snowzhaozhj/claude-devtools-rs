## Why

原版 TS Electron 版本（`../claude-devtools`）支持 "Enable server mode" 设置——开启后启动一个本地 HTTP 服务器，让用户从 Chrome 浏览器直接访问完整的应用（嵌入 iframe、跨设备 LAN 同访问、给浏览器扩展或外部脚本调用 API）。Rust 端口已在 `cdt-api` crate 实现完整的 HTTP 路由 + SSE 桥接基础设施（被 `cdt-cli` 复用），但 Tauri 桌面应用 (`src-tauri/src/lib.rs`) 始终未接线该服务器，前端 (`ui/`) 也未做浏览器 runtime 适配。本 change 把"已就绪的后端"接到桌面 app 与浏览器入口，对齐原版功能。

## What Changes

- **新增** Tauri 后端 server-mode 生命周期管理：3 个 IPC commands (`http_server_start` / `http_server_stop` / `http_server_status`)、启动时按持久化配置自动恢复、退出时优雅关闭。
- **新增** `cdt-api::http` 的 CORS 中间件接入：仅放行 `localhost` / `127.0.0.1` 来源（沿用原版安全模型），通过 `tower-http::cors` 接到现有 router（依赖已在 workspace 但未 layer）。
- **新增** `cdt-api::http` 静态文件 serve + SPA fallback：生产 mode serve `ui/dist` 前端 bundle，浏览器访问根路径返回 SPA 入口，未命中 API 路由的 GET 请求 fallback 到 `index.html`（dev mode 通过 Vite proxy 处理，生产 mode 通过 axum `tower-http::services::ServeDir`）。
- **新增** `cdt-api::http` 路由完整性：镜像目前仅在 IPC 暴露的 lazy / 辅助 command（`get_project_memory` / `read_memory_file` / `get_subagent_trace` / `get_image_asset` / `get_tool_output` / `add_trigger` / `remove_trigger` / `pin_session` / `unpin_session` / `hide_session` / `unhide_session` / `get_project_session_prefs`），让浏览器 runtime 能跑通完整 UI 流程（图片/工具输出展开、subagent trace、pin/hide、trigger CRUD）。
- **新增** Settings UI "Browser Access" section（仅 Tauri runtime 显示）：toggle "Enable server mode" + 当前状态绿点 + `http://localhost:{port}` URL + Copy 按钮。
- **新增** 浏览器 runtime transport 适配层：检测 `window.__TAURI_INTERNALS__` 不存在时切换到 HTTP/SSE client，让现有所有 IPC 调用透明走网络 transport；浏览器 runtime 隐藏桌面专属 UI（如 Browser Access toggle、自动更新条目）。
- **修改** 配置持久化语义：`HttpServerConfig.enabled` 字段在 server 启停时同步写入；启动 Tauri app 时按 `enabled=true` 自动启动 server。
- **修改** `http-data-api` 端口冲突策略：明确"启动时遵循 SHALL NOT switch silently，但 IPC 启动失败 SHALL 返回 specific error 让 UI 提示"——保持当前 spec 不变，新增错误展示约定。

## Capabilities

### New Capabilities

- `server-mode`：本机 HTTP 服务器在 Tauri 桌面应用内的生命周期管理（start/stop/status IPC、启动时自动恢复、退出时关闭、静态文件 serve、CORS 中间件、浏览器 runtime 检测）。包含 IPC 契约 + 后端 server lifecycle + 浏览器 transport 适配三部分。

### Modified Capabilities

- `http-data-api`：新增 CORS Requirement（默认仅放行 localhost / 127.0.0.1 origin；`tower-http::cors::CorsLayer` 在 `start_server` 内 layer 到 router）；新增静态文件 serve Requirement（生产 mode serve `ui/dist`）。
- `configuration-management`：扩展现有 "Validate configuration fields before persistence" Requirement——新增 `httpServer.enabled` 持久化与 lifecycle 协同语义（IPC start/stop 自动持久化、启动时按 enabled 自动恢复）。
- `settings-ui`：新增 "Browser Access section" Requirement（toggle + URL 显示 + Copy；仅 Tauri runtime 渲染）。
- `ipc-data-api`：新增 server-mode 控制相关 3 个 Tauri command 的字段契约（`http_server_start(port: number)` / `http_server_stop()` / `http_server_status()`）。

## Impact

- **新增 crate 依赖**：无（`tower-http` cors feature 已存在；`axum` / `serde` 等已在 `cdt-api`）。
- **新增 ui 依赖**：无（浏览器 runtime 检测用 `window.__TAURI_INTERNALS__` 判断；HTTP client 用浏览器原生 `fetch` + `EventSource`）。
- **改动文件预估**：
  - `crates/cdt-api/src/http/{mod.rs, routes.rs, cors.rs(新)}`：CORS layer + static serve
  - `src-tauri/src/lib.rs` + `src-tauri/src/server_mode.rs(新)`：lifecycle + 3 个 IPC commands + invoke_handler 注册
  - `src-tauri/tauri.conf.json` + `capabilities/default.json`：可能新增 capability 声明（待 design.md 确认）
  - `ui/src/lib/api.ts` + `ui/src/lib/transport.ts(新)` + `ui/src/lib/runtime.ts(新)`：transport 抽象层
  - `ui/src/routes/SettingsView.svelte`：新增 Browser Access section
  - `ui/index.html` 或 `ui/vite.config.ts`：dev mode 浏览器 runtime 探测兼容
- **行为契约影响**：用户首次开启 server mode 后，`HttpServerConfig.enabled=true` 持久化；下次启动 Tauri app 自动恢复 server。浏览器访问 `http://localhost:3456` 等价于桌面 app 完整功能。**通知列表 / 标已读 / 删除 / 清空等数据 API 走 HTTP（已有路由）**——浏览器 runtime 仅以下能力不可用：OS native toast notification 推送、Dock badge (`setBadgeCount`)、系统托盘交互、应用内自动更新（`check_for_update` / `tauri-plugin-updater`）、Rosetta 翻译检测（`is_running_under_rosetta`）。前端按 runtime 隐藏对应入口（不依赖错误处理兜底）。
- **安全模型**：server 仅监听 `127.0.0.1`，CORS 仅放行 localhost origin，无 token 鉴权（与原版 TS 一致）；任何 LAN / 远程访问需用户自行套反向代理（非目标）。
- **性能影响**：server idle 时 HTTP listener 开销极低（< 1MB RSS、~0% CPU）；浏览器并发访问时与桌面 app 共享同一 `LocalDataApi`，预算遵循 `.claude/rules/perf.md`。

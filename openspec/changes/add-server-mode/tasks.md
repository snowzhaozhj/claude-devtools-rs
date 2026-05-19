## 1. cdt-api：CORS 中间件 + 静态文件 serve + lazy endpoint 镜像

- [x] 1.1 在 `crates/cdt-api/src/http/mod.rs` 把 `start_server` 签名扩展为接受 `static_dir: Option<PathBuf>` 参数；旧调用方（`cdt-cli`）传 `None` 保持现状
- [x] 1.2 新建 `crates/cdt-api/src/http/cors.rs`：导出 `localhost_cors_layer() -> CorsLayer`，用 `AllowOrigin::predicate` 接 closure 判断 origin 是否匹配 `^https?://(localhost|127\.0\.0\.1)(:\d+)?$`；allow-methods 含 `GET/POST/PATCH/DELETE/OPTIONS`，allow-headers 含 `Content-Type`
- [x] 1.3 在 `start_server` 内 `.layer(localhost_cors_layer())` 接到 router；同源请求路径不变
- [x] 1.4 实现静态文件 serve：`static_dir = Some(p)` 且 `p.is_dir()` → router 末尾接 `tower_http::services::ServeDir` + SPA fallback（未命中 GET 请求返 `index.html`）；`p` 无效仅 `tracing::warn!`，`/api/*` 仍 serve
- [x] 1.5 单测：`tests/http_cors.rs` 覆盖 4 个 Scenario（localhost / 127.0.0.1 / `localhost.evil.com` / preflight OPTIONS）；用 axum `tower::ServiceExt::oneshot` 直接打 router，不起真 listener
- [x] 1.6 单测：`tests/http_static_serve.rs` 覆盖 6 个 Scenario（GET / 返回 index.html / 静态资产命中 / SPA fallback / `/api/*` 不被拦截 / `static_dir = None` / 无效路径仅警告）

### 1.A 镜像 lazy 与辅助 IPC commands 到 HTTP（Mirror lazy and auxiliary IPC commands Requirement）

- [x] 1.A.1 `GET /api/projects/{projectId}/memory` → `LocalDataApi::get_project_memory`
- [x] 1.A.2 `POST /api/projects/{projectId}/memory-files` body `{ file }` → `LocalDataApi::read_memory_file`
- [x] 1.A.3 `GET /api/sessions/{rootSessionId}/subagents/{subagentSessionId}/trace` → `LocalDataApi::get_subagent_trace`
- [x] 1.A.4 `GET /api/sessions/{rootSessionId}/subagents/{sessionId}/blocks/{blockId}/image` → `LocalDataApi::get_image_asset`（返回 base64 字符串）
- [x] 1.A.5 `GET /api/sessions/{rootSessionId}/subagents/{sessionId}/tools/{toolUseId}/output` → `LocalDataApi::get_tool_output`（保留 `outputBytes`/`outputOmitted` 语义）
- [x] 1.A.6 `POST /api/notifications/triggers` → `LocalDataApi::add_trigger`；`DELETE /api/notifications/triggers/{triggerId}` → `LocalDataApi::remove_trigger`（trait 提升：原 inherent method 移至 `impl DataApi for LocalDataApi`）
- [x] 1.A.7 `POST /api/projects/{projectId}/sessions/{sessionId}/pin` / `DELETE` → `LocalDataApi::pin_session` / `unpin_session`
- [x] 1.A.8 `POST /api/projects/{projectId}/sessions/{sessionId}/hide` / `DELETE` → `LocalDataApi::hide_session` / `unhide_session`
- [x] 1.A.9 `GET /api/projects/{projectId}/session-prefs` → `LocalDataApi::get_project_session_prefs`
- [x] 1.A.10 集成测试：`tests/http_lazy_endpoints.rs` 7 个测试覆盖 5 个 http-data-api Scenario（GET project memory mirrors IPC / POST add trigger persists caller-provided id / pin & unpin 互逆 / hide & unhide 互逆 / DELETE remove trigger 返更新 config）+ read_memory_file 404 + 路由 smoke（image asset / tool output / subagent trace 三个 lazy endpoint）
- [x] 1.A.11 在 `openspec/specs/http-data-api/spec.md` 当前的"路由清单 SHALL 至少包含"列表新增这些 endpoint（archive 时自动 sync）

## 2. cdt-api / cdt-config：HttpServerConfig 持久化语义补全

- [x] 2.1 检查 `cdt-config::HttpServerConfig` serde 字段名：`enabled` / `port` 序列化对应 `enabled` / `port`（已是默认 camelCase 单词，无需 rename）；缺字段时通过 `#[serde(default = "<fn>")]` 物化默认 `enabled=false / port=3456`，加单测验证
- [x] 2.2 暴露 `ConfigManager::set_http_server_enabled(bool)` 与 `set_http_server_port(u16)` 方法（若已有 `update_http_server` 复合方法可复用），保证 atomic 写盘；`enabled=false` SHALL **不**重置 `port` 字段
- [x] 2.3 单测：覆盖 4 个 configuration-management Scenario（启动持久化 enabled+port / stop 仅写 enabled=false / 启动失败不写持久化 / 老配置无 httpServer 字段时默认）

## 3. src-tauri：server-mode lifecycle + 3 个 IPC commands

- [x] 3.1 新建 `src-tauri/src/server_mode.rs` 模块，定义 `ServerHandle { task: JoinHandle<()>, port: u16 }` 与全局状态 `ServerState { handle: Mutex<Option<ServerHandle>>, api: Arc<LocalDataApi>, app_handle: AppHandle, static_dir: Option<PathBuf> }`
- [x] 3.2 实现核心 fn：`async fn start(state: &ServerState, port: u16) -> Result<(), String>`、`async fn stop(state: &ServerState) -> Result<(), String>`、`fn status(state: &ServerState) -> ServerStatus { running: bool, port: u16 }`；start 前先 abort 现有 handle 串行化；start 成功 SHALL 调 `ConfigManager::set_http_server_enabled(true) + set_http_server_port(port)`；start 失败 SHALL **不**写持久化；stop SHALL 调 `set_http_server_enabled(false)`
- [x] 3.3 在 `src-tauri/src/lib.rs` 加 3 个 `#[tauri::command]`：`http_server_start(port: u16)` / `http_server_stop()` / `http_server_status() -> ServerStatus`，转发到 `server_mode::*`
- [x] 3.4 把 3 个 command 名加进 `invoke_handler!`（`src-tauri/src/lib.rs`）
- [x] 3.5 在 `tauri::Builder::setup` 阶段：构建 `ServerState`、`manage(state)`；启动时若 `HttpServerConfig.enabled = true` 用 `tauri::async_runtime::spawn` 调 `server_mode::start(...)`；start 失败仅 `tracing::warn!` + `app_handle.emit("http-server-status", ...)` 不阻塞 setup
- [x] 3.6 解析 `static_dir`：dev mode（`cfg!(debug_assertions)`）传 `None`；release 通过 `app.path().resource_dir()` 拼前端 bundle 子路径，`cargo tauri build` 实测后填入正确子路径（已记 design.md Open Question）
- [x] 3.7 注册 `RunEvent::Exit` handler：调 `server_mode::stop(...)` 释放端口
- [x] 3.8 单测 / 集成测：在 `src-tauri/tests/` 或 `cdt-api/tests/` 新增针对 `server_mode` 的覆盖（mock `ConfigManager` + 真实 axum bind 到 `127.0.0.1:0` 自动选闲端口验证 start/stop/status 行为）；4 个 server-mode lifecycle Scenario 全覆盖

## 4. cdt-api/tests/ipc_contract：3 个新 command + httpServer config 同步

- [x] 4.1 在 `crates/cdt-api/tests/ipc_contract.rs::EXPECTED_TAURI_COMMANDS` 加入 `http_server_start` / `http_server_stop` / `http_server_status`
- [x] 4.2 加 3 个 contract test：`test_http_server_start_request_shape` / `test_http_server_status_response_shape` / `test_http_server_stop_response_shape`，断言 camelCase（`running` / `port` / `lastError`）与字段类型；`lastError` 字段 SHALL 在初始 + 启动失败 + 启动成功 三种状态下断言
- [x] 4.3 加 round-trip 测试：`update_config_http_server_round_trip`（默认 `{ enabled: false, port: 3456 }` / 改 `port` / 改回 / 非法 port 拒绝），覆盖 `ConfigManager::update_http_server` 的 match 分支
- [x] 4.4 跑 `cargo test -p cdt-api --test ipc_contract` 验证

## 5. ui：transport 抽象层 + Browser Access section

- [x] 5.1 新建 `ui/src/lib/runtime.ts`：导出 `isTauriRuntime(): boolean`（检测 `window.__TAURI_INTERNALS__`）；`getServerBaseUrl(): string`（浏览器 runtime 用 `window.location.origin`）
- [x] 5.2 新建 `ui/src/lib/transport.ts`：定义 `Transport` interface（`invoke<T>(cmd, args): Promise<T>` + `subscribeEvents(handler): Unsubscribe`）；实现 `TauriTransport`（包 `invoke` + `listen`）与 `BrowserTransport`（HTTP fetch + EventSource）；导出 `getTransport(): Transport`（按 runtime 选择）
- [x] 5.3 浏览器 transport 内桌面专属 IPC（`check_for_update` / `is_running_under_rosetta` / `setBadgeCount` / 通知交互等）SHALL throw `BrowserUnsupportedError`；列出豁免清单（grep `invoke('xxx')` 找出所有调用点，分类哪些是桌面专属）
- [x] 5.4 把 `ui/src/lib/api.ts` 的 `invoke` 调用全部改走 `getTransport().invoke(...)`；现有事件订阅（`listen('file-change', ...)`）改走 `getTransport().subscribeEvents(...)`
- [x] 5.5 资源 URL 重写：grep 找出所有 `tauri://localhost/...` 类 URL，替换为相对路径或运行时 `getServerBaseUrl() + '/api/...'`
- [x] 5.6 在 `ui/src/lib/tauriMock.ts::KNOWN_TAURI_COMMANDS` 加 `http_server_start` / `http_server_stop` / `http_server_status`
- [x] 5.7 在 `ui/src/lib/api.ts` 加 wrapper：`startHttpServer(port)` / `stopHttpServer()` / `getHttpServerStatus()`，对应 IPC command
- [x] 5.7a 在 `ui/src/lib/api.ts`（或类型文件）添加 `HttpServerConfig` / `HttpServerStatus` TypeScript interface（`enabled: boolean / port: number` 与 `running / port / lastError`）；扩展 `AppConfig` 含 `httpServer?: HttpServerConfig` 字段
- [x] 5.7b 同步 `ui/src/lib/__fixtures__/*.ts` 现有 config fixture，新增 `httpServer: { enabled: false, port: 3456 }` 默认值；`tauriMock.ts` 的 `update_config` 处理增加 `section === "httpServer"` 分支
- [x] 5.8 在 `ui/src/routes/SettingsView.svelte` 的 General section 加 "Browser Access" 子区块（仅 `isTauriRuntime()` 时渲染）：`SettingsToggle` + 状态行（绿点 + URL + Copy 按钮）+ 端口数字输入；按 5 个 settings-ui Scenario 实现交互
- [x] 5.9 i18n：标题 "Browser Access"、toggle "Enable server mode"、副文案 "Start an HTTP server to access the UI from a browser or embed in iframes"、"Running on" / "Failed to start: port may be in use" / "Copy URL" / "Copied"——用现有 i18n 体系（沿用与 Settings 一致的语言切换）
- [x] 5.10 vitest：mockIPC 覆盖 startHttpServer 成功 / 失败、getHttpServerStatus 返回 `{ running, port }`、Browser Access section 在 mockIPC = browser 时不渲染

## 6. 文档与运行时验证

- [x] 6.1 更新 `README.md` 加 Browser Access 用法段（如何开启 / URL / 安全模型）
- [x] 6.2 自动化 smoke：`SettingsView.browserAccess.test.svelte.ts` + `transport.test.ts` 覆盖 Settings toggle、浏览器 runtime 隐藏 Browser Access、HTTP transport 列项目 / lazy endpoint / SSE 映射；PR 描述记录未跑手动截图 smoke
- [x] 6.3 release static_dir 路径按 Tauri `frontendDist = "../ui/dist"` 规则使用 `resource_dir()` 根目录；跨平台 bundle 形态由 release CI 矩阵覆盖，PR 描述记录本地未跑 Windows/Linux 打包
- [x] 6.4 跑 `just preflight` (fmt / clippy / test / spec-validate) 全过

## N. 发布

- [x] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

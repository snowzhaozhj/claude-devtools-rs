## Context

claude-devtools-rs 的后端 HTTP API 基础设施在 `cdt-api` crate 内**已完整实现**——14 类路由、SSE 桥接、`LocalDataApi` 跨 transport 复用，`cdt-cli` 也已经能独立启动 server 验证基础设施可用。但 Tauri 桌面应用 (`src-tauri/src/lib.rs`) 的 `tauri::Builder` setup 里完全不引用 `cdt-api::http`，前端 (`ui/`) 也未做浏览器 runtime 适配——所有 IPC 调用直接 `import { invoke } from '@tauri-apps/api/core'`，浏览器打开 `http://localhost:3456/index.html` 时这层 import 会爆错。

原版 TS Electron 实现走的是同一思路：Electron main 进程在 Settings toggle 时启动 Fastify HTTP server（`src/main/services/infrastructure/HttpServer.ts`），renderer 通过 `window.electronAPI` 探测当前是 Electron renderer 还是浏览器，分别走 IPC 或 `HttpAPIClient` 两个 transport（`src/renderer/api/index.ts`）。两条路径调同一套 API contract。

本 change 把"已就绪的后端"接到 Tauri 桌面 app 里 + 给前端补浏览器 transport 适配，对齐原版功能。**默认仅监听 `127.0.0.1`、CORS 仅放行 localhost、无 token 鉴权**——与 TS 原版安全模型一致；远程访问 / TLS / 鉴权属于显式非目标。

参考事实链：
- 后端 server 入口：`crates/cdt-api/src/http/mod.rs:22-37`（已实现）
- 配置 schema：`cdt-config::HttpServerConfig { enabled: bool, port: u16 }`，默认 `enabled=false, port=3456`
- 端口校验：`cdt-config::validate_http_port`（已实现）
- CORS 依赖：`tower-http` 含 `cors` feature 但未 layer 到 router
- Tauri capability：Tauri 2 不管控后端 Rust `TcpListener::bind`（OS 级权限），无需新增 capability 声明
- IPC contract 权威清单：`crates/cdt-api/tests/ipc_contract.rs::EXPECTED_TAURI_COMMANDS`

## Goals / Non-Goals

**Goals:**

- Tauri 桌面 app 启动时按 `HttpServerConfig.enabled` 自动启停 HTTP server，进程退出时优雅关闭。
- Settings UI 暴露 "Browser Access" toggle + 当前 URL + Copy 按钮（仅 Tauri runtime 显示）。
- 浏览器访问 `http://localhost:{port}` 加载与桌面 app 等价的完整 UI（除桌面专属功能：通知、托盘、setBadgeCount、自动更新——这些 UI 入口在浏览器 runtime 隐藏）。
- 默认安全：仅 `127.0.0.1` listen + CORS 仅放行 `localhost` / `127.0.0.1` origin。
- 不破坏现有 spec：`http-data-api` 端口冲突策略保持 "SHALL NOT switch silently"。

**Non-Goals:**

- 远程 / LAN 访问（不会 bind `0.0.0.0`，TLS / 反向代理用户自行套外部组件）。
- Token / 密码鉴权（依赖 localhost-only 安全模型，与 TS 一致）。
- 浏览器 runtime 复刻桌面专属能力——通知 / 托盘 / Dock badge / 应用内自动更新在浏览器中**不**可用，UI 层显式隐藏入口避免误用。
- 兼容老版本 IPC 字段（本 change 不改任何现有 IPC contract，仅新增 3 个 command）。
- 多 server 实例（一个 Tauri 进程同时只开一个 server）。

## Decisions

### D1 capability 划分：新建 `server-mode` + 4 个现有 capability delta

**采用方案：混合**——核心 lifecycle 行为放新 capability `server-mode`，跨域影响放对应已有 capability 的 delta。

| 项 | 落入 capability | 理由 |
|---|---|---|
| Server 生命周期（启停 / 启动恢复 / 退出关闭 / 单实例） | **新建 `server-mode`** | 这是一个全新的横向能力，与现有 capability 都不完全对齐 |
| 3 个新 IPC commands 的字段契约（`http_server_start` / `_stop` / `_status`） | **新建 `server-mode`**（同 spec 内独立 Requirement） | IPC 契约是 server-mode 独有的对外接口；与 `ipc-data-api` 现存 Requirement（讨论 metadata 推送 / pin/hide）属性不同 |
| 浏览器 runtime 检测 + transport 适配（`window.__TAURI_INTERNALS__`） | **新建 `server-mode`** | 这是 server-mode 的"客户端契约"——浏览器入口只在 server-mode 上下文里有意义 |
| CORS 中间件 + 静态文件 serve | **修改 `http-data-api`** | 这两点是 HTTP server 自身的横切行为，扩展 `http-data-api` 现有"绑端口 / 路由"族 Requirement 更自然 |
| `httpServer.enabled` 持久化 + 与 lifecycle 协同 | **修改 `configuration-management`** | 配置字段持久化语义在 `configuration-management`；lifecycle 协同（启动恢复）跨域，spec 通过引用 [[server-mode]] 维持单一真相源 |
| Settings UI "Browser Access" section | **修改 `settings-ui`** | UI section 列表是 `settings-ui` 的现有职责（如 General / Display / Notifications） |

**选项对比**：

| 方案 | 优点 | 缺点 | 选择？ |
|---|---|---|---|
| A 单一新 capability `server-mode` 全包 | spec 集中易读 | 跨 4 个已有 capability 的修改藏在新 spec 里，reviewer 不易看到 cross-cutting impact | ❌ |
| B 全部用现有 capability delta（不新建） | 不增加 capability 数量 | server lifecycle 没有自然归属 capability，硬塞进 `http-data-api` 会让该 spec 既管 routing 又管进程生命周期，职责混杂 | ❌ |
| C 混合（新建 `server-mode` + 4 个 delta） | 每条 Requirement 都落在职责自然归属的 capability；reviewer 通过 delta 文件能看到 cross-cutting 全貌 | capability 数量 +1 | ✅ |

### D2 浏览器 transport 适配：本 change 必含

**采用方案：本 change 必含浏览器 runtime transport 适配**。

理由：
- 用户原话"可以直接从chrome浏览器中访问整个应用"明确要求完整 UI 走浏览器，而非仅 API 可访问。
- 后端 HTTP routes 已就绪，前端只需在 IPC wrapper 层 (`ui/src/lib/api.ts`) 加 transport 抽象——工作量集中在一处而非散布。
- 拆成"MVP1 = 后端 + UI toggle"+"MVP2 = 浏览器适配"会让 MVP1 没有可验证场景（toggle 开了但浏览器进去 UI 加载报错），违反"独立可验证"原则。

**实施分解**（在同一 PR 内，但可独立 commit 推进）：
1. C1：后端 server lifecycle + IPC commands + 启动恢复
2. C2：CORS layer + 静态文件 serve
3. C3：Settings UI section
4. C4：浏览器 runtime detection + transport 抽象层

### D3 CORS 策略：硬编码 localhost / 127.0.0.1

**采用方案：硬编码 `^https?://(localhost|127\.0\.0\.1)(:\d+)?$` 正则**，不暴露配置项。

| 选项 | 选择？ | 理由 |
|---|---|---|
| A 硬编码 localhost / 127.0.0.1 | ✅ | 与本 change 安全模型（仅 listen 127.0.0.1）一致；不引入额外配置面 |
| B 支持 env var `CDT_CORS_ORIGIN` 放宽（同 TS） | ❌ | TS 该机制是为 docker / standalone server 场景；本 Tauri 应用没有这两类部署，配置面冗余 |
| C 完全关闭 CORS（不 layer） | ❌ | 浏览器 origin 与 server origin 同域时不需要 CORS，但 iframe 嵌入 / 用户 hosts 改 / 本地 dev 跨端口都会触发，layer 一道严格 CORS 比裸跑更明确 |

未来若需 LAN 访问 → 开 follow-up change，不在本 change 范围。

### D4 端口冲突策略：保持现 spec "SHALL NOT switch silently"

**采用方案：bind 失败 → IPC 返回 specific error → Settings UI 提示用户改 port**，不自动 +1 试。

| 选项 | 选择？ | 理由 |
|---|---|---|
| A 保持现 spec | ✅ | 用户配的端口 = 实际端口（无歧义）；冲突时 UI 给明确反馈而非沉默切换 |
| B 自动 +1 试到 11 次（同 TS） | ❌ | 用户配 3456 实际跑在 3458，URL 显示需多走一次同步逻辑；TS 原版是历史遗留，没必要复刻 |
| C 找最近空闲端口 | ❌ | 体验更糟（端口不可预测） |

`http_server_start(port)` 若 bind 失败，IPC 返回 `Err("port {port} is in use, please choose another")`（中文文案前端处理）。

## Risks / Trade-offs

- **风险**：浏览器加载 UI bundle 时若 `vite.config.ts` / `tauri.conf.json` 的 `frontendDist` 路径与 axum static serve 路径不一致 → 浏览器进去 404 → **缓解**：实现时在 `start_server` 接受 `static_dir: Option<PathBuf>` 参数，Tauri runtime 传 `tauri::path::resource_dir() + "ui/dist"`，CLI 模式仍可不传（保持现状）。
- **风险**：浏览器 runtime 调用桌面专属 IPC（`check_for_update` / 通知相关）会 404 → **缓解**：transport 抽象层在浏览器分支显式 throw `BrowserUnsupportedError`，前端按 runtime 隐藏对应 UI 入口（不依赖错误处理兜底）。
- **风险**：server 启停 race（用户连点 toggle）→ 同一时刻两个 `http_server_start` → **缓解**：后端用 `tokio::sync::Mutex<Option<ServerHandle>>` 串行化 start/stop；start 前先 abort 现有 handle。
- **风险**：进程被强杀（`SIGKILL` / 用户 force quit）→ port 占用一段时间没释放 → **缓解**：Linux / macOS 内核 ~1min 释放；Windows 立即释放；不做特殊处理，下次启动若冲突照走 D4 错误提示。
- **风险**：CORS regex 写错放过非 localhost origin → **缓解**：写单测覆盖 `^https?://(localhost|127\.0\.0\.1)(:\d+)?$`，包含 `https://localhost.evil.com` 等绕过尝试；`tower-http::cors::CorsLayer` 用 `AllowOrigin::predicate` 接 closure 显式判断，避免正则注入。
- **风险**：浏览器跨设备访问失败困惑（用户期望"开 server 就能 LAN 用"）→ **缓解**：Settings UI hint 文案明确写 "Local browser only — does not expose to network"；非目标在 proposal 已写明。
- **取舍**：不做鉴权 → 任何在本机能跑代码的程序（包括恶意 subprocess）都能调 API → **可接受**：与 Electron 原版一致，桌面 app 自身就有同等本机权限；用户主动开关 toggle 表示信任本机环境。

## Migration Plan

无 schema migration（`HttpServerConfig` 字段已存在，本 change 仅启用其语义）。

老用户首次升级到含本 change 的版本：
1. `HttpServerConfig.enabled` 默认 `false` → server 不启动 → 行为与升级前 100% 一致。
2. 用户主动开 Settings → "Browser Access" → "Enable server mode" → server 启动 → `enabled=true` 持久化。
3. 重启 Tauri app → 自动恢复（按 D1 server-mode lifecycle Requirement）。

回滚：用户在 Settings 里关掉 toggle 即可（写 `enabled=false`）。代码层面 revert 本 change 即可，配置文件中残留的 `enabled=true` 在老版本里会被 `validate_http_port` / config schema 兼容地忽略（字段不存在不报错）。

## Open Questions

- **静态文件 dir 解析**：Tauri 在不同打包形态（`.app` bundle / Linux deb / Windows MSI / dev mode）下 `frontendDist` 实际落点可能不同。实现 task C2 时需要先 `cargo tauri info` + `cargo tauri build` 实测各平台 `resource_dir()` 返回值，决定是 `resource_dir().join("dist")` 还是 `resource_dir().join("ui/dist")`。dev mode (`cargo tauri dev`) 走 Vite proxy，server-mode 静态文件可暂留空（开 server 时浏览器访问会 404 ui，但 API 仍可用——dev 验证场景，不阻塞）；release 时再实测路径。
- **浏览器 runtime 下的 file 协议**：原版前端有少量 `tauri://localhost/...` URL（图片资产），浏览器 runtime 下需要重写到 `/api/...` HTTP 路径。实现时全文 grep 找出所有这类 URL 并加 transport 抽象。

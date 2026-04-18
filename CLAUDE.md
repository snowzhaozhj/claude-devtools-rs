# claude-devtools-rs

Rust port of [claude-devtools](../claude-devtools) — the Electron app that
visualizes Claude Code session execution.数据层 13 个 capability 已全部完成，
UI 层新增 6 个行为 spec（tab-management / session-display / sidebar-navigation / ui-search / settings-ui / notification-ui），
当前工作重心是 UI 层（Tauri 2 + Svelte 5 桌面应用）。

## Parent repo

The TypeScript source is at `/Users/zhaohejie/RustroverProjects/claude-devtools`.
It is the historical reference only；所有行为契约以 `openspec/specs/` 为准。
TS 侧的 7 个 impl-bug 已全部在 Rust port 中修复（详见 `openspec/followups.md`）。

## Workspace layout

```
claude-devtools-rs/
├── Cargo.toml                # workspace root
├── rust-toolchain.toml       # stable channel
├── crates/
│   ├── cdt-core/             # shared types + traits (no runtime deps)
│   ├── cdt-parse/            # session-parsing
│   ├── cdt-analyze/          # chunk-building + tool-linking + context-tracking + team-metadata
│   ├── cdt-discover/         # project-discovery + session-search
│   ├── cdt-watch/            # file-watching
│   ├── cdt-config/           # configuration-management + notification-triggers
│   ├── cdt-ssh/              # ssh-remote-context
│   ├── cdt-api/              # ipc-data-api + http-data-api (facade + HTTP server)
│   └── cdt-cli/              # binary entrypoint (bin = cdt)
├── ui/                       # Svelte 5 + Vite 前端
├── src-tauri/                # Tauri 2 Rust 后端 (excluded from workspace)
├── openspec/
│   ├── specs/                # 13 data specs + 6 UI specs (authoritative)
│   ├── followups.md          # TS impl-bugs to fix, not replicate
│   └── README.md             # workflow + capability map
└── .claude/rules/rust.md     # Rust coding conventions
```

## Capability → crate map

13/13 全部 done。Capability→crate 映射：`cdt-parse`(session-parsing)、`cdt-analyze`(chunk-building/tool-linking/context-tracking/team-metadata)、`cdt-discover`(project-discovery/session-search)、`cdt-watch`(file-watching)、`cdt-config`(configuration-management/notification-triggers)、`cdt-ssh`(ssh-remote-context)、`cdt-api`(ipc-data-api/http-data-api)。详见 `openspec/changes/archive/`。

## UI 层 (Tauri 2 + Svelte 5)

### 架构

- `ui/`：Svelte 5 + Vite 前端；`src-tauri/`：Tauri 2 Rust 后端（独立 Cargo.toml，excluded from workspace），通过 path deps 引用 `crates/`
- Tauri IPC commands 直接调用 `LocalDataApi`（不走 HTTP）。当前 16 个 commands（见 `src-tauri/src/lib.rs` 的 `invoke_handler!`）：session CRUD（list_projects / list_sessions / get_session_detail / search_sessions）、config（get_config / update_config）、通知（get_notifications / mark_notification_read / add_trigger / remove_trigger）、agents（read_agent_configs）、pin/hide（pin_session / unpin_session / hide_session / unhide_session / get_project_session_prefs）
- **Trigger CRUD 走独立方法**：`LocalDataApi::add_trigger()` / `remove_trigger()` 是非 trait 公开方法（独立 `impl` 块），不在 `DataApi` trait 中

### 布局与组件

- **布局**：Sidebar（可拖拽宽度 200~500px）+ TabBar + Main 三层。无 tab 时显示 Dashboard 项目概览。Tab 支持 session / settings / notifications 三种类型（settings/notifications 为单例 tab）
- **页面**：SessionDetail、SettingsView、NotificationsView、DashboardView（项目卡片网格，替代空状态）
- **组件**：BaseItem、StatusDot、OutputBlock、SearchBar（Cmd+F）、CommandPalette（Cmd+K 全局搜索）、ContextPanel（Category/Ranked 双视图 + DirectoryTree）、DiffViewer（LCS 行级 diff）、DirectoryTree（递归目录树）、SessionContextMenu（右键菜单 5 项）、SidebarHeader、TabBar（bell+齿轮+未读 badge 30s 轮询）、Tool Viewer（Read/Edit/Write/Bash/Default）
- **SVG 图标**：`ui/src/lib/icons.ts` 导出 lucide 风格 SVG path 常量，BaseItem 通过 `svgIcon` prop 渲染

### 状态与主题

- **状态管理**：`tabStore.svelte.ts` 管理 tabs/activeTabId/per-tab UI 状态/session 缓存/notificationUnreadCount。`sidebarStore.svelte.ts` 管理 sidebar 宽度、per-project Pin/Hide 状态（内存级）。Settings/Notifications 状态在各自组件内管理
- **主题切换**：`app.css` 中 `:root` 浅色 + `[data-theme="dark"]` 深色 + `@media prefers-color-scheme` 跟随系统。`lib/theme.ts` 的 `applyTheme()` 设置 `data-theme` 属性，App 启动时从 config 读取

### 数据流

- **Context Panel**：后端 `cdt-api` → `cdt-analyze::context::process_session_context_with_phases` → `ContextInjection[]`；CLAUDE.md 通过 `cdt-config::read_all_claude_md_files` 文件系统扫描
- **session 元数据**：`cdt-api/session_metadata.rs` 轻量扫描 JSONL 提取标题（前 200 行）+ 消息计数
- **通知实时更新**：后端 `mark_notification_read` 后通过 `app.emit("notification-update")` 推送；前端 `listen()` 监听立即刷新 badge；TabBar 额外每 30 秒轮询 unreadCount

### 陷阱

- `src-tauri/` 必须在 workspace `Cargo.toml` 的 `exclude` 列表里；`beforeDevCommand` 从 `src-tauri/` 目录执行，路径用 `../ui`
- 浏览器直接访问 `localhost:5173` 会报 `invoke` undefined——必须通过 `cargo tauri dev` 的窗口测试
- Vite HMR 只更新前端；后端改动需 `pkill -f claude-devtools-tauri && cargo tauri dev` 重启
- `npm run check --prefix ui` 必须从项目根目录执行，从 `src-tauri/` 目录跑会找不到 `package.json`
- Tauri `setup` 里启动后台 task 用 `tauri::async_runtime::spawn`，不要裸 `tokio::spawn`；订阅后端 `broadcast::Receiver` 转 `emit(...)` 是典型模式（见 `src-tauri/src/lib.rs` 的 FileWatcher + notifier bridge）
- **桌面通知 / 系统托盘**：`tauri-plugin-notification`（`src-tauri/Cargo.toml` + `capabilities/default.json` 加 `notification:default`）+ `TrayIconBuilder::with_id("main-tray")`（`setup` 里构建，icon 取 `app.default_window_icon()`）。后端从 Rust 发通知用 `app_handle.notification().builder().title(..).body(..).sound("default").show()`；前端 Dock badge 用 `getCurrentWindow().setBadgeCount()`（macOS 独占）。参见 commit `f546b88`。

## Common commands

所有任务通过 `just` 跑（见 `justfile`）。首次执行 `brew install just` 安装（若未装）。

```bash
just                     # 列出所有 recipes
just build               # workspace build
just build-tauri         # src-tauri build（独立 manifest）
just test                # Rust + 前端全测（cdt-watch 自动单线程补跑避 FSEvents flake）
just test-crate cdt-analyze   # 单 crate 测试
just lint                # workspace + src-tauri clippy 严格模式
just fmt                 # cargo fmt --all
just check-ui            # svelte-check + tsc
just dev                 # cargo tauri dev
just spec-validate       # openspec validate --all --strict
just preflight           # fmt + lint + test + spec-validate（提交前一把梭）
just bootstrap           # npm install --prefix ui（首次）
```

直接跑 `cargo xxx` 仍可用，但注意：`cd src-tauri/` 后 Bash tool 的 cwd 会持久化，后续 `cargo test --workspace` 会误入 tauri 子 manifest——优先用 `just` 或 `--manifest-path`。

## macOS 开发陷阱

- `TempDir` 返回 `/var/...` 但 `notify`/FSEvents 返回 `/private/var/...`（symlink canonicalization）。涉及路径比较时必须 `canonicalize()`。
- `notify-debouncer-mini` 的 timer 不受 `tokio::time::pause()` 控制，测试不确定。优先用 `notify` 裸接 + 自实现 tokio debounce。
- `cdt-watch` 的 `tests/file_watching.rs` 在 macOS 并发跑 flaky（FSEvents 时序依赖）；`just test` 已经把它单拎出来用 `--test-threads=1` 跑。直接 `cargo test --workspace` 偶尔会挂，优先用 `just test`。

## Conventions

- **Error types**: library crates use `thiserror` enums; the `cdt-cli` binary uses `anyhow::Result`.
- **Async runtime**: `tokio` is added only to leaf crates that need I/O; `cdt-core` stays sync.
- **Logging**: `tracing`; subscriber initialized once in `cdt-cli`.
- **No `unwrap()` in library code** — use `?` or typed errors.
- **No cross-crate imports of internal modules** — go through each crate's public API.
- **Serde camelCase**：所有面向前端（Tauri IPC）的 struct 必须 `#[serde(rename_all = "camelCase")]`；enum 用 `rename_all_fields = "camelCase"` 给字段、`rename_all = "snake_case"` 给 tag 值。例外：`TokenUsage` 保持 snake_case（与 Anthropic API 原始格式一致）。
- **`ContextInjection` serde 格式**：`#[serde(tag = "category", rename_all = "kebab-case")]` 是 internally-tagged，JSON 为 `{ "category": "claude-md", "id": "...", ... }`（不是 `{ "ClaudeMd": {...} }`）。前端按 `inj.category` 字段 switch 匹配。
- **`is_meta` 消息过滤**：JSONL 中 `isMeta: true` 的 user 消息（skill prompt、system-reminder 注入）在 `build_chunks` 中跳过，不产出 `UserChunk`；但其中的 `tool_result` 仍合并到 assistant buffer。
- **Svelte 5 `$effect` 依赖陷阱**：`$effect` 中读取的所有响应式变量自动成为依赖。若需要在 effect 中读取但不触发重跑的变量，用 `untrack(() => variable)` 包裹。典型场景：session 切换 effect 中清理搜索状态。
- **Svelte 5 `<button>` 嵌套禁止**：`<button>` 内不能嵌套 `<button>`，浏览器会修复 DOM 结构导致 Svelte 假设失效。用 `<span role="button" tabindex="-1">` 替代。
- **Settings 乐观更新模式**：config 修改不能依赖 `updateConfig` 返回值刷新 UI，应先乐观更新本地 `$state`，异步调 API，失败时回滚（重新 `getConfig`）。
- **Svelte 5 `@const` 位置限制**：`{@const}` 只能是 `{#if}`/`{:else}`/`{#each}`/`{#snippet}`/`<Component>` 的直接子级，不能放在 `<div>` 等 HTML 元素内。需要在块开头集中声明。
- **前端渲染依赖**：`marked`（markdown→HTML）+ `highlight.js`（语法高亮，按需加载语言）+ `dompurify`（XSS 防护）+ `mermaid`（图表渲染，动态 import）。highlight.js 不引入预制主题 CSS，用 `app.css` 中自定义 Soft Charcoal token 颜色。
- **原版 UI 参考**：前端文本清洗逻辑移植自 `../claude-devtools/src/shared/utils/contentSanitizer.ts`（`sanitizeDisplayContent`）。扩展 UI 功能时优先查原版 `src/renderer/` 和 `src/shared/` 对应实现，直接移植而非自己造轮子。
- **Tauri IPC 透传**：`src-tauri/src/lib.rs` 的 commands 返回 `serde_json::Value`，`cdt-api` 类型扩展字段（如 `SessionSummary` 加 `title`）自动透传，不需要改 Tauri 层。
- **`LocalDataApi` 构造器扩展**：需要注入新基础设施（FileWatcher、SSH pool 等）时新增 `new_with_<xxx>()` 构造器，**不改** `new()` 签名——旧构造器被 `crates/cdt-api/tests/*.rs` 依赖，改签名会批量破坏集成测试。
- **后台服务的本机路径参数化**：涉及 `~/.claude/projects/` 的后台服务（notifier、未来的 history scanner）不要在函数内直接 `path_decoder::get_projects_base_path()`，显式从构造器传 `projects_dir: PathBuf`，否则集成测试会命中真实本机路径。
- **clippy pedantic**：workspace 开启 pedantic，PostToolUse hook 会在每次 `.rs` 编辑后自动跑 clippy 报错。最常踩的：`doc_markdown`（注释里标识符要反引号）、`cast_possible_wrap`（`u64 as i64` → `i64::try_from`）、`uninlined_format_args`（`format!("{}", x)` → `format!("{x}")`）。其余照 clippy 输出修即可。
- **insta 快照接受**：没装 `cargo-insta` 就用 `INSTA_UPDATE=always cargo test -p <crate>`；提交生成的 `tests/snapshots/*.snap`。
- **同步解析入口**：`cdt-analyze` 的集成测试不引入 tokio——用 `cdt_parse::parse_entry_at(line, n)` 逐行解析 fixture，再跑 `dedupe_by_request_id`。
- **自动化**：
  - Hooks（`.claude/hooks/`）：`.rs` 编辑后自动跑所属 crate 的 `cargo clippy -- -D warnings`；`.svelte` 编辑后自动跑 `svelte-check`；`git commit` 前自动跑 `openspec validate --strict`。
  - **spec 变更约定**：**修改**已有 spec 必须走 `openspec/changes/<name>/specs/` 的 delta，archive 时 sync 回主 spec。**新增** spec（如 UI 行为 spec）可直接写入 `openspec/specs/<name>/spec.md`。
  - Subagent：`spec-fidelity-reviewer` 按 capability 审计 scenario→test 覆盖。
  - Skill：`/ts-parity-check <capability>` 对比 TS 源与 Rust 端口 + followups。
  - MCP：`.mcp.json` 注册 GitHub MCP，需要 `GITHUB_PERSONAL_ACCESS_TOKEN` 环境变量。
- **opsx:apply 推进节拍**：详见 `.claude/rules/opsx-apply-cadence.md`。核心：Edit → clippy → fmt → test → npm check → validate → 勾 checkbox → 文本总结，不得中途停手。
- Detailed rules: `.claude/rules/rust.md`.

## UI 已知遗留问题

剩余一条（详细实现路径见 `openspec/followups.md` "实时会话刷新" 段第二条）：

1. **Session "in progress" + 中断检测未实现**：原版 `checkMessagesOngoing` 算法 + `OngoingIndicator`（sidebar 绿点 / 会话底部 "Session is in progress..." 横幅）未 port。附带 impl-bug：`crates/cdt-parse/src/noise.rs:13` 把 `[Request interrupted by user` 当 hard noise 过滤，与原版"保留为 `interruption` semantic step"相反，需要先从 `HardNoise` 拎出来成独立 category。依赖**实时 `file-change` 桥**（已在 `2026-04-18-realtime-session-refresh` 修复）才能"看到绿点变白"——前置条件已就绪。

建议顺序：上面 #1 → Execution Trace / 多 Pane / 虚拟滚动。桌面通知/系统托盘见 commit `f546b88`；实时刷新见 change `2026-04-18-realtime-session-refresh`。

## What to do first in a fresh session

1. Run `cargo build --workspace` 确认 data layer 可编译；`cargo test --workspace` 跑一遍回归。
2. 13 个 data layer capability 已全部完成。当前工作重心是 UI 层（Tauri + Svelte）。
3. `cargo tauri dev` 启动桌面应用验证当前状态。
4. UI 功能迭代：**大功能**（多步骤、跨模块、需要设计决策）走 openspec（propose → apply → archive）；**小改动**（主题切换、Trigger CRUD、样式修复等单点改动）直接写 + commit，不走 openspec。判断标准：是否需要 design.md 记录架构决策。

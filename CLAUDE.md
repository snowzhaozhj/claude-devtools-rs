# claude-devtools-rs

[claude-devtools](../claude-devtools)（Electron 原版）的 Rust 端口，
Tauri 2 + Svelte 5 桌面应用。13 个数据层 capability + 6 个 UI 行为 spec 均已实现；
首个 release `v0.1.0` 已发，后续迭代不再直接在 `main` 上开发。
用户视角的安装 / 开发 / 发布流程见 `README.md`，本文件是 contributor 专用的约定和陷阱手册。

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
- **session 元数据**：`list_sessions` IPC 返回**骨架** SessionSummary（title=null/messageCount=0/isOngoing=false），后台 JoinSet+Semaphore(8) 并发扫描，每条通过 `subscribe_session_metadata()` broadcast → Tauri emit `session-metadata-update` → Sidebar 按 sessionId in-place patch。HTTP 路径走 `list_sessions_sync` 保留同步完整返回。
- **通知实时更新**：后端 `mark_notification_read` 后通过 `app.emit("notification-update")` 推送；前端 `listen()` 监听立即刷新 badge；TabBar 额外每 30 秒轮询 unreadCount

### 陷阱

- `src-tauri/` 必须在 workspace `Cargo.toml` 的 `exclude` 列表里；`beforeDevCommand` 从 `src-tauri/` 目录执行，路径用 `../ui`
- 浏览器直接访问 `localhost:5173` 会报 `invoke` undefined——必须通过 `cargo tauri dev` 的窗口测试
- Vite HMR 只更新前端；后端改动需 `pkill -f claude-devtools-tauri && cargo tauri dev` 重启
- `npm run check --prefix ui` 必须从项目根目录执行，从 `src-tauri/` 目录跑会找不到 `package.json`
- Tauri `setup` 里启动后台 task 用 `tauri::async_runtime::spawn`，不要裸 `tokio::spawn`；订阅后端 `broadcast::Receiver` 转 `emit(...)` 是典型模式（见 `src-tauri/src/lib.rs` 的 FileWatcher + notifier bridge）
- **桌面通知 / 系统托盘**：`tauri-plugin-notification`（`src-tauri/Cargo.toml` + `capabilities/default.json` 加 `notification:default`）+ `TrayIconBuilder::with_id("main-tray")`（`setup` 里构建，icon 取 `app.default_window_icon()`）。后端从 Rust 发通知用 `app_handle.notification().builder().title(..).body(..).sound("default").show()`；前端 Dock badge 用 `getCurrentWindow().setBadgeCount()`（macOS 独占）。参见 commit `f546b88`。
- **`devtools` feature 不要进 release**：`src-tauri/Cargo.toml` 的 `tauri = { features = [...] }` **不要**写 `"devtools"`——加了之后 release bundle 会带 web inspector。tauri 2 在 debug 构建自动启 inspector 不依赖 feature，`cargo tauri dev` 照旧能开。调用 `window.open_devtools()` 必须用 `#[cfg(debug_assertions)]` **编译时** gate（不是 `if cfg!(debug_assertions)` 运行时宏）——后者 release 构建里会因 `open_devtools` API 不存在（它本身就有 `#[cfg(any(debug_assertions, feature = "devtools"))]`）而编译失败。

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
- `cdt-watch` 的 `tests/file_watching.rs` 在 macOS 跑 flaky（FSEvents 时序依赖）；`just test` 单线程补跑也可能 5/6 timeout（**不只是** `burst_of_writes_debounced`）。判断是否真回归：`cargo test -p cdt-watch <test_name>` 单 case 跑——单跑能通过即视为环境 flake，可继续 archive；改 watcher 行为时才需要纠结全套通过。

## Conventions

- **Error types**: library crates use `thiserror` enums; the `cdt-cli` binary uses `anyhow::Result`.
- **Async runtime**: `tokio` is added only to leaf crates that need I/O; `cdt-core` stays sync.
- **Logging**: `tracing`; subscriber initialized once in `cdt-cli`.
- **No `unwrap()` in library code** — use `?` or typed errors.
- **No cross-crate imports of internal modules** — go through each crate's public API.
- **Serde camelCase**：所有面向前端（Tauri IPC）的 struct 必须 `#[serde(rename_all = "camelCase")]`；enum 用 `rename_all_fields = "camelCase"` 给字段、`rename_all = "snake_case"` 给 tag 值。例外：`TokenUsage` 保持 snake_case（与 Anthropic API 原始格式一致）。
- **`ContextInjection` serde 格式**：`#[serde(tag = "category", rename_all = "kebab-case")]` 是 internally-tagged，JSON 为 `{ "category": "claude-md", "id": "...", ... }`（不是 `{ "ClaudeMd": {...} }`）。前端按 `inj.category` 字段 switch 匹配。
- **chunk-building 语义契约**：`is_meta` / slash / interruption 三类消息的完整行为契约在 `openspec/specs/chunk-building/spec.md`（Scenario 级覆盖）。port 专属踩坑：
  - **`is_meta` 过滤**：跳过产 `UserChunk`，但 `tool_result` 仍合并到 assistant buffer（spec 待补）。
  - **Slash 双产出 + 紧邻约束**：slash user 消息既要产 `UserChunk`（UI 气泡）又要挂到下一个 `AIChunk.slash_commands`；`instructions` 来自 `is_meta=true + parent_uuid=slash.uuid` 的 follow-up；普通 user 消息产 `UserChunk` 前必须 `pending_slashes.clear()`。TS 原版通过"只看紧邻前 UserGroup"实现，勿回退。
  - **Interruption 分类**：`[Request interrupted by user` 起首的 user 消息是 `MessageCategory::Interruption`（**非** hard noise），产 `SemanticStep::Interruption` 追加到前一 AIChunk。TS 侧曾当 hard noise 过滤，port 已反向修复，勿回退。
- **Svelte 5 `$effect` 依赖陷阱**：`$effect` 中读取的所有响应式变量自动成为依赖。若需要在 effect 中读取但不触发重跑的变量，用 `untrack(() => variable)` 包裹。典型场景：session 切换 effect 中清理搜索状态。
- **Svelte 5 `<button>` 嵌套禁止**：`<button>` 内不能嵌套 `<button>`，浏览器会修复 DOM 结构导致 Svelte 假设失效。用 `<span role="button" tabindex="-1">` 替代。
- **Settings 乐观更新模式**：config 修改不能依赖 `updateConfig` 返回值刷新 UI，应先乐观更新本地 `$state`，异步调 API，失败时回滚（重新 `getConfig`）。
- **Svelte 5 `@const` 位置限制**：`{@const}` 只能是 `{#if}`/`{:else}`/`{#each}`/`{#snippet}`/`<Component>` 的直接子级，不能放在 `<div>` 等 HTML 元素内。需要在块开头集中声明。
- **前端渲染依赖**：`marked`（markdown→HTML）+ `highlight.js`（语法高亮，按需加载语言）+ `dompurify`（XSS 防护）+ `mermaid`（图表渲染，动态 import）。highlight.js 不引入预制主题 CSS，用 `app.css` 中自定义 Soft Charcoal token 颜色。
- **原版 UI 参考**：前端文本清洗逻辑移植自 `../claude-devtools/src/shared/utils/contentSanitizer.ts`（`sanitizeDisplayContent`）。扩展 UI 功能时优先查原版 `src/renderer/` 和 `src/shared/` 对应实现，直接移植而非自己造轮子。
- **port 状态判定要顺着 main 进程查兜底**：原版纯算法 ts（`sessionStateDetection.ts` / `tokenFormatting.ts` 等）只定结构性判定；最终落到 UI 的字段（`isOngoing` / `messageCount` / `gitBranch`...）常在 `src/main/services/discovery/ProjectScanner.ts` 等**调用方**叠加 mtime / count / threshold 兜底。port 时只看算法文件会漏，必须 grep 调用方"该字段被赋值的地方"——本仓 isOngoing 缺 5min `STALE_SESSION_THRESHOLD_MS` 的根因（见 change `session-ongoing-stale-check`）。
- **Svelte 列表/详情自动刷新反闪烁三原则**：(1) `{#each}` 必须带稳定 key（AIChunk 用 `responses[0].uuid`，UserChunk/System/Compact 用 `uuid`，SessionSummary 用 `sessionId`），否则 file-change 刷新时整段 DOM 重建 + mermaid/highlight.js 重跑；(2) `loadX(..., silent = false)` 加 silent 参数，file-change handler 传 `silent=true` 保留旧列表直到新数据到达，**不要**经过"加载中..."中间态；(3) ongoing/interruption 等状态指示器嵌入已有 slot（如 `<OngoingBanner>` 替代最后 AIChunk 的 `lastOutput`，对齐原版 `LastOutputDisplay.tsx::isLastGroup && isSessionOngoing` 语义），**不要**作为独立节点追加到流尾部——否则显隐切换时 scrollHeight 跳变引发贴底滚动视觉抖动。
- **Tauri IPC 透传**：`src-tauri/src/lib.rs` 的 commands 返回 `serde_json::Value`，`cdt-api` 类型扩展字段（如 `SessionSummary` 加 `title`）自动透传，不需要改 Tauri 层。
- **file-change 节流链**：后端 `cdt-watch::FileWatcher` debounce 100 ms；前端 `ui/src/lib/fileChangeStore::dedupeRefresh` 仅合并 in-flight 期间的并发调用，**不做时间节流**。活跃 Claude 会话高频写 JSONL 时会触发每几百 ms 一次 re-render——如需降频，给 `dedupeRefresh` 加 250 ms cooldown 或用 trailing debounce 包 handler。
- **`LocalDataApi` 构造器扩展**：需要注入新基础设施（FileWatcher、SSH pool 等）时新增 `new_with_<xxx>()` 构造器，**不改** `new()` 签名——旧构造器被 `crates/cdt-api/tests/*.rs` 依赖，改签名会批量破坏集成测试。
- **IPC vs HTTP 行为分叉**：trait 加默认方法 fallback 到通用版本，`LocalDataApi` 自己 override 真版本——HTTP 跑 LocalDataApi 拿完整结果，其他实现安全降级。例：`DataApi::list_sessions_sync` 默认调 `list_sessions`（骨架），LocalDataApi override 为同步全扫。
- **后台任务 per-key 取消**：触发新一轮后台扫描前需 abort 同 key 的旧任务时，用 `Arc<std::sync::Mutex<HashMap<K, AbortHandle>>>`：spawn 后 `insert(key, handle.abort_handle())`；新调用进入先 `remove(key).map(|h| h.abort())`；任务尾部从 map 自清理。例见 `LocalDataApi::list_sessions` 的 `active_scans`。
- **Svelte 5 `{@attach}` 挂副作用**：DOM 元素需要副作用 + cleanup（ResizeObserver、IntersectionObserver、scroll listener 等）时用 `{@attach (el) => { ...setup; return () => cleanup; }}`，比 `bind:this + onMount + onDestroy` 三段式更内聚。例见 `Sidebar.svelte::session-list` 容器挂 ResizeObserver。
- **后台服务的本机路径参数化**：涉及 `~/.claude/projects/` 的后台服务（notifier、未来的 history scanner）不要在函数内直接 `path_decoder::get_projects_base_path()`，显式从构造器传 `projects_dir: PathBuf`，否则集成测试会命中真实本机路径。
- **`src-tauri` 独立 manifest 的 cargo cache 坑**：改 `cdt-*` crate 的 `pub use` 后，workspace `cargo check` 能过，但 `cargo clippy --manifest-path src-tauri/Cargo.toml` 会报 "no xxx in the root" —— src-tauri 的 `target/` 独立缓存未感知 workspace API 变化。修法：`cargo clean -p <crate-name> --manifest-path src-tauri/Cargo.toml` 后重跑。`just lint` 跑全（两个 manifest）时首次也会触发这坑。
- **Windows NTFS 目录名禁用字符**：`< > : " / \ | ? *` 不能做文件/目录名。测试 fixture 里用 `encode_path(r"C:\Users\...")` 会产 `-C:-Users-...` 含 `:`，Windows CI 上 `create_dir_all` 报 error 267 NotADirectory。凡需"真在磁盘上建 encoded project 目录"的集成测试（见 `crates/cdt-api/tests/agent_configs.rs`），用纯字母/数字/`-` 的 hardcoded 名（如 `-ws-my-proj`），cwd 真实磁盘路径由 JSONL `cwd` 字段提供，scanner 依赖字段不依赖 encoded 名与磁盘路径的对应。
- **`tokio::time::pause` 测试的 send-advance 顺序**：`#[tokio::test(start_paused = true)]` + `tokio::time::advance` 精确控制虚拟时钟时，`send → advance` 直觉顺序会失败——loop task 尚未被 poll，pending 仍空，`advance` 不触发 flush。正确模板：`tx.send(...) → yield_now (loop 收 event 写 pending) → advance(duration) → yield_now (sleep_until wake + flush)`。例：`cdt-watch::watcher::tests` 的 5 个 debounce 单元测。需要 `tokio` dev-dep 带 `test-util` feature。
- **跨平台路径工具统一入口**（Windows 兼容硬约束，见 change `windows-platform-support`）：
  - **home 解析**：凡需 `~/.claude/` 或用户 home 的代码都调 `cdt_discover::home_dir()`，**不要**直接用 `dirs::home_dir()`。前者四级 fallback `HOME → USERPROFILE → HOMEDRIVE+HOMEPATH → dirs::home_dir()`，对齐 TS `pathDecoder.ts::getHomeDir`；后者在 Windows 上若 `USERPROFILE` 未设但 `HOMEDRIVE+HOMEPATH` 设了会返 None，fallback 到 `.` 导致找不到数据目录。
  - **绝对路径判断**：凡校验/接受绝对路径的代码都用 `cdt_discover::looks_like_absolute_path(&str)`，**不要**直接用 `Path::is_absolute()`。前者跨平台识别 POSIX `/foo`、Windows `C:[\/]...`、UNC `\\...`；后者只认当前平台的风格，Windows 上拒 POSIX（但 SSH 远端 / WSL / JSONL `cwd` 字段都可能是 POSIX）。
  - **路径编解码**：`encode_path` / `decode_path` / `is_valid_encoded_path` 是**跨 crate 唯一实现源**，在 `cdt_discover::path_decoder`。`cdt-config::claude_md`、`cdt-api/tests/agent_configs.rs` 等调用方 `use cdt_discover::encode_path`，**禁止**再写私有副本（历史上有两份分叉副本，是 Windows auto-memory 找不到文件的根因之一）。
- **clippy pedantic**：workspace 开启 pedantic，PostToolUse hook 会在每次 `.rs` 编辑后自动跑 clippy 报错。最常踩的：`doc_markdown`（注释里标识符要反引号）、`cast_possible_wrap`（`u64 as i64` → `i64::try_from`）、`uninlined_format_args`（`format!("{}", x)` → `format!("{x}")`）、`map_unwrap_or`（`x.map(f).unwrap_or(d)` → `x.map_or(d, f)`）、`is_some_and`（`opt.map(f).unwrap_or(false)` → `opt.is_some_and(f)`）、`if_not_else`（`if x != y { A } else { B }` → 倒顺序）、`manual_let_else`（`match X { Ok(v)=>v, Err(_)=>return ... }` → `let Ok(v) = X else { return ... }`）。其余照 clippy 输出修即可。
- **IPC payload 瘦身模式**：default-cap 字段不能直接 drop（前端 header 仍要用），用 `#[serde(default)]` 加 derived header 字段 + `xxx_omitted: bool` flag + 新加 `get_xxx_lazy(...)` IPC + 顶部 `const OMIT_XXX: bool = true` 一行回滚开关；前端按 `xxxOmitted` 走 fallback 链兼容老后端。参见 change `subagent-messages-lazy-load` / `session-detail-image-asset-cache` / `session-detail-response-content-omit`。**`const OMIT_XXX` + `apply_xxx_omit` 函数 + `get_session_detail` 调用点必须同一轮 Edit 完成**——分步骤加 clippy `dead_code` 立即拒，PostToolUse hook 每步阻塞。Tauri webview IPC 吞吐实测 ≈ **6.5 KB/ms**（含 V8 JSON.parse 反序列化端到端；纯网络字节是 13 KB/ms 但前端 console 视角看到的是含 parse 的端到端），>1 MB payload 就该考虑瘦身。
- **insta 快照接受**：没装 `cargo-insta` 就用 `INSTA_UPDATE=always cargo test -p <crate>`；提交生成的 `tests/snapshots/*.snap`。
- **同步解析入口**：`cdt-analyze` 的集成测试不引入 tokio——用 `cdt_parse::parse_entry_at(line, n)` 逐行解析 fixture，再跑 `dedupe_by_request_id`。
- **自动化**：
  - Hooks（`.claude/hooks/`）：`.rs` 编辑后自动跑所属 crate 的 `cargo clippy -- -D warnings`；`.svelte` 编辑后自动跑 `svelte-check`；`git commit` 前自动跑 `openspec validate --strict`。
  - **spec 变更约定**（硬约束，违反需要修复）：
    1. **修改**已有 spec 必须走 `openspec/changes/<slug>/specs/<cap>/spec.md` delta（含 `ADDED` / `MODIFIED` / `REMOVED` 块），`openspec archive <slug> -y` 时由命令自动 sync 回主 spec `openspec/specs/<cap>/spec.md`。**禁止**直接 Edit `openspec/specs/<cap>/spec.md`——那是 archive 的产出物，不是输入源。
    2. **新增** spec（如全新 UI 行为 spec）可直接写入 `openspec/specs/<name>/spec.md`，不需要 change delta。
    3. **archive 是历史快照**：`openspec/changes/archive/<日期>-<slug>/` 内的所有文件（含 `specs/<cap>/spec.md` delta、proposal、design、tasks）**冻结**——绝对不要事后 Edit；如需修订同一 capability 行为，开新 change 走 delta。
    4. **引用约定**：CLAUDE.md / followups.md / commit message 引用一个已归档 change 时，**只**写 `change <slug>`（如 `change session-detail-lazy-render`），**不要**写 `archive 2026-XX-XX-<slug>` 也**不要**写 `openspec/changes/archive/...` 路径——理由：日期前缀只是文件系统位置，不是引用单位；行为契约的真实来源是主 spec。需要溯源到具体 Requirement 时，引用 `openspec/specs/<cap>/spec.md` `<Requirement 标题>`。
    5. **archive 顺序坑（多 change 同 Requirement）**：`openspec archive <slug>` 用 delta 的 `MODIFIED Requirement` 完整 body **替换**主 spec 对应 Requirement，**不做三方合并**。如果你刚 archive 了 change A（修改 Req X），紧接着 archive 一个更老的 change B（也修改 Req X 但 delta 里没有 A 的内容），B 的 archive 会把 A 写入主 spec 的内容覆盖丢掉。规避：(a) 按 change 创建顺序 archive，先老后新；(b) 已经倒序 archive 时手工 diff 主 spec、把丢失的段落 merge 回去再 commit。本仓库 commit `1173885` 是案例。
    6. **行为契约级改动先 propose 再 apply**：涉及 IPC 字段语义 / 后端算法 / 状态判定 / 数据 omit 策略 / Tauri command 协议的改动，**先**写 `proposal.md` + `tasks.md`（空 checkbox）+ spec delta 并 `openspec validate <slug> --strict`，**再**动 code 边写边勾 checkbox，最后 archive。事后补 change 是已被否决的下策（reviewer 看 PR 时 spec 还旧、propose 阶段的设计取舍机会被跳过）。纯视觉对齐 / 文案 / SVG 路径仍按"小改动直接 commit"。判断不准默认走 openspec。
    7. **OpenSpec 工作流走 skill，不要手写**：开 change 用 `/opsx:propose <slug>` 一次生成 proposal + design + tasks + specs delta + validate；apply 用 `/opsx:apply <slug>` 按 tasks.md 推进；archive 用 `/opsx:archive <slug>`（或等价 CLI `openspec archive <slug> -y`）。**禁止**手 `mkdir openspec/changes/<slug>` + `Write` 三件套——易漏 design.md、易写错 delta 格式。`design.md` **不是可选项**——任何 change 都要写明 D1/D2/D3... 决策记录（候选方案 / 取舍 / 风险），让 reviewer 能从设计层评估。
  - **spec delta 写法**：`ADDED/MODIFIED Requirement` 体的**第一段**必须含 `SHALL` 或 `MUST`，否则 `openspec validate --strict` 报 `must contain SHALL or MUST`；中文背景描述要放在规约句之后。
  - Subagent：`spec-fidelity-reviewer` 按 capability 审计 scenario→test 覆盖。
  - Skill：`/ts-parity-check <capability>` 对比 TS 源与 Rust 端口 + followups。
  - MCP：`.mcp.json` 注册 GitHub MCP，需要 `GITHUB_PERSONAL_ACCESS_TOKEN` 环境变量。
- **opsx:apply 推进节拍**：详见 `.claude/rules/opsx-apply-cadence.md`。核心：Edit → clippy → fmt → test → npm check → validate → 勾 checkbox → 文本总结，不得中途停手。
- Detailed rules: `.claude/rules/rust.md`.

## 发布与分支策略

- `main` 是发布分支，**不直接提交**。日常走 `feat/xxx` / `fix/xxx` 分支 → PR → merge（详见 README）
- 版本号同步在三处：`Cargo.toml`（workspace）、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json`
- 打 `vX.Y.Z` tag 触发 `.github/workflows/release.yml` —— macOS arm64/x64 + Linux + Windows 矩阵构建 Tauri bundle 到 Draft Release
- 发布前跑 `just release-check`（版本三处一致 + 工作树干净 + preflight）

## 性能回归监测

大会话首屏优化历经多轮 IPC 瘦身：lazy markdown render（`session-detail-lazy-render`）→ subagent.messages 懒加载（`subagent-messages-lazy-load`，砍 60%）→ image base64 OMIT + `asset://` 懒加载（`session-detail-image-asset-cache`，砍 71-88%）→ response.content OMIT（`session-detail-response-content-omit`，砍 40%）→ tool_exec.output 懒加载（`session-detail-tool-output-lazy-load`）→ tool_output OMIT 携带 size 元数据消除 token 抖动（`tool-output-omit-preserve-size`）。
"模式"沉淀为 Conventions 的 **IPC payload 瘦身模式**；具体 phase 实现查 `git log --grep="feat(perf)"`。
回归入口：`cargo test --release -p cdt-api --test perf_get_session_detail -- --ignored --nocapture` —— 输出各阶段后端耗时 + 字段级 payload breakdown + raw vs IPC OMIT 对比。
后端探针：`tracing::info!(target: "cdt_api::perf", ...)`；前端：`[perf]` console.info。

## What to do first in a fresh session

1. `cargo build --workspace` + `just test` 确认回归绿
2. `just dev` 启动桌面应用验证当前状态
3. UI 功能迭代：**行为契约改动**（IPC 字段 / 后端算法 / 状态判定 / 数据流语义）走 openspec（`/opsx:propose` → `/opsx:apply` → `/opsx:archive`，design.md 必备）；**纯视觉对齐 / 单点样式修复 / Trigger CRUD 等**直接写 + PR，不走 openspec。判断不准默认走 openspec（成本是写一份 proposal + delta，收益是契约清晰）。
4. 开新工作前先 `git checkout -b feat/<slug>`，不要直接在 `main` 上写代码
5. 性能 / 卡顿排查：用"性能回归监测"段的入口，**先看数据再定方向**

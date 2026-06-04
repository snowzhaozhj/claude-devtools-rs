# src-tauri/ — Tauri 2 Rust 后端 + IPC

仅在 Claude 读写 `src-tauri/**` 下的文件时由 Claude Code 自动加载。跨域共识在根 `CLAUDE.md`，IPC 数据结构契约在 `crates/CLAUDE.md`。

## 架构

- 独立 Cargo.toml，**excluded from workspace**（workspace 根 `Cargo.toml` 的 `exclude` 列表里必须含 `src-tauri`）；通过 path deps 引用 `crates/`。
- `beforeDevCommand` 从 `src-tauri/` 目录执行，路径用 `../ui`。
- Tauri IPC commands 直接调用 `LocalDataApi`（不走 HTTP）。command 权威清单 = `crates/cdt-api/tests/ipc_contract.rs::EXPECTED_TAURI_COMMANDS`，`src-tauri/src/lib.rs::invoke_handler!` 与之同步。`list_sessions_sync` trait method 保留作为非 SSE-aware 客户端 fallback，但 axum HTTP route 现已与 IPC 共用 `list_sessions`（骨架 + SSE push）实现（change `unify-session-list-loading-strategy`）。
- **Trigger / pin / hide / session prefs 在 `DataApi` trait 中**：`add_trigger` / `remove_trigger` / `pin_session` / `unpin_session` / `hide_session` / `unhide_session` / `get_project_session_prefs` 历史上是独立 inherent 方法，change `add-server-mode` 起提升到 trait，让 HTTP 路径（浏览器 runtime）能镜像 IPC 同名 command；trait `default impl` 返回 not-implemented，`LocalDataApi` 在 `impl DataApi` 块 override 真实实现。

## 后台任务模式

- Tauri `setup` 里启动后台 task 用 `tauri::async_runtime::spawn`，**不要**裸 `tokio::spawn`。
- 订阅后端 `broadcast::Receiver` 转 `app.emit(...)` 是典型模式——见 `src-tauri/src/lib.rs` 的 FileWatcher + notifier bridge。

## IPC payload 瘦身模式（>1 MB 必走）

Tauri webview IPC 吞吐实测：**6.5 KB/ms（含 V8 JSON.parse 端到端，前端 console 视角）/ 13 KB/ms（纯网络字节）**。`>1 MB payload` 就该考虑瘦身。

模式（参见已归档 change `subagent-messages-lazy-load` / `session-detail-image-asset-cache` / `session-detail-response-content-omit`）：
1. **不能直接 drop**（前端 header 仍要用）：用 `#[serde(default)]` 加 derived header 字段 + `xxx_omitted: bool` flag
2. **新加 `get_xxx_lazy(...)` IPC**：前端按 `xxxOmitted` 走 fallback 链兼容老后端
3. **一行回滚开关**：顶部 `const OMIT_XXX: bool = true`
4. **`const OMIT_XXX` + `apply_xxx_omit` 函数 + `get_session_detail` 调用点必须同一轮 Edit 完成**——分步骤加 clippy `dead_code` 立即拒，PostToolUse hook 每步阻塞

## IPC 字段改动 checklist（硬约束）

改 `LocalDataApi` 公开方法返回字段或加新 Tauri command，SHALL 同一 PR 内同步：
- (a) `crates/cdt-api/tests/ipc_contract.rs`：加 / 改 contract test，断言新字段 camelCase 形态
- (b) `ui/src/lib/api.ts`：interface 字段同步
- (c) `ui/src/lib/__fixtures__/*.ts`：fixture 数据按新形态填
- (d) 新 command 还要同步 `EXPECTED_TAURI_COMMANDS`（`cdt-api/tests/ipc_contract.rs`）+ `KNOWN_TAURI_COMMANDS`（`ui/src/lib/tauriMock.ts`）+ `invoke_handler!`（`src-tauri/src/lib.rs`）三处

## Tauri IPC 透传

`src-tauri/src/lib.rs` 的 commands 返回 `serde_json::Value`，`cdt-api` 类型扩展字段（如 `SessionSummary` 加 `title`）自动透传，不需要改 Tauri 层。

## 桌面通知 / 系统托盘

- `tauri-plugin-notification`：`Cargo.toml` + `capabilities/default.json` 加 `"notification:default"`
- `TrayIconBuilder::with_id("main-tray")` 在 `setup` 里构建，tray icon 独立于 app icon（见 `icons/tray-icon*.png`）
- 后端发通知：`app_handle.notification().builder().title(..).body(..).sound("default").show()`
- 前端 Dock badge：`getCurrentWindow().setBadgeCount()`（macOS 独占）

## `devtools` feature 不要进 release

- `src-tauri/Cargo.toml` 的 `tauri = { features = [...] }` **不要**写 `"devtools"`——加了之后 release bundle 会带 web inspector。
- Tauri 2 在 debug 构建自动启 inspector 不依赖 feature，`cargo tauri dev` 照旧能开。
- 调用 `window.open_devtools()` 必须用 `#[cfg(debug_assertions)]` **编译时** gate（**不是** `if cfg!(debug_assertions)` 运行时宏）——后者 release 构建里会因 `open_devtools` API 不存在（它本身就有 `#[cfg(any(debug_assertions, feature = "devtools"))]`）而编译失败。

## `tauri-plugin-updater` 配置链 + 签名密钥治理

发版分发走 GitHub Release `latest.json` endpoint（`tauri-action` 自动生成 + attach）。

**配置链一致性硬约束**（任一缺失即不签名 / 校验失败）：
1. `tauri.conf.json::bundle.createUpdaterArtifacts: true`
2. `tauri.conf.json::plugins.updater.{endpoints,pubkey}`
3. `capabilities/default.json::permissions` 含 `"updater:default"` + `"process:default"`
4. `Cargo.toml` 含 `tauri-plugin-updater` + `tauri-plugin-process`
5. `lib.rs` 注册两个 plugin
6. `release.yml` env 注入 `TAURI_SIGNING_PRIVATE_KEY` + 密码

**最大坑：私钥不可换**——已发布版本里 `pubkey` 已 commit 入库，老用户客户端只信这把公钥；私钥丢失 / rotate → 老用户**永远**无法验证新签名，只能手动到 GitHub Release 重装含新公钥的版本。**多副本备份私钥**（密码管理器 + 物理介质）。

**Linux `.deb` 不支持 in-place 升级**（Tauri 限制），前端调 `update.downloadAndInstall()` 会抛错，UI 层捕获后弹"请到 GitHub 下载"对话框。

**手动检查 IPC** = `check_for_update`（忽略 `skippedUpdateVersion`）；**启动 5s 后台检查**走 `updater://available` event 推送（gate 在 `ConfigData::updater::auto_update_check_enabled`）。

**版本号命名硬约束**：Windows MSI bundler 不接受 pre-release 含字母（`v0.3.0-rc.1` 在 Windows 单 fail：`pre-release identifier must be numeric-only`），本仓发版直接走正式版 + hotfix 序列（`v0.3.0` → `v0.3.1`），**不**用 `-rc.1` / `-beta` 命名；演练应用内更新链路靠真实 hotfix release。详见 change `updater-auto-update`。

## `src-tauri` 独立 manifest 的 cargo cache 坑

改 `cdt-*` crate 的 `pub use` 后，workspace `cargo check` 能过，但 `cargo clippy --manifest-path src-tauri/Cargo.toml` 会报 "no xxx in the root"——src-tauri 的 `target/` 独立缓存未感知 workspace API 变化。修法：`cargo clean -p <crate-name> --manifest-path src-tauri/Cargo.toml` 后重跑。`just lint` 跑全（两个 manifest）时首次也会触发这坑。

## 重启与开发流

- Vite HMR 只更新前端；后端改动需 `pkill -f claude-devtools-tauri && cargo tauri dev` 重启。
- `cd src-tauri/` 后 Bash tool cwd 会持久化——后续 `cargo test --workspace` 会误入 tauri 子 manifest。优先用 `just` 或 `--manifest-path` 显式指定。

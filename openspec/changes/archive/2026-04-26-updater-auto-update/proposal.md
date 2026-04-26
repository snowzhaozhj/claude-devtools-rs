## Why

当前发布新版本后，用户需要手动到 GitHub Release 下载安装包覆盖安装，这个流程对桌面应用体验很糟——版本飞涨期（v0.x 高频迭代）老用户根本意识不到新版本存在，更别提升级。竞品（包括 TS 原版的 Electron 自带 `autoUpdater`）都是启动后台静默检查 + 应用内一键更新；Tauri 2 自带 `tauri-plugin-updater` 与 `tauri-action` GitHub Actions 集成，发版流程改动极小但用户体验提升明显，应该在 v0.2 这个相对稳定窗口提前打底。

## What Changes

- 引入 `tauri-plugin-updater`：应用启动后延迟 5 秒后台静默检查更新；`SettingsView` 新增「检查更新」按钮触发手动检查
- 新增 `UpdateBanner.svelte` UI：发现新版本时顶部横幅展示版本号 + release notes，提供「立即更新 / 稍后提醒 / 跳过此版本」三按钮
- 用户可禁用自动检查：`SettingsView` 新增「启动时自动检查更新」开关（默认开启），关闭后跳过启动检查但保留手动检查能力；`ConfigData` 加 `autoUpdateCheckEnabled: bool` 字段
- 跳过版本号持久化：`ConfigData` 加 `skippedUpdateVersion: Option<String>` 字段，下次启动检查命中该版本则不弹横幅
- 签名密钥基础设施：本地一次性 `tauri signer generate` 出 minisign keypair；公钥进 `tauri.conf.json`（commit 入库），私钥进 GitHub Secrets `TAURI_SIGNING_PRIVATE_KEY` + `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`，`release.yml` 通过 env 注入到 `tauri-action`
- `tauri.conf.json` 新增 `bundle.createUpdaterArtifacts: true` + `plugins.updater.endpoints` + `plugins.updater.pubkey`；`capabilities/default.json` 加 `updater:default` 权限
- 发版分发走 GitHub Release `latest.json` endpoint（`tauri-action` 在 build 时自动生成 + attach 到 Release assets，零额外 step）
- README 加发版前生成密钥的一次性说明；CLAUDE.md 加 updater 相关陷阱条目（密钥丢失风险、Linux .deb 不支持等）

## Capabilities

### New Capabilities

- `app-auto-update`: 桌面应用层的自动检查更新与应用内更新能力。覆盖启动后台检查、手动触发检查、横幅 UI 三按钮交互（立即更新 / 稍后提醒 / 跳过此版本）、跳过版本持久化、updater bundle 签名校验、跨平台覆盖策略（macOS / Windows / Linux AppImage）。

### Modified Capabilities

- `configuration-management`: `ConfigData` 新增 `autoUpdateCheckEnabled: bool`（默认 true）和 `skippedUpdateVersion: Option<String>` 两个字段，遵循既有 schema 演进约定（前者 `#[serde(default = "fn_returns_true")]`，后者 `#[serde(default, skip_serializing_if = "Option::is_none")]`）

## Impact

- **新增依赖**：`src-tauri/Cargo.toml` 加 `tauri-plugin-updater = "2"`
- **新增 IPC command**：`check_for_update` invoke command（手动检查触发用），需同步 `crates/cdt-api/tests/ipc_contract.rs::EXPECTED_TAURI_COMMANDS`、`ui/src/lib/tauriMock.ts::KNOWN_TAURI_COMMANDS`、`src-tauri/src/lib.rs::invoke_handler!` 三处
- **Tauri event**：updater 进度走后端 `app.emit("updater-status", ...)` → 前端 `listen()` 模式（与现有 `notification-update` / `session-metadata-update` 一致）
- **配置链同步**：`tauri.conf.json` + `capabilities/default.json` + `Cargo.toml` features + `src-tauri/src/lib.rs` 注册 plugin 四处需要保持一致
- **CI/CD**：`.github/workflows/release.yml` 加 `TAURI_SIGNING_PRIVATE_KEY` + 密码两个 env 变量；首次发版前需要在 GitHub repo 配置 Secrets
- **签名密钥治理**：私钥一旦丢失老版本永远无法升级——design.md 中明确备份策略
- **Linux 平台**：`.deb` 包不支持 updater（Tauri 限制），只对 AppImage 生效；`.deb` 用户仍需手动下载新包（首次安装场景仍正常）
- **前端组件树**：`App.svelte` 顶层新增 `<UpdateBanner />` 槽位；`SettingsView.svelte` 新增「关于 / 更新」分组
- **测试**：Vitest 单测覆盖 `UpdateBanner` 三按钮交互 + `skippedUpdateVersion` 命中跳过逻辑；Playwright 暂不覆盖（依赖真实 updater endpoint）

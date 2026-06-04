## Why

桌面端与 CLI (`cdt`) 分开分发，用户需要两次独立安装操作。桌面端用户未必知道 CLI 的存在，也没有便捷途径获取。两者共享相同版本号和 Release 产物，但更新机制完全割裂——桌面端有 tauri-plugin-updater 自动检查，CLI 需要手动 `cdt self-update`。

本 change 让桌面端成为 CLI 的发现和安装入口：Settings 新增 CLI section，启动时异步检测 CLI 版本，一键安装/更新到与桌面端同版本的 CLI binary。

## What Changes

- 新增 Tauri IPC command `get_cli_status`：异步检测系统 PATH 中的 `cdt` binary 版本
- 新增 Tauri IPC command `install_cli`：从 GitHub Release 下载对应平台 CLI binary 到 `~/.local/bin/cdt`
- 启动时异步调用 `get_cli_status`，结果缓存内存供 Settings 页面消费
- Settings 新增 "CLI" section（插入在"键盘快捷键"与"诊断"之间），展示安装状态、版本、操作按钮
- 复用 `cdt-cli/src/update.rs` 中已有的 `platform_asset_name` / `download_and_extract` / `replace_binary` 逻辑（提取为共享模块）

## Capabilities

### New Capabilities

- `cli-distribution`: 桌面端对 CLI binary 的检测、安装、更新能力

### Modified Capabilities

- `settings-ui`: 新增 CLI section 到 Settings 页面，含状态展示和操作按钮

## Impact

- **后端 crate**：`cdt-cli/src/update.rs` 的下载/解压/替换逻辑需提取到可共享位置（`cdt-core` 或新建 `cdt-updater` 内部模块）
- **Tauri IPC**：新增 2 个 command（`get_cli_status` / `install_cli`），需在 `src-tauri/` 注册
- **前端**：`SettingsView.svelte` 新增 CLI section + 启动时异步检测逻辑
- **安全**：下载 binary 需校验 HTTP status / content-length / magic bytes；macOS 需清除 quarantine 属性
- **性能影响**：启动时 1 次异步 `which` + `cdt --version`（~50ms），不阻塞 UI，零感知

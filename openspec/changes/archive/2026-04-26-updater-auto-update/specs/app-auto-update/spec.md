## ADDED Requirements

### Requirement: Updater plugin 集成与签名校验

系统 SHALL 通过 `tauri-plugin-updater` 实现应用内自动更新，所有更新包 MUST 用维护者持有的 minisign 私钥签名，updater plugin MUST 在下载完成后用 `tauri.conf.json::plugins.updater.pubkey` 内嵌的公钥校验签名，签名校验失败时 SHALL NOT 应用更新。

#### Scenario: 合法签名包通过校验

- **WHEN** 用户触发更新流程，下载到的 bundle 与签名匹配
- **THEN** updater plugin SHALL 校验通过，把更新包写入安装位置并准备重启

#### Scenario: 签名校验失败

- **WHEN** 下载到的 bundle 签名与公钥不匹配（中间人或私钥被替换）
- **THEN** updater plugin SHALL 拒绝安装、清理临时文件、返回 signature error
- **AND** 前端 SHALL 弹出严重错误对话框提示「更新包签名无效，请到 GitHub Release 验证后手动下载」
- **AND** `tracing::error!(target: "cdt_tauri::updater", ...)` SHALL 记录该事件

#### Scenario: 公钥未配置导致启动失败

- **WHEN** `tauri.conf.json::plugins.updater.pubkey` 为空字符串或字段缺失
- **THEN** Tauri 应用启动 SHALL 报配置错误，避免发布无法验证签名的版本

### Requirement: 启动后台静默检查

系统 SHALL 在应用启动后**延迟 5 秒**通过 `tauri::async_runtime::spawn` 在后台调用 updater plugin 的 `check()`，且 MUST NOT 阻塞主窗口渲染。该检查 SHALL 受 `ConfigData::auto_update_check_enabled` 开关控制，开关关闭时 MUST NOT 调用 `check()`、MUST NOT emit 任何 updater event。

#### Scenario: 启动检查发现新版本

- **WHEN** 启动后 5 秒，`auto_update_check_enabled == true`
- **AND** updater 拉取 `latest.json` 拿到的版本号 > 当前版本号
- **AND** 该新版本号 ≠ `ConfigData::skipped_update_version`
- **THEN** 后端 SHALL 通过 `app.emit("updater://available", payload)` 推送事件
- **AND** 前端 `UpdateBanner` 组件 SHALL 在主窗口顶部显示横幅，含版本号、release notes（markdown 渲染）与三按钮

#### Scenario: 自动检查被禁用

- **WHEN** 启动后 5 秒，`auto_update_check_enabled == false`
- **THEN** 后端 SHALL NOT 调用 `app.updater().check()`
- **AND** SHALL NOT emit `updater://available` 或其它 updater event
- **AND** `tracing::debug!(target: "cdt_tauri::updater", "auto check disabled, skip startup check")` SHALL 记录该路径

#### Scenario: 启动检查命中跳过版本

- **WHEN** 拿到的新版本号 == `ConfigData::skipped_update_version`
- **THEN** 后端 SHALL NOT 推送 `updater://available` 事件
- **AND** `tracing::info!(target: "cdt_tauri::updater", skipped_version = %v, ...)` SHALL 记录跳过

#### Scenario: 启动检查无网络或 endpoint 不可达

- **WHEN** updater 拉取 `latest.json` 失败（网络错误 / DNS 失败 / 503 等）
- **THEN** 后端 SHALL 静默吞掉错误，仅 `tracing::warn!(target: "cdt_tauri::updater", error = ?, ...)` 记录
- **AND** 前端 SHALL NOT 收到任何事件、SHALL NOT 显示横幅或错误提示

#### Scenario: 启动检查时已是最新版本

- **WHEN** 拿到的新版本号 ≤ 当前版本号
- **THEN** 后端 SHALL NOT 推送 `updater://available` 事件
- **AND** `tracing::debug!(target: "cdt_tauri::updater", current_version = %v, "no update available")` SHALL 记录

### Requirement: 手动检查更新 IPC

系统 SHALL 暴露 `check_for_update` 这个 Tauri invoke command，返回当前更新状态结构体，供 SettingsView 的「检查更新」按钮调用。手动检查 MUST 忽略 `auto_update_check_enabled` 开关——用户主动触发即视为同意检查。

#### Scenario: 手动检查返回最新版本

- **WHEN** 前端调 `invoke("check_for_update")` 且 endpoint 返回的版本号 ≤ 当前版本
- **THEN** invoke SHALL 返回 `{ status: "upToDate", currentVersion: "X.Y.Z" }`

#### Scenario: 手动检查返回新版本

- **WHEN** 前端调 `invoke("check_for_update")` 且 endpoint 返回的版本号 > 当前版本
- **THEN** invoke SHALL 返回 `{ status: "available", currentVersion, newVersion, notes, signatureOk: true }`
- **AND** 即使该版本号在 `skippedUpdateVersion` 中，invoke 仍 SHALL 如实返回（手动检查忽略跳过状态——用户主动按按钮意味着他想知道）

#### Scenario: 手动检查网络错误

- **WHEN** 前端调 `invoke("check_for_update")` 且网络/endpoint 错误
- **THEN** invoke SHALL 返回 `{ status: "error", message: "<错误描述>" }`
- **AND** 前端 SHALL 在按钮附近显示错误消息 + 重试入口

#### Scenario: IPC 字段约定同步

- **WHEN** `check_for_update` command 被加入 invoke_handler!
- **THEN** `crates/cdt-api/tests/ipc_contract.rs::EXPECTED_TAURI_COMMANDS` MUST 包含 `"check_for_update"`
- **AND** `ui/src/lib/tauriMock.ts::KNOWN_TAURI_COMMANDS` MUST 包含 `"check_for_update"`
- **AND** `src-tauri/src/lib.rs::invoke_handler!` MUST 注册 `check_for_update` 函数

### Requirement: UpdateBanner 三按钮交互

系统 SHALL 在前端实现 `UpdateBanner.svelte` 组件，发现新版本时在主窗口顶部展示横幅，提供「立即更新 / 稍后提醒 / 跳过此版本」三按钮，每按钮对应明确语义。

#### Scenario: 立即更新按钮

- **WHEN** 用户点击「立即更新」
- **THEN** 前端 SHALL 调用 updater plugin JS API 的 `update.downloadAndInstall()`
- **AND** 横幅 SHALL 显示下载进度（订阅 `updater://download-progress` event，按字节比例渲染）
- **AND** 下载 + 校验 + 安装完成后 SHALL 调用 `app.restart()` 重启应用

#### Scenario: 稍后提醒按钮

- **WHEN** 用户点击「稍后提醒」
- **THEN** 横幅 SHALL 关闭
- **AND** 当前应用进程内 SHALL NOT 再次显示该版本横幅
- **AND** 下次应用启动 SHALL 重新检查并可能再次显示同一版本横幅
- **AND** `ConfigData::skipped_update_version` SHALL NOT 被修改

#### Scenario: 跳过此版本按钮

- **WHEN** 用户点击「跳过此版本」
- **THEN** 前端 SHALL 调 `update_config` IPC 把 `skippedUpdateVersion` 设为该版本号字符串
- **AND** 横幅 SHALL 关闭
- **AND** 后续启动检查命中同一版本号时 SHALL NOT 再显示横幅

#### Scenario: 用户关闭横幅 X 按钮

- **WHEN** 用户点击横幅右上角关闭按钮
- **THEN** 行为 SHALL 等价于「稍后提醒」

#### Scenario: 下载过程中关闭横幅

- **WHEN** 用户在「立即更新」下载进行中点击关闭按钮
- **THEN** 前端 SHALL 弹确认对话框「确定取消下载？」
- **AND** 用户确认后 SHALL 中断下载、清理临时文件、关闭横幅
- **AND** 用户取消则 SHALL 保持下载继续

### Requirement: SettingsView 自动检查开关

`SettingsView`「关于 / 更新」分组 SHALL 提供「启动时自动检查更新」开关，绑定到 `ConfigData::auto_update_check_enabled`，默认开启；切换开关 SHALL 立即通过 `update_config` IPC 持久化（乐观更新模式：本地 state 先翻 → 异步调 IPC → 失败时回滚）。

#### Scenario: 开关切换为关闭

- **WHEN** 用户在 SettingsView 把「启动时自动检查更新」开关切到关闭
- **THEN** 前端 SHALL 立即把本地 `$state` 设为 `false`
- **AND** SHALL 调 `update_config` IPC 把 `autoUpdateCheckEnabled` 设为 `false`
- **AND** IPC 失败时 SHALL 回滚本地 `$state` 并 `getConfig` 重新拉取
- **AND** 下次应用启动时后端 SHALL NOT 执行 5s 后台检查

#### Scenario: 开关切换为开启

- **WHEN** 用户把开关从关闭切到开启
- **THEN** 前端 SHALL 立即把本地 `$state` 设为 `true`，持久化为 `true`
- **AND** 下次应用启动时后端 SHALL 恢复 5s 后台检查
- **AND** 当前会话 SHALL NOT 立即触发后台检查（用户想要立即检查应该按「检查更新」按钮）

#### Scenario: 关闭开关不影响手动检查

- **WHEN** `auto_update_check_enabled == false` 且用户按「检查更新」按钮
- **THEN** 系统 SHALL 正常调 `check_for_update` IPC，按 `status` 返回结果

### Requirement: 跳过版本语义比较

系统 SHALL 用 semver 比较跳过版本与新版本号；若新版本号 > 跳过版本号，SHALL 重新展示横幅（即跳过状态自动失效）。

#### Scenario: 用户跳过 v0.3.0 后发布 v0.3.1

- **GIVEN** `ConfigData::skipped_update_version == "0.3.0"` 且当前版本是 `v0.2.x`
- **WHEN** 启动检查拿到 `latest.json` 版本号为 `0.3.1`
- **THEN** 后端 SHALL 视为新版本，emit `updater://available`
- **AND** 横幅 SHALL 正常展示

#### Scenario: 用户跳过 v0.3.0 后发布 v0.3.0-hotfix.1

- **GIVEN** `skipped_update_version == "0.3.0"`
- **WHEN** 启动检查拿到 `0.3.0-hotfix.1`（pre-release 后缀）
- **THEN** 按 semver 比较 `0.3.0-hotfix.1 < 0.3.0`，命中跳过逻辑

注：以上语义比较 MUST 用 `semver` crate 严格判断，禁止字符串比较。

### Requirement: 跨平台覆盖与 .deb 降级

系统 SHALL 在 macOS（arm64 + x64）/ Windows / Linux AppImage 上启用 in-place 应用内更新；Linux .deb 包用户由于 Tauri updater 限制 MUST 降级到「显示新版本提示，但点击立即更新时给出错误并跳转 GitHub Release」。

#### Scenario: macOS / Windows / AppImage 立即更新

- **WHEN** 用户在以上平台点击「立即更新」
- **THEN** updater plugin SHALL 完成下载 + 签名校验 + in-place 替换 + 自启

#### Scenario: .deb 包用户立即更新降级

- **WHEN** 用户在 .deb 安装的应用中点击「立即更新」
- **THEN** updater plugin SHALL 返回错误（不支持 .deb 替换）
- **AND** 前端 SHALL 捕获错误，弹对话框「自动更新不支持当前安装方式（.deb 包），请到 GitHub Release 手动下载新包」+ 跳转链接按钮

### Requirement: 更新分发流程

`tauri.conf.json::bundle.createUpdaterArtifacts` SHALL 设为 `true`，且 `plugins.updater.endpoints` SHALL 包含 GitHub Release `latest.json` URL；`.github/workflows/release.yml` SHALL 通过 env 注入 `TAURI_SIGNING_PRIVATE_KEY` 与 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` 给 `tauri-action`。

#### Scenario: 配置链一致性

- **WHEN** 维护者修改 updater 相关配置
- **THEN** 以下文件 MUST 保持一致：
  - `src-tauri/tauri.conf.json::bundle.createUpdaterArtifacts: true`
  - `src-tauri/tauri.conf.json::plugins.updater.endpoints: ["https://github.com/<repo>/releases/latest/download/latest.json"]`
  - `src-tauri/tauri.conf.json::plugins.updater.pubkey: "<base64 minisign pubkey>"`
  - `src-tauri/capabilities/default.json::permissions` 包含 `"updater:default"`
  - `src-tauri/Cargo.toml::dependencies` 包含 `tauri-plugin-updater = "2"`
  - `src-tauri/src/lib.rs::run()` 注册 `tauri_plugin_updater::Builder::new().build()`

#### Scenario: 发版流程产出 latest.json

- **WHEN** 维护者打 `vX.Y.Z` tag 触发 release.yml
- **THEN** `tauri-action` SHALL 检测到 `TAURI_SIGNING_PRIVATE_KEY` env，签所有 bundle、生成 `latest.json` manifest、attach 到 Draft Release assets
- **AND** maintainer 手动 publish draft 后，updater endpoint `https://github.com/<repo>/releases/latest/download/latest.json` SHALL 立即指向最新 published release

#### Scenario: 私钥未注入导致发版失败

- **WHEN** GitHub Secrets 中 `TAURI_SIGNING_PRIVATE_KEY` 缺失或为空
- **THEN** `tauri-action` SHALL 报错并 fail CI，不产出未签名 bundle

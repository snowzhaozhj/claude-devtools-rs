## MODIFIED Requirements

### Requirement: Updater plugin 集成与签名校验

系统 SHALL 通过 `tauri-plugin-updater` 实现应用内自动更新，所有更新包 MUST 用维护者持有的 minisign 私钥签名，updater plugin MUST 在下载完成后用配置内嵌的公钥校验签名，签名校验失败时 SHALL NOT 应用更新。

#### Scenario: 合法签名包通过校验

- **WHEN** 用户触发更新流程，下载到的 bundle 与签名匹配
- **THEN** updater plugin SHALL 校验通过，把更新包写入安装位置并准备重启

#### Scenario: 签名校验失败

- **WHEN** 下载到的 bundle 签名与公钥不匹配（中间人或私钥被替换）
- **THEN** updater plugin SHALL 拒绝安装、清理临时文件、返回 signature error
- **AND** 前端 SHALL 弹出严重错误对话框提示「更新包签名无效，请到 GitHub Release 验证后手动下载」

#### Scenario: 公钥未配置导致启动失败

- **WHEN** updater 公钥配置位为空字符串或字段缺失
- **THEN** Tauri 应用启动 SHALL 报配置错误，避免发布无法验证签名的版本

### Requirement: 启动后台静默检查

系统 SHALL 在应用启动后**延迟 5 秒**通过后台 task 调用 updater plugin 的 `check()`，且 MUST NOT 阻塞主窗口渲染。该检查 SHALL 受 `ConfigData::auto_update_check_enabled` 开关控制，开关关闭时 MUST NOT 调用 `check()`、MUST NOT emit 任何 updater event。

#### Scenario: 启动检查发现新版本

- **WHEN** 启动后 5 秒，`auto_update_check_enabled == true`
- **AND** updater 拉取 `latest.json` 拿到的版本号 > 当前版本号
- **AND** 该新版本号 ≠ `ConfigData::skipped_update_version`
- **THEN** 后端 SHALL 通过 `updater://available` 事件推送 payload
- **AND** 前端 `UpdateStatusPill` SHALL 在 `UnifiedTitleBar` 右侧 `zone-status` 渲染 `available` 态药丸，含 icon-download + 版本号文本
- **AND** SHALL NOT 渲染任何全宽横幅或推挤页面内容

#### Scenario: 自动检查被禁用

- **WHEN** 启动后 5 秒，`auto_update_check_enabled == false`
- **THEN** 后端 SHALL NOT 触发 updater check
- **AND** SHALL NOT emit `updater://available` 或其它 updater event

#### Scenario: 启动检查命中跳过版本

- **WHEN** 拿到的新版本号 == `ConfigData::skipped_update_version`
- **THEN** 后端 SHALL NOT 推送 `updater://available` 事件

#### Scenario: 启动检查无网络或 endpoint 不可达

- **WHEN** updater 拉取 `latest.json` 失败（网络错误 / DNS 失败 / 503 等）
- **THEN** 后端 SHALL 静默吞掉错误
- **AND** 前端 SHALL NOT 收到任何事件、SHALL NOT 显示 status pill 或错误提示

#### Scenario: 启动检查时已是最新版本

- **WHEN** 拿到的新版本号 ≤ 当前版本号
- **THEN** 后端 SHALL NOT 推送 `updater://available` 事件

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

### Requirement: 更新分发流程

更新打包配置 SHALL 启用 updater artifacts 产物，updater endpoints SHALL 配置为 GitHub Release `latest.json` URL；CI release workflow SHALL 通过 env 注入 `TAURI_SIGNING_PRIVATE_KEY` 与 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`，让 `tauri-action` 能在打包时签名 bundle 并产出 `latest.json` manifest。

#### Scenario: 配置链一致性

- **WHEN** 维护者修改 updater 相关配置
- **THEN** 以下契约 SHALL 同步保持：
  - Tauri bundle 配置 SHALL 启用 updater artifacts
  - Tauri updater endpoints SHALL 指向 GitHub Release `latest.json`
  - Tauri updater 配置 SHALL 内嵌 base64 minisign 公钥（非空）
  - Tauri capabilities SHALL 含 `updater:default` 权限
  - `tauri-plugin-updater` 依赖 SHALL 出现在桌面端 Cargo manifest
  - Tauri runtime SHALL 注册 updater plugin

#### Scenario: 发版流程产出 latest.json

- **WHEN** 维护者打 `vX.Y.Z` tag 触发 release workflow
- **THEN** `tauri-action` SHALL 检测到 `TAURI_SIGNING_PRIVATE_KEY` env、签所有 bundle、生成 `latest.json` manifest、attach 到 Draft Release assets
- **AND** maintainer 手动 publish draft 后，updater endpoint `https://github.com/<repo>/releases/latest/download/latest.json` SHALL 立即指向最新 published release

#### Scenario: 私钥未注入导致发版失败

- **WHEN** GitHub Secrets 中 `TAURI_SIGNING_PRIVATE_KEY` 缺失或为空
- **THEN** `tauri-action` SHALL 报错并 fail CI，不产出未签名 bundle


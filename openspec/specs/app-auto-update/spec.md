# app-auto-update Specification

## Purpose

桌面应用层的自动检查更新与应用内更新能力。基于 `tauri-plugin-updater` + GitHub Release `latest.json` 分发，覆盖启动后台静默检查（受 `ConfigData::updater::auto_update_check_enabled` 开关控制，延迟 5 秒后台 spawn）、`SettingsView` 手动「检查更新」IPC（`check_for_update`）、`UpdateBanner` 三按钮交互（立即更新 / 稍后提醒 / 跳过此版本，后者用 `semver` 比较与 `skipped_update_version` 持久化）、minisign 签名校验、跨平台覆盖策略（macOS / Windows / Linux AppImage 走 in-place 替换；Linux `.deb` 因 Tauri 限制降级为「请到 GitHub Release 手动下载」）、以及发版流程的配置链一致性（`tauri.conf.json` + `capabilities/default.json` + `Cargo.toml` + `release.yml` Secrets 注入）。本 capability 由 `src-tauri` 与前端 `updateStore.svelte.ts` + `UpdateBanner.svelte` 共同实现，配置位由 `configuration-management` 的 `UpdaterConfig` section 提供。
## Requirements
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

### Requirement: UpdateStatusPill 状态机与 popover

系统 SHALL 在前端实现 `UpdateStatusPill.svelte` 组件，挂载于 `UnifiedTitleBar` 的 `zone-status` 内，根据 `updateStore` 状态机渲染 5 种 pill 形态，并在点击时展开 popover 承载「立即更新 / 稍后提醒 / 跳过此版本」三按钮与 release notes。Pill MUST 在 `idle` 态下不渲染（不占布局空间）。

| 状态 | pill 形态 | 触发 |
|---|---|---|
| `idle` | 不渲染 | 默认 / 无 update event |
| `available` | `<icon-download> vX.Y.Z` 蓝色边框 | `updater://available` event |
| `downloading` | 环形进度（替代 icon） + 百分比文本 | `updater://download-progress` event |
| `downloaded` | `<icon-restart> 重启更新` 绿色填充 | 下载 + 校验 + 安装就绪 |
| `error` | `<icon-warn> !` 红色填充 | 签名校验失败 / 下载失败 |

#### Scenario: pill idle 态不渲染

- **WHEN** `updateStore.status == "idle"`
- **THEN** `UpdateStatusPill` SHALL NOT 渲染任何 DOM 节点
- **AND** SHALL NOT 在 `zone-status` 占用任何宽度

#### Scenario: pill available 态点击展开 popover

- **WHEN** `updateStore.status == "available"` AND 用户点击 pill
- **THEN** popover SHALL 在 chrome 右下方展开，width 360 px
- **AND** popover SHALL 包含版本号文本、release notes（markdown 渲染）、三按钮（立即更新 / 稍后提醒 / 跳过此版本）
- **AND** pill 自身 SHALL 保持 `available` 态可见

#### Scenario: popover 在窄窗口居中 fallback

- **WHEN** 窗口宽度 `< 640 px` AND popover 展开
- **THEN** popover SHALL 在 viewport 内水平居中
- **AND** SHALL NOT 被裁剪到 viewport 外

#### Scenario: 立即更新按钮

- **WHEN** popover 已展开 AND 用户点击「立即更新」
- **THEN** 前端 SHALL 调用 updater plugin JS API 的 `update.downloadAndInstall()`
- **AND** popover 内 SHALL 显示下载进度条
- **AND** pill 状态 SHALL 切换为 `downloading`，pill 内 SHALL 渲染环形进度 + 百分比文本
- **AND** 下载 + 校验 + 安装完成后 SHALL 调用 `app.restart()` 重启应用

#### Scenario: 稍后提醒按钮

- **WHEN** popover 已展开 AND 用户点击「稍后提醒」
- **THEN** popover SHALL 关闭
- **AND** pill SHALL 切换回 `idle` 态（不渲染）
- **AND** 当前应用进程内 SHALL NOT 再次显示该版本 pill 或 popover
- **AND** 下次应用启动 SHALL 重新检查并可能再次显示同一版本 pill
- **AND** `ConfigData::skipped_update_version` SHALL NOT 被修改

#### Scenario: 跳过此版本按钮

- **WHEN** popover 已展开 AND 用户点击「跳过此版本」
- **THEN** 前端 SHALL 调 `update_config` IPC 把 `skippedUpdateVersion` 设为该版本号字符串
- **AND** popover SHALL 关闭
- **AND** pill SHALL 切换回 `idle` 态
- **AND** 后续启动检查命中同一版本号时 SHALL NOT 再显示 pill

#### Scenario: 点击 pill 外或按 Esc 关闭 popover

- **WHEN** popover 已展开 AND 用户点击 popover 与 pill 之外区域 OR 按下 `Esc` 键
- **THEN** popover SHALL 关闭
- **AND** pill 状态 SHALL 保持不变（仅是关闭 popover，不改 update 状态）

#### Scenario: downloading 中关闭 popover 不中断下载

- **WHEN** pill 状态为 `downloading` AND 用户点击 pill 外或按 `Esc` 关闭 popover
- **THEN** pill SHALL 保持 `downloading` 态可见 + 持续显示进度
- **AND** 下载 SHALL 继续，SHALL NOT 中断（Tauri updater plugin 当前无 mid-download abort 能力，详见 REMOVED 段 BREAKING 2）
- **AND** popover 内 SHALL NOT 提供「取消下载」按钮
- **AND** 用户 SHALL 可再次点击 pill 重新展开 popover 看进度

#### Scenario: popover 已展开期间 store 被外部切到 idle

- **WHEN** popover 已展开 AND `updateStore.status` 被外部调用（典型：`updateStore::dismiss()`）切换到 `idle`
- **THEN** popover SHALL 立即关闭
- **AND** pill SHALL 从 chrome `zone-status` 移除（idle 态不渲染）
- **AND** popover 内的 outside-click listener、`Esc` listener、focus trap SHALL 全部释放
- **AND** 焦点 SHALL 还给触发元素，触发元素已不存在时 fallback 到 `document.body`

#### Scenario: 下载失败或签名校验失败

- **WHEN** 下载报错或签名校验失败
- **THEN** pill 状态 SHALL 切换为 `error`，pill 内 SHALL 渲染红色 warn icon + `!` 文本
- **AND** pill SHALL 提供 hover tooltip 描述错误原因
- **AND** 点击 pill SHALL 展开 popover 含错误详情 + 重试 / 关闭按钮

#### Scenario: downloaded 态点击 pill 直接重启

- **WHEN** 下载 + 校验完成 AND pill 状态切换为 `downloaded`
- **THEN** pill 文本 SHALL 显示「重启更新」
- **AND** 点击 pill SHALL 直接调用 `app.restart()`（无需打开 popover）

#### Scenario: pill 与 popover 可访问性

- **WHEN** pill 渲染
- **THEN** pill SHALL 有 `aria-label` 描述当前状态（如「可用更新 v0.5.4，点击展开详情」）
- **AND** pill 支持键盘 `Enter` / `Space` 触发展开 popover
- **AND** popover 展开后焦点 SHALL 移到 popover 内第一个按钮
- **AND** `Tab` 键 SHALL 在 popover 内循环焦点


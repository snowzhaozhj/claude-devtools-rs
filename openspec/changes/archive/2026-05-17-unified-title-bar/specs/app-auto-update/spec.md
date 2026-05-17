## MODIFIED Requirements

### Requirement: 启动后台静默检查

系统 SHALL 在应用启动后**延迟 5 秒**通过 `tauri::async_runtime::spawn` 在后台调用 updater plugin 的 `check()`，且 MUST NOT 阻塞主窗口渲染。该检查 SHALL 受 `ConfigData::auto_update_check_enabled` 开关控制，开关关闭时 MUST NOT 调用 `check()`、MUST NOT emit 任何 updater event。

#### Scenario: 启动检查发现新版本

- **WHEN** 启动后 5 秒，`auto_update_check_enabled == true`
- **AND** updater 拉取 `latest.json` 拿到的版本号 > 当前版本号
- **AND** 该新版本号 ≠ `ConfigData::skipped_update_version`
- **THEN** 后端 SHALL 通过 `app.emit("updater://available", payload)` 推送事件
- **AND** 前端 `UpdateStatusPill` SHALL 在 `UnifiedTitleBar` 右侧 `zone-status` 渲染 `available` 态药丸，含 icon-download + 版本号文本
- **AND** SHALL NOT 渲染任何全宽横幅或推挤页面内容

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
- **AND** 前端 SHALL NOT 收到任何事件、SHALL NOT 显示 status pill 或错误提示

#### Scenario: 启动检查时已是最新版本

- **WHEN** 拿到的新版本号 ≤ 当前版本号
- **THEN** 后端 SHALL NOT 推送 `updater://available` 事件
- **AND** `tracing::debug!(target: "cdt_tauri::updater", current_version = %v, "no update available")` SHALL 记录

## REMOVED Requirements

### Requirement: UpdateBanner 三按钮交互

**Reason**: 全宽 `UpdateBanner.svelte` 横幅出现时整页向下推 40 px，撕裂窗口 chrome 一体感（详见 `proposal.md::Why` 与 change `unified-title-bar` design D3）。本 change 把横幅模式整体替换为右上 status pill + 按需 popover，按钮三态语义不变，承载容器改变。

**Migration**: 由本 spec 同一 delta 内的 `ADDED Requirement: UpdateStatusPill 状态机与 popover` 提供新的行为契约。`UpdateBanner.svelte` 组件文件 SHALL 删除，所有引用方 SHALL 改用 `UpdateStatusPill.svelte` + 现有 `updateStore.svelte.ts`（store 字段不变）。`update.downloadAndInstall()` / `app.restart()` / `update_config` 的 IPC 调用、`skippedUpdateVersion` 持久化、签名校验、跨平台覆盖策略保持不变。

**BREAKING 1（pill 不含 X 快捷关闭）**：旧 banner spec `Scenario: 用户关闭横幅 X 按钮`（"等价稍后提醒"语义）**不**在新 pill 上提供。Pill 视觉成本 24 px（vs banner 40 px 推页面）远小于 banner，"必须秒撤"需求消失；用户要忽略本次更新 SHALL 展开 popover 选「稍后提醒」或「跳过此版本」明确表态。Pill 本身只承载状态显示与 popover 触发，无独立"关闭"动作。

**BREAKING 2（取消下载移除）**：旧 banner spec `Scenario: 下载过程中关闭横幅`（弹「确定取消下载？」确认对话框 → 中断下载、清理临时文件）**不**在新 popover 提供。Tauri 2 `tauri-plugin-updater` 当前 JS API `update.downloadAndInstall()` 不暴露 mid-download `AbortSignal`，前端无法原子地中断下载并清理临时文件。下载启动后 SHALL 由其自然完成、失败或安装；downloading 中关闭 popover 仅隐藏 UI，pill 持续显示进度。后续上游 plugin 加 cancel API 后开 follow-up change 恢复，已记入 `openspec/followups.md`。

## ADDED Requirements

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

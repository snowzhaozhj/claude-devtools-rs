## Context

claude-devtools-rs 当前发版流程：开发者在本地打 `vX.Y.Z` tag → push → `.github/workflows/release.yml` 触发 `tauri-action@v0` 矩阵构建（macOS arm64 / macOS x64 / Linux ubuntu-24.04 / Windows）→ Draft Release with bundles。用户从 GitHub Release 页面手动下载 `.dmg` / `.deb` / `.AppImage` / `.msi` 安装。

现状缺陷：
- 老用户感知不到新版本，必须主动访问 GitHub repo 才知道有更新
- 高频迭代期（v0.2.x）每次升级都要重走「下载 → 关闭旧版本 → 安装新版本」三步操作，体验远不如同类桌面工具
- TS 原版的 Electron `autoUpdater` 已实现该能力（参见 `../claude-devtools/src/main/services/AutoUpdater.ts`），是该应用类目的事实标准

技术约束：
- Tauri 2 官方提供 `tauri-plugin-updater`，与 `tauri-action` 集成成熟
- `tauri-action` 在检测到 `TAURI_SIGNING_PRIVATE_KEY` env 时自动签名所有 bundle、生成 `latest.json` manifest 并 attach 到 GitHub Release assets
- minisign 签名校验在 plugin 内置，无需自己写校验逻辑
- updater 不支持 `.deb` / `.rpm` 包（Tauri 限制：只支持 macOS `.app`/`.dmg`、Windows `.msi`/`.nsis`、Linux `.AppImage`）

## Goals / Non-Goals

**Goals:**
- 启动后台静默检查更新，发现新版本时友好提示用户
- 设置面板提供手动「检查更新」按钮，无网络/无更新时给出明确反馈
- 设置面板提供「自动检查更新」开关，用户可以关闭启动后台检查（手动检查仍可用）
- 三按钮交互：立即更新（下载 + 重启）/ 稍后提醒（关闭横幅，下次启动再检查）/ 跳过此版本（持久化版本号，命中即跳过）
- 跨平台覆盖 macOS / Windows / Linux AppImage；Linux `.deb` 用户不影响首次安装但不享受应用内更新
- 签名密钥治理流程文档化，避免私钥丢失导致老用户无法升级的灾难
- 发版流程改动最小化（理想：只在 release.yml 加 env、tauri.conf.json 加 3 个字段）

**Non-Goals:**
- 不做差量更新 / 增量补丁（Tauri updater 默认走全量包替换，差量是 wishlist 但不在本 change 范围）
- 不做更新通道分流（stable / beta / nightly）—— 当前只有 stable 一条线
- 不实现自定义 update server（坚持方案 A: GitHub Release `latest.json`）
- 不在浏览器调试模式（`?mock=1`）下走真实 updater 流程；mockIPC 只 stub `check_for_update` 返回 fixture
- Linux `.deb` 包不增加 in-place 升级能力（Tauri 限制，等上游支持）

## Decisions

### D1: updater plugin 选型 —— 用官方 `tauri-plugin-updater`

**候选：**
- (a) `tauri-plugin-updater`（Tauri 官方）
- (b) 自撸 + `reqwest` 拉 release manifest + `self_update` crate 替换二进制
- (c) 第三方框架（electron-updater 风格的 Rust 实现，几乎不存在生态）

**选 (a)。**

**理由：**
- 与 `tauri-action` 零配置集成，发版流程改动最小
- minisign 签名校验内置，安全性默认正确
- 跨平台 in-place 替换 + 自启逻辑是 Tauri 维护，不用自己处理「Windows 旧 exe 在用所以无法覆盖」「macOS DMG 解压 + 复制到 /Applications」「AppImage chmod +x + 替换原文件」这些平台陷阱
- 与现有 `tauri-plugin-notification` / `tauri-plugin-log` 风格一致

**风险：**
- 强绑定 Tauri updater protocol，未来若要切自建分发需要重写——但当前没有这种规划
- minisign 签名格式与 `signify` / OpenBSD 互通但与 GPG 不互通，无法复用既有 GPG key

### D2: update manifest endpoint —— GitHub Release `latest.json`

**候选：**
- (a) `https://github.com/<repo>/releases/latest/download/latest.json`
- (b) GitHub Pages 静态站托管 `latest.json`
- (c) 自起 Cloudflare Worker / Vercel function 动态返回 manifest

**选 (a)。**

**理由：**
- `tauri-action` 在 build 时自动生成 `latest.json` 并 attach 到 Release assets，零额外 step
- GitHub `releases/latest/download/<asset>` 是稳定 redirect，永远指向最新 published release（draft 不算）
- 不引入额外基础设施 / 部署 / DNS / 维护成本
- 如果未来要换分发渠道（CDN 加速、国内镜像），endpoint 可改为多个并配置 fallback（updater plugin 支持 endpoints 数组）

**风险：**
- GitHub Release 在中国大陆访问偶发不稳定 → mitigation：updater plugin 支持 endpoints 数组，未来可加镜像 endpoint
- Draft release 不会被 `releases/latest/download/...` 指向 → 当前 release.yml 默认 `releaseDraft: true`，需要手动 publish 才生效；这是 feature 不是 bug（避免误推未审核版本）

### D3: 签名密钥治理 —— 一次性本地生成 + Secrets 注入 + 多副本备份

**流程：**

1. 维护者本地一次性运行 `cargo tauri signer generate -w ~/.tauri/claude-devtools-rs.key`（带密码保护）
2. 公钥内容（base64 编码的 minisign public key）写入 `tauri.conf.json::plugins.updater.pubkey`，commit 入库
3. 私钥**绝不**入 git；私钥文件 + 解锁密码同时保存到密码管理器（1Password / Bitwarden / KeePass），并在物理介质（U 盘）多副本备份
4. GitHub repo Settings → Secrets and variables → Actions：
   - `TAURI_SIGNING_PRIVATE_KEY`：私钥文件**全文**（包括 `untrusted comment:` 行；不是 base64 内容）
   - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`：解锁密码
5. `release.yml` 在 `tauri-action` step 的 env 注入这两个变量，`tauri-action` 自动用其签名

**风险：私钥丢失或泄露**
- 丢失 → 已发布版本 ≤ 当前版本的用户**永远无法升级**（updater 只信公钥，公钥已 commit 不可改；理论上可以发新公钥但所有老版本都信旧公钥所以必须手动重装）
- 泄露 → 攻击者可以签恶意更新分发给所有老用户
- **Mitigation：** 密码管理器 + 物理介质双副本；私钥文件命名 `claude-devtools-rs-updater-key-DO-NOT-LOSE.key`；README 维护章节加大字号警告
- **次坏路径：** 万一私钥丢了，下次发版前临时方案是禁用 updater（前端横幅文案改为「请到 GitHub Release 手动下载」），并在所有老版本里推一次「最后一更」公告引导用户去 GitHub 重装新公钥版本

### D4: 启动检查时机 —— 延迟 5 秒后台 spawn

**候选：**
- (a) `tauri::async_runtime::spawn` 立即检查
- (b) 延迟 5 秒（避开冷启动期）
- (c) 用户首次切换 tab 后才检查

**选 (b)。**

**理由：**
- 冷启动期（前 3-5 秒）应用要做 `LocalDataApi::new_with_*` + 项目扫描 + IPC handler 注册，CPU/网络都吃紧；updater 检查是低优先级，让一让
- 5 秒已足够覆盖大多数冷启动场景，但又比 (c) 快得多——用户启动应用就看到「有更新」横幅是合理体验
- 5 秒不会让用户感到「应用启动很慢」（用户在前 5 秒已经在和应用交互了）

**实现：** `tauri::async_runtime::spawn` 启 task，`tokio::time::sleep(Duration::from_secs(5)).await`，然后调用 updater plugin 的 `check()`

### D5: 三按钮交互 + 跳过版本持久化

**UI 形态：** 顶部横幅（参考原版 `../claude-devtools/src/renderer` 中的 update banner，若有则直接移植；否则按本设计实现）

**横幅内容：**
- 标题：`发现新版本 vX.Y.Z`
- 副标题：当前版本 → 新版本
- 折叠区：release notes（取 `latest.json` 的 `notes` 字段，markdown 渲染）
- 三按钮：
  - **立即更新**：调用 `update.downloadAndInstall()`，下载进度显示在按钮上（如 `更新中 32%`），完成后调用 `app.restart()`
  - **稍后提醒**：关闭横幅，本次启动期间不再弹；下次启动重新检查
  - **跳过此版本**：调用 `update_config({ skipped_update_version: "X.Y.Z" })` 持久化，关闭横幅；下次启动检查若拿到的还是同一版本则跳过

**跳过版本字段：**

- 字段：`ConfigData::skipped_update_version: Option<String>`
- 序列化：`#[serde(default, skip_serializing_if = "Option::is_none", rename = "skippedUpdateVersion")]`
- 命中跳过逻辑：启动检查拿到 `update.version()` 后比较 `config.skipped_update_version`，相等则不弹横幅但仍记录到 `tracing::info!`（方便 debug）
- 用户检查到更新版本号 `>` 跳过版本号时（语义版本比较）自动失效跳过状态——这块用 `semver` crate 做严格比较

**字段持久化原因（vs localStorage）：**
- 数据 SHALL 跨设备同步——但当前 `ConfigData` 也只存本地，所以这点不是决定因素
- 真实理由：`ConfigData` 已经是配置中心，新加字段走既有路径（`update_config` IPC）；localStorage 在 Tauri webview 里持久化跨进程语义不清晰，且 mock 环境难处理

### D6: 跨平台覆盖

| 平台 | bundle 类型 | updater 支持 | 备注 |
|------|------------|--------------|------|
| macOS arm64 | `.dmg` / `.app` | ✓ | 应用内 in-place 替换 + 自启 |
| macOS x64 | `.dmg` / `.app` | ✓ | 同上 |
| Windows | `.msi` / `.nsis exe` | ✓ | 替换 `.exe` 后自启 |
| Linux AppImage | `.AppImage` | ✓ | 替换 AppImage 文件 + chmod + 自启 |
| Linux .deb | `.deb` | ✗ | Tauri 限制；用户用 apt 包管理器升级或手动下载新 .deb |

**`.deb` 用户处理策略：**
- 启动检查检测到当前 bundle 是 .deb（运行时通过 `std::env::current_exe()` 路径判断不可靠；改用 platform-specific build flag 或检测 `/usr/bin/` 路径前缀）→ 横幅文案降级为「检测到新版本 vX.Y.Z，请到 [GitHub Release](link) 下载新 .deb 包安装」，三按钮变两按钮（去掉「立即更新」）
- **简化方案（v1）：** 不做 `.deb` 检测；如果 updater plugin 在 .deb 上调 `downloadAndInstall()` 失败，捕获错误，前端显示「自动更新不支持当前安装方式，请到 GitHub Release 手动下载」+ 跳转链接。这样 v1 实现成本最低，体验也可接受
- **采纳：简化方案 v1**

### D7: IPC 协议形态 —— `check_for_update` invoke + 多个 Tauri event

**Invoke command（前端 → 后端，请求-响应）：**
- `check_for_update() -> CheckUpdateResult`
  - `CheckUpdateResult` enum：`UpToDate { current_version }` / `Available { current_version, new_version, notes, signature_ok }` / `Error { message }`
  - 用于 SettingsView 手动检查按钮（按下 → 立即查 → 立即返回结果）

**Tauri event（后端 → 前端，推送）：**
- `updater://available`：启动检查发现新版本时推送，payload 同 `Available`
- `updater://download-progress`：下载中推送 `{ chunk_length, content_length, downloaded }`
- `updater://download-finished`：下载完成
- `updater://error`：异步流程任意阶段出错

**为什么混合：**
- 启动检查是后台静默推送场景，event 模式更自然
- 手动检查需要立即响应（用户按下按钮要看到 loading → 结果），invoke 更直接
- 下载进度需要持续推送，必须 event
- 与现有 `notification-update` / `session-metadata-update` event 模式一致

**IPC 字段约定同步（硬约束）：**
- `crates/cdt-api/tests/ipc_contract.rs::EXPECTED_TAURI_COMMANDS` 加 `"check_for_update"`
- `ui/src/lib/tauriMock.ts::KNOWN_TAURI_COMMANDS` 同步
- `src-tauri/src/lib.rs::invoke_handler!` 同步
- `CheckUpdateResult` 用 `#[serde(tag = "status", rename_all_fields = "camelCase", rename_all = "snake_case")]`，前端按 `result.status` switch

### D8: 失败/网络错误处理

**启动检查失败：**
- 网络错误 / endpoint 503 / DNS 失败：静默吞掉，记 `tracing::warn!(target: "cdt_tauri::updater", error = ?, "startup update check failed")`，**不弹横幅**（避免离线用户每次启动都被打扰）
- 签名校验失败：弹严重错误对话框「更新包签名无效，可能存在中间人攻击。请到 GitHub Release 验证后手动下载」+ 上报 `tracing::error!`

**手动检查失败：**
- 网络错误：横幅式短消息「检查更新失败：网络错误」+ 重试按钮
- 签名校验失败：同启动检查（严重对话框）

**下载失败：**
- 网络中断：进度条停在断点，按钮变「重试下载」，可重试
- 磁盘空间不足 / 权限错误：弹错误对话框 + 跳转打开下载目录

### D10: 允许用户禁用自动检查

**字段：** `ConfigData::auto_update_check_enabled: bool`，序列化字段名 `autoUpdateCheckEnabled`，默认值 `true`

**为什么 `bool` 而非 `Option<bool>`：**
- 该 setting 必须有明确默认值（启用），用 `Option<bool>` 时 `None` 与 `Some(true)` 语义重合，反而更绕
- 用 `#[serde(default = "default_auto_update_enabled")]` + 函数返回 `true`，老配置缺字段反序列化为启用，符合预期
- 与原版 claude-devtools 风格一致（`updater.checkForUpdatesOnStartup: boolean`）

**作用范围：**
- **仅影响启动后台静默检查**：`auto_update_check_enabled == false` 时，启动 5s 后台 task 直接 return，不调 `app.updater().check()`，不 emit `updater://available`
- **不影响手动检查**：`SettingsView` 的「检查更新」按钮无论开关状态都可用——用户主动按按钮就是想知道
- **不影响已弹出的横幅**：如果用户在启动检查弹出横幅后再去关闭开关，横幅不会消失（已经在显示中），但下次启动不再后台检查

**UI 形态：**
- `SettingsView`「关于 / 更新」分组里第一行就是开关：「启动时自动检查更新」（默认开），切换立即写 config（乐观更新模式，参考 CLAUDE.md 既有约定）
- 开关下方是「检查更新」按钮 + 当前版本号 + 跳过版本号管理

**与 `skipped_update_version` 的关系：**
- 两个字段独立。用户可以：开自动检查 + 跳过 v0.3.0；也可以：关自动检查 + 不跳过任何版本
- 关自动检查不会清空 `skipped_update_version`——用户重新打开自动检查时跳过状态仍生效

**风险：**
- 用户关掉后忘记手动检查，落后好几个版本——可接受，这是用户自主选择
- 隐私敏感场景（公司内网 / 完全离线）开关给出明确退路，符合用户预期

### D9: `mock=1` 调试模式行为

`tauriMock.ts` 加 `check_for_update` stub：
- 默认返回 `UpToDate`
- 通过 URL 参数 `?mock=1&update=available` 切换为返回 `Available { ... fixture }`
- 不模拟 event 推送（启动检查 event 在 mock 模式下不触发）；如需端到端 UpdateBanner 测试，前端单测直接 mock event listener

## Risks / Trade-offs

[**风险：私钥丢失导致老用户无法升级**] → mitigation 见 D3：密码管理器 + 物理介质双副本；README 加显眼警告；首次发版前演练「丢失场景应急方案」

[**风险：`tauri-action` 行为变更打破现有签名流程**] → mitigation：`release.yml` 锁版本（当前 `@v0` 跟随主分支，可考虑改 `@v0.5.x` 或具体 commit hash）；首次发版后写一份「实测可行流程」snapshot 到 README

[**风险：GitHub Release 在大陆访问不稳定**] → mitigation：updater plugin endpoint 支持数组，未来可加 fastgit / cf-mirror endpoint；本 change 范围不实现

[**风险：updater plugin 升级到 `2.x` 大版本时 IPC 协议变化**] → mitigation：跟 `tauri = "2"` 主版本同步升级；CHANGELOG 必看

[**Trade-off：跳过版本号字段进 `ConfigData`**] vs **独立 file** → 选 ConfigData：复用既有 IPC + 持久化基础设施；代价是 `update_config` 偶发非必要写盘——但写盘频率（每次跳过版本一次）远低于 trigger / SSH host 编辑，无性能问题

[**Trade-off：启动检查延迟 5s**] vs **延迟 30s / 用户交互后** → 选 5s：平衡冷启动让让和响应速度；如果实测启动期间网络/CPU 负载仍重，下一版可改为 `tauri::Event::Ready` 事件 + 二次延迟

[**Trade-off：Linux .deb 不享受应用内更新**] → 接受，写明文档；如果实际 .deb 用户占比不可忽略，下一版做精确平台检测 + 降级横幅

## Migration Plan

**部署顺序（首次发版前必须按序完成）：**

1. **本地一次性密钥生成**（任何 `cargo tauri build` 之前）
   - `cargo tauri signer generate -w ~/.tauri/claude-devtools-rs.key`
   - 私钥文件 + 密码立即三处备份（密码管理器 + U 盘 + 离线纸质）
2. **GitHub Secrets 配置**
   - `TAURI_SIGNING_PRIVATE_KEY` = 私钥**全文**
   - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` = 密码
3. **代码改动**（本 change 的 tasks.md 覆盖）
4. **首次签名发版演练**
   - 打 `v0.3.0-rc.1` tag → CI 跑通 → Draft Release 检查 `latest.json` 是否包含、是否签名、签名 base64 是否非空
   - 手动 publish draft → 用 `v0.2.x` 应用打开 → 应该弹更新横幅（v0.2.x 没集成 updater 所以这步会跳过；正式 v0.3.0 之后才能验证）
5. **回滚预案**
   - 如果 `v0.3.0` 出严重 bug，**不要**用 yank/delete release（旧用户已经升级）；走标准 hotfix 流程发 `v0.3.1`
   - 如果 updater 流程本身炸了（签名校验失败、endpoint 404），回滚 = 临时给 `v0.2.x` 老用户发邮件/issue 通知到 GitHub Release 手动下载

**回滚策略：**
- updater 配置错误（签名 / endpoint）→ 不影响老用户（他们 v0.2.x 没集成 updater），只影响 v0.3.x → v0.3.y 之间的升级；hotfix v0.3.y 修配置即可
- 私钥泄露 → 立即 rotate：发新公钥版本 v0.4.0 + GitHub Release 加显眼公告「请手动下载 v0.4.0」+ 撤销旧 Secrets

## Open Questions

1. **是否需要 release notes 渲染 markdown？** —— 当前设计假设是。`marked` + `dompurify` 在前端已存在依赖，零成本。决定：✓ 渲染
2. **横幅是否需要折叠/可展开？** —— release notes 可能很长。决定：默认折叠 release notes 段（只显示前两行 + 「展开」），三按钮始终可见
3. **下载进度数据源走前端直调还是后端中转？** —— 决定：v1 走前端直调 plugin JS API（路径 A），后端不推 `updater://download-progress` event。理由：当前没有 Rust 侧业务逻辑要在下载过程介入；少一层 IPC 转发，进度数据天然在前端 reactive。如果未来要加下载 audit log / 业务勾子，再切到路径 B
4. **mockIPC 对 updater 的覆盖深度？** —— 决定：D9 已定方案，URL 参数控制，不模拟 event

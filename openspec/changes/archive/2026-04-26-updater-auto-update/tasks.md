## 1. 一次性签名密钥治理（手动，本地外执行）

- [ ] 1.1 维护者本地运行 `cargo tauri signer generate -w ~/.tauri/claude-devtools-rs.key`，设强密码
- [ ] 1.2 私钥文件 `claude-devtools-rs.key` + 解锁密码同时备份到密码管理器（1Password / Bitwarden 等）
- [ ] 1.3 物理介质再做一份离线副本（U 盘 / 加密硬盘），命名带 `DO-NOT-LOSE` 提示
- [ ] 1.4 `.tauri/` 与所有衍生 key 文件确认已在 `.gitignore` 中（**不入 git**）
- [ ] 1.5 GitHub repo Settings → Secrets and variables → Actions 添加：
  - `TAURI_SIGNING_PRIVATE_KEY` = 私钥文件全文（含 `untrusted comment:` 行）
  - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` = 解锁密码

## 2. `crates/cdt-config`：`ConfigData` 加 updater 相关字段

- [x] 2.1 新增 `UpdaterConfig` struct（`auto_update_check_enabled: bool` 默认 `true` + `skipped_update_version: Option<String>`），`AppConfig` 加 `updater: UpdaterConfig` 字段，`ConfigSection` 加 `Updater` variant
- [x] 2.2 `ConfigManager::update_updater` 方法支持 `autoUpdateCheckEnabled`（bool）和 `skippedUpdateVersion`（含 set 与 clear 为 null）；未知键 warn 但不报错
- [x] 2.3 单测覆盖：默认启用 / 缺字段合并默认 / 切关后 reload 持久化 / 跳过版本 set 然后 clear / 序列化缺省字段不出现 / 未知键忽略
- [x] 2.4 `cargo test -p cdt-config` 通过（109 测试）

## 3. `src-tauri`：updater plugin 依赖与配置

- [x] 3.1 `src-tauri/Cargo.toml` 加依赖：`tauri-plugin-updater = "2"`、`semver = "1"`、`tracing = "0.1"`
- [x] 3.2 `src-tauri/tauri.conf.json` 加配置：`bundle.createUpdaterArtifacts: true` + `plugins.updater.endpoints` + `plugins.updater.pubkey: ""`（**占位空串，发版前必须替换为真公钥**）
- [x] 3.3 `src-tauri/capabilities/default.json::permissions` 加 `"updater:default"`
- [x] 3.4 `src-tauri/src/lib.rs::run()` 链上 `.plugin(tauri_plugin_updater::Builder::new().build())`
- [x] 3.5 `cargo build --manifest-path src-tauri/Cargo.toml` 通过

## 4. `src-tauri`：启动 5s 后台静默检查

- [x] 4.1 `setup` 中 `tauri::async_runtime::spawn` 5s 后 task → 读 config gate（`autoUpdateCheckEnabled`，缺省 true）→ 关闭则 `tracing::debug!` return → 否则调 `updater().check()` → 命中 `skippedUpdateVersion`（semver 比较）则 `tracing::info!` return → 否则 emit `updater://available`。实现拆到 `run_startup_update_check` 函数
- [x] 4.2 错误静默：config 读失败 / updater init 失败 / check 失败均 `tracing::warn!(target = "cdt_tauri::updater")` 不弹横幅
- [x] 4.3 emit payload 用 `serde_json::json!` 直接 camelCase（`currentVersion / newVersion / notes / signatureOk`）

## 5. `src-tauri`：`check_for_update` invoke command

- [x] 5.1 `check_for_update` async fn 直接定义在 `src-tauri/src/lib.rs`（与既有 commands 风格一致，未拆 module）
- [x] 5.2 `CheckUpdateResult` enum 三 variant，`#[serde(tag = "status", rename_all = "snake_case", rename_all_fields = "camelCase")]`
- [x] 5.3 实现忽略 `skipped_update_version`（用户主动按按钮即视为同意检查），signature_ok 默认 true（plugin 已校验签名才返回 Some）
- [x] 5.4 `invoke_handler!` 注册 `check_for_update`
- [x] 5.5 `EXPECTED_TAURI_COMMANDS` + count meta 测试同步到 23；`KNOWN_TAURI_COMMANDS` 同步；ipc_contract test 通过

## 6. `src-tauri`：下载进度 / 错误 event 推送

- [x] 6.1 v1 决定：前端直接调 `@tauri-apps/plugin-updater` JS API，后端不中转 download progress event
- [x] 6.2 design.md Open Questions Q3 已记录该决策与切换路径
- [x] 6.3 后端 `lib.rs` 不实现 `download_update` IPC，前端走 plugin JS API（S8 实施）

## 7. UI：`UpdateBanner.svelte` 组件

- [x] 7.1 移植原版 `UpdateBanner.tsx` + `UpdateDialog.tsx` 思路，CSS 变量沿用项目现有 token；v1 合为单个 banner 组件
- [x] 7.2 `ui/src/components/UpdateBanner.svelte` 新建：四态横幅（available/downloading/downloaded/error）+ 三按钮（立即/稍后/跳过）+ 关闭 X + release notes 折叠（>120 字符显示「展开」）
- [x] 7.3 `App.svelte` 顶层挂 `<UpdateBanner />`，新加 `app-root` 包裹 banner + 既有 layout
- [x] 7.4 `ui/src/lib/updateStore.svelte.ts` 用 Svelte 5 `$state` class，管理 status / 版本号 / notes / 进度 / visible / errorMessage

## 8. UI：updater event 监听 + plugin JS API 调用

- [x] 8.1 `App.svelte::onMount` `listen<UpdateAvailablePayload>("updater://available", e => updateStore.showAvailable(e.payload))`，onDestroy 释放
- [x] 8.2 立即更新：updateStore.downloadAndInstall() 内调 `check() → update.downloadAndInstall(event)`，进度回调按 Started/Progress/Finished 写 store
- [x] 8.3 下载完调 `relaunch()`（@tauri-apps/plugin-process）
- [x] 8.4 .deb 错误捕获在 UpdateBanner.handleInstall：弹 alert 跳转 GitHub Release（v1 简化方案，原版的 dialog 后续移植）
- [x] 8.5 `ui/package.json` 装 `@tauri-apps/plugin-updater` + `@tauri-apps/plugin-process`；后端同步加 `tauri-plugin-process` + `process:default` capability

## 9. UI：「稍后提醒」/「跳过此版本」/ 关闭 X 行为

- [x] 9.1 「稍后提醒」：updateStore.remindLater() → visible=false，不改 status / 不写 config
- [x] 9.2 「跳过此版本」：updateStore.skipVersion() → invoke `update_config { section: "updater", configData: { skippedUpdateVersion: version } }`，失败回滚 visible
- [x] 9.3 关闭 X：handleClose → updateStore.dismiss() = 关 banner + status 回 idle（非 downloading）
- [x] 9.4 下载中关闭 X：UpdateBanner.handleClose 弹 `confirm()` 让用户确认（取消下载在 v1 暂未实现 abort，仅关 banner 让 task 自然完成）

## 10. UI：`SettingsView` 加「关于 / 更新」分组

- [x] 10.1 `ui/src/routes/SettingsView.svelte` 新增「关于 / 更新」tab section
- [x] 10.2 第一行：「启动时自动检查更新」`SettingsToggle` 绑定 `config.updater.autoUpdateCheckEnabled`，乐观更新（updateUpdater 函数调 `update_config "updater"` 失败 `getConfig` 回滚）
- [x] 10.3 第二行：当前版本（`getVersion()`）+「检查更新」按钮，按 `result.status` 显示三态文案；available 时同时写 store 让横幅展示
- [x] 10.4 第三行：跳过版本号 + 「清除跳过」按钮（仅 `config.updater.skippedUpdateVersion` 非空时显示），调 update_config 置 null

## 11. UI：mock 模式与单测

- [x] 11.1 `tauriMock.ts::KNOWN_TAURI_COMMANDS` 加 `"check_for_update"`
- [x] 11.2 mock 实现：默认 `{ status: "up_to_date", currentVersion: "0.2.0" }`；URL `?update=available` 返回 Available fixture（含 release notes）
- [x] 11.3 `updateStore.test.ts` 5 个单测：showAvailable 字段写入 / remindLater 保留 status / dismiss 回 idle / skipVersion 成功调 IPC / skipVersion 失败回滚 visible（mock plugin-updater + plugin-process + invoke）
- [x] 11.3a 后端 `cdt-config` 已覆盖 `auto_update_check_enabled` 写入与持久化（S2 单测）；前端 SettingsView 开关交互在 mockIPC 链路通；端到端在 Playwright 后续覆盖
- [ ] 11.4 `npm run test:unit --prefix ui` 通过

## 12. `release.yml`：注入签名 env

- [x] 12.1 `release.yml::tauri-action` step env 加两个 Secrets 引用
- [x] 12.2 YAML 改动最小（只在已有 env: 块下加两行），下次实际打 tag 验证；本 PR 内不模拟

## 13. 文档

- [x] 13.1 README「发布流程」段后追加「应用内自动更新（首次发版前必读）」5 步操作清单
- [x] 13.2 README 加显眼警告：私钥丢失 = 老用户永远无法应用内升级，多副本备份
- [x] 13.3 CLAUDE.md「UI 层 - 陷阱」加单条覆盖：配置链一致性、私钥不可换、.deb 降级、check_for_update IPC 同步三处、event 命名约定 `updater://available`

## 14. preflight 与 PR

- [x] 14.1 `cargo fmt --all`
- [x] 14.2 `cargo clippy --workspace --all-targets -- -D warnings` 通过
- [x] 14.3 `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` 通过
- [x] 14.4 `cargo test --workspace` 通过（cdt-config 109 测；ipc_contract 含 23 个 command 守护）
- [x] 14.5 `npm run check --prefix ui` 通过（0 errors，已有 5 warnings 不在本次修改范围）
- [x] 14.6 `npm run test:unit --prefix ui` 通过（54/54）
- [x] 14.7 `openspec validate updater-auto-update --strict` 通过
- [ ] 14.8 同一 PR 内 archive：`openspec archive updater-auto-update -y` 作为最后一个 commit
- [ ] 14.9 推 PR 走 review → merge

## 15. 首次发版演练（merge 后）

- [ ] 15.1 打 `v0.3.0-rc.1` tag → push → 等 release.yml 跑完
- [ ] 15.2 检查 Draft Release assets：`latest.json` 存在、`signature` 字段非空、各平台 bundle + `.sig` 配对
- [ ] 15.3 用 v0.2.x 安装的应用打开 → 不应触发更新（因为 v0.2.x 没集成 updater）
- [ ] 15.4 publish v0.3.0-rc.1 → 安装到测试机
- [ ] 15.5 打 `v0.3.0-rc.2` tag → 等 release publish → 测试机的 v0.3.0-rc.1 应用应弹更新横幅
- [ ] 15.6 验证三按钮：立即更新走通 / 稍后提醒下次启动再弹 / 跳过此版本写入 config 后启动不再弹
- [ ] 15.7 验证手动检查：SettingsView 「检查更新」三个分支均显示正确文案
- [ ] 15.8 演练通过后正式发 `v0.3.0`

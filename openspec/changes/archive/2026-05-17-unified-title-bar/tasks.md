## 1. 新组件骨架

- [x] 1.1 新增 `ui/src/components/UnifiedTitleBar.svelte`：四 zone flex 布局 + macOS UA 判定 + `data-tauri-drag-region` 声明 + 高度 44 px CSS 变量化（`--chrome-height: 44px`）
- [x] 1.2 新增 `ui/src/components/UpdateStatusPill.svelte`：消费 `updateStore`，5 态机渲染（idle 不渲染、available/downloading/downloaded/error 各形态）、`aria-label` 描述当前状态、`Enter` / `Space` 键盘触发
- [x] 1.3 新增 `ui/src/components/UpdatePopover.svelte`：从 UpdateStatusPill 拆出的 popover 子组件，包含版本号 + release notes markdown 渲染 + 三按钮（立即更新 / 稍后提醒 / 跳过此版本） + downloading 态下载进度条（仅展示，无取消按钮——详见 spec REMOVED Migration BREAKING 2） + `Esc` 关闭 + 焦点循环 + outside-click 关闭 + `$effect` 监听 `updateStore.status === "idle"` 时强制 `popoverOpen = false`（D3b idle race）
- [x] 1.4 新增 `ui/src/components/RosettaStatusIcon.svelte`：黄色三角 icon + hover tooltip + 点击展开详情或跳 release page
- [x] 1.5 新增 `ui/src/lib/icons.ts` 内补 `DOWNLOAD_ICON` / `RESTART_ICON` / `WARN_TRIANGLE_ICON` SVG path 常量（lucide 风格）

## 2. 组件迁移与抽离

- [x] 2.1 从 `ui/src/components/SidebarHeader.svelte` 抽出项目选择下拉到独立 component `ProjectSelect.svelte`，挂载到 `UnifiedTitleBar.zone-left-center`；删除 SidebarHeader 内项目下拉相关代码 + `isMac` 局部变量 + `padding-left: 76px` 与 `class:header-row-mac`
- [x] 2.2 从 `ui/src/components/SidebarHeader.svelte` 抽出 sidebar 折叠按钮到独立 component `SidebarToggleButton.svelte`，挂载到 `UnifiedTitleBar.zone-left-center`；折叠按钮 icon 随 sidebar 状态切换（展开/折叠 icon）
- [x] 2.3 SidebarHeader 简化为只承担"会话搜索框 + filter chip"语义；如简化后 SidebarHeader 仅包含搜索框，考虑直接合并到 Sidebar 顶部，删除 SidebarHeader.svelte 文件
- [x] 2.4 从 `ui/src/components/TabBar.svelte` 抽出通知按钮 + 设置按钮（如它们是 inline JSX 则提取为 `NotificationsButton.svelte` / `SettingsButton.svelte`），挂载到 `UnifiedTitleBar.zone-status`；删除 TabBar 内通知/设置相关代码 + `reserveTrafficLightSpace` derived + `padding-left: 72px` 与 `class:tab-bar-mac-collapsed`
- [x] 2.5 TabBar 内保留：tab 列表 + 折叠态"展开 sidebar 快捷按钮"（在最左）+ 自身 JS drag region；移除所有 traffic-light padding 与 `isMac` 判定

## 3. App.svelte 拓扑切换

- [x] 3.1 `ui/src/App.svelte` 顶层 `<div class="app-root">` 加入 `<UnifiedTitleBar />` 作为第一个子节点
- [x] 3.2 删除 `<RosettaBanner />` 与 `<UpdateBanner />` 渲染行（保留 store 与 listen 逻辑，由新组件接管）
- [x] 3.3 调整 `.app-root` / `.app-layout` 的 CSS：app-layout 顶部偏移由 banner 显隐 → 固定 `--chrome-height: 44px`；删除原 banner 动态高度计算

## 4. 删除老 banner

- [x] 4.1 删除 `ui/src/components/UpdateBanner.svelte`
- [x] 4.2 删除 `ui/src/components/RosettaBanner.svelte`
- [x] 4.3 全仓 grep 确认无残留 `UpdateBanner` / `RosettaBanner` import 或类型引用

## 5. 视觉加粗与分隔线收敛（D8）

- [x] 5.1 `ui/src/components/TabBar.svelte:312` active tab indicator 从 `border-bottom: 2px solid var(--color-border-emphasis)` 改为 `box-shadow: inset 0 -2px 0 var(--color-accent)`；同步调整 active tab height 计算避免 1 px 偏移
- [x] 5.2 `ui/src/routes/SessionDetail.svelte:1072` 顶部紧贴 TabBar 的 `border-bottom: 1px solid var(--color-border)` 删除；保留 `1759` 处内部章节分隔
- [x] 5.3 **audit only**：grep `ui/src/routes/` 与 `ui/src/components/` 找其它 pane content view（SettingsView / NotificationsView / DashboardView）的顶部第一个 border，逐一对照实际渲染位置：若该 border 是 view 内部章节分隔（如 settings nav 与 setting body 之间、notifications header 与 list 之间）则 SHALL 保留；仅当该 border 在 TabBar 行底紧贴下方造成 1 px 重叠加粗时才删除。codex 已确认 NotificationsView:296/301 与 SettingsView:1121/1130 是内部分隔，不动
- [x] 5.4 chrome (`UnifiedTitleBar`) 底部 `border-bottom: 1px solid var(--color-border)`，确认与 sidebar / pane TabBar 顶部无叠加

## 6. CSS 变量与平台 padding 收敛

- [x] 6.1 `ui/src/app.css` 新增 `--chrome-height: 44px`、`--chrome-mac-padding-left: 80px`、`--chrome-control-gap: 8px` CSS 变量
- [x] 6.2 删除散在 SidebarHeader / TabBar / UpdateBanner 三处的不一致 padding 数值（76 px / 72 px / 84 px），全部由 `UnifiedTitleBar` 单点用 `--chrome-mac-padding-left` 应用
- [x] 6.3 status zone 内子组件间距统一用 `--chrome-control-gap` (8 px)

## 7. 行为契约校验测试（spec scenarios）

- [x] 7.1 vitest 单测 `ui/src/components/UpdateStatusPill.test.ts`：5 态切换、idle 不渲染、available 点击触发 popover、Esc 关闭、downloading 中关闭 popover 不中断下载、aria-label 内容
- [x] 7.2 vitest 单测 `ui/src/components/UpdatePopover.test.ts`：三按钮点击行为（立即更新 / 稍后提醒 / 跳过此版本）、焦点循环 `Tab` 键、outside-click 关闭、`Esc` 关闭、popover 已展开期间 `updateStore.dismiss()` 强制关 popover 释放 listener 与焦点（D3b idle race scenario）、downloading 态 popover 内**无**「取消下载」按钮
- [x] 7.3 vitest 单测 `ui/src/components/UnifiedTitleBar.test.ts`：macOS UA 渲染 80 px platform padding、Windows UA 不渲染 platform padding、status zone 内子组件顺序、idle 态 status zone 仅 2 个按钮（通知 + 设置）
- [x] 7.4 mockIPC fixture 同步：`ui/src/lib/__fixtures__/` 内 fixture 不需要改（store 字段无变化）；确认 `tauriMock.ts::KNOWN_TAURI_COMMANDS` 无需新增

## 8. Playwright e2e

- [x] 8.1 新增 `ui/tests/e2e/unified-title-bar.spec.ts`：macOS UA viewport（默认）下截图 chrome，断言 chrome 左 padding ≥ 76 px、chrome 高度 44 px、status zone 在右上角
- [x] 8.2 同 spec 内 Windows UA viewport（`page.setExtraHTTPHeaders` + UA override）下截图 chrome，断言 chrome 左 padding ≤ 8 px
- [x] 8.3 同 spec 内模拟 `updater://available` event（通过 `window.__cdtTest` 暴露的 dev-only helper），断言 pill 出现 + 点击 pill 展开 popover + popover 包含三按钮
- [x] 8.4 同 spec 内 480 px 窄窗口断言 popover fallback 居中
- [x] 8.5 a11y snapshot：chrome 内所有 button / pill 有 `aria-label`
- [x] 8.6 在 `main.ts` dev-only `window.__cdtTest` 内补一个 `triggerUpdaterAvailable(version, notes)` helper 用于 e2e 注入 update event

## 9. 后端零改动验证

- [x] 9.1 确认 `src-tauri/src/lib.rs::invoke_handler!` 无变化、`crates/cdt-api/tests/ipc_contract.rs::EXPECTED_TAURI_COMMANDS` 无变化、`updater://available` / `updater://download-progress` event 字符串不变
- [x] 9.2 跑 `cargo test -p cdt-api --test ipc_contract` 确保 0 失败
- [x] 9.3 跑 `cargo clippy --workspace --all-targets -- -D warnings` 全绿
- [x] 9.4 跑 `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` 全绿
- [x] 9.5 跑 `pnpm --dir ui run check` 全绿（svelte-check）

## 10. 性能 / Perf 验证

- [x] 10.1 跑 `bash scripts/run-perf-bench.sh` 对比 wall / user / sys / RSS / user-real 比基线无回归（按 `.claude/rules/perf.md` 阈值，UI-only 重构预期无回归）
- [x] 10.2 PR 描述附 Perf impact 段贴四维数据；若回归 < 阈值仍写明实测变化方向

## 11. 手动 just dev 验证

- [x] 11.1 macOS：`just dev` 启动桌面应用，肉眼确认 traffic light 与 chrome 内项目下拉 / sidebar 折叠按钮垂直对齐
- [x] 11.2 macOS：手动 stub `updater://available` event（在 SettingsView「检查更新」按钮触发或通过 dev menu），观察 pill 出现 + 点击展开 popover + popover 三按钮可点 + downloading 态 popover 内**无**「取消下载」按钮（仅显示进度条）
- [x] 11.3 macOS：拖动 chrome 中央空白区，窗口正常移动；双击空白区切换最大化
- [x] 11.4 多 pane split view：`UnifiedTitleBar` 仍是一条覆盖整窗口宽度；每 pane 内 TabBar 各自显示自己的 tab；status zone 一份不复制
- [x] 11.5 切换 sidebar 折叠 / 展开：chrome 内项目下拉 + 折叠按钮位置 SHALL 不动
- [x] 11.6 macOS：sidebar 完全折叠时确认搜索功能仍可通过 `Cmd+K` Command Palette 入口完成（搜索是 chrome 之外能力，不依赖 SidebarHeader）
- [x] 11.7 浏览器调试 `pnpm --dir ui run dev` + `?mock=1`：用 DevTools 切 UA 为 Windows / Linux，确认 chrome 左 padding 消失、chrome 自身仍 44 px
- [x] 11.8 真 Windows 机器（如有）：确认 OS 原生 title bar 在 chrome 之上，UnifiedTitleBar 直接从 OS title bar 下方起；window controls 由 OS 绘制，UnifiedTitleBar 内不重复

## 12. 文档与 release notes

- [x] 12.1 `CHANGELOG.md` 加 BREAKING 段：`UpdateBanner.svelte` / `RosettaBanner.svelte` 删除，前端组件依赖 `UpdateStatusPill` / `RosettaStatusIcon`
- [x] 12.2 `CLAUDE.md` 的 "UI 层 (Tauri 2 + Svelte 5)" 段更新组件清单 + 布局描述
- [x] 12.3 `openspec/followups.md` 视需求加一条："后续可考虑统一 pane 内 TabBar drag region 改 `data-tauri-drag-region` 与 chrome 一致"（design D7 取舍）
- [x] 12.4 `openspec/followups.md` 加一条 BREAKING 跟进项："Tauri `tauri-plugin-updater` 上游若加 mid-download cancel API，开 follow-up change 在 UpdatePopover 恢复「取消下载」按钮"（来自 unified-title-bar BREAKING 2）

## N. 发布

- [x] N.1 push 分支 + 开 PR
- [x] N.2 wait-ci 全绿
- [x] N.3 codex 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [x] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）

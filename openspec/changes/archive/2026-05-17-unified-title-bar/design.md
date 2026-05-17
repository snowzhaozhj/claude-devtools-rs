## Context

应用顶部目前由 `RosettaBanner` + `UpdateBanner` + 左侧 `SidebarHeader` + 右侧每 pane 的 `TabBar` 四类组件**纵向 / 横向**堆叠构成，每个组件各自处理 macOS traffic-light padding。具体痛点见 `proposal.md::Why`。

社区参考（截图调研 + 2026-05 抓取的产品现状）：

| 应用 | 模式 | 关键技术 |
|---|---|---|
| Apple HIG | *Unified Toolbar* —— `NSWindow.titlebarAppearsTransparent = true` + `NSToolbar` 与 window controls 共享同一行 | `NSWindowStyleMask.fullSizeContentView` |
| Linear (mac) | Unified 44 px title bar，traffic lights 内嵌左侧 toolbar；右上 status zone 含 search / inbox / profile | Electron `titleBarStyle: 'hiddenInset'` + `trafficLightPosition` |
| Cursor / VSCode | Custom title bar（44 px）+ 独立 tab strip；window controls 与 toolbar 同行 | Electron `titleBarOverlay` |
| Arc Browser | Sidebar-first，traffic lights 浮在 sidebar 顶；右侧 content 区无 chrome | macOS native + 自绘 sidebar |
| Notion (mac) | Unified compact title bar，左 sidebar 切换 + page breadcrumb + 右 status | Electron `titleBarStyle: 'hidden'` |

**共同结论**：现代桌面 app 把窗口 chrome 收敛为**单条**横向 toolbar，window controls / 主导航 / status 都在同一行；瞬态横幅类组件（更新提示 / 警告）走右上 status zone 的 pill / icon + popover，**绝不**占顶部独立一行。

Tauri 2 端的能力：
- `tauri.conf.json::windows[].titleBarStyle: "Overlay"` 已开启（见 `src-tauri/tauri.conf.json:22`），macOS traffic-light 由系统绘制并定位到 `(12, 20)`
- `data-tauri-drag-region` 属性可在 DOM 任意元素声明拖窗区，无需 JS
- Windows / Linux 当前未隐藏原生 title bar，仍由 OS 绘制窗口控件

约束：
- 后端 IPC 零改动（Update / Rosetta 状态推送链路保持）
- Update 业务语义（按钮三态、跳过版本、签名校验、跨平台覆盖）不变
- Sidebar 宽度拖拽、Pin / Hide、虚拟滚动等性能机制保留
- 性能预算：UI-only 重构，冷启动 wall 不回归 > 2 ms、RSS 不回归 > 1 MB（按 `.claude/rules/perf.md`）

## Goals / Non-Goals

**Goals:**
- 应用顶部 `UnifiedTitleBar` 自身**单条 chrome**（高度恒 44 px），无任何瞬态横幅推挤页面内容
- macOS 平台：`UnifiedTitleBar` 就是窗口顶部唯一的 chrome（traffic light 由系统绘制并叠加在 chrome 左 80 px padding 区）
- Windows / Linux 平台：OS 原生 title bar（含 minimize / maximize / close 按钮）仍由 OS 在窗口最顶绘制，`UnifiedTitleBar` 直接渲染在 OS title bar 下方 44 px；这是 OS 行为差异，本 change 不为 Win/Linux 自绘窗口控件（详见 Risks）
- 三平台共同点：`UnifiedTitleBar` 自身高度 / 内部 zone 布局 / status zone 子组件 / drag region 行为完全一致；差异仅限"是否有 OS native title bar 叠加在 chrome 之上"
- 更新提示从横幅模式改为 status pill + 按需 popover：idle 态对用户**完全不可见**；available / downloading / downloaded / error 各有清晰视觉状态
- Rosetta 警告同模式归并到 status zone
- 项目选择 + sidebar 折叠按钮收敛到 chrome，让 sidebar header 只承担"会话搜索 + filter"语义
- 重构对后端零侵入：不动 IPC、不动 store 字段、不动 `EXPECTED_TAURI_COMMANDS`

**Non-Goals:**
- 不重构多 pane 模型（保留每 pane 内 TabBar；chrome 与 content tabs 职责分离）
- 不引入新的 Tauri plugin、新的 store、新的后端事件
- 不改 update 按钮语义（立即更新 / 稍后提醒 / 跳过此版本依旧三按钮，只是从横幅移到 popover）
- 不引入设计 token 体系（颜色 / 间距走现有 `app.css` 的 CSS 变量）
- 不动 Sidebar 宽度拖拽行为 / SessionDetail / SettingsView / NotificationsView
- 不做"完全 frameless"（不学 Zed）—— traffic light 仍是 macOS 系统绘制
- 不为 Windows / Linux 自绘 minimize / maximize / close 按钮（保留 OS 原生，避免分平台分支膨胀）

## Decisions

### D1: 拍平 chrome 为单条 UnifiedTitleBar（44 px）

**选择**：`App.svelte` 顶层 `<div class="app-root">` 从 `[RosettaBanner?, UpdateBanner?, app-layout]` 改为 `[UnifiedTitleBar, app-layout]`。`UnifiedTitleBar` 是必现组件，高度恒 44 px。

**候选**：
- **A**: 单条 44 px chrome（**选**）—— Apple HIG / Linear / Cursor 共识；视觉简洁；可被 status zone 灵活承载状态
- B: 保留 banner 但只修对齐 —— 治标不治本，banner 显隐仍推 40 px、撕裂 chrome
- C: 完全 frameless（Zed 模式，tab strip 即 chrome）—— 与本仓库多 pane / 多 tab 类型（session / settings / notifications）的语义复杂度不匹配；强行复用会让 tab strip 承担太多职责

**取舍**：
- chrome 高度 44 px：HIG unified toolbar 最小 28 px，标准 38 px；本仓库需要让 traffic light（macOS 系统按钮 12 px 直径）+ 项目下拉（行高 28 px）+ status pill（高度 24 px）三者垂直居中并留 8 px 上下气，44 px 是最小整数解
- chrome 高度 44 px **vs** 旧 SidebarHeader 40 px：净增 4 px 顶部占用，但**净减** 一个 UpdateBanner 40 px（出现时）+ 一个 RosettaBanner 40 px（出现时）+ 一个 TabBar 仅作 chrome 用的 40 px 角色冗余

### D2: 项目下拉 + sidebar 折叠按钮上移到 UnifiedTitleBar 左中 zone

**选择**：从 `SidebarHeader.svelte` 抽出项目选择下拉与 sidebar 折叠按钮，挂载到 `UnifiedTitleBar` 的 `<div class="zone-left-center">`。SidebarHeader 仅保留会话搜索框 + filter chip。

**候选**：
- A: 上移到 chrome（**选**）—— 与 Notion / Linear "page-level breadcrumb 在 title bar" 一致；释放 sidebar 第一行做搜索框，搜索/filter 与会话列表视觉连贯
- B: 留在 SidebarHeader 内但 padding 对齐 traffic light —— 仍有"两段 40 px"撕裂问题；右侧 TabBar 也得同步调，分散
- C: 把项目下拉放到 status zone 右侧 —— 与"选择项目"是主导航语义冲突（status zone 应为辅助状态）

**取舍**：
- sidebar 折叠时，"项目下拉 + 折叠按钮" 仍在 chrome 可见；展开时也在 chrome 同位置，**不**跟随 sidebar 宽度变化。这与 Linear 行为一致——主导航控件锚定在 chrome，不随 sidebar 抖动
- 折叠按钮 icon 行为：折叠态 = 展开 icon，展开态 = 折叠 icon（与现有 `SidebarHeader.svelte` 同语义，迁移即可）

### D3: UpdateBanner 改 UpdateStatusPill + popover

**选择**：新建 `UpdateStatusPill.svelte` 挂在 status zone。状态机 5 态：

| 状态 | pill 形态 | 触发 |
|---|---|---|
| `idle` | **不渲染** | 无 update event 或 store.status == idle |
| `available` | `<icon-download> v0.5.4` 蓝色边框 | `updater://available` event |
| `downloading` | `<ring-progress 32%> 32%` 环形进度替代 icon | `updater://download-progress` event |
| `downloaded` | `<icon-restart> 重启更新` 绿色填充 | download 完成 |
| `error` | `<icon-warn> !` 红色填充 | download / 签名校验失败 |

点击 pill 打开 popover（width 360 px，右上角锚定）承载：版本号 + release notes（markdown）+ 三按钮（立即更新 / 稍后提醒 / 跳过此版本）。点击 pill 外或按 `Esc` 关闭 popover。Popover 关闭 ≠ 中断下载——pill 持续显示 downloading 态。

**候选**：
- A: status pill + popover（**选**）—— 业界惯例；idle 态对用户不可见；available 态以**最小视觉成本**告知（24 px 高度 vs 横幅 40 px）；popover 承载 release notes 避免在 chrome 内堆积
- B: 保留横幅但缩为 24 px —— 仍占独立一行推页面；release notes / 三按钮在 24 px 行装不下，得截断或滚动
- C: 改用 toast 通知（屏幕右下浮窗）—— Tauri-plugin-notification 已用于系统通知；二者混用语义混乱；toast 自动消失会让用户错过 release notes
- D: 改命令面板 `Cmd+K` 入口 —— 用户不主动查就发现不了更新；违背"主动告知新版本"语义

**取舍**：
- popover 位置：默认右贴 chrome 右边缘，向下展开；窗口宽度 < 640 px 时 fallback 到 viewport 居中
- popover 内"立即更新"按钮按下后，popover **不自动关**，进入 downloading 态由 popover 内进度条 + chrome pill 双重显示；用户可点 popover 外关闭让 pill 接管显示
- "稍后提醒" / "跳过此版本" 行为与 Scenario 完全对齐 `app-auto-update::UpdateBanner 三按钮交互` 现有按钮语义，仅按钮所在容器从 banner 改 popover
- **BREAKING 1：pill 本身不带 X 快捷关闭按钮**。旧 banner spec scenario 「用户关闭横幅 X 按钮 = 等价稍后提醒」语义**不**在新 pill 上保留。理由：pill 视觉成本 24 px 高（vs banner 40 px 推页面），用户感知到的"干扰"远小于横幅，"必须秒撤"的需求不成立。要暂时忽略本次更新，用户 SHALL 展开 popover 选「稍后提醒」或「跳过此版本」明确表态。这降低误操作（横幅 X 一不小心点掉但用户其实想看 release notes）。
- **BREAKING 2：取消下载功能移除**。Tauri 2 `tauri-plugin-updater` 当前 JS API `update.downloadAndInstall()` 不暴露 mid-download `AbortSignal`（已验证 plugin 源码无 cancel API），无法在前端原子地中断下载并清理临时文件。旧 banner spec scenario「下载过程中关闭横幅」+「确定取消下载？」对话框路径**不**在新 popover 保留。下载启动后用户只能等其自然完成或失败；downloading 中关闭 popover 仅是隐藏 UI，pill 持续显示进度。后续可在 `tauri-plugin-updater` 上游加 cancel API 后开 follow-up change 恢复此能力（已记入 `openspec/followups.md`）。

### D3b: popover 生命周期与 store idle race

**问题**：`updateStore.svelte.ts::dismiss()` 可被外部调用把 `status` 切到 `idle`（典型场景：用户从 SettingsView 手动「检查更新」拿到新结果后旧 store 被 reset；或单元测试 cleanup）。如果此时 popover 已展开，原始 9 个 scenario 没规定 popover 必须同步关闭，会出现"pill 已消失但 popover 还浮在 chrome 下方"幻影。

**选择**：popover 生命周期 SHALL 严格绑定 pill 可见性：
- `WHEN` store 状态切到 `idle` AND popover 已展开 → `THEN` popover SHALL 立即关闭，焦点 SHALL 还给触发元素（或 document.body 兜底），所有 popover 内的 event listener / outside-click handler SHALL 释放
- pill 与 popover 共享单一 `open` 状态：pill 不渲染 → popover 也不渲染（Svelte `{#if pill}` 嵌套 `{#if popoverOpen}`，pill 消失时整个 subtree 自动 unmount）

实施层面：用 `$effect` 监听 `updateStore.status === "idle"` 时强制 `popoverOpen = false`，并在 popover 组件 `onDestroy` 内显式释放 outside-click listener 与 focus trap。

### D4: RosettaBanner 改 RosettaStatusIcon + tooltip

**选择**：新建 `RosettaStatusIcon.svelte` 挂在 status zone 最左（在 update pill 之前）。仅在 Apple Silicon 检测到 Rosetta 翻译模式时渲染：黄色三角 icon（16 px）+ hover tooltip 描述详情。点击跳转 release 页或弹设置说明（保留现有 RosettaBanner 链接语义）。

**候选**：
- A: status icon + tooltip（**选**）—— 与"Rosetta 警告是次要、长期可见、不需用户立刻 act" 的语义匹配
- B: 删除 RosettaBanner（不再提示）—— Apple Silicon Mac 装 x64 build 跑在 Rosetta 下确实有性能损失，告知有价值，不能删
- C: 保留全宽横幅 —— 与 D1 拍平 chrome 冲突

### D5: Pane 内 TabBar 保留，chrome 与 content tabs 职责分离

**选择**：每个 pane 内仍保留独立 TabBar 承载该 pane 的 tab 列表（session / settings / notifications），TabBar 内**移除** bell + 齿轮 + macOS traffic-light padding，**保留** tab 列表 + 展开 sidebar 按钮（折叠态时）+ drag region。

**候选**：
- A: 保留 pane 内 TabBar（**选**，Cursor 模型）—— chrome 是窗口级 chrome，content tabs 与每 pane 内容绑定；多 pane split view 时每 pane 有自己的 tab 切换，pane 间互不干扰
- B: 顶部统一 TabBar 管所有 pane tabs —— 与多 pane split 模型冲突（一条 TabBar 怎么表达 4 pane × N tab × focused pane？）
- C: 删除多 pane 模型 —— 用户已有多 pane 习惯，不能 BREAKING；超出本 change 范围

**取舍**：
- bell 与齿轮按钮 component 本身（`NotificationsButton.svelte` / `SettingsButton.svelte`，如存在则按现状）从 TabBar 抽出挂载到 UnifiedTitleBar status zone；按钮内的 unread badge / popover 行为完全不变
- pane 内 TabBar 与 chrome 视觉上靠 1 px `--border-subtle` 横线分隔；chrome 与 sidebar 之间无横线（chrome 与 sidebar 都贴 chrome 底部 border）

### D6: macOS 判定走 `navigator.userAgent.includes("Macintosh")`

**选择**：沿用现有 `SidebarHeader.svelte:58` / `TabBar.svelte:32` / `UpdateBanner.svelte:9` 的 UA 判定方式，集中到 `UnifiedTitleBar.svelte` 单点判定，通过 `class:chrome-mac={isMac}` 控制左 padding 80 px。

**候选**：
- A: UA 判定（**选**）—— 现有代码已用；零依赖；浏览器调试模式（`?mock=1`）也能模拟 Windows / macOS 切 UA 测试
- B: 引入 `@tauri-apps/api/os::platform()` —— 异步 + 需新增 plugin permission；与 `?mock=1` 浏览器调试模式不兼容（plugin 在浏览器侧无 mock）

**取舍**：
- 集中判定后 `SidebarHeader.svelte` / `TabBar.svelte` / `UpdateBanner.svelte` 的 `isMac` 与 traffic-light padding 代码全部删除
- 当前 padding 数值 76 px（SidebarHeader）/ 72 px（TabBar 折叠态）/ 84 px（UpdateBanner）三处不一致，统一为 chrome 的 `padding-left: 80px`（macOS）/ `0`（其它）。80 px = 12 px window 左边距 + 3 × 14 px traffic-light + 2 × 8 px 间距 + 14 px 与第一个 chrome 控件的留白

### D8: 消除 TabBar / SessionDetail header 拼接处的视觉加粗

**问题**：当前红框区域（用户 2026-05-17 截图反馈）感觉"左右没对齐"的根因是**三条 border 在 8 px 高度内并列**：
- `TabBar.svelte:221` —— `border-bottom: 1px solid var(--color-border)`（行底全宽 1 px）
- `TabBar.svelte:312` —— `border-bottom: 2px solid var(--color-border-emphasis)`（active tab indicator 2 px，仅 tab 宽度）
- `SessionDetail.svelte:1072` —— `border-bottom: 1px solid var(--color-border)`（session header 底全宽 1 px）

视觉上 TabBar 整行 1 px 与 session header 紧贴的 1 px 叠成 "加粗"；又因 active tab indicator 是 2 px **仅占 tab 宽**而下方 1 px 是全宽，制造"左侧 tab 区与右侧空白区分割不对齐"的错觉。

**选择**：
- TabBar 行底 1 px 保留（chrome 与 pane content 必要分隔）
- **仅删** `SessionDetail.svelte:1072` 处 1 px border（这是用户截图反馈的具体重影点）；其它 view 顶部 border（`NotificationsView:296/301` 内部分隔、`SettingsView:1121/1130` 窄屏 nav、`DashboardView` 各 section）不在 TabBar 行底重叠面上，**不动**
- Active tab indicator 从 `border-bottom: 2px solid var(--color-border-emphasis)` 改为 `box-shadow: inset 0 -2px 0 var(--color-border-emphasis)` 渲染到 tab 内部，不参与外部 border 计算；颜色 token **沿用** `--color-border-emphasis`（与原行为一致，不改语义只改实现层）；indicator 长度 SHALL 与 tab 内容宽度对齐，不再与行底 border 拼缝产生重影
- chrome 与下方 sidebar / pane 行底分隔同样**只有一条** 1 px
- `box-shadow` 与 tab hover / focus 状态叠加：用 CSS multi-shadow 语法 `box-shadow: <focus-ring>, inset 0 -2px 0 <indicator-color>` 保证 focus ring 与 indicator 共存；disabled tab 通过 `--color-border-emphasis: transparent` 自然消除 indicator

**候选**：
- A: 删 SessionDetail header 1 px + active indicator 改 inset shadow（**选**）—— 视觉最干净；不动 TabBar 整体语义
- B: 删 TabBar 行底 1 px 留 SessionDetail header —— 让 chrome 与 content 失去明确分隔；多 pane split 时 pane 间分隔失效
- C: 同时保留三条但调高度 / 颜色 —— 治标不治本；color contrast 调暗后又影响夜间模式可读

**取舍**：
- active indicator 改 `box-shadow` 会让 indicator 与 tab 文本基线距离精确为 2 px（不再随 border-bottom 影响 tab 高度计算）
- SessionDetail 1759 处 border 是内部章节分隔，保留
- 此条 D8 与 D1 / D5 同一 PR 落地（视觉问题本身是 chrome 拍平自然该治的副作用）

### D7: drag region 用 `data-tauri-drag-region` 属性声明，不写 JS

**选择**：`UnifiedTitleBar.svelte` 内 chrome 容器与中央 flex 空白区声明 `data-tauri-drag-region`；项目下拉 / 折叠按钮 / status zone 的所有按钮加 `data-tauri-drag-region="false"`（属性子树覆盖）阻止穿透。

**候选**：
- A: 声明式 `data-tauri-drag-region`（**选**）—— Tauri 2 推荐路径；无 JS 开销；自动跨平台
- B: JS `onmousedown` + `getCurrentWindow().startDragging()`（现有 `PaneView.svelte` 模式）—— 维护复杂；按钮排除逻辑得手写；mock 模式不可用

**取舍**：
- pane 内 TabBar 仍保留现有 JS drag region（短期不改）—— 改 chrome 与改 pane 内 tab 是两个独立 scope，本 change 只动 chrome；pane drag 行为可在后续 follow-up change 统一切换
- 双击 chrome 空白区让 macOS 触发 `toggleMaximize` 由 Tauri 自动处理，无需手写

## Risks / Trade-offs

- **[Risk] popover 在窄窗口被裁剪** → 默认右贴 chrome 右边缘 + 向下展开；窗口宽度 `< 640px` 时 fallback 到 viewport 水平居中。Playwright e2e 加 480 px viewport 验证。

- **[Risk] downloading 中关闭 popover 用户不知如何取消** → pill 持续显示 downloading 态可点击重新展开 popover，popover 内"取消下载"按钮保留。下载完成 / 失败时 pill 状态切换 → 用户可一眼感知。

- **[Risk] Windows / Linux 平台行为差异** → Windows 系统标题栏（28 px）+ UnifiedTitleBar（44 px）= 顶部 72 px，比 macOS 44 px 多 28 px。这是 OS 行为不一致，业内（Linear / Cursor）也是如此；不为 Windows 自绘 chrome（D5 Non-Goals）。Playwright e2e 以 macOS UA / Windows UA 两套 viewport 截图对照断言 chrome 视觉一致。

- **[Risk] sidebar 折叠时 sidebar header 内的搜索框消失，用户失去搜索入口** → sidebar 折叠时按现状 sidebar 整体宽度收缩到 0（仅 chrome 内的折叠按钮可见），搜索 / filter 当前就是隐藏的；本 change 不改这一点。

- **[Risk] 旧用户视觉感知陡变** → chrome 仍居顶部、左侧仍有项目选择、右上 bell + 齿轮位置基本不变；变化主要在"banner 不再出现 / 多了个 status pill"。无配置开关回滚（chrome 是基础布局，无法 toggle）。BREAKING 在 CHANGELOG / release notes 显式标注。

- **[Trade-off] chrome 44 px 比当前 SidebarHeader 40 px 高 4 px** → 视觉 trade-off，但消除了 banner 出现时多 40 px 推挤的更大噪音。

- **[Trade-off] 不为 Windows 自绘 chrome** → Windows 用户看到两条 chrome（OS 标题栏 + UnifiedTitleBar），与原状一致；好处是规避 Windows native chrome 自绘的大量 edge case（DPI 缩放 / 高对比度 / Aero Snap）。

## Migration Plan

**Phase 1（实施期）**：新组件并行存在 + 老组件保留
1. 新增 `UnifiedTitleBar.svelte` + `UpdateStatusPill.svelte` + `RosettaStatusIcon.svelte`，单独可被 vitest mock import 与 Playwright 路由测试
2. `App.svelte` 顶层加 `UnifiedTitleBar`，**先**保留旧 `RosettaBanner` + `UpdateBanner` 注释掉渲染（让 git diff 清晰）
3. `SidebarHeader.svelte` / `TabBar.svelte` 抽出项目下拉 / 折叠按钮 / bell / 齿轮 / macOS traffic-light padding 到 UnifiedTitleBar

**Phase 2（清理期）**：删除老组件 + 收尾
4. 删除 `RosettaBanner.svelte` / `UpdateBanner.svelte` 文件
5. 删除 `SidebarHeader.svelte` / `TabBar.svelte` 内 traffic-light padding 代码与 `isMac` 局部变量
6. 删除 `App.svelte` 注释掉的渲染行

**Phase 3（验证期）**：
7. vitest 单测：`UpdateStatusPill` 5 态切换 + popover 开关 + 焦点管理
8. Playwright e2e：`unified-title-bar.spec.ts`，覆盖
   - macOS UA：chrome 左 padding = 80 px，pill 可点击展开 popover
   - Windows UA：chrome 左 padding = 0
   - 窄窗口（480 px）popover fallback 居中
9. 手动 `just dev`：真 Tauri 窗口 macOS 上观察 traffic-light 对齐、update / rosetta pill 可点

**Rollback**：单 PR 内可 revert。无后端 / 数据 / config 改动，rollback 不留数据残留。

## Open Questions

- **多 pane 时 status zone 是否需要 per-pane 化？** —— 暂决定全局一份（chrome 是窗口级）。如果未来 update / rosetta / 通知出现 per-project / per-pane 语义，再开 follow-up change。

- **popover 内的 release notes 长度** —— 现有 UpdateBanner markdown 渲染未做截断，超长 release notes 会让横幅过高。popover 限制 max-height: 60vh + 内部滚动；超过 60vh 仍可滚动阅读完整 release notes。

- **status pill 的可访问性** —— pill 有 `aria-label` 描述当前状态；popover 触发支持键盘 `Enter`；popover 内焦点循环。Playwright 加 a11y snapshot 校验。

## MODIFIED Requirements

### Requirement: Quick Anchor Navigation

SessionDetail SHALL 在长会话场景下提供「跳到最新消息」快速锚点：当 conversation scroll 容器距底距离 > 300px 时浮现右下角按钮 + `keyboard-shortcuts` registry 的 `session.jump-to-latest` 当前 binding（默认 mac `⌘↓` / Win+Linux `Ctrl+End`，双 binding spec）触发 smooth scroll 到末尾。该锚点 SHALL 仅作为**瞬时 affordance** 存在（动作不再适用即隐去），SHALL NOT 作为持久导航或装饰；SHALL NOT 引入除 neutral surface / border / text 外的色彩通道（不复用 Focus Blue / Execution Green / Failure Red / Compaction Amber / Thinking Purple）。

`session.jump-to-latest` spec 的 `allowInInput` SHALL 为 `false`（input focus 时让位给浏览器原生光标导航）；spec 的 `preventDefault` SHALL 为 `true`。

**注册位置**：该快捷键 SHALL 由 `PaneView`（或同层 controller）在 mount 阶段**只注册一次** shared handler，**不**由各 SessionDetail 实例分别注册（参见 `add-keyboard-shortcut-system::design.md::D8`：单 binding 单 spec 1:1 关系不允许多 instance 重复 `registerShortcut` 同一 ID）。shared handler 内部 SHALL 按 `getActiveTabId()` 找当前 focused pane 的 active tab，若该 tab 关联的是 SessionDetail 实例则调用其 `jumpToLatest()` 并返回 `true`；若 active tab 不是 SessionDetail（如 Dashboard / Settings）SHALL 返回 `false` 让 dispatcher 不 preventDefault、放行浏览器原生 `Cmd+↓` / `Ctrl+End` 行为。该快捷键 SHALL 由用户在 `Settings → Keyboard Shortcuts` 中自定义。

#### Scenario: 距底 ≤ 300px 时按钮隐藏
- **WHEN** conversation 容器满足 `scrollTop + clientHeight ≥ scrollHeight - 300`
- **THEN** 按钮 SHALL 不可见（`opacity: 0` 且 `pointer-events: none`），且 SHALL NOT 截获键盘 focus

#### Scenario: 距底 > 300px 时按钮显现
- **WHEN** 用户向上滚动使 `scrollTop + clientHeight < scrollHeight - 300`
- **THEN** 按钮 SHALL 在 conversation 容器右下角浮现（`position: absolute; bottom: 16px; right: 16px`）
- **AND** 进出动效 SHALL 为 `opacity + translateY(8px → 0)`，duration 200ms，曲线 `cubic-bezier(0.16, 1, 0.3, 1)`

#### Scenario: 点击按钮 smooth 滚动到末尾
- **WHEN** 用户点击按钮
- **THEN** conversation 容器 SHALL 调用 `scrollTo({ top: scrollHeight, behavior: 'smooth' })`
- **AND** 滚动期间 SHALL 设置 `isProgrammaticScroll = true` 抑制按钮重新显隐判定

#### Scenario: programmatic-scroll 状态机由 scrollend / 距底兜底 / 用户输入三路终止
- **WHEN** `isProgrammaticScroll = true` 期间，conversation 容器触发 `scrollend` 事件
- **THEN** SHALL 立即清 `isProgrammaticScroll = false` 并 `clearTimeout` 任何挂起的 fallback timer
- **AND-WHEN** 在 scrollend 不触发的边缘环境（如 `prefers-reduced-motion: reduce` 下的 `behavior: 'auto'` 路径），1500ms fallback timer SHALL 兜底清除该 flag
- **AND-WHEN** 滚动期间用户主动 `wheel` / `touchmove` / 非本快捷键 `keydown`（即用户打断 smooth scroll）
- **THEN** SHALL 立即清 `isProgrammaticScroll = false` 让按钮按当前距底距离重新派生可见性
- **AND-WHEN** 滚动期间 conversation 距底已 ≤ 16px
- **THEN** SHALL 立即清 `isProgrammaticScroll = false`（提前结束）

#### Scenario: 重复触发跳底不互相干扰
- **WHEN** `isProgrammaticScroll = true` 期间，用户再次点击按钮或再次按下 `session.jump-to-latest` 当前 binding
- **THEN** SHALL 先 `clearTimeout` 旧 fallback timer，再触发新 smooth scroll，重新 set `isProgrammaticScroll = true` 和新 fallback timer
- **AND** 旧 timer 不得提前清掉新 scroll 的 flag

#### Scenario: macOS 键盘快捷键触发跳底
- **WHEN** 平台为 macOS 且 `document.activeElement` 不是 `input` / `textarea` / `contenteditable` 元素
- **AND** 用户按下 `session.jump-to-latest` 当前 binding（默认 `⌘↓`）
- **THEN** registry dispatcher SHALL 命中 `session.jump-to-latest` spec
- **AND** PaneView shared handler SHALL 调 `getActiveTabId()` 找当前 focused SessionDetail 实例
- **AND** 若 active tab 是 SessionDetail，handler SHALL 调用其 `jumpToLatest()` 并返回 `true`，dispatcher SHALL 调用 `event.preventDefault()`
- **AND** SHALL 触发与按钮点击相同的 smooth 滚动到末尾路径

#### Scenario: Windows / Linux 键盘快捷键触发跳底
- **WHEN** 平台非 macOS 且 `document.activeElement` 不是 `input` / `textarea` / `contenteditable` 元素
- **AND** 用户按下 `session.jump-to-latest` 当前 binding（默认 `Ctrl+End`）
- **THEN** registry dispatcher SHALL 命中 `session.jump-to-latest` spec
- **AND** PaneView shared handler 同 macOS 路径，调 `getActiveTabId()` 找 active SessionDetail 实例并调用 `jumpToLatest()`
- **AND** dispatcher SHALL 调用 `event.preventDefault()`
- **AND** SHALL 触发与按钮点击相同的 smooth 滚动到末尾路径

#### Scenario: active tab 非 SessionDetail 时不消费
- **WHEN** focused pane 的 active tab 是 Dashboard / Settings / 其它非 SessionDetail
- **AND** 用户按下 `session.jump-to-latest` 当前 binding
- **THEN** PaneView shared handler SHALL 返回 `false`
- **AND** dispatcher SHALL NOT 调用 `event.preventDefault()`
- **AND** 浏览器原生 `Cmd+↓` / `Ctrl+End` 行为 SHALL 不被打断

#### Scenario: input focused 时键盘不拦截
- **WHEN** `document.activeElement` 是 `input` / `textarea` / `contenteditable` 元素（典型如 SessionDetail 内 SearchBar 输入框）
- **AND** 用户按下平台对应的跳底快捷键
- **THEN** registry dispatcher SHALL 在 `allowInInput=false` 守卫处直接 return
- **AND** SessionDetail SHALL NOT `preventDefault()`，SHALL 让浏览器原生光标导航生效（`Cmd+↓` 移光标到 input 末尾、`Ctrl+End` 同义）

#### Scenario: 多 pane 场景仅 focused pane 内 active SessionDetail 触发滚动
- **WHEN** PaneView 有 ≥ 2 个 pane 且每个 pane 内都有 SessionDetail tab 处于 mount 状态
- **AND** 用户按下 `session.jump-to-latest` 当前 binding
- **THEN** PaneView 顶层注册的 shared handler SHALL 通过 `getActiveTabId()` 找出当前 focused pane 的 active tab
- **AND** 仅该 active tab 关联的 SessionDetail（其 `tabId === getActiveTabId()`）SHALL 被调用 `jumpToLatest()` 触发 smooth 滚到底
- **AND** 其它 pane 的 SessionDetail（不是 active tab）的 `jumpToLatest()` SHALL NOT 被调用，SHALL NOT 触发滚动，保留原视口位置
- **AND** registry SHALL 在整个应用生命周期内对 `session.jump-to-latest` 仅持有 1 个 spec（由 PaneView 注册），SHALL NOT 因 SessionDetail mount / unmount 重复 register / unregister

#### Scenario: ContextPanel 打开时按钮让位
- **WHEN** ContextPanel 处于打开状态（`contextPanelVisible = true`）
- **THEN** 按钮的 `right` 偏移 SHALL 为 `CONTEXT_PANEL_WIDTH + 16px`（与 ContextPanel 既有宽度常量保持一致）
- **AND** ContextPanel 关闭后 SHALL 恢复 `right: 16px`

#### Scenario: reduced-motion 降级
- **WHEN** 用户系统设置 `prefers-reduced-motion: reduce`
- **THEN** 按钮进出 SHALL 为即时显隐（不做 opacity / translateY 过渡）
- **AND** 滚动到末尾 SHALL 使用 `behavior: 'auto'` 而非 `'smooth'`

#### Scenario: 切 tab 来回时按钮可见性重新判定
- **WHEN** 用户从 SessionDetail tab 切走再切回
- **THEN** 按钮可见性 SHALL 根据切回时的 `scrollTop / scrollHeight` 重新派生（不持久化按钮显隐 flag）
- **AND** 既有 `uiState.scrollTop` 恢复机制 SHALL 保持不变（按钮可见性是 scrollTop 的 derived）

#### Scenario: 按钮形态遵循 floating affordance 契约
- **WHEN** 按钮处于 visible 态
- **THEN** 视觉 SHALL 为 28×28 hit area + 14px `chevrons-down` icon + `6px` radius
- **AND** 颜色 SHALL 用 `--color-surface-raised` bg + 1px `--color-border-emphasis` + `--color-text-secondary` 图标色（不复用 Focus Blue / 任何语义色）
- **AND** Elevation SHALL 为 `0 2px 8px rgba(0,0,0,0.06)`，hover 升至 `0 4px 12px rgba(0,0,0,0.08)`
- **AND** SHALL 提供 `aria-label`（如「跳到最新消息」）+ 平台分流的 `title` tooltip 显示 `formatShortcut(currentBinding)` 的输出

# frontend-context-menu Specification

## Purpose
TBD - created by archiving change frontend-context-menu. Update Purpose after archive.
## Requirements
### Requirement: 全局右键事件兜底策略

应用 SHALL 在启动时注册 window-level `contextmenu` 事件监听器，对未被任何元素自定义菜单处理过的右键事件统一调用 `event.preventDefault()`，阻止 WKWebView 默认菜单与 macOS 系统菜单弹出。监听器 SHALL 在事件 bubble 阶段执行（`capture: false`），并仅在 `event.defaultPrevented === false` 时调用 `preventDefault`，以确保元素自身的 `oncontextmenu` 处理优先生效。

#### Scenario: 右键空白区或非交互元素

- **WHEN** 用户在应用窗口内任何**未挂** `use:contextMenu` 也**不在**白名单内的位置右键
- **THEN** 应用 SHALL `preventDefault` 阻止默认菜单
- **AND** 不弹出任何菜单（既不弹 app 自家菜单，也不弹 WKWebView / 系统菜单）

#### Scenario: 元素已自处理 contextmenu

- **WHEN** 元素挂了 `use:contextMenu` 或 `oncontextmenu` handler 并已调用 `e.preventDefault()`
- **THEN** 全局兜底监听器 SHALL 检测 `event.defaultPrevented === true` 并跳过自身的 `preventDefault` 调用
- **AND** 元素自定义菜单 SHALL 正常弹出

#### Scenario: 右键 input / textarea / contenteditable

- **WHEN** 用户在 `<input>` / `<textarea>` / `[contenteditable="true"]` 元素内右键
- **THEN** 全局兜底监听器 SHALL 通过 `target.closest()` 检测到这些元素并跳过 `preventDefault`
- **AND** 浏览器原生输入菜单（粘贴 / 拼写检查 / 朗读等）SHALL 正常弹出，保留输入便利

#### Scenario: 显式 opt-in 保留系统菜单

- **WHEN** 元素或其祖先带有 `data-allow-native-context` 属性
- **THEN** 全局兜底监听器 SHALL 跳过 `preventDefault`
- **AND** 浏览器原生菜单 SHALL 正常弹出

### Requirement: AppContextMenu 通用浮层组件

应用 SHALL 提供 `AppContextMenu` 通用 Svelte 组件作为所有右键菜单的视觉与交互真相源。组件 SHALL 接受 `items` props（数组形式 `{ label: string; icon?: string; action: () => void; disabled?: boolean; danger?: boolean; separator?: boolean }[]`）与定位 `x` / `y` 坐标 props，渲染单 column 文字菜单浮层。视觉 token SHALL 沿用现 `SessionContextMenu` 形态（`--color-surface` bg、1px `--color-border-emphasis` border、`8px` radius、`4px` padding、`0 4px 16px rgba(0, 0, 0, 0.15)` shadow、`200px` min-width、`.cm-item` `7px 12px` padding `4px` radius `13px / 1.4 / 400` font），不引入 `backdrop-filter` 或装饰性渐变。

#### Scenario: 渲染基本菜单 items

- **WHEN** 调用方传入 `items` 数组（含 label + action 的纯文字 item）
- **THEN** 组件 SHALL 在 `(x, y)` 位置渲染浮层
- **AND** 每个 item 渲染为单行 `role="menuitem"` 元素
- **AND** 浮层顶层元素 SHALL 带 `role="menu"` 与 `aria-orientation="vertical"`

#### Scenario: 渲染 separator

- **WHEN** items 数组中某 item 含 `separator: true`
- **THEN** 该位置 SHALL 渲染为 `1px solid var(--color-border)` 的水平分隔线
- **AND** separator 不参与 keyboard 导航 focus 环

#### Scenario: 渲染 disabled item

- **WHEN** 某 item 含 `disabled: true`
- **THEN** 该 item SHALL 渲染为 opacity 0.45、`cursor: not-allowed`
- **AND** SHALL 加 `aria-disabled="true"` 而非原生 `disabled` 属性，使屏幕阅读器仍宣告"菜单项 X of N，已禁用"
- **AND** 鼠标点击 SHALL 不触发 action 也不关闭菜单
- **AND** 键盘 ↑↓ 导航 SHALL **经过** disabled items（不跳过），保留键盘可达性
- **AND** 在 disabled item 上按 Enter / Space SHALL no-op（不调用 action 不关闭菜单）

#### Scenario: 渲染 danger item

- **WHEN** 某 item 含 `danger: true`
- **THEN** 该 item 文字色 SHALL 为 `--color-danger`
- **AND** hover 时 bg SHALL 染淡红色（`--color-danger` 极低 opacity 混入）

#### Scenario: viewport 边界 clamp

- **WHEN** 触发位置 `(x, y)` 距 viewport 右/下边距小于 `菜单宽度 + 8px` 或 `菜单高度 + 8px`
- **THEN** 浮层位置 SHALL 自动收缩使距离 viewport 边 ≥ 8px
- **AND** 菜单完整可见，不溢出窗口

### Requirement: AppContextMenu 键盘可达性

`AppContextMenu` SHALL 完整支持键盘操作，严格对齐 WAI-ARIA Authoring Practices Guide 的 menu pattern。菜单打开后 SHALL 立即将 focus 移到第一个 menuitem（无论鼠标右键、Menu 键还是 Shift+F10 触发，行为一致）；用户 SHALL 可通过键盘 ↑↓ 在所有 menuitem 间循环移动 focus（**经过** `aria-disabled` items，不跳过——保留可达性；分隔符跳过）；Enter / Space SHALL 触发当前 focus item 的 action 并关闭菜单（`aria-disabled` items 上 no-op）；Esc SHALL 关闭菜单并将 focus 还回 trigger 元素；鼠标 hover SHALL 同步 `activeIndex` 到该 item，使键盘与鼠标焦点状态合一。

#### Scenario: 菜单打开后焦点进第一个 menuitem

- **WHEN** 用户通过任意触发方式（鼠标右键 / Menu 键 / Shift+F10）打开菜单
- **THEN** focus SHALL 立即进入第一个非 separator menuitem 并 active（不区分触发源）
- **AND** active item SHALL 渲染键盘焦点视觉提示（`outline: 2px solid rgba(59, 130, 246, 0.15)`）

#### Scenario: ↑↓ 循环移动 focus，经过 disabled

- **WHEN** 菜单已打开，用户按 ↓
- **THEN** focus SHALL 移到下一个 menuitem（**不**跳过 `aria-disabled`，仅跳过 separator）
- **AND** 在最后一项继续按 ↓ SHALL 循环回第一项
- **AND** 第一项按 ↑ SHALL 循环到最后一项

#### Scenario: Enter / Space 触发 enabled item

- **WHEN** 用户在 enabled active item 上按 Enter 或 Space
- **THEN** 该 item 的 `action` SHALL 被调用
- **AND** 菜单 SHALL 关闭
- **AND** focus SHALL 还回 trigger 元素

#### Scenario: Enter / Space 在 aria-disabled item 上 no-op

- **WHEN** 用户在 `aria-disabled="true"` 的 active item 上按 Enter 或 Space
- **THEN** SHALL 不调用 action
- **AND** 菜单 SHALL 不关闭
- **AND** focus 保持在该 item

#### Scenario: 鼠标 hover 同步键盘 active

- **WHEN** 用户在键盘 ↑↓ 选中某项后，鼠标移到另一 menuitem 上
- **THEN** `activeIndex` SHALL 同步到鼠标 hover 的 item
- **AND** 后续键盘 Enter SHALL 触发鼠标 hover 项的 action
- **AND** 避免出现"鼠标位置与键盘焦点分裂"的两个独立焦点模型

#### Scenario: Esc 关闭菜单

- **WHEN** 菜单已打开，用户按 Esc
- **THEN** 菜单 SHALL 关闭
- **AND** focus SHALL 还回 trigger 元素
- **AND** 不触发任何 item action

#### Scenario: 键盘 contextmenu 触发位置

- **WHEN** 用户在 trigger 元素上按 Menu 键或 Shift+F10
- **THEN** 菜单 SHALL 在 trigger 元素 `getBoundingClientRect()` 中心位置渲染
- **AND** 同样适用上述"焦点进第一个 menuitem"规则

### Requirement: AppContextMenu 浮层 portal 到 document.body

`AppContextMenu` SHALL 在每次右键触发时通过 Svelte 5 `mount()` API 渲染到 `document.body` 末尾，**不**作为 trigger 元素的子节点 inline 渲染。这是为了避免 `overflow: hidden` / `overflow: auto` 父容器（如 sidebar 虚拟滚动列表、tab list 横向滚动）clipping 菜单浮层、避免祖先 `transform` / `filter` / `contain` 创建新 stacking context 隔离菜单 z-index、确保 `menuEl.contains(target)` 外点判断对 trigger 元素返回 false。`use:contextMenu` action 在内部持有 mount instance 引用，新的右键事件触发时 SHALL 先 unmount 旧 instance 再 mount 新 instance；action destroy 钩子 SHALL 兜底 unmount 任何残余 instance。

#### Scenario: 触发右键时菜单挂到 document.body

- **WHEN** 用户在挂了 `use:contextMenu` 的元素上右键
- **THEN** 菜单 DOM 节点 SHALL 插入到 `document.body` 末尾
- **AND** 菜单 DOM SHALL **不**作为 trigger 元素或其任何祖先的子节点

#### Scenario: 父容器 overflow: hidden 不 clip 菜单

- **WHEN** trigger 元素位于 `overflow: hidden` 父容器内（如 sidebar 虚拟滚动列表）且菜单尺寸超出父容器边界
- **THEN** 菜单 SHALL 完整可见（依赖 portal 到 body，不被父容器 clip）
- **AND** 仅 viewport 边界 clamp 规则约束位置

#### Scenario: 多次右键替换浮层 instance

- **WHEN** 用户连续在不同元素上右键
- **THEN** 旧菜单 instance SHALL 在新 instance mount 前被 unmount
- **AND** `document.body` 中同时存在的 `AppContextMenu` instance 数量 SHALL 始终 ≤ 1

#### Scenario: trigger 元素 destroy 时菜单兜底卸载

- **WHEN** 挂了 `use:contextMenu` 的元素被 Svelte 移除（如 tab close / sidebar 虚拟滚动滚出视口）
- **THEN** action destroy 钩子 SHALL 调用 unmount 清理任何残余菜单 instance
- **AND** `document.body` 不留菜单 DOM 残骸

### Requirement: AppContextMenu 关闭触发条件

菜单 SHALL 在以下任一事件时关闭：(a) document `mousedown` 发生在 menuEl 外；(b) document `keydown` Esc；(c) window `blur`；(d) 任意祖先 `scroll` 事件；(e) window `resize` 事件。关闭后 SHALL 移除所有事件监听器避免泄漏。

#### Scenario: 外点关闭

- **WHEN** 菜单已打开，用户在菜单浮层外的任意位置 mousedown
- **THEN** 菜单 SHALL 关闭

#### Scenario: 切到其他应用关闭

- **WHEN** 菜单已打开，用户切到其他应用（窗口失焦）
- **THEN** 菜单 SHALL 关闭
- **AND** 用户切回应用时 SHALL 不再看到菜单浮层

#### Scenario: 滚动关闭

- **WHEN** 菜单已打开，用户在任意祖先容器内滚动
- **THEN** 菜单 SHALL 关闭（不做位置 reposition）

#### Scenario: 窗口大小变化关闭

- **WHEN** 菜单已打开，用户调整窗口大小或最大化
- **THEN** 菜单 SHALL 关闭

### Requirement: use:contextMenu Svelte action

应用 SHALL 提供 `use:contextMenu` Svelte action 作为元素挂载自定义右键菜单的唯一入口。Action SHALL 接受 `provider` 参数（`ContextMenuItem[]` 或 `(event) => ContextMenuItem[]` 函数），在元素上注册 `oncontextmenu` 监听器：右键触发时 SHALL 调用 `e.preventDefault()` 阻止默认菜单 + `e.stopPropagation()` 阻止全局兜底重复处理 + 在事件位置渲染 `AppContextMenu`。Provider 函数形式 SHALL 在每次右键时被调用计算最新 items，支持基于 trigger 时刻的动态状态（如 `isPinned` / `canSplit`）。

#### Scenario: 静态 items 数组

- **WHEN** 调用方使用 `use:contextMenu={[item1, item2, item3]}`
- **THEN** action SHALL 在元素上注册右键监听
- **AND** 右键时直接使用该数组渲染菜单

#### Scenario: 动态 items 函数

- **WHEN** 调用方使用 `use:contextMenu={(e) => buildItems(state)}`
- **THEN** action SHALL 在每次右键事件触发时调用该函数获取最新 items
- **AND** 调用方无需手动管理 `$derived` 反应链

#### Scenario: 阻止事件 bubble 到全局兜底

- **WHEN** action 处理右键事件
- **THEN** 处理函数 SHALL 调用 `e.preventDefault()` 与 `e.stopPropagation()`
- **AND** 全局兜底 listener SHALL 不再处理该事件（`e.defaultPrevented === true` 同时事件不再 bubble 到 window）

#### Scenario: 元素 unmount 清理监听

- **WHEN** action 挂载的元素被 destroy
- **THEN** action 的 destroy 函数 SHALL 移除 `contextmenu` 与 `mousedown` 与 `keydown` 监听器
- **AND** 若菜单仍打开 SHALL 立即关闭

### Requirement: WKWebView smart-select 防护

`use:contextMenu` action SHALL 在元素上注册 `mousedown` 监听器，对**右键 mousedown** 事件检测当前 selection 状态：若 `window.getSelection().toString().length === 0`（无选区），SHALL 调用 `e.preventDefault()` 阻止 WKWebView 在 mousedown 阶段触发的 smart-selection（自动选中光标下的"词"）；若已有选区（用户 drag-select 后右键），SHALL 不调用 `preventDefault`，保留选区供后续 Phase 2 文本菜单消费。

#### Scenario: 右键无选区元素

- **WHEN** 用户在挂了 `use:contextMenu` 的元素文本上右键，且当前无任何 selection
- **THEN** action 的 mousedown handler SHALL 检测 `e.button === 2` 与 `selection.toString().length === 0`
- **AND** 调用 `e.preventDefault()` 阻止 WKWebView smart-select
- **AND** 用户感知：右键时无任何文字被自动选中

#### Scenario: 右键已选区域

- **WHEN** 用户先 drag-select 一段文本，再在选区附近右键
- **THEN** action 的 mousedown handler SHALL 检测 `selection.toString().length > 0`
- **AND** **不**调用 `preventDefault`
- **AND** 选区保留给后续 contextmenu handler 消费（Phase 2 文本菜单依赖此行为）

#### Scenario: 左键 mousedown 不触发防护

- **WHEN** 用户左键点击挂了 `use:contextMenu` 的元素
- **THEN** mousedown handler SHALL 检测 `e.button !== 2`
- **AND** 立即 return，不影响左键 selection 行为

### Requirement: 全局兜底初始化时机

应用 SHALL 在 `ui/src/main.ts` 启动序列内调用 `installGlobalContextMenuFallback()` 注册 window-level `contextmenu` 监听器。注册 SHALL 在所有 Svelte 组件 mount 之前完成，确保启动后任何位置的右键事件都被覆盖。该函数 SHALL 是幂等的——重复调用不重复注册监听器（开发模式下 HMR 触发模块重载时不应叠加）。

#### Scenario: 应用启动注册兜底

- **WHEN** 应用启动 main.ts 执行
- **THEN** SHALL 调用 `installGlobalContextMenuFallback()` 注册 window 级 `contextmenu` 监听
- **AND** 启动后任何位置的右键事件 SHALL 经过兜底处理

#### Scenario: HMR 重复调用幂等

- **WHEN** 开发模式 Vite HMR 触发模块重载，`installGlobalContextMenuFallback()` 被再次调用
- **THEN** SHALL 检测已注册并跳过重复注册
- **AND** window 上仅存在一个 listener

### Requirement: Sidebar session-item 兜底 user-select 防护

`Sidebar.svelte::.session-item` CSS SHALL 包含 `user-select: none; -webkit-user-select: none` 作为兜底防护——即使 `use:contextMenu` 的 mousedown 防护因任何原因失效（如未来代码改动误移除 action），WKWebView 也无法在 session-item 内 smart-select 文本。会话标题与 metadata 不是用户用来选词复制的内容（用户复制 sessionId / 恢复命令均从右键菜单走），加 `user-select: none` 不损失功能。

#### Scenario: 右键 worktree chip 不选中文字

- **WHEN** 用户在 sidebar 会话项的 worktree chip 区域（如 `#claude-devtools-rs`）上右键
- **THEN** SHALL 不触发 WKWebView smart-select（CSS 层面已禁止 user-select）
- **AND** SHALL 弹出 `SessionContextMenu`（保持现行为）

### Requirement: SessionContextMenu / TabContextMenu 重构兼容

`SessionContextMenu.svelte` 与 `TabContextMenu.svelte` SHALL 改用 `AppContextMenu` 通用组件实现，但外部 API（props / 调用面 / 触发位置）SHALL 保持兼容。`Sidebar.svelte` 与 `TabBar.svelte` 内现有 `oncontextmenu={(e) => onContextMenu(e, ...)}` 调用 SHALL 在重构后**改用** `use:contextMenu={() => buildItems(...)}` action 形式（让动态 items 计算更内聚），但用户可见行为（菜单项内容、文案、顺序、动作语义、触发位置、复制反馈"已复制!"600ms 关闭）一一对齐。

#### Scenario: Sidebar 会话项右键菜单回归

- **WHEN** 重构完成后，用户在 sidebar 会话项上右键
- **THEN** SHALL 弹出含"在当前标签页打开 / 在新标签页打开 / 在新 Pane 打开 / 置顶/取消置顶 / 隐藏/取消隐藏 / 复制 Session ID / 复制恢复命令"的菜单
- **AND** 各 item 顺序、文案、动作 SHALL 与重构前一致
- **AND** 复制 item 触发后 SHALL 显示"已复制!"反馈 600ms 后关闭

#### Scenario: TabBar 标签项右键菜单回归

- **WHEN** 重构完成后，用户在 TabBar 标签项上右键
- **THEN** SHALL 弹出与重构前内容一致的关闭/移到新 pane 类菜单
- **AND** 各 item 动作 SHALL 与重构前一致


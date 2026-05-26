# frontend-context-menu Specification

## Purpose
TBD - created by archiving change frontend-context-menu. Update Purpose after archive.
## Requirements
### Requirement: 全局右键事件兜底策略

应用 SHALL 在启动时注册 window-level contextmenu 事件监听器，对未被任何元素自定义菜单处理过的右键事件统一调用 preventDefault 阻止 WKWebView 默认菜单与 macOS 系统菜单弹出。监听器 SHALL 在事件 bubble 阶段执行，并仅在 `event.defaultPrevented === false` 时调用 preventDefault，以确保元素自身的 contextmenu 处理优先生效。

#### Scenario: 右键空白区或非交互元素

- **WHEN** 用户在应用窗口内任何**未挂**右键 action 也**不在**白名单内的位置右键
- **THEN** 应用 SHALL preventDefault 阻止默认菜单
- **AND** 不弹出任何菜单（既不弹 app 自家菜单，也不弹 WKWebView / 系统菜单）

#### Scenario: 元素已自处理 contextmenu

- **WHEN** 元素挂了右键 action 或自定义 oncontextmenu handler 并已调用 preventDefault
- **THEN** 全局兜底监听器 SHALL 检测到 defaultPrevented 并跳过自身的 preventDefault 调用
- **AND** 元素自定义菜单 SHALL 正常弹出

#### Scenario: 右键 input / textarea / contenteditable

- **WHEN** 用户在 input / textarea / `[contenteditable="true"]` 元素内右键
- **THEN** 全局兜底监听器 SHALL 检测到这些元素并跳过 preventDefault
- **AND** 浏览器原生输入菜单（粘贴 / 拼写检查 / 朗读等）SHALL 正常弹出，保留输入便利

#### Scenario: 显式 opt-in 保留系统菜单

- **WHEN** 元素或其祖先带有 `data-allow-native-context` 属性
- **THEN** 全局兜底监听器 SHALL 跳过 preventDefault
- **AND** 浏览器原生菜单 SHALL 正常弹出

### Requirement: AppContextMenu 通用浮层组件

应用 SHALL 提供通用右键菜单组件作为所有右键菜单的视觉与交互真相源。组件 SHALL 接受 items 数组（含 label / icon? / action / disabled? / danger? / separator? 字段）与定位坐标 props，渲染单 column 文字菜单浮层。视觉 token SHALL 沿用既有右键菜单形态（`--color-surface` 背景、1px `--color-border-emphasis` 边框、8px radius、4px padding、shadow `0 4px 16px rgba(0, 0, 0, 0.15)`、min-width 200px、单 item `7px 12px` padding），不引入 `backdrop-filter` 或装饰性渐变。

#### Scenario: 渲染基本菜单 items

- **WHEN** 调用方传入 items 数组（含 label + action 的纯文字 item）
- **THEN** 组件 SHALL 在指定 (x, y) 位置渲染浮层
- **AND** 每个 item 渲染为单行 `role="menuitem"` 元素
- **AND** 浮层顶层元素 SHALL 带 `role="menu"` 与 `aria-orientation="vertical"`

#### Scenario: 渲染 separator

- **WHEN** items 数组中某 item 含 `separator: true`
- **THEN** 该位置 SHALL 渲染为 `1px solid var(--color-border)` 的水平分隔线
- **AND** separator 不参与 keyboard 导航 focus 环

#### Scenario: 渲染 disabled item

- **WHEN** 某 item 含 `disabled: true`
- **THEN** 该 item SHALL 渲染为 opacity 0.45、`cursor: not-allowed`
- **AND** SHALL 加 `aria-disabled="true"` 而非原生 disabled 属性，使屏幕阅读器仍宣告"菜单项 X of N，已禁用"
- **AND** 鼠标点击 SHALL 不触发 action 也不关闭菜单
- **AND** 键盘 ↑↓ 导航 SHALL **经过** disabled items（不跳过），保留键盘可达性
- **AND** 在 disabled item 上按 Enter / Space SHALL no-op（不调用 action 不关闭菜单）

#### Scenario: 渲染 danger item

- **WHEN** 某 item 含 `danger: true`
- **THEN** 该 item 文字色 SHALL 为 `--color-danger`
- **AND** hover 时 bg SHALL 染淡红色（`--color-danger` 极低 opacity 混入）

#### Scenario: viewport 边界 clamp

- **WHEN** 触发位置距 viewport 右/下边距小于 `菜单尺寸 + 8px`
- **THEN** 浮层位置 SHALL 自动收缩使距离 viewport 边 ≥ 8px
- **AND** 菜单完整可见，不溢出窗口

### Requirement: AppContextMenu 键盘可达性

通用右键菜单 SHALL 完整支持键盘操作，严格对齐 WAI-ARIA Authoring Practices Guide 的 menu pattern。菜单打开后 SHALL 立即将 focus 移到第一个 menuitem（无论鼠标右键、Menu 键还是 Shift+F10 触发，行为一致）；用户 SHALL 可通过键盘 ↑↓ 在所有 menuitem 间循环移动 focus（**经过** aria-disabled items，不跳过——保留可达性；分隔符跳过）；Enter / Space SHALL 触发当前 focus item 的 action 并关闭菜单（aria-disabled items 上 no-op）；Esc SHALL 关闭菜单并将 focus 还回 trigger 元素；鼠标 hover SHALL 同步 activeIndex 到该 item，使键盘与鼠标焦点状态合一。

#### Scenario: 菜单打开后焦点进第一个 menuitem

- **WHEN** 用户通过任意触发方式（鼠标右键 / Menu 键 / Shift+F10）打开菜单
- **THEN** focus SHALL 立即进入第一个非 separator menuitem 并 active（不区分触发源）
- **AND** active item SHALL 渲染键盘焦点视觉提示（淡蓝 outline）

#### Scenario: ↑↓ 循环移动 focus，经过 disabled

- **WHEN** 菜单已打开，用户按 ↓
- **THEN** focus SHALL 移到下一个 menuitem（**不**跳过 aria-disabled，仅跳过 separator）
- **AND** 在最后一项继续按 ↓ SHALL 循环回第一项；第一项按 ↑ SHALL 循环到最后一项

#### Scenario: Enter / Space 触发 enabled / no-op disabled

- **WHEN** 用户在 enabled active item 上按 Enter 或 Space
- **THEN** 该 item 的 action SHALL 被调用；菜单 SHALL 关闭；focus SHALL 还回 trigger
- **WHEN** 在 aria-disabled active item 上按 Enter 或 Space
- **THEN** SHALL 不调用 action；菜单 SHALL 不关闭；focus 保持在该 item

#### Scenario: 鼠标 hover 同步键盘 active

- **WHEN** 用户在键盘 ↑↓ 选中某项后，鼠标移到另一 menuitem 上
- **THEN** activeIndex SHALL 同步到鼠标 hover 的 item
- **AND** 后续键盘 Enter SHALL 触发鼠标 hover 项的 action
- **AND** 避免出现"鼠标位置与键盘焦点分裂"的两个独立焦点模型

#### Scenario: Esc 关闭菜单 + 还回 trigger

- **WHEN** 菜单已打开，用户按 Esc
- **THEN** 菜单 SHALL 关闭；focus SHALL 还回 trigger 元素；不触发任何 item action

#### Scenario: 键盘 contextmenu 触发位置

- **WHEN** 用户在 trigger 元素上按 Menu 键或 Shift+F10
- **THEN** 菜单 SHALL 在 trigger 元素 boundingClientRect 中心位置渲染
- **AND** 同样适用上述"焦点进第一个 menuitem"规则

### Requirement: AppContextMenu 浮层 portal 到 document.body

通用右键菜单组件 SHALL 在每次右键触发时通过框架 mount API 渲染到 document.body 末尾，**不**作为 trigger 元素的子节点 inline 渲染。这是为了避免 `overflow: hidden` / `overflow: auto` 父容器（典型侧栏虚拟滚动列表、tab list 横向滚动）clipping 菜单浮层、避免祖先 transform / filter / contain 创建新 stacking context 隔离菜单 z-index、确保 menuEl.contains(target) 外点判断对 trigger 元素返回 false。右键 action 在内部持有 mount instance 引用，新的右键事件触发时 SHALL 先 unmount 旧 instance 再 mount 新 instance；action destroy 钩子 SHALL 兜底 unmount 任何残余 instance。

#### Scenario: 触发右键时菜单挂到 document.body

- **WHEN** 用户在挂了右键 action 的元素上右键
- **THEN** 菜单 DOM 节点 SHALL 插入到 document.body 末尾
- **AND** 菜单 DOM SHALL **不**作为 trigger 元素或其任何祖先的子节点

#### Scenario: 父容器 overflow: hidden 不 clip 菜单

- **WHEN** trigger 元素位于 overflow hidden 父容器内（典型侧栏虚拟滚动列表）且菜单尺寸超出父容器边界
- **THEN** 菜单 SHALL 完整可见（依赖 portal 到 body，不被父容器 clip）
- **AND** 仅 viewport 边界 clamp 规则约束位置

#### Scenario: 多次右键替换浮层 instance

- **WHEN** 用户连续在不同元素上右键
- **THEN** 旧菜单 instance SHALL 在新 instance mount 前被 unmount
- **AND** document.body 中同时存在的菜单 instance 数量 SHALL 始终 ≤ 1

#### Scenario: trigger 元素 destroy 时菜单兜底卸载

- **WHEN** 挂了右键 action 的元素被框架移除（典型 tab close / 侧栏虚拟滚动滚出视口）
- **THEN** action destroy 钩子 SHALL 调 unmount 清理任何残余菜单 instance
- **AND** document.body 不留菜单 DOM 残骸

### Requirement: AppContextMenu 关闭触发条件

菜单 SHALL 在以下任一事件时关闭：(a) document mousedown 发生在 menuEl 外；(b) document keydown Esc；(c) window blur；(d) 任意祖先 scroll 事件；(e) window resize 事件。关闭后 SHALL 移除所有事件监听器避免泄漏。

#### Scenario: 外点关闭

- **WHEN** 菜单已打开，用户在菜单浮层外的任意位置 mousedown
- **THEN** 菜单 SHALL 关闭

#### Scenario: 切到其他应用关闭

- **WHEN** 菜单已打开，用户切到其他应用（窗口失焦）
- **THEN** 菜单 SHALL 关闭
- **AND** 用户切回应用时 SHALL 不再看到菜单浮层

#### Scenario: 滚动 / resize 关闭

- **WHEN** 菜单已打开，用户在任意祖先容器内滚动或调整窗口大小
- **THEN** 菜单 SHALL 关闭（不做位置 reposition）

### Requirement: use:contextMenu Svelte action

应用 SHALL 提供右键 action 作为元素挂载自定义右键菜单的唯一入口。Action SHALL 接受 provider 参数（items 数组或返回 items 数组的函数），在元素上注册 oncontextmenu 监听器：右键触发时 SHALL preventDefault 阻止默认菜单 + stopPropagation 阻止全局兜底重复处理 + 在事件位置渲染通用菜单组件。Provider 函数形式 SHALL 在每次右键时被调用计算最新 items，支持基于 trigger 时刻的动态状态（典型 isPinned / canSplit）。

#### Scenario: 静态 items 数组

- **WHEN** 调用方使用 action 传入静态 items 数组
- **THEN** action SHALL 在元素上注册右键监听
- **AND** 右键时直接使用该数组渲染菜单

#### Scenario: 动态 items 函数

- **WHEN** 调用方使用 action 传入返回 items 数组的函数
- **THEN** action SHALL 在每次右键事件触发时调用该函数获取最新 items
- **AND** 调用方无需手动管理派生反应链

#### Scenario: 阻止事件 bubble 到全局兜底

- **WHEN** action 处理右键事件
- **THEN** 处理函数 SHALL preventDefault + stopPropagation
- **AND** 全局兜底 listener SHALL 不再处理该事件（defaultPrevented 同时事件不再 bubble 到 window）

#### Scenario: 元素 unmount 清理监听

- **WHEN** action 挂载的元素被 destroy
- **THEN** action 的 destroy 函数 SHALL 移除 contextmenu / mousedown / keydown 监听器
- **AND** 若菜单仍打开 SHALL 立即关闭

### Requirement: WKWebView smart-select 防护

右键 action SHALL 在元素上注册 mousedown 监听器，对**右键 mousedown** 事件检测当前 selection 状态：若当前无选区，SHALL preventDefault 阻止 WKWebView 在 mousedown 阶段触发的 smart-selection（自动选中光标下的"词"）；若已有选区（用户 drag-select 后右键），SHALL 不调 preventDefault，保留选区供后续文本菜单消费。

#### Scenario: 右键无选区元素

- **WHEN** 用户在挂了右键 action 的元素文本上右键，且当前无任何 selection
- **THEN** action 的 mousedown handler SHALL 检测 button === 2 与 selection 长度为 0
- **AND** preventDefault 阻止 WKWebView smart-select
- **AND** 用户感知：右键时无任何文字被自动选中

#### Scenario: 右键已选区域

- **WHEN** 用户先 drag-select 一段文本，再在选区附近右键
- **THEN** action 的 mousedown handler SHALL 检测选区长度 > 0
- **AND** **不**调 preventDefault；选区保留给后续 contextmenu handler 消费

#### Scenario: 左键 mousedown 不触发防护

- **WHEN** 用户左键点击挂了右键 action 的元素
- **THEN** mousedown handler SHALL 检测 button !== 2
- **AND** 立即 return，不影响左键 selection 行为

### Requirement: 全局兜底初始化时机

应用 SHALL 在启动序列内调用全局右键兜底初始化入口，在所有组件 mount 之前完成 window 级 contextmenu 监听器注册，确保启动后任何位置的右键事件都被覆盖。该入口 SHALL 是幂等的——重复调用不重复注册监听器（开发模式下 HMR 触发模块重载时不应叠加）。

#### Scenario: 应用启动注册兜底

- **WHEN** 应用启动序列执行
- **THEN** SHALL 完成 window 级 contextmenu 监听注册
- **AND** 启动后任何位置的右键事件 SHALL 经过兜底处理

#### Scenario: HMR 重复调用幂等

- **WHEN** 开发模式 HMR 触发模块重载，初始化入口被再次调用
- **THEN** SHALL 检测已注册并跳过重复注册
- **AND** window 上仅存在一个 listener

### Requirement: Sidebar session-item 兜底 user-select 防护

侧栏会话项 CSS SHALL 包含 `user-select: none; -webkit-user-select: none` 作为兜底防护——即使右键 action 的 mousedown 防护因任何原因失效（典型未来代码改动误移除 action），WKWebView 也无法在会话项内 smart-select 文本。会话标题与 metadata 不是用户用来选词复制的内容（用户复制 sessionId / 恢复命令均从右键菜单走），加 user-select none 不损失功能。

#### Scenario: 右键 worktree chip 不选中文字

- **WHEN** 用户在侧栏会话项的 worktree chip 区域上右键
- **THEN** SHALL 不触发 WKWebView smart-select（CSS 层面已禁止 user-select）
- **AND** SHALL 弹出会话项右键菜单（保持现行为）

### Requirement: SessionContextMenu / TabContextMenu 重构兼容

侧栏会话项与 tab 项的右键菜单 SHALL 改用通用菜单组件实现，但外部 API（props / 调用面 / 触发位置）SHALL 保持兼容。侧栏 / tab bar 内现有 oncontextmenu 直挂调用 SHALL 在重构后**改用**右键 action 形式（让动态 items 计算更内聚），但用户可见行为（菜单项内容、文案、顺序、动作语义、触发位置、复制 item 反馈短显示后关闭）一一对齐。

#### Scenario: Sidebar 会话项右键菜单回归

- **WHEN** 重构完成后，用户在侧栏会话项上右键
- **THEN** SHALL 弹出含 在当前标签页打开 / 在新标签页打开 / 在新 Pane 打开 / 置顶/取消置顶 / 隐藏/取消隐藏 / 复制 Session ID / 复制恢复命令 的菜单
- **AND** 各 item 顺序、文案、动作 SHALL 与重构前一致
- **AND** 复制 item 触发后 SHALL 显示"已复制!"反馈 600ms 后关闭

#### Scenario: TabBar 标签项右键菜单回归

- **WHEN** 重构完成后，用户在 tab 项上右键
- **THEN** SHALL 弹出与重构前内容一致的关闭 / 移到新 pane 类菜单
- **AND** 各 item 动作 SHALL 与重构前一致

### Requirement: 文本选区菜单（window-level handler）

应用 SHALL 注册 window-level contextmenu 监听器作为 surface-level 右键 action 与全局兜底之间的中间层（**Layer 2**）：当 surface 未拦截事件且当前选区文本非空时，SHALL preventDefault 阻止系统菜单并弹出选区专属菜单（含 复制选中文本 / 复制为引用 Markdown / 在浏览器搜索 等 items）。Layer 2 SHALL 在全局兜底注册之前注册以保证执行顺序；handler 内 SHALL 跳过 input / textarea / contenteditable / `data-allow-native-context` 元素让浏览器原生菜单接管。

#### Scenario: 选中文本后右键空白区弹选区菜单

- **WHEN** 用户先 drag-select 一段文本，再在选区附近未挂右键 action 的位置右键
- **THEN** Layer 2 handler SHALL 检测选区非空
- **AND** preventDefault 阻止系统菜单
- **AND** 弹出通用菜单组件，items 由选区菜单 factory 构造
- **AND** 全局兜底（Layer 3）SHALL 检测 defaultPrevented 并 skip 自身处理

#### Scenario: 选中文本后右键已挂 surface action 的元素

- **WHEN** 用户先选中一段文本，再在挂了 surface 右键 action 的元素上右键
- **THEN** Layer 1 surface action SHALL 优先触发并 stopPropagation
- **AND** Layer 2 SHALL **不**触发（事件不冒泡到 window）
- **AND** Surface 的 factory SHALL 检测有选区并在首段首项前动态插入"复制选中文本" item
- **AND** 用户感知：弹 surface 菜单 + 含"复制选中文本"项

#### Scenario: 无选区时 Layer 2 跳过

- **WHEN** 用户在未挂右键 action 的位置右键且选区为空
- **THEN** Layer 2 SHALL 跳过（不弹选区菜单不 preventDefault）
- **AND** Layer 3 全局兜底 SHALL 接管 preventDefault，不弹任何菜单

#### Scenario: 选中文本后右键 input/textarea 走原生菜单

- **WHEN** 用户在 input / textarea / contenteditable 元素内选中文本后右键
- **THEN** Layer 2 SHALL 通过 `target.closest()` 检测并跳过
- **AND** 浏览器原生菜单 SHALL 正常弹出（粘贴 / 拼写检查 / 朗读等）

#### Scenario: HMR 重复注册幂等

- **WHEN** 开发模式 HMR 触发模块重载，选区菜单初始化入口再次被调用
- **THEN** SHALL 通过 window sentinel flag 检测已注册并跳过
- **AND** window 上仅存在一个 selection contextmenu listener

### Requirement: menu-items 函数库

应用 SHALL 按 surface 拆分提供右键菜单 items factory 函数（用户消息 / 助手消息 / Bash 工具 / 文件类工具 / worktree chip / project card / 选区 等），每个返回 items 数组。所有 factory 接受统一上下文（含 sessionId / projectId / settings / 5 个 IPC 调用闭包：copyToClipboard / openInEditor / openInTerminal / revealInDir / openUrl + selectionText 当前选区文本快照），让 item.action 自包含——factory 内**不**直接 import IPC 模块、**不**直接读 DOM（含 `getSelection` / `activeElement`），所有 IPC 走 ctx.dispatch 间接调用以便单测 mock，所有运行时浏览器状态 SHALL 通过 ctx 字段传入。Factory SHALL 是纯函数：给定输入 → 确定输出，不持有外部状态、不读 DOM。

调用方 SHALL 在 oncontextmenu 触发瞬间预先读选区文本后通过 ctx.selectionText 传入 factory，统一 selection 快照源避免 factory 内部读 DOM 的 jsdom / SSR / 测试稳定性问题。

#### Scenario: factory 返回纯数据

- **WHEN** 单测调用某 factory 传入 mock ctx 含 mock dispatch
- **THEN** 返回值 SHALL 是 items 数组
- **AND** items 内的 action 闭包仅引用 ctx.dispatch 与传入的数据，不调真 IPC
- **AND** mock dispatch 后调用 action SHALL 只触发 mock 函数，不发真 IPC
- **AND** 单测 SHALL **不**需要 jsdom 的 getSelection polyfill

#### Scenario: separator 自动插入按 kind 分组

- **WHEN** factory 返回的 items 含相邻 kind 不同的 item（典型 copy 后跟 navigate）
- **THEN** factory 内部 SHALL 在 kind 切换处插入 separator
- **AND** factory SHALL trim 首尾孤立 separator

#### Scenario: 有选区时融合"复制选中文本"

- **WHEN** 调用方在 oncontextmenu 触发瞬间读选区文本长度 > 0 后调 factory（含 selectionText）
- **THEN** factory SHALL 在首段（kind=copy）首项前动态插入"复制选中文本" item（含快捷键 hint）
- **AND** 该 item 的 action 调 ctx.dispatch.copyToClipboard

#### Scenario: 无选区时不插入选区项

- **WHEN** 调用方传入 selectionText 为空字符串
- **THEN** factory SHALL **不**插入"复制选中文本" item
- **AND** 返回 items 与 selectionText 为空字符串时调用结果一致（确定性纯函数）

### Requirement: ContextMenuItem 类型扩展

通用菜单 item 类型 SHALL 扩展四个 optional 字段：shortcut（右侧灰色快捷键 hint，仅 display 不绑定真实快捷键）/ submenu（二级菜单数组，有值时 action 与 shortcut 忽略）/ kind（语义分类 copy / navigate / external，factory 内部 separator 插入用，菜单组件不消费）/ pathLabel（路径中段截断形态，含 short 与 full，渲染层用 short 做 label + full 做 tooltip）。所有新字段 SHALL 是 optional，已落地的侧栏 / tab 右键菜单 SHALL 无需改动即兼容。

#### Scenario: shortcut 字段渲染

- **WHEN** item 含 shortcut 字段
- **THEN** 菜单组件 SHALL 在 item 行内右对齐显示该字符串
- **AND** 文字颜色 SHALL 为 muted、字体为等宽 11px
- **AND** 无 shortcut 字段时该位置留空（不渲染空 placeholder）

#### Scenario: submenu 字段渲染 chevron

- **WHEN** item 含非空 submenu 数组
- **THEN** 菜单组件 SHALL 在 item 行内右对齐渲染 `›` chevron 指示器
- **AND** 该 item 的 action 与 shortcut 字段 SHALL 被忽略
- **AND** chevron 与 shortcut hint 互斥（同 item 不会同时渲染）

#### Scenario: pathLabel 字段渲染中段截断

- **WHEN** item 含 pathLabel（短形式 + 完整路径）
- **THEN** 菜单组件 SHALL 用 short 作为 label 显示文本（覆盖 label 字段）
- **AND** SHALL 加 title 属性让 hover 浮 tooltip 显示完整路径

#### Scenario: kind 字段不渲染

- **WHEN** item 含 kind 字段
- **THEN** 菜单组件 SHALL **不**消费 kind 字段（无视觉变化）
- **AND** factory 内部按 kind 决定 separator 插入位置

### Requirement: AppContextMenu submenu 渲染

通用菜单 SHALL 扩展支持 submenu 渲染：检测 item.submenu 非空时挂 chevron + 进入 hover 状态后短延迟弹出二级菜单（具体阈值见 Scenario，同样通过 mount 到 document.body）；ArrowRight SHALL 即时打开 submenu + focus 进 submenu 首项；ArrowLeft SHALL 关闭 submenu + focus 还回 parent；Esc SHALL 关闭整棵菜单树；submenu 视觉规格与父菜单完全相同（同 bg / border / radius / shadow），不做层级递进。submenu 渲染深度 SHALL 限制为 ≤ 2。

#### Scenario: hover 短延迟打开 submenu

- **WHEN** 用户鼠标 hover 含 submenu 的 item 持续 200ms
- **THEN** SHALL 在 parent item 右侧弹出 submenu 浮层
- **AND** parent item SHALL 保持 active bg 锁定直到 submenu 关闭
- **AND** viewport 右边距不足时 submenu SHALL 翻转到左侧展开

#### Scenario: ArrowRight 即时打开 + focus 进 submenu

- **WHEN** 用户键盘导航至含 submenu 的 active item，按 ArrowRight
- **THEN** submenu SHALL 立即弹出（无 200ms 延迟）
- **AND** focus SHALL 进入 submenu 首项

#### Scenario: ArrowLeft 关闭 submenu + focus 回 parent

- **WHEN** submenu 已打开且 focus 在 submenu 内某项，用户按 ArrowLeft
- **THEN** submenu SHALL 关闭；focus SHALL 还回 parent item

#### Scenario: Esc 关闭整棵菜单树

- **WHEN** submenu 已打开，用户按 Esc
- **THEN** submenu 与 parent 菜单 SHALL 同时关闭；focus SHALL 还回 trigger 元素

#### Scenario: submenu 视觉与父菜单完全一致

- **WHEN** submenu 渲染
- **THEN** SHALL 复用父菜单的 bg / border / radius / padding / shadow token
- **AND** SHALL **不**加深 bg 或追加额外 shadow（遵守 `DESIGN.md::§1 Overview` flat + tonal layering 原则）

#### Scenario: 渲染深度上限 2

- **WHEN** 调用方传入 nested submenu 三层以上
- **THEN** 菜单组件 SHALL 在 depth 2 后忽略后续 submenu 字段
- **AND** depth 2 的 item 即使含 submenu 也按 leaf item 渲染

### Requirement: AppContextMenu 视觉规格扩展

通用菜单 Phase 2 视觉规格 SHALL 在 Phase 1 基础上扩展：(a) min-width 200px / max-width 320px，超长 label 用 ellipsis 截断；(b) shortcut hint 行内右对齐 + muted + 等宽 11px；(c) submenu chevron 行内右对齐与 shortcut hint 互斥渲染；(d) keyboard active state 维持 Phase 1 outline + hover bg。Phase 2 SHALL **不**引入 icon 渲染也 **不**引入 danger item 视觉首落地。

#### Scenario: max-width 截断长 label

- **WHEN** item label 长度超过菜单最大宽度渲染
- **THEN** label SHALL 通过 `nowrap + overflow hidden + ellipsis` 末段截断
- **AND** 路径类 item 用 pathLabel 字段提前 JS 中段截断（CSS 末段截断作 fallback）

#### Scenario: 暗色模式视觉规格不变

- **WHEN** 应用切到暗色主题
- **THEN** 菜单 SHALL 用暗色对应 token（surface / border / shadow）
- **AND** submenu 视觉与父菜单完全相同（不加深 bg）

### Requirement: open_in_terminal IPC 契约

应用 SHALL 暴露 `open_in_terminal` Tauri command（HTTP 镜像同名）：入参 `{ path: String }`（绝对路径）；返回 `Result<(), ApiError>`。command handler 在后端 SHALL 校验 path 是绝对路径 + canonicalize 解析（拒绝相对路径与不存在路径），从配置读取首选终端 app 设置后按平台 dispatch 子进程（macOS / Windows / Linux 各按平台默认终端 app + 工作目录参数形态）。

**安全不变量**：command 入参 SHALL **不**接受任意 shell command 字符串，仅接受 path；macOS / Linux / Windows Terminal 一律 OS-argv 传参（零注入）；Windows cmd / PowerShell fallback SHALL 通过环境变量传入 path，命令字符串内仅引用环境变量、**严禁**把 path 拼进命令字符串以避免 shell parser 解释 metacharacters；Windows cmd fallback 在 path 含 cmd metacharacters（典型 `&` / `|` / `<` / `>` / `^` / `(` / `)` / `%` / `!` / `'` / `"` / 换行）时 SHALL 直接拒绝返回 ValidationError（cmd parser 在 env var 展开后仍 re-tokenize，无法 100% 安全）。

#### Scenario: macOS 调用 Terminal

- **WHEN** macOS 上配置首选终端为 Terminal，前端调 open_in_terminal 携带绝对目录路径
- **THEN** 后端 SHALL 用 OS-argv 形态启动 Terminal 打开该目录
- **AND** Terminal app SHALL 弹窗口 cd 到目标目录
- **AND** 返回成功

#### Scenario: Windows 三级 fallback

- **WHEN** Windows 上配置首选终端为 Windows Terminal
- **THEN** 后端 SHALL 优先尝试 Windows Terminal 命令打开该目录
- **AND** 失败时（命令未装 / 当前不可用）SHALL fallback 到 PowerShell；再失败时 SHALL fallback 到 cmd
- **AND** 三级全失败返回 ExternalApp 错误含 reason

#### Scenario: 相对路径拒绝

- **WHEN** 前端调 open_in_terminal 携带相对路径
- **THEN** 后端 SHALL 校验失败返回 ValidationError
- **AND** **不** spawn 任何子进程

#### Scenario: 不存在路径返 NotFound

- **WHEN** 前端调 open_in_terminal 携带不存在路径
- **THEN** 后端 canonicalize SHALL 失败返回 NotFound 错误

#### Scenario: 文件路径自动取父目录

- **WHEN** 前端调 open_in_terminal 携带文件路径（非目录）
- **THEN** 后端 SHALL 降级到取父目录路径
- **AND** 终端 app 打开父目录

#### Scenario: 跨平台 terminalApp 不匹配 fallback

- **WHEN** macOS 上配置首选终端为非本平台值（典型 Windows / Linux 同步配置过来）
- **THEN** 后端 SHALL warn 级日志记录 mismatch
- **AND** fallback 到当前平台默认终端继续 spawn
- **AND** **不**返回错误

### Requirement: open_in_editor IPC 契约

应用 SHALL 暴露 `open_in_editor` Tauri command（HTTP 镜像同名）：入参 `{ path: String, line: Option<u32>, column: Option<u32> }`；返回 `Result<(), ApiError>`。command handler SHALL 校验 path（同 open_in_terminal），从配置读取外部编辑器后按白名单 dispatch CLI（VS Code / Cursor / Zed / Sublime → 各自 `--goto path:line:col` 等价命令；system → OS 默认开启命令，行号参数忽略）。line 为 None 时 SHALL 省略行号后缀；CLI 不存在 SHALL 返回 ExternalApp 错误引导用户去 Settings 修改。

#### Scenario: 编辑器跳行号

- **WHEN** 配置外部编辑器为 VS Code，前端调 open_in_editor 携带 path + 行 + 列
- **THEN** 后端 SHALL 调对应 CLI 命令使其打开文件并跳到目标行/列

#### Scenario: 行号缺失时省略后缀

- **WHEN** 前端调 open_in_editor 不携带行号 / 列
- **THEN** 后端 SHALL 调编辑器命令仅传 path，不附加 `:line:col` 后缀

#### Scenario: System fallback OS 默认

- **WHEN** 配置外部编辑器为 system
- **THEN** 后端 SHALL 走 macOS / Windows / Linux 各自的 OS 默认开启命令
- **AND** line / column 参数 SHALL 被忽略

#### Scenario: 编辑器 CLI 未装返 ExternalApp

- **WHEN** 配置外部编辑器对应 CLI 不在 PATH
- **THEN** 后端 spawn 失败 SHALL 返回 ExternalApp 错误引导用户
- **AND** 前端 SHALL 弹 toast 显示该 message

#### Scenario: spawn 非阻塞

- **WHEN** 后端调外部编辑器 spawn
- **THEN** SHALL 立即返回成功，不等待编辑器进程退出
- **AND** 编辑器进程 SHALL 独立于本应用生命周期运行

### Requirement: list_available_terminals IPC 契约

应用 SHALL 暴露 `list_available_terminals` Tauri command（HTTP 镜像同名）：入参空；返回当前 OS 合法终端枚举的字符串数组（snake_case）。前端 Settings dropdown 用此 IPC 过滤选项，避免在某平台显示其它平台的终端选项。

#### Scenario: 当前平台过滤

- **WHEN** 前端调 list_available_terminals
- **THEN** 后端 SHALL 按当前平台返回合法集合（macOS / Windows / Linux 各自子集）
- **AND** 返回值 SHALL 仅含当前平台的合法终端枚举值


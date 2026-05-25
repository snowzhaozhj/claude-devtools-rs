# keyboard-shortcuts Specification

## Purpose
TBD - created by archiving change add-keyboard-shortcut-system. Update Purpose after archive.
## Requirements
### Requirement: 集中式快捷键注册中心

UI 全局键盘快捷键 SHALL 通过统一注册中心管理。每个全局快捷键 SHALL 在应用启动阶段（或其 owning controller 的 mount 阶段）调用注册接口注册一次；运行期 SHALL NOT 在组件级别监听全局 mod-key 组合。注册中心 SHALL 在内部维护 `NormalizedKey → ShortcutSpec` 的映射并以单一全局 keydown listener 在 document 上 dispatch。

注册中心 SHALL 强制 **单 binding 单 spec 1:1 关系**：同一 NormalizedKey 在任意时刻只能由一个 spec 占位；如需"同键不同作用域"分派，SHALL 由该 spec 的 handler 内部按当前焦点 / store state / 当前 active surface 等条件判断分派；handler 返回 false 仅让 dispatcher 不 preventDefault 给浏览器原生行为放行，**不会** fallthrough 到"另一个注册的 spec"。

跨 pane / 多 instance 共享同一 ID 的 spec SHALL 由其 owning controller 层注册一次 shared handler，handler 内部按 active 实例分派。**禁止**多 instance 各自重复注册同一 ID——会触发 "重复 ID" 错误。

ShortcutSpec 数据模型 SHALL 包含字段：id（唯一标识，kebab-case，形如 `<category>.<action>`）、category（5 个固定枚举：global / tabs / sidebar / search / session）、description（用户可读，简体中文）、defaultBinding（字符串或按平台分写形态）、handler（事件回调，返回 false 仅取消 preventDefault）、allowInInput（默认 false）、preventDefault（默认 true）。

#### Scenario: 启动时注册并 dispatch

- **WHEN** 应用启动并完成 bootstrap 调用
- **THEN** 注册中心 SHALL 暴露所有内置 ShortcutSpec 的 keymap snapshot
- **AND** 单一 keydown listener SHALL 已挂在 document 上、bubble phase
- **AND** 任意组件 SHALL NOT 重复 listen 全局 mod-key 组合

#### Scenario: 注册重复 ID 报错

- **WHEN** 同一 id 被注册两次
- **THEN** 第二次注册 SHALL 抛错并指明重复 id

#### Scenario: 运行期更新绑定

- **WHEN** 调用 update 接口替换某 id 的 binding
- **THEN** 内存 keymap SHALL 立即反映新绑定
- **AND** 旧绑定的 NormalizedKey 入口 SHALL 被清除
- **AND** 后续 keydown 命中新绑定 SHALL 触发对应 handler

### Requirement: 跨平台修饰键归一化

注册中心 SHALL 提供 `mod` 关键字作为跨平台修饰键抽象：在 macOS 平台 `mod` SHALL 映射为 Meta 键（Command）、在 Windows / Linux SHALL 映射为 Control 键。归一化层 SHALL 提供以下行为入口（具体函数命名属实现细节）：

- **事件归一化**：把 KeyboardEvent 转换为按字母顺序排列的"修饰键 + 主键"字符串（如 mac 上 `meta+shift+k`），供 keymap 索引
- **binding 归一化**：把 binding 字面量中的 `mod` 展开为当前平台对应键并按相同规则排序
- **录键产物**：把 KeyboardEvent 转换为**跨平台 `mod` 字面量** binding（即 mac `meta+` → `mod+`、win/linux `ctrl+` → `mod+`），仅按下 modifier（无主键）时返 null。该入口 SHALL 是录键 widget commit binding 的唯一来源——录键产物 SHALL NOT 直接使用平台特化字面量，确保配置文件持久化 binding 跨平台一致
- **存量字面量迁移**：把存量平台特化字面量（mac 上录的 meta 同步到 Windows，或老版本录的 ctrl）转为 `mod` 表达，行为契约：
  1. 幂等（多次调用结果相同）
  2. 跨平台一致（在任意平台跑结果相同）
  3. 保留辅助修饰键（alt / shift / 双修饰键里非主修饰键的 ctrl）
  4. token-level 不重排（重排由 binding 归一化在 register 时统一处理）
  5. 主修饰键优先级 meta > ctrl（不含主修饰键的 binding 原样返回）
- **平台展示**：在 macOS 输出符号字面量（⌘ / ⌥ / ⇧ / ⌃）+ 大写字母，在 Windows / Linux 输出 `Ctrl+` / `Alt+` / `Shift+` 文本前缀 + 大写字母。Space 在 macOS 输出 `␣`、其他平台输出 `Space` 文本

#### Scenario: macOS / Windows / Linux 平台 mod 展开

- **WHEN** 调用 binding 归一化入口对 `mod+<key>` 形态
- **THEN** macOS SHALL 返回 `meta+<key>`，Windows / Linux SHALL 返回 `ctrl+<key>`

#### Scenario: 修饰键顺序归一化

- **WHEN** 用户按下含多 modifier 的组合键
- **THEN** 归一化输出 SHALL 按字母顺序排列 modifier
- **AND** 注册时 binding 字面量内 modifier 顺序变化 SHALL 命中同一 entry

#### Scenario: 平台展示输出符号 / 文本前缀

- **WHEN** 在 macOS / Windows / Linux 上调用展示入口
- **THEN** macOS 返回符号字面量（⌘ / ⌥ / ⇧ / ⌃）+ 大写字母；Windows / Linux 返回文本前缀（Ctrl+ / Alt+ / Shift+）+ 大写字母

#### Scenario: 物理位置受影响键以 event.code 兜底

- **WHEN** 用户按下物理位置敏感键（典型 `[` / `]` / `\` / `/` / 数字行 1-9 / `-` / `=`）
- **THEN** 事件归一化 SHALL 优先使用 event.code 兜底（非 QWERTY 布局下 event.key 可能不一致）

#### Scenario: 录键产出跨平台 mod 字面量

- **WHEN** 用户在录键 widget 内按下含主修饰键的组合键
- **THEN** 录键产物 SHALL 是跨平台 `mod` 字面量（不是平台特化字面量）

#### Scenario: 仅按下 modifier 时录键产物为 null

- **WHEN** 用户按下单 modifier 键（无主键）
- **THEN** 录键入口 SHALL 返回 null，widget SHALL 继续等待主键

#### Scenario: 字面量迁移幂等且跨平台一致

- **WHEN** 调用迁移入口对存量字面量
- **THEN** 含 meta 主修饰键的 binding（典型 `meta+shift+p`）SHALL 转为 `mod+shift+p`
- **AND** 含 ctrl 主修饰键的 binding（典型 `ctrl+k`）SHALL 转为 `mod+k`
- **AND** 已含 `mod` 的 binding SHALL 幂等返回（多次调用结果相同）
- **AND** 不含 meta / ctrl 主修饰键的 binding（典型 `alt+x` / `shift+x` / 单字符 / 功能键）SHALL 原样返回

#### Scenario: 字面量迁移保留辅助修饰键

- **WHEN** 调用迁移入口对含 alt 或 ctrl 辅助修饰键的 binding
- **THEN** SHALL 仅替换主修饰键（meta 优先于 ctrl），保留 alt / 非主修饰键的 ctrl 作为辅助

#### Scenario: 字面量迁移处理异常 meta+mod 共存

- **WHEN** 调用迁移入口对异常字面量（`meta+mod+x` 同时含 meta 和 mod）
- **THEN** SHALL 移除多余 meta token，保留 mod

#### Scenario: macOS 录键产出双修饰键 mod 字面量

- **WHEN** macOS 用户按下 Cmd+Ctrl+某主键
- **THEN** 录键产物 SHALL 仅反写主修饰键 meta → mod，保留 ctrl token（`ctrl+mod+<key>` 在 mac 展开等价原事件）

#### Scenario: macOS / Windows Space 展示

- **WHEN** 调用展示入口对 `mod+Space`
- **THEN** macOS 返回 `⌘␣`，Windows / Linux 返回 `Ctrl+Space`

### Requirement: dispatcher 命中与守卫

注册中心 dispatcher SHALL 在 document 的 keydown 事件 **bubble phase** 按顺序执行守卫与命中流程：

1. **IME composition 守卫**：若 isComposing 或 keyCode === 229，SHALL 直接 return 不进 dispatch
2. **key-repeat 守卫**：若 event.repeat === true（长按系统连发），SHALL 直接 return
3. **归一化**：转换为 NormalizedKey 字符串
4. **查表**：以 NormalizedKey 查 keymap，若无命中 SHALL return
5. **input 焦点守卫**：若命中 spec 的 allowInInput !== true 且当前焦点是输入元素（input / textarea / contenteditable），SHALL return
6. **handler 调用**：调用 spec.handler，若返回 false SHALL return（spec 显式选择不消费）
7. **preventDefault**：若 spec 的 preventDefault !== false，SHALL 调用 event.preventDefault()

bubble phase 监听 SHALL 让组件级局部 listener（CommandPalette / Modal / SearchBar 内部的 Escape / Enter / 方向键）先于 dispatcher 命中；组件 SHALL 通过 stopPropagation 阻止局部键继续传播到 dispatcher。

事件归一化 SHALL 执行以下平台 / 物理键规则：

- **non-mac 平台禁止把 metaKey 识别为 mod**：非 mac 时 metaKey 为 true SHALL 不被加入 modifier 列表（防 Win 键 / 神秘键盘的误触发）；mac 平台 metaKey SHALL 被识别为 meta
- **Numpad 数字键 / 功能键归一化**：Numpad 数字键 SHALL 归一为顶部数字行同义；NumpadEnter / NumpadAdd / NumpadSubtract / NumpadMultiply / NumpadDivide / NumpadDecimal SHALL 归一为对应主行键同义；录键 widget 录入 Numpad 系列时 SHALL 同步使用归一化结果

dispatcher 命中路径（步骤 3-5）SHALL 在常态下足够轻（前端单测 microbench 守门，毫秒级）。

#### Scenario: IME composition 期间不 dispatch

- **WHEN** 用户在中文输入法激活时按下任意键
- **THEN** dispatcher SHALL 直接 return
- **AND** 已注册 spec 的 handler SHALL NOT 被调用
- **AND** preventDefault SHALL NOT 被调用

#### Scenario: input 焦点跳过非 allowInInput 快捷键

- **WHEN** 当前焦点是 input / textarea / contenteditable 元素
- **AND** 用户按下命中 spec 的 NormalizedKey（spec.allowInInput 为 false / undefined）
- **THEN** dispatcher SHALL return；handler SHALL NOT 被调用
- **AND** 浏览器原生行为 SHALL 不被打断

#### Scenario: allowInInput=true 的快捷键在 input 内仍触发

- **WHEN** 当前焦点是输入元素
- **AND** 用户按下命中 spec 的 NormalizedKey（spec.allowInInput 为 true）
- **THEN** dispatcher SHALL 继续走 handler；preventDefault SHALL 被调用

#### Scenario: handler 返回 false 不触发 preventDefault

- **WHEN** 命中 spec 的 handler 返回 false
- **THEN** dispatcher SHALL NOT 调用 preventDefault；浏览器 / 上层 bubble listener SHALL 仍能处理该事件

#### Scenario: 未注册的快捷键不 preventDefault

- **WHEN** 用户按下任意未注册的 mod-key 组合
- **THEN** dispatcher SHALL return（无命中）；浏览器原生行为 SHALL 不被打断

#### Scenario: key-repeat 不 dispatch

- **WHEN** 用户长按某组合键，浏览器以系统重复速率连发 keydown
- **THEN** 仅首次（event.repeat === false）SHALL 触发 handler；之后的 repeat 事件 SHALL 被跳过

#### Scenario: 非 macOS 平台 metaKey 不被识别为 mod

- **WHEN** 平台为 Windows，用户按下 Windows / Super 键 + 某主键
- **THEN** 事件归一化 SHALL NOT 把 meta 加入 modifier 列表
- **AND** 该事件 SHALL 命中无 mod 修饰的 NormalizedKey 而非 `meta+<key>`

#### Scenario: macOS metaKey 被识别为 mod

- **WHEN** 平台为 macOS，用户按下 Cmd+某主键
- **THEN** 事件归一化 SHALL 把 meta 加入 modifier；NormalizedKey 含 meta 前缀；dispatcher SHALL 命中对应 spec

#### Scenario: Numpad 数字 / 功能键归一化

- **WHEN** 用户按下 Numpad 数字键或功能键（NumpadEnter / Add / Subtract / Multiply / Divide / Decimal）
- **THEN** 事件归一化 SHALL 与顶部数字键 / 主行同名键同义
- **AND** 命中同一 NormalizedKey 入口

#### Scenario: dispatcher bubble phase 让组件 listener 先命中

- **WHEN** Modal 打开且 Modal 自身 keydown listener 已挂在 modal 容器上，用户按下 Escape
- **THEN** Modal 自身 listener SHALL 在事件冒泡到 document 之前先处理
- **AND** 若 Modal listener stopPropagation，dispatcher SHALL NOT 收到该事件

### Requirement: 用户自定义覆盖

用户 SHALL 可以为任何已注册 ID 自定义新绑定。覆盖 SHALL 持久化到配置 `keyboardShortcuts: HashMap<id, binding>` 字段（仅存 diff，未覆盖的 ID 走内置 default）；通过配置读取 / 更新 IPC 通道下发，camelCase。持久化 binding 字面量 SHALL 用 `mod` token 表达跨平台主修饰键（与内置 default binding 表达一致）；录键产物 SHALL 已是 `mod` 表达，save handler SHALL NOT 在 IPC 写入前再次归一。

启动时 effective keymap SHALL 由 mergeOverrides(defaults, overrides) 计算：

1. 对每个内置 ID，若 overrides 中存在该 ID，SHALL 使用 override binding；否则使用 default binding
2. **存量字面量迁移**：mergeOverrides SHALL 对每个 override binding 调字面量迁移入口把存量平台特化字面量（mac 上录的 meta 同步到 Windows，或老版本 UI 录的 ctrl）转为 `mod`。该迁移 SHALL 幂等且无信息丢失（详 `跨平台修饰键归一化`）。运行期单次更新 SHALL 同样调迁移入口作为护栏
3. overrides 中包含未在内置注册的"幽灵" ID（典型老版本删除的 ID）SHALL 被忽略不报错
4. **IPC 失败 fallback**：若读取配置抛异常（IPC 层失败 / 反序列化失败 / 文件不可读），bootstrap SHALL 让注册中心走纯 builtin defaults 启动；UI 在 Settings → Keyboard Shortcuts tab 顶部 SHALL 显示非阻塞错误条 + 重试入口；点击重试 SHALL 重调读取配置，成功时 SHALL mergeOverrides + bootstrap 重新建表；应用启动 SHALL NOT 因为该 IPC 失败而阻断或弹模态错误

持久化路径 SHALL 由 Settings → Keyboard Shortcuts tab 的 **Save 按钮显式提交**触发——录键过程的修改仅保留在 panel 内存的 pending overlay；点 Save 时 SHALL 单次配置更新 IPC 写入全部 pending 改动 + 一次性把内存 keymap 切到新值。**SHALL NOT** 在录键 commit 时 debounce 自动写——避免"用户改了一半切到其它 tab，配置已经留下半成品 override"。

#### Scenario: 仅持久化覆盖

- **WHEN** 用户改动某 ID 的 binding，其它内置 ID 均未改动
- **THEN** 配置 keyboardShortcuts 视图 SHALL 仅包含该 ID → 新 binding 一项
- **AND** 用户从未改动任何快捷键时视图为 empty 序列化为 `{}`（详 [[configuration-management]]）

#### Scenario: 启动时 merge defaults + overrides

- **WHEN** 配置 keyboardShortcuts 持有部分 ID 的覆盖
- **AND** 应用启动调用 bootstrap
- **THEN** effective keymap SHALL 让覆盖的 ID 用 override binding、其他 ID 用 builtin defaults

#### Scenario: 幽灵 ID 被忽略

- **WHEN** 配置 keyboardShortcuts 持有一个老版本删除 / 当前未注册的 ID
- **AND** 应用启动调用 bootstrap
- **THEN** 启动 SHALL 不报错；该 entry SHALL 被跳过

#### Scenario: Save 显式提交单次 IPC 写入

- **WHEN** 用户在录键 widget 内连续改动若干不同 ID 的绑定（pending 累计）
- **AND** 用户点击 Save 按钮
- **THEN** SHALL 触发**单次**配置更新 IPC 写入包含全部 pending 改动
- **AND** 内存 keymap SHALL 在 IPC resolved 后一次性切到新值（IPC 失败 SHALL 回滚 pending）
- **AND** 用户未点 Save 就切走 / 关闭 Settings SHALL NOT 触发 IPC 写入

#### Scenario: IPC 失败 fallback builtin defaults

- **WHEN** 应用启动调用读取配置 IPC 但抛异常
- **THEN** 注册中心 SHALL 用 builtin defaults bootstrap，不阻断启动
- **AND** dispatcher SHALL 对所有 builtin 快捷键正常工作
- **AND** Settings → Keyboard Shortcuts tab 顶部 SHALL 显示非阻塞错误条 + 重试入口
- **AND** 用户点击重试且读取成功 SHALL 重调 mergeOverrides + bootstrap，错误条 SHALL 消失

#### Scenario: 存量字面量启动迁移为 mod

- **WHEN** 配置 keyboardShortcuts 持有存量平台特化字面量（典型 mac 用户录的 meta+ 同步到 Windows，或旧版 Windows 用户录的 ctrl+）
- **AND** 应用启动调用 bootstrap
- **THEN** mergeOverrides SHALL 把该 binding 归一为 mod 表达
- **AND** effective keymap 在当前平台展开后 SHALL 与原事件等价，用户按对应组合 SHALL 命中对应 spec

### Requirement: 冲突检测

注册中心 SHALL 提供冲突查询入口：在 effective keymap 与可选 overlay（典型 pending overrides 视图）合并后的视图上查重，返回该 binding 已被占用的另一 ID（若无占用返回 null），可选传入排除自身 id。

冲突检测 SHALL 在两个时机触发：

1. **录键时**（UI 层）：用户在录键 widget 录入新 binding 后，Settings panel SHALL 把当前 panel 的 pending overrides 作为 overlay 参数传入，实时显示冲突反馈，保存按钮 SHALL disabled 直到冲突解掉
2. **保存时**（注册中心层）：save handler SHALL 在 IPC 写入之前对 pending overrides 中每个 entry 再走一遍冲突检测；任意冲突 SHALL 让 save 返回 Err（含冲突 id 与源 id），UI 切回 conflict 态、SHALL NOT 触发 IPC 写入

**关键约束**：录键时 SHALL 把 pending overrides 一并算进检测视图。否则用户先把 ID-A 改成 X、再把 ID-B 改成 X：两次都基于 effective 看不出冲突（因 ID-A 还没 commit），但 Save 后两条直接冲突。合并 pending overlay 让录键时就能拦住第二次冲突输入。

v1 SHALL NOT 提供"接受覆盖"路径——用户必须先解掉冲突（清空对方或改对方）才能保存自己的新键。

#### Scenario: 冲突查询命中 / 排除自身

- **WHEN** keymap 中已注册某 binding → 某 ID
- **AND** 调用冲突查询入口对该 binding 不传排除自身
- **THEN** SHALL 返回已占用的 ID
- **WHEN** 调用冲突查询入口对该 binding + 排除占用者自身
- **THEN** SHALL 返回 null

#### Scenario: 单次更新冲突时返 Err

- **WHEN** keymap 中已注册某 binding → ID-A
- **AND** 调用单次更新接口尝试把 ID-B 改成同 binding
- **THEN** 接口 SHALL 返回 Err（含冲突类别与冲突 id）
- **AND** 内存 keymap SHALL 不变；配置 SHALL NOT 被写入

#### Scenario: pending overlay 串行冲突检测

- **WHEN** effective keymap 中若干 ID 各自占用不同 binding
- **AND** 用户在 panel 录键：先把 ID-X 改为某 binding（pending overlay 加一条）
- **AND** 再把 ID-Y 改为同一 binding（试图与 pending 中的 ID-X 冲突）
- **THEN** 第二次录键时冲突查询 SHALL 在 effective + pending overlay 合并视图上检测，SHALL 返回 ID-X
- **AND** UI SHALL 切到 conflict 态、Save 按钮 SHALL disabled
- **AND** 用户继续 Save SHALL 失败（save handler 二次校验拦截）

### Requirement: 录键守卫

录键 widget 在 recording 状态期间 SHALL 调用注册中心的 suspend 接口暂停 dispatcher（避免录入的 mod-key 错误触发已注册 spec）；recording 退出（commit 或 cancel）SHALL 调 resume 恢复。recording 期间 SHALL preventDefault 阻止字符落入 input。多次 suspend SHALL 不互相覆盖（引用计数）。

录键 widget 在 recording 态调录键产物入口**之前**做 Win 键守卫：当平台为非 macOS 且事件 metaKey 为 true 时，SHALL NOT commit binding、SHALL NOT 调 blur、SHALL NOT 退出 recording 态；widget 内部 SHALL 切到 warning 子态显示提示文本（说明 Windows 不支持 Win 键作为修饰键）。warning 提示 SHALL 通过屏幕阅读器友好的 live region 宣告（避免在 focus / pressed 等容器属性变化时双宣告 noise）。

warning 子态 SHALL 在以下场景自动清除：

1. 用户按下不含 metaKey 的下一次 keydown（无论是否触发 commit——仅按修饰单键也清除）
2. 用户按 Esc 退出 recording
3. recorder blur / Tab 失焦

warning 子态视觉 SHALL 复用 `DESIGN.md::The Conflict Is Warning Not Error Rule` 的 token，不引入新 token 与新 Named Rule。提示文本优先级 SHALL 为 winKeyWarning > conflict > recording > idle。

#### Scenario: 录键期间不触发已注册快捷键

- **WHEN** 用户进入录键 recording 态，按下已被某 spec 占用的组合键
- **THEN** dispatcher SHALL NOT 调用该 spec 的 handler
- **AND** 录键 widget SHALL 把组合键录入并显示冲突反馈

#### Scenario: 录键退出后 dispatcher 恢复

- **WHEN** 录键完成（用户保存或取消）
- **THEN** resume 接口 SHALL 被调用
- **AND** 后续按下任意已注册组合 SHALL 正常 dispatch

#### Scenario: suspend / resume 引用计数

- **WHEN** 两个并存 widget 各自 suspend 后其中一个 resume
- **THEN** dispatcher SHALL 仍处于 suspended 状态（引用计数 > 0）
- **AND** 第二个 widget 也 resume 后 dispatcher SHALL 恢复

#### Scenario: Windows Win 键守卫（单独按下 / 与其它修饰键组合）

- **WHEN** 平台为 Windows，用户在 recording 态按下含 Win 键的组合
- **THEN** 录键 widget SHALL NOT commit、SHALL NOT blur、SHALL 保持 recording 态
- **AND** 切到 warning 视觉态 + 屏幕阅读器宣告提示文本

#### Scenario: macOS Cmd 不触发 Win 键守卫

- **WHEN** 平台为 macOS，用户在 recording 态按下 Cmd+某主键
- **THEN** 录键 widget SHALL 走正常 commit 路径
- **AND** 录键产物 SHALL 是 `mod+<key>` 字面量

#### Scenario: warning 子态自动清除

- **WHEN** Windows 用户按 Win 键组合触发 warning 子态
- **AND** 后续任一种条件发生：(a) 按下不含 metaKey 的下一次 keydown / (b) 按 Esc 退出 recording / (c) recorder blur
- **THEN** warning 子态 SHALL 清除
- **AND** Esc 路径 SHALL 调 resume 恢复 dispatcher 且 SHALL NOT commit 任何 binding

### Requirement: 内置快捷键清单

应用 SHALL 列出本期纳入注册中心的全部内置快捷键，覆盖以下分类与最小 ID 集（具体 binding 默认值由实现层决定，跨平台差异通过双 binding 表达）：

- **global**：命令面板切换（mod+主键）/ 侧栏切换（mod+主键）/ 搜索聚焦
- **tabs**：tab.switch.\<n\> 系列（mod+1 ~ mod+9）/ tab close（mod+主键）/ tab next / tab prev / pane split / pane focus next / pane focus prev
- **search**：会话内搜索（mod+主键）
- **session**：跳到最新（mac mod+方向键 / 其他 Ctrl+End，双平台 binding）

每条 spec SHALL 提供完整中文 description 字段，用于 Settings UI 列表渲染。

#### Scenario: 列表完整性

- **WHEN** 调用注册中心列出全部内置 spec
- **THEN** SHALL 返回上述 5 个 category 共 ≥ 14 条 spec
- **AND** 每条 spec SHALL 含非空 id / category / description / defaultBinding

#### Scenario: 双平台 binding

- **WHEN** spec 的 defaultBinding 含双平台分写
- **THEN** macOS 平台 SHALL 解析为 mac 版本（典型 meta + 方向键）；Windows / Linux 平台 SHALL 解析为对应版本（典型 Ctrl + End）

### Requirement: 局部 keydown 不并入 registry

下列局部键盘交互 SHALL 保持各组件自行 listen，不并入注册中心：

- 任意 modal / dropdown / popover / context menu / lightbox 的 Escape 关闭
- 命令面板内部方向键 / Enter 选项导航
- 搜索栏内部 Enter / Shift+Enter 跳转匹配项
- tab / session 等 surface context menu 内部方向键导航

注册中心 SHALL NOT 暴露 when-clause / scope 表达式；以上局部键作用域仅为该 surface focus 时，与全局 dispatcher 心智模型不一致。

#### Scenario: Modal Escape 不依赖 registry

- **WHEN** Modal 处于打开状态，用户按下 Escape
- **THEN** Modal SHALL 由自身 listener 关闭
- **AND** registry dispatcher SHALL 不参与该事件

#### Scenario: 命令面板方向键不走 registry

- **WHEN** 命令面板打开，用户按方向键
- **THEN** 命令面板 SHALL 由自身 listener 切换选中项
- **AND** dispatcher SHALL 不命中任何已注册 spec


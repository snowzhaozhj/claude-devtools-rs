# keyboard-shortcuts Specification

## Purpose
TBD - created by archiving change add-keyboard-shortcut-system. Update Purpose after archive.
## Requirements
### Requirement: 集中式快捷键注册中心

UI 全局键盘快捷键 SHALL 通过统一的 `ui/src/lib/keyboard/registry.ts` 注册中心管理。每个全局快捷键 SHALL 在应用启动阶段（或其 owning controller 的 mount 阶段）调用 `registerShortcut(spec)` 注册一次；运行期 SHALL NOT 通过 `document.addEventListener("keydown", ...)` 在组件级别监听全局 mod-key 组合。注册中心 SHALL 在内部维护 `Map<NormalizedKey, ShortcutSpec>` 并以单一全局 `keydown` listener 在 `document` 上 dispatch。

注册中心 SHALL 强制 **单 binding 单 spec 1:1 关系**：同一 NormalizedKey 在任意时刻只能由一个 spec 占位；如需"同键不同作用域"分派（如 `mod+W` 在 Settings 内关闭对话框 vs 主界面关闭 tab），SHALL 由该 spec 的 `handler` 内部按 `document.activeElement` / store state / focused pane 等条件判断分派；handler 返回 `false` 仅让 dispatcher 不 preventDefault 给浏览器原生行为放行，**不会** fallthrough 到"另一个注册的 spec"。

跨 pane / 多 instance 共享同一 ID 的 spec（如 `session.jump-to-latest`）SHALL 由其 owning controller 层（如 `PaneView`）注册一次 shared handler，handler 内部按 `getActiveTabId()` 等机制找当前 active 实例分派。**禁止**多 instance 各自重复 `registerShortcut` 同一 ID——会触发 "重复 ID" 错误。

`ShortcutSpec` 数据模型 SHALL 包含字段：`id: string`（唯一标识，kebab-case，形如 `sidebar.toggle`）、`category: "global" | "tabs" | "sidebar" | "search" | "session"`、`description: string`（用户可读，简体中文）、`defaultBinding: string | { mac: string; other: string }`、`handler: (e: KeyboardEvent) => boolean | void`、`allowInInput?: boolean`（默认 false）、`preventDefault?: boolean`（默认 true）。

#### Scenario: 启动时注册并 dispatch
- **WHEN** 应用启动并完成 `bootstrap()` 调用
- **THEN** 注册中心 SHALL 暴露所有内置 ShortcutSpec 的 keymap snapshot
- **AND** 单一 `keydown` listener SHALL 已挂在 `document` 上、phase 为 `bubble`（`capture: false`）
- **AND** 任意组件 SHALL NOT 重复 listen 全局 mod-key 组合

#### Scenario: 注册重复 ID 报错
- **WHEN** 同一 `id` 被 `registerShortcut` 调用两次
- **THEN** 第二次注册 SHALL 抛出 `Error("Shortcut id already registered: <id>")`
- **AND** dev 期间 SHALL 在 console 输出 stack trace 帮助定位

#### Scenario: 运行期更新绑定
- **WHEN** 调用 `registry.update(id, newBinding)`
- **THEN** 内存 keymap SHALL 立即反映新绑定
- **AND** 旧绑定的 NormalizedKey 入口 SHALL 被清除
- **AND** 后续 keydown 命中新绑定 SHALL 触发对应 handler

### Requirement: 跨平台修饰键归一化

注册中心 SHALL 提供 `mod` 关键字作为跨平台修饰键抽象：在 macOS 平台 `mod` SHALL 映射为 `Meta` 键（Command）、在 Windows / Linux SHALL 映射为 `Control` 键。归一化函数 `normalize(event: KeyboardEvent): string` SHALL 把 `KeyboardEvent` 转换为按字母顺序排列的修饰键 + 主键的字符串（如 `"meta+shift+k"`），供 Map 索引；`normalizeBinding(binding: string): string` SHALL 把 `mod` 展开为当前平台对应键并按相同规则排序。

平台展示函数 `formatShortcut(binding: string): string` SHALL 在 macOS 输出符号字面量（`⌘` / `⌥` / `⇧` / `⌃`）+ 大写字母；在 Windows / Linux 输出 `Ctrl+` / `Alt+` / `Shift+` 文本前缀 + 大写字母（如 `"Ctrl+K"`）。

#### Scenario: macOS 平台 mod 展开为 Meta
- **WHEN** 平台为 macOS
- **AND** 调用 `normalizeBinding("mod+k")`
- **THEN** SHALL 返回 `"meta+k"`

#### Scenario: Windows / Linux 平台 mod 展开为 Control
- **WHEN** 平台为 Windows 或 Linux
- **AND** 调用 `normalizeBinding("mod+k")`
- **THEN** SHALL 返回 `"ctrl+k"`

#### Scenario: 修饰键顺序归一化
- **WHEN** 用户按下 `Shift + Cmd + K`（修饰键被一起按下，平台为 macOS）
- **THEN** `normalize(event)` SHALL 返回 `"meta+shift+k"`（按字母顺序）
- **AND** 注册时写 `"shift+mod+k"` 与 `"mod+shift+k"` SHALL 命中同一 entry

#### Scenario: macOS formatShortcut 输出符号字面量
- **WHEN** 平台为 macOS
- **AND** 调用 `formatShortcut("mod+shift+k")`
- **THEN** SHALL 返回 `"⇧⌘K"`（修饰键按 Apple HIG 推荐顺序：⌃⌥⇧⌘）

#### Scenario: Windows / Linux formatShortcut 输出文本前缀
- **WHEN** 平台为 Windows 或 Linux
- **AND** 调用 `formatShortcut("mod+shift+k")`
- **THEN** SHALL 返回 `"Ctrl+Shift+K"`

#### Scenario: 物理位置受影响键以 event.code 兜底
- **WHEN** 用户按下 `[` 键（在 AZERTY 等非 QWERTY 布局下 `event.key` 可能不是 `[`）
- **THEN** `normalize(event)` SHALL 优先使用 `event.code === "BracketLeft"` 兜底
- **AND** 同样规则适用于 `]` / `\\` / `/` / 数字行 1-9 / `-` / `=`

### Requirement: dispatcher 命中与守卫

注册中心 dispatcher SHALL 在 `document` 的 `keydown` 事件 **bubble phase**（`addEventListener` 调用时 `capture: false`）按顺序执行守卫与命中流程：

1. **IME composition 守卫**：若 `event.isComposing === true` 或 `event.keyCode === 229`，SHALL 直接 return 不进 dispatch
2. **key-repeat 守卫**：若 `event.repeat === true`（长按系统连发），SHALL 直接 return
3. **归一化**：调用 `normalize(event)` 得到 NormalizedKey 字符串
4. **查表**：以 NormalizedKey 查 keymap，若无命中 SHALL return
5. **input 焦点守卫**：若命中 spec 的 `allowInInput !== true` 且 `document.activeElement` 是 `<input>` / `<textarea>` / `[contenteditable="true"]`，SHALL return
6. **handler 调用**：调用 `spec.handler(event)`，若返回 `false` SHALL return（spec 显式选择不消费）
7. **preventDefault**：若 spec 的 `preventDefault !== false`，SHALL 调用 `event.preventDefault()`

bubble phase 监听 SHALL 让组件级局部 listener（CommandPalette / Modal / SearchBar 内部的 Escape / Enter / 方向键）先于 dispatcher 命中；组件 SHALL 通过 `event.stopPropagation()` 阻止局部键继续传播到 dispatcher。

`normalize(event)` SHALL 在归一化时执行以下平台 / 物理键规则：

- **non-mac 平台禁止把 metaKey 识别为 mod**：`isMac() === false` 时 `event.metaKey === true` SHALL 不被加入 modifier 列表（防 Win 键 / 神秘键盘的误触发）；mac 平台 `event.metaKey` SHALL 被识别为 `meta`
- **Numpad 数字键**：`event.code === "Numpad0".."Numpad9"` SHALL 归一化为顶部数字（`"0".."9"`），与 `Digit0..Digit9` 同义
- **Numpad 功能键**：`"NumpadEnter"` SHALL 归一为 `"Enter"`、`"NumpadAdd"` 为 `"+"`、`"NumpadSubtract"` 为 `"-"`、`"NumpadMultiply"` 为 `"*"`、`"NumpadDivide"` 为 `"/"`、`"NumpadDecimal"` 为 `"."`，与 main row 对应键同义；录键 widget 录入 Numpad 系列时 SHALL 同步使用归一化结果

dispatcher 命中路径（步骤 3-5）SHALL 在 baseline 测试条件下 ≤ 0.5ms（vitest microbench 守门）。

#### Scenario: IME composition 期间不 dispatch
- **WHEN** 用户在中文输入法激活时按下任意键（`event.isComposing === true`）
- **THEN** dispatcher SHALL 直接 return
- **AND** 任何已注册 spec 的 handler SHALL NOT 被调用
- **AND** `event.preventDefault()` SHALL NOT 被调用

#### Scenario: input 焦点跳过非 allowInInput 快捷键
- **WHEN** `document.activeElement` 是 `<input>` 元素
- **AND** 用户按下 `mod+B`（spec.allowInInput 为 false / undefined）
- **THEN** dispatcher SHALL return
- **AND** `sidebar.toggle` 的 handler SHALL NOT 被调用
- **AND** 浏览器原生行为（输入字符）SHALL 不被打断

#### Scenario: allowInInput=true 的快捷键在 input 内仍触发
- **WHEN** `document.activeElement` 是 `<input>` 元素
- **AND** 用户按下 `mod+K`（spec.allowInInput 为 true）
- **THEN** dispatcher SHALL 继续走 handler
- **AND** `event.preventDefault()` SHALL 被调用

#### Scenario: handler 返回 false 不触发 preventDefault
- **WHEN** 命中 spec 的 handler 返回 `false`
- **THEN** dispatcher SHALL NOT 调用 `event.preventDefault()`
- **AND** 浏览器 / 上层 bubble listener SHALL 仍能处理该事件

#### Scenario: 未注册的快捷键不 preventDefault
- **WHEN** 用户按下任意未注册的 mod-key 组合
- **THEN** dispatcher SHALL return（无命中）
- **AND** 浏览器原生行为 SHALL 不被打断

#### Scenario: key-repeat 不 dispatch
- **WHEN** 用户长按 `mod+W`，浏览器以系统重复速率连发 keydown
- **AND** 第 2 次及以后的事件 `event.repeat === true`
- **THEN** dispatcher SHALL 在 repeat-guard 处直接 return
- **AND** 仅首次（`event.repeat === false`）SHALL 触发 `tab.close` handler

#### Scenario: 非 macOS 平台 metaKey 不被识别为 mod
- **WHEN** 平台为 Windows
- **AND** 用户按下 Windows / Super 键 + `K`（`event.metaKey === true`）
- **THEN** `normalize(event)` SHALL NOT 把 `meta` 加入 modifier 列表
- **AND** 该事件 SHALL 命中 NormalizedKey `"k"` 而非 `"meta+k"`
- **AND** dispatcher SHALL NOT 触发 `command-palette.toggle`（其默认 binding `mod+K` 在 Windows 平台展开为 `ctrl+k`）

#### Scenario: macOS metaKey 被识别为 mod
- **WHEN** 平台为 macOS
- **AND** 用户按下 `Cmd+K`（`event.metaKey === true`）
- **THEN** `normalize(event)` SHALL 把 `meta` 加入 modifier
- **AND** NormalizedKey SHALL 为 `"meta+k"`
- **AND** dispatcher SHALL 命中 `command-palette.toggle`

#### Scenario: Numpad 数字键归一化
- **WHEN** 用户按下 Numpad `1` 键（`event.code === "Numpad1"`）配合 mod
- **THEN** `normalize(event)` SHALL 归一为 `"meta+1"`（mac）或 `"ctrl+1"`（其他）
- **AND** 与按下顶部数字行 `1`（`event.code === "Digit1"`）同义命中 `tab.switch.1`

#### Scenario: dispatcher bubble phase 让组件 listener 先命中
- **WHEN** Modal 打开且 Modal 自身 keydown listener 已挂在 modal 容器上
- **AND** 用户按下 Escape
- **THEN** Modal 自身 listener SHALL 在事件冒泡到 `document` 之前先处理（关闭 modal）
- **AND** 若 Modal listener 调 `event.stopPropagation()`，dispatcher SHALL NOT 收到该事件
- **AND** Escape 不在已注册 keymap（无 spec 占用），即使收到也 SHALL return

### Requirement: 用户自定义覆盖

用户 SHALL 可以为任何已注册 ID 自定义新绑定。覆盖 SHALL 持久化到 `cdt-config::keyboard_shortcuts: HashMap<id, binding>` 字段（仅存 diff，未覆盖的 ID 走内置 default）；通过 `LocalDataApi::get_config` / `set_config` IPC 通道下发，serde camelCase（`keyboardShortcuts`）。

启动时 effective keymap SHALL 由 `mergeOverrides(defaults, overrides)` 计算：

1. 对每个内置 ID，若 overrides 中存在该 ID，SHALL 使用 override binding；否则使用 default binding
2. overrides 中包含未在内置注册的 "幽灵" ID（如老版本删除的 ID）SHALL 被忽略不报错
3. **IPC 失败 fallback**：若 `get_config` 抛出异常（IPC 层失败 / 反序列化失败 / 文件不可读），bootstrap SHALL 让 registry 走纯 builtin defaults 启动；UI 在 Settings → Keyboard Shortcuts tab 顶部 SHALL 显示非阻塞错误条："无法加载快捷键自定义：<reason> [重试]"；点击"重试"SHALL 重调 `get_config`，成功时 SHALL `mergeOverrides + registry.bootstrap` 重新建表；应用启动 SHALL NOT 因为该 IPC 失败而阻断或弹模态错误。

持久化路径 SHALL 由 Settings → Keyboard Shortcuts tab 的 **Save 按钮显式提交**触发——录键过程的修改仅保留在 panel 内存的 `pendingOverrides` overlay；点 Save 时 SHALL 单次 IPC `set_config` 写入全部 pending 改动 + 一次性把内存 keymap 切到新值。**SHALL NOT** 在录键 commit 时 debounce 自动写——避免"用户改了一半切到 Notifications tab，cdt-config 已经留下半成品 override"。

#### Scenario: 仅持久化覆盖
- **WHEN** 用户改动 `sidebar.toggle` 的绑定从 `mod+b` 到 `mod+shift+b`
- **AND** 其他 13+ 条均未改动
- **THEN** `cdt-config::keyboard_shortcuts` SHALL 仅包含 `{"sidebar.toggle": "mod+shift+b"}` 一项
- **AND** 用户从未改动任何快捷键时 `Config::keyboard_shortcuts` 为 empty HashMap SHALL 序列化为 `"keyboardShortcuts": {}`（详 `configuration-management/spec.md::Persist keyboard shortcut overrides`）—— **不**加 `skip_serializing_if`，让 IPC 字段必含 + 文件 reader 不需 undefined fallback

#### Scenario: 启动时 merge defaults + overrides
- **WHEN** `cdt-config::keyboard_shortcuts` 持有 `{"sidebar.toggle": "mod+shift+b"}`
- **AND** 应用启动调用 `bootstrap()`
- **THEN** effective keymap SHALL 让 `sidebar.toggle` 用 `mod+shift+b`、其他 13+ 条用 builtin defaults

#### Scenario: 幽灵 ID 被忽略
- **WHEN** `cdt-config::keyboard_shortcuts` 持有 `{"removed.legacy.action": "mod+x"}`（一个老版本删除的 ID）
- **AND** 当前内置无 `removed.legacy.action` 注册
- **THEN** 启动 SHALL 不报错
- **AND** `registry.bootstrap()` SHALL 跳过该 entry

#### Scenario: Save 显式提交单次 IPC 写入
- **WHEN** 用户在 KeyboardShortcutsPanel 的录键 widget 内连续改动 3 个不同 ID 的绑定（pending 累计）
- **AND** 用户点击 Save 按钮
- **THEN** SHALL 触发**单次** `set_config` IPC 写入包含全部 3 个 override 的 `keyboardShortcuts` HashMap
- **AND** 内存 keymap SHALL 在 IPC resolved 后一次性切到新值（如 IPC 失败 SHALL 回滚 pending）
- **AND** 用户未点 Save 就切走 / 关闭 Settings SHALL NOT 触发 `set_config`

#### Scenario: IPC 失败 fallback builtin defaults
- **WHEN** 应用启动调用 `get_config` 但 IPC 层抛异常（反序列化失败 / 文件不可读 / IPC 通道异常）
- **THEN** registry SHALL 用 `defaults.ts` 的 builtin defaults bootstrap，不阻断启动
- **AND** dispatcher SHALL 对 14+ 条 builtin 快捷键正常工作
- **AND** Settings → Keyboard Shortcuts tab 顶部 SHALL 显示非阻塞错误条 "无法加载快捷键自定义：<reason> [重试]"
- **AND** 用户点击"重试"且 `get_config` 成功 SHALL 重调 `mergeOverrides + registry.bootstrap`，错误条 SHALL 消失

### Requirement: 冲突检测

注册中心 SHALL 提供 `findConflict(binding: string, excludeId?: string, overlay?: Map<string, string>): string | null` 函数：在 effective keymap 与 `overlay`（可选 pendingOverrides 视图）合并后的视图上查重，返回该 binding 已被占用的另一 ID，若无占用返回 `null`；`excludeId` 用于排除自身。

冲突检测 SHALL 在两个时机触发：

1. **录键时**（UI 层）：用户在 `KeyRecorderInput` 录入新 binding 后，KeyboardShortcutsPanel SHALL 把当前 panel 的 `pendingOverrides` 作为 `overlay` 参数传入 `findConflict`，实时显示冲突反馈，保存按钮 SHALL disabled 直到冲突解掉
2. **保存时**（registry 层）：Save handler SHALL 在 `set_config` 之前对 pendingOverrides 中每个 entry 再走一遍 `findConflict(binding, sourceId, pendingOverlayMinusSelf)`；任意冲突 SHALL 让 Save 返回 `Result.Err({ kind: "Conflict", conflictId, sourceId })`，UI 切回 conflict 态、SHALL NOT 触发 IPC 写入

**关键约束**：录键时 SHALL 把 `pendingOverrides` 一并算进检测视图（见 `add-keyboard-shortcut-system::design.md::D4`）。否则用户先把 ID-A 改成 X、再把 ID-B 改成 X：两次都基于"effective"看不出冲突（因 ID-A 还没 commit），但 Save 后两条直接冲突。合并 pending overlay 让录键时就能拦住第二次冲突输入。

v1 SHALL NOT 提供"接受覆盖"路径——用户必须先解掉冲突（清空对方或改对方）才能保存自己的新键。

#### Scenario: findConflict 命中
- **WHEN** keymap 中已注册 `mod+b → sidebar.toggle`
- **AND** 调用 `findConflict("mod+b", "search.toggle")`
- **THEN** SHALL 返回 `"sidebar.toggle"`

#### Scenario: findConflict 排除自身
- **WHEN** keymap 中已注册 `mod+b → sidebar.toggle`
- **AND** 调用 `findConflict("mod+b", "sidebar.toggle")`
- **THEN** SHALL 返回 `null`

#### Scenario: registry.update 冲突时返回 Err
- **WHEN** keymap 中已注册 `mod+b → sidebar.toggle`
- **AND** 调用 `registry.update("search.toggle", "mod+b")`
- **THEN** SHALL 返回 `Result.Err({ kind: "Conflict", conflictId: "sidebar.toggle" })`
- **AND** 内存 keymap SHALL 不变
- **AND** `cdt-config` SHALL NOT 被写入

#### Scenario: pending overlay 串行冲突检测
- **WHEN** effective keymap 中 `sidebar.toggle = mod+b`、`command-palette.toggle = mod+k`、`search.in-session = mod+f`
- **AND** 用户在 KeyboardShortcutsPanel 录键：先把 `command-palette.toggle` 改为 `mod+x`（pendingOverrides 加一条）
- **AND** 再把 `search.in-session` 改为 `mod+x`（试图与 pending 中的 `command-palette.toggle` 冲突）
- **THEN** 第二次录键时 `findConflict("mod+x", "search.in-session", pendingOverrides)` SHALL 返回 `"command-palette.toggle"`
- **AND** UI SHALL 切到 conflict 态、Save 按钮 SHALL disabled
- **AND** 用户继续 Save SHALL 失败（registry 层 Save handler 二次校验拦截）

### Requirement: 录键守卫

`KeyRecorderInput.svelte` 在 recording 状态期间 SHALL 调用 `registry.suspend()` 暂停 dispatcher（避免录入的 `mod+B` 错误触发已注册的 `sidebar.toggle`）；recording 退出（commit 或 cancel）SHALL 调用 `registry.resume()` 恢复。recording 期间 SHALL `event.preventDefault()` 阻止字符落入 input。

`registry.suspend()` SHALL 让 dispatcher 在所有 keydown 上直接 return（IME guard 之后立即放行），`resume` 后恢复正常 dispatch；多次 suspend SHALL 不互相覆盖（引用计数）。

#### Scenario: 录键期间不触发已注册快捷键
- **WHEN** 用户进入 `KeyRecorderInput` recording 态
- **AND** 按下 `mod+B`（已被 `sidebar.toggle` 占用）
- **THEN** dispatcher SHALL NOT 调用 `sidebar.toggle` handler
- **AND** Sidebar SHALL 不切换折叠状态
- **AND** 录键 widget SHALL 把 `mod+B` 录入并显示冲突反馈

#### Scenario: 录键退出后 dispatcher 恢复
- **WHEN** 录键完成（用户保存或取消）
- **THEN** `registry.resume()` SHALL 被调用
- **AND** 后续按下任意已注册组合 SHALL 正常 dispatch

#### Scenario: suspend / resume 引用计数
- **WHEN** 两个并存 widget 各自 `suspend()` 后其中一个 `resume()`
- **THEN** dispatcher SHALL 仍处于 suspended 状态（引用计数 > 0）
- **AND** 第二个 widget 也 `resume()` 后 dispatcher SHALL 恢复

### Requirement: 内置快捷键清单

`ui/src/lib/keyboard/defaults.ts` SHALL 列出本期纳入注册中心的全部内置快捷键，覆盖以下分类与 ID：

- **global**: `command-palette.toggle`（`mod+K`）/ `sidebar.toggle`（`mod+B`，对应 `sidebar-navigation` capability 既有 SHALL）/ `search.focus`（`/`，DashboardView 聚焦搜索）
- **tabs**: `tab.switch.<n>`（`mod+1` ~ `mod+9`，n=1..9）/ `tab.close`（`mod+W`）/ `tab.next`（`mod+]`）/ `tab.prev`（`mod+[`）/ `pane.split`（`mod+\\`）/ `pane.focus.next`（`mod+alt+ArrowRight`）/ `pane.focus.prev`（`mod+alt+ArrowLeft`）
- **search**: `search.in-session`（`mod+F`，对应 `ui-search` capability 既有 SHALL）
- **session**: `session.jump-to-latest`（mac `mod+ArrowDown` / 其他 `Ctrl+End`，双 binding，对应 `session-display` capability 既有 SHALL）

每条 spec SHALL 提供完整 `description` 字段（中文），用于 Settings UI 列表渲染。

#### Scenario: 列表完整性
- **WHEN** 调用 `registry.listAll()`
- **THEN** SHALL 返回上述 5 个 category 共 ≥ 14 条 spec
- **AND** 每条 spec SHALL 含非空 `id` / `category` / `description` / `defaultBinding`

#### Scenario: 双平台 binding
- **WHEN** spec `session.jump-to-latest` 的 `defaultBinding` 在 macOS 平台
- **THEN** SHALL 解析为 `meta+ArrowDown`
- **WHEN** 在 Windows / Linux 平台
- **THEN** SHALL 解析为 `ctrl+End`

### Requirement: 局部 keydown 不并入 registry

下列局部键盘交互 SHALL 保持各组件自行 listen，不并入注册中心：

- 任意 modal / dropdown / popover / context menu / lightbox 的 Escape 关闭
- `CommandPalette.svelte` 内部 `ArrowUp` / `ArrowDown` / `Enter` 选项导航
- `SearchBar.svelte` 内部 `Enter` / `Shift+Enter` 跳转匹配项
- `TabContextMenu` / `SessionContextMenu` 内部方向键导航

注册中心 SHALL NOT 暴露 when-clause / scope 表达式；以上局部键作用域仅为该 surface focus 时，与全局 dispatcher 心智模型不一致。

#### Scenario: Modal Escape 不依赖 registry
- **WHEN** Modal 处于打开状态
- **AND** 用户按下 Escape
- **THEN** Modal SHALL 关闭（由 `Modal.svelte` 自身 listener 处理）
- **AND** registry dispatcher SHALL 不参与该事件

#### Scenario: CommandPalette 方向键不走 registry
- **WHEN** CommandPalette 打开
- **AND** 用户按 ArrowDown
- **THEN** CommandPalette SHALL 选中下一项（由 `CommandPalette.svelte` 自身 listener 处理）
- **AND** dispatcher SHALL 不命中任何已注册 spec


## Context

当前 keydown 处理散落在 8+ 个 Svelte 组件，每处独立判断 `e.metaKey || e.ctrlKey` 并自己 `preventDefault`；唯一跨平台抽象是 `ui/src/lib/platform.ts::isMac()`（PR #218 引入）。后端 `cdt-config` 当前不持久化任何键盘相关字段，亦无 IPC 通道。Tauri 未启用 `tauri-plugin-global-shortcut`，无 OS 级热键能力。

约束：

1. **Tauri webview 平台差异**：macOS 走 WKWebView、Windows 走 WebView2、Linux 走 WebKitGTK，三家 `KeyboardEvent.code` / `key` / `metaKey` 的语义已基本对齐 W3C，但 `key` 在 IME composition 期间会变成 `"Process"`——dispatcher MUST 在 `event.isComposing === true` 或 `event.keyCode === 229` 时直接放行，不进 dispatch。
2. **现有交互不能回归**：14+ 条快捷键 + 各组件 Escape/Enter 已被用户肌肉记忆，迁移期 SHALL 保持端到端等价（同样的物理按键产生同样的效果）。
3. **辅助工具能耗预算**（详 `.claude/rules/perf.md`）：dispatcher 必须 O(1) 命中（按 normalized key string 走 Map），每次 keydown 解析路径 SHALL ≤ 0.5ms（按 baseline budget），不允许 keydown 路径触发 storage 写入或 IPC 调用。
4. **stakeholder**：Settings UI 改动需符合 `DESIGN.md` 现有 Named Rules（The Border Before Shadow Rule / The Tool Density Rule / The Persistent Selection Is Quiet Rule）；`PRODUCT.md` 强调"审计优先、辅助工具不抢主线"，自定义快捷键 UI 不能成为另一个 attention spike。

## Goals / Non-Goals

**Goals:**

- 提供 single source of truth 的 `ui/src/lib/keyboard/` 注册中心：`registerShortcut(id, defaultBinding, handler, options)` 一次性注册，dispatcher O(1) 命中。
- 跨平台修饰键以 `mod` 抽象（macOS = `Meta` / 其他 = `Control`），开发者写 `mod+K` 不再手写 `metaKey || ctrlKey` 模板。
- 全量迁移 14+ 条现有全局快捷键到 registry。组件级 Escape / Enter / 上下键（在弹窗 / Modal / Dropdown / SearchBar 等局部 surface 内）保留组件自己的 keydown listener，不强制走 registry——避免把"局部 modal 关闭键"也卷入全局 keymap 增加耦合。
- 用户可在 Settings 自定义任意已注册 ID 的绑定，支持冲突高亮 / 重置默认 / "重置全部"。
- 后端 `cdt-config` 持久化用户覆盖（仅存 diff），通过 `LocalDataApi::get_config` / `set_config` IPC 已有通道下发，不引入新 IPC command。
- 性能：dispatcher 每次 keydown 解析路径 ≤ 0.5ms（vitest microbench 守门），不在 hot path 写 storage / 触发 IPC。
- 测试覆盖：14+ 条迁移每条 vitest 单测 + 至少 1 条 playwright 用户故事；冲突检测 / IME composition guard / `<input>` 焦点跳过单独单测。

**Non-Goals:**

- **不引入** `tauri-plugin-global-shortcut`（OS 级热键，应用未聚焦也响应）。原因：(a) Linux Wayland 全局热键需 portal/desktop 协议适配，覆盖面参差；(b) macOS 需 Accessibility 权限提示打断信任链；(c) 用户当前未明确诉求 global hotkey。如未来引入，作为独立 capability `global-shortcuts` 开新 change。
- **不引入** chord / sequence 快捷键（如 Emacs 风格的 `Ctrl+K Ctrl+S` 两键序列）。当前 14+ 条全是单键，YAGNI。
- **不动**组件级 Escape / Enter / 方向键的局部 keydown 监听（CommandPalette / Modal / Dropdown / SearchBar / ImageBlock 等）。这些与"关闭当前 modal"的语义紧耦合，强行抽出反而增加 indirection。
- **不实现**自定义键盘布局补偿（QWERTY / AZERTY / Dvorak）。本期按 `event.key` + `event.code` fallback 处理（详 D2），布局完全不同的用户可手动改键。
- **不持久化**未被用户改动的快捷键。`cdt-config::keyboard_shortcuts` 只存 diff（`HashMap<id, binding>`），保持 config 文件简洁。

## Decisions

### D1：dispatcher 数据模型与命中算法

**决策**：注册中心用单一 `Map<NormalizedKey, ShortcutSpec>` 索引；keydown 来时把 `event` 归一化为 `NormalizedKey` 字符串（如 `"mod+shift+k"`），Map 直接查。

**数据模型**：

```ts
interface ShortcutSpec {
  id: string;                      // e.g. "sidebar.toggle"
  category: ShortcutCategory;      // "global" | "tabs" | "sidebar" | "search" | "session"
  description: string;             // i18n 后展示给用户
  defaultBinding: ShortcutBinding; // { mac: "mod+b", other: "mod+b" } 或单 string
  handler: (e: KeyboardEvent) => boolean | void; // 返回 false 表示不消费、放行
  allowInInput?: boolean;          // 默认 false：input/textarea 焦点时跳过
  preventDefault?: boolean;        // 默认 true
}
```

**dispatcher 流程**：

```
on keydown(event):                                  // listener 挂在 document，phase = bubble (capture=false)
  if event.isComposing or event.keyCode === 229:    // IME guard
    return
  if event.repeat:                                  // key-repeat guard：长按连发不重复 dispatch
    return
  normalized = normalize(event)                     // e.g. "mod+k"
  spec = registry.get(normalized)
  if !spec: return
  if !spec.allowInInput and isInputFocused():
    return
  consumed = spec.handler(event)
  if consumed !== false and spec.preventDefault:
    event.preventDefault()
```

**关键约束**（架构级，不可降级）：

- **同 binding 在 registry 中只能注册一个 spec**（Map 是 1:1，不是 1:N）。如需"同键不同作用域"分派（如 `mod+W` 在某 modal 内关闭对话框 vs 在主界面关闭 tab），SHALL 由该 spec 的 handler 内部按 `document.activeElement` / store state 判断分派；如需让另一个 spec 接管，handler 返回 `false` 让 dispatcher 不 preventDefault，该路径不会触发"另一个 spec"——**dispatcher 不做 fallthrough chain**，handler 返回 `false` 仅用于"不消费、放行给浏览器原生行为"。
- **bubble phase 监听**：`document.addEventListener("keydown", dispatcher, { capture: false })`。这让组件级局部 listener（CommandPalette / Modal / SearchBar 内部的 Escape / Enter / 方向键）先于 dispatcher 命中，组件可在自己的 listener 内 `event.stopPropagation()` 阻止 dispatcher 介入。capture phase 监听是反模式——会先于组件 listener 触发并 preventDefault 吞掉 Enter / Escape。
- **`event.repeat` 守卫**：长按某键时浏览器会以系统重复速率连发 keydown；dispatcher SHALL 在 `event.repeat === true` 时 return，避免 14 条快捷键里任何一个被连发（如 long-press `mod+W` 关闭多个 tab）。

**理由**：
- 14+ 条快捷键命中走 Map.get(string) 是 O(1)；线性遍历（如 vscode `IKeybinding[]` 列表）在 N 大时变慢，本期没必要承担。
- IME guard / repeat guard / input focus guard 是已踩过的坑（`SearchBar.svelte::handleKeyDown` 的 IME 处理已经手动加过；repeat guard 在 `keyboard-shortcuts/spec.md::dispatcher 命中与守卫` 里以 SHALL 句沉淀），dispatcher 内置一次。
- `handler` 返回 `false` 表示"虽然命中但不消费"——给少数边缘场景（如同 binding 但当前作用域不应处理）一个 escape hatch；**注意**这只让 dispatcher 跳过 preventDefault，不会触发"另一个注册的 spec"——同 binding 唯一 spec 的设计让 fallthrough 行为不存在。

**Alternatives considered**：

- `Map<key, ShortcutSpec[]>` 多 spec 链 + 每个 spec 自带 when-clause 决定是否 handle：能力强（VSCode `IKeybinding[]` 风格），本期 14+ 条按 D6 边界全是"全局唯一作用域"，没有"同 binding 多 spec"诉求；引入 chain 后 SHALL 句要规约 dispatch 顺序、SHALL 句要规约 capture vs bubble 与 chain 的交互——复杂度爆炸。决策：单 spec 1:1、作用域分派内嵌 handler。
- VSCode 风格的 `IKeybinding[]` + when-clause 表达式：同上，能力强但实现复杂；`allowInInput` 已覆盖最常见 case，YAGNI。
- 直接用 `mousetrap` / `hotkeys-js` 第三方库：引入 ~5KB 依赖 + 多一层 indirection；本期需求简单，自写 normalize 函数 ~50 行。

### D2：跨平台修饰键归一化

**决策**：注册时绑定字符串支持以下 token：`mod` / `ctrl` / `alt` / `shift` / `meta` / 单字母数字 / 命名键（`ArrowUp` / `End` / `Escape` / `Backspace` / `Slash` / `Backslash` 等）。`mod` 在归一化时按平台展开：mac = `meta`、其他 = `ctrl`。

**归一化规则**：

```
normalize(event) -> string:
  parts = []
  // 平台分流：non-mac 平台禁止识别 metaKey 为 mod 输入（防 Win 键 / 神秘键盘的误触发）
  if isMac() and event.metaKey: parts.push("meta")
  if event.ctrlKey: parts.push("ctrl")
  if event.altKey:  parts.push("alt")
  if event.shiftKey: parts.push("shift")
  parts.push(canonicalKey(event.key, event.code))
  // canonicalKey: 优先 event.key，"Meta"/"Control"/"Alt"/"Shift" 自身被过滤；
  // 字母统一小写；arrow keys / 功能键保持 PascalCase；
  // 物理位置相关（如 "[" / "]" / "\\" / "/"）使用 event.code 兜底；
  // Numpad 数字键归一化为对应顶部数字（"Numpad1" -> "Digit1" -> "1"），
  // Numpad 功能键（"NumpadEnter" / "NumpadAdd" 等）归一化为"具体功能"（"Enter" / "+"），
  // 录键 widget 在录入 Numpad 系列时同步规则——同一物理意图按一致 binding 处理。
  return parts.join("+")

normalizeBinding(binding: string) -> string:
  // "mod+k" -> mac 上变 "meta+k"，其他变 "ctrl+k"
  // 内部统一按字母顺序排列 modifier，避免 "shift+mod" 与 "mod+shift" 命中不同
```

**展示给用户**（`formatShortcut`）：

| 平台 | mod | alt | shift |
|---|---|---|---|
| macOS | `⌘` | `⌥` | `⇧` |
| Windows / Linux | `Ctrl` | `Alt` | `Shift` |

普通字母大写：`⌘K` / `Ctrl+K`。

**修饰键展示顺序**（mac）：mac 输出按 Apple HIG 推荐顺序 **⌃⌥⇧⌘ + 主键**（即 Control / Option / Shift / Command）；如 `mod+shift+K` 在 mac 输出 `⇧⌘K`、`mod+alt+shift+K` 输出 `⌥⇧⌘K`。**与内部 normalize 的字母顺序排列解耦**——`formatShortcut` 是纯展示层函数，按 HIG 重排修饰键；内部 Map 索引仍走字母顺序的 `meta+shift+k`。windows / linux 输出按 `Ctrl + Alt + Shift + 主键` 顺序。

**理由**：
- `mod` 是 Electron Accelerator / Mousetrap 等社区已验证的抽象关键字，开发者一眼看懂。
- 两份 binding（`{ mac, other }`）只在少数场景必要（如 `Cmd+↓` vs `Ctrl+End` 跳到最新——见 PR #218 实现）。常规情况单 `"mod+k"` 字符串两平台同时覆盖。
- `event.code` 兜底 `[` / `]` / `\\` 这类受 KeyboardLayout 影响的物理键——不同布局下 `event.key` 可能是别的字符，`event.code` 始终是 `BracketLeft` / `Backslash`。

**Alternatives considered**：

- 完全用 `event.code` 不用 `event.key`：解决了布局问题但用户改键时录到 `KeyD` 而非 `D`，UI 展示反人类。决策按 `key` 优先 + `code` 兜底特定键。
- 完全跨平台单一 binding（macOS 也用 Ctrl）：违反 macOS 用户习惯，已被 PR #218 用 isMac() 双绑定方案否决。

### D3：用户自定义存储格式

**决策**：`crates/cdt-config/src/lib.rs` 的 `Config` 结构新增字段：

```rust
#[serde(default, skip_serializing_if = "HashMap::is_empty")]
pub keyboard_shortcuts: HashMap<String, String>,  // id -> normalized binding string
```

仅持久化用户覆盖；未覆盖的 ID 走 `defaults.ts` 内置默认值（启动时合并）。

**前端流转**：

```
启动时:
  defaults = readDefaultsFromBuiltin()
  try:
    overrides = await invoke("get_config").keyboardShortcuts
    effective = mergeOverrides(defaults, overrides)
  catch ipcErr:
    // IPC 失败 fallback：bootstrap 走纯 builtin defaults，UI 显示非阻塞错误条
    effective = defaults
    notifyConfigLoadFailed(ipcErr)
  registry.bootstrap(effective)

录键过程（KeyboardShortcutsPanel 内的 pending overlay）:
  recorder commit -> pendingOverrides[id] = newBinding   // 仅在 panel 内存中，未持久化
  findConflict(binding, excludeId) 输入 effective + pendingOverrides 合并后的 map
  registry.update(id, newBinding) 是非持久化的"试运行"——仅保存按钮点击后才执行

用户点 Save:
  for id, binding in pendingOverrides:
    registry.update(id, binding)            // 内存生效
  await invoke("set_config", { ...config, keyboardShortcuts: mergedOverrides })  // 单次 IPC 写入
  panel reset pending overlay
```

**写入策略修订（D3b）**：原稿写"`registry.update(id, newBinding)` 立即生效 + 500ms debounce 写 `cdt-config`"，与 `settings-ui/spec.md::Keyboard Shortcut 持久化与恢复` 的"修改 SHALL 通过 `Save` 按钮显式提交（不自动保存）"矛盾（codex 二审 #3）。修订决策：**Save 显式提交**为唯一持久化路径；录键 panel 内部维护 `pendingOverrides` overlay，未点 Save 前 `cdt-config` 与 registry **不变**；Save 时单次 IPC 写入全部 pending 改动 + 一次性把内存 keymap 切到新值。**没有 debounce 自动写**——避免"用户改了一半切到 Notifications tab，cdt-config 已经留下半成品 override"。

**冲突解决**：用户输入新 binding 时，录键 panel 在 effective + pendingOverrides 合并后的 map 上调 `findConflict`；若有占用，UI 高亮冲突项 + 录键 widget 显示警告（"Conflict with: <other-shortcut-name>"），保存按钮 disabled。**关键**：必须把 pendingOverrides 也算进检测——否则用户先把 ID-A 改成 X、再把 ID-B 改成 X，两次都不冲突但 Save 后冲突（codex 二审 #4）。v1 不实现"接受覆盖"（详 D4）。

**理由**：
- camelCase serde 已是仓库约定（详 `crates/CLAUDE.md`）。
- diff-only 持久化保持 config 文件简洁、用户可读；后续即使新增内置快捷键也不会影响老用户的存档。
- 启动时 merge 让默认值改动可被新版本"接管"未被用户改过的 ID。
- IPC 失败 fallback builtin defaults 让 cdt-config 不可读时（首启 / 文件损坏 / 权限错）应用仍能用快捷键，仅自定义功能临时不可用——配合非阻塞错误条提示用户。

**Alternatives considered**：

- 持久化全量映射：升级时新增的内置快捷键被用户存档"冻结"在旧默认值上，需要迁移逻辑。diff-only 自然规避。
- 独立 `keyboard.toml` 文件：增加 IO + 多一层 path 抽象，本期 `cdt-config` 已有完整 IPC 通道，复用最省。

**D3c：apply 阶段反转 `skip_serializing_if = "HashMap::is_empty"` 草案**：原稿 L141 `#[serde(default, skip_serializing_if = "HashMap::is_empty")]` 与 `configuration-management/spec.md::Persist keyboard shortcut overrides` 既有契约（empty `HashMap<String, String>` MUST 序列化为 `{}`）+ `crates/CLAUDE.md::serde camelCase` 约定冲突。`skip_serializing_if` 让"用户重置全部"（值 `{}`）与"老 config 不含该字段"（缺失 → undefined）合并为同一 undefined 形态，前端 `customization` 层失去区分依据，重置后下次 bootstrap 误回退到老 config 的语义。修订决策：**不**加 `skip_serializing_if`，empty `HashMap` 始终序列化为 `{}`。落地：

- `crates/cdt-config/src/types.rs::AppConfig::keyboard_shortcuts` 仅加 `#[serde(default)]`，注释明确 reason
- §10.4 Playwright e2e `update_config(keyboardShortcuts={})` 验证空 HashMap round-trip 仍含 `{}` 字段
- `crates/cdt-config/tests/` round-trip 单测同步覆盖 empty 场景
- spec delta `Scenario: 仅持久化覆盖` AND 子句、proposal.md L28、tasks.md §2.1/§2.2 三处文字同步反转

### D4：冲突检测时机

**决策**：录键 widget 在每次按键 commit（用户松开所有键，即 `keyup` 把"修饰+主键"凑齐之后）时实时校验 + UI 反馈；保存路径再校验一次防御。

**两阶段**：

1. **录键时**（`KeyRecorderInput.svelte`）：用户按下 `Ctrl+Shift+K`，松开后录入器把 binding 拿到 panel pending overlay，panel 内部 `findConflict(binding, excludeId)` 在 **effective map ∪ pendingOverrides** 合并视图上查重，返回 null 或冲突 id；UI 在录入器下方显示 "Conflict with: '切换 Sidebar 折叠' (`Ctrl+B`)"，保存按钮 disabled。
2. **保存时**：Save handler 在 IPC 写入前再把整套 pendingOverrides 走一遍 `findConflict`（防御串行注入）；任意一条冲突 SHALL 返回 `Result.Err({ kind: "Conflict", conflictId, sourceId })`，UI 切回 conflict 态。

**为什么 effective ∪ pending 合并**（codex 二审 #4）：用户先把 ID-A 改成 X、再把 ID-B 改成 X——两次都基于"effective map"看不出冲突（因为 ID-A 的修改还没 commit 到 effective），但 Save 后两条都改成 X 直接冲突。合并 pending overlay 让录键时就能拦住第二次冲突输入。

**v1 不支持"接受覆盖"**：用户必须先解掉冲突（清空对方或改对方）才能保存自己的新键。理由：避免静默覆盖造成用户找不到原来的快捷键；UI 反馈即给"清空对方"快捷入口（点冲突项弹出"Reset to default"）。

**Alternatives considered**：

- 完全允许覆盖（最后写入获胜）：用户体验差，常见反例是"我刚改了 Cmd+K，结果之前 Cmd+B 也变了"。
- 仅保存时校验：录键时无反馈，用户不知道按出来的组合冲突，保存按钮突然 disabled 没有 affordance。

### D5：迁移策略 — 一次性全迁 + dev-only escape hatch

**决策**：本次 change 在同一 PR 内完成 14+ 条迁移；不引入 feature flag 灰度。dev 期间保留 `window.__keyboardRegistry` debug 入口（用于在 devtools console 列出当前 keymap、模拟 fire），release build 通过 `import.meta.env.DEV` 排除。

**迁移顺序**（apply tasks.md 内）：

1. 先建 `ui/src/lib/keyboard/` 模块 + 单测，registry 空运行
2. 迁 `App.svelte::handleGlobalKeydown` 全部 9 条全局快捷键（`mod+K` / `mod+B` / `mod+1~9` / `mod+W` / `mod+[` / `mod+]` / `mod+\\` / `mod+Alt+←/→`）
3. 迁 `SessionDetail.svelte` 跳到最新（`mod+ArrowDown` / `Ctrl+End` 双 binding）
4. 迁 `DashboardView.svelte` `/` 聚焦搜索
5. 验证 14+ 条端到端等价（playwright user story 跑全套）
6. 才上 Settings UI（避免 Settings UI 用错的 default 把 registry 推坏）

**Alternatives considered**：

- 灰度迁移（feature flag 切换 old vs new dispatcher）：引入双轨期复杂度 + 用户配置在两套系统间不同步。本期 14+ 条 scope 可控、playwright 已有覆盖，一次性切。
- 按 capability 分多 PR：每个 PR 都要重复 registry 边界测试，cycle time 长且 review 上下文割裂。同 PR 全迁更省。

### D6：组件级局部 keydown 不并入 registry

**决策**：注册中心**只管全局 mod-key 组合**（含 `/` 这类全局焦点切换键）。组件内部 Escape / Enter / 方向键 / Shift+Enter 等"作用域仅限当前 modal / dropdown / searchbar"的局部键，**保持原样**自己 listen。

**判定标准**：

| 类型 | 走 registry？ | 例 |
|---|---|---|
| 全局 mod-key（`mod+...`） | ✓ | `mod+K` / `mod+B` |
| 全局非 mod 单键（`/` 聚焦搜索） | ✓ | `DashboardView` 的 `/` |
| 弹窗 / Dropdown / Modal 内的 Escape 关闭 | ✗ | `Modal.svelte` / `Dropdown.svelte` / `CommandPalette.svelte` 的 Escape |
| Modal / list 内的方向键 / Enter 选择 | ✗ | `CommandPalette.svelte` 的上下键 |
| `SearchBar.svelte` 内的 Enter / Shift+Enter 跳转 | ✗ | 仅在搜索栏 focused 时才有意义 |

**理由**：
- 局部键的语义"仅在某 surface focus 时有效"与全局 dispatcher 心智模型不一致；强行注册要么得引入 when-clause（D1 已 NG），要么 dispatcher 得知道当前哪个组件处于 active 状态，破坏 registry 的纯函数性质。
- 局部键不可能被用户自定义（"Modal Escape" 改成别的会让所有 modal 失控），不需要 Settings UI 入口。
- 维持 14+ 条全局快捷键与组件级 8+ 处局部 listener 的清晰边界。

**与 D1 phase 决策的协同**：D1 dispatcher 用 bubble phase 监听（`capture: false`）让组件级 listener 先命中——组件可在自己的 keydown handler 内 `event.stopPropagation()` 阻止 dispatcher 介入，让局部键彻底不进 dispatcher。这意味着 dispatcher 与组件级 listener 不需要任何"互相知道"，纯靠 DOM 事件传播分层。

### D7：IPC 失败 fallback 与启动健壮性

**决策**：bootstrap 阶段任何 `get_config` IPC 异常（IPC 层 panic / 反序列化失败 / 文件不可读 / 权限错）SHALL 让 registry 走纯 builtin defaults 启动，UI 在 SettingsView 的 Keyboard Shortcuts tab 顶部显示非阻塞错误条："无法加载快捷键自定义：<reason> [重试]"。**不阻断**应用启动，**不阻断**全局 14+ 条 builtin 快捷键的使用——仅"用户自定义"功能临时不可用。

错误条点击"重试"SHALL 重新调 `get_config`；成功 SHALL `mergeOverrides + registry.bootstrap` 重新建表（无需重启应用）。

**为什么**（codex 二审 #7）：spec 原稿只覆盖了 "幽灵 ID 跳过" 这种 happy-path-near 场景，没规约 IPC 失败 fallback。生产环境 cdt-config 文件损坏 / 权限丢 / 升级 schema 不兼容是真实可能的——硬失败让应用无快捷键可用违反"辅助工具不抢主线"。fallback builtin defaults 是最低损耗方案。

### D8：多 instance 同 ID 注册策略（PaneView 共享 handler）

**决策**：跨 pane 的 `session.jump-to-latest` SHALL 只**在 PaneView 层注册一次** shared handler（而非每个 SessionDetail 实例各自注册），handler 内部按 `getActiveTabId()` 找当前 focused pane 的 SessionDetail 实例分派 smooth-scroll 调用。

**为什么不能每实例各注册**（codex 二审 #9）：D1 决定 `Map<key, ShortcutSpec>` 单 spec 占位；如果 4 个 pane 各自 mount 时调 `registerShortcut("session.jump-to-latest", ...)`，第 2 个调用会被 D1 的 "重复 ID 抛错" 拦下。改成 4 个不同 ID（`session.jump-to-latest.pane.<n>`）则违反"用户可自定义"语义——用户改键得改 4 次。

**实现路径**：

```
// ui/src/routes/PaneView.svelte (或同层级 controller)
onMount(() => {
  const unregister = registerShortcut({
    id: "session.jump-to-latest",
    defaultBinding: { mac: "mod+ArrowDown", other: "ctrl+End" },
    allowInInput: false,
    handler: (e) => {
      const activeId = getActiveTabId();
      const detail = findSessionDetailByTabId(activeId);  // 通过 tab registry 找 active SessionDetail 实例
      if (!detail) return false;                          // 没有 active SessionDetail → 让 dispatcher 不消费、放行浏览器
      detail.jumpToLatest();
      return true;
    },
  });
  onDestroy(unregister);
});
```

handler 返回 `false` 让 dispatcher 跳过 preventDefault（按 D1）——active pane 不是 SessionDetail 时浏览器原生 `Cmd+↓` / `Ctrl+End` 行为不被吞。

**为什么不在 App.svelte 注册**：PaneView 是 SessionDetail tab 的容器层，它知道 active tab 与 active pane 的关系；放在 App.svelte 会引入跨层依赖。其他需要"多 instance shared dispatch"的快捷键（未来可能新增）按相同策略由其 owning controller 层注册。

### D-V1（视觉）：Surface 决策 — 新建 Settings tab

**决策**：在 `ui/src/routes/Settings.svelte` 现有 Section 导航中新增独立 tab "Keyboard Shortcuts"，与 General / Notifications / Connection 平级。**不**塞进 General Section、**不**用 modal 弹窗。

**理由**：
- 14+ 条快捷键列表 + 录键 widget 体量足够单独成 tab；塞进 General 会让 General 变成杂物间，违反 `DESIGN.md::The Tool Density Rule`（"靠布局和字重补足层级"，不靠堆密度）。
- modal 弹窗会要求用户在调试快捷键时反复打开关闭，且查找具体某个 ID 时无法长期保持上下文；tab 是持久 surface，自然支持"边录边滚边查"。
- 链回 `PRODUCT.md` 审计优先：keyboard shortcuts 是低频但深度功能，独立 tab 让深度用户找得到、新手完全无感。

**Anti-references**：
- 反例：把"录键"做成 dashboard 浮 chip 或常驻 toolbar item（违反 `DESIGN.md::The Floating Is Affordance, Not Decoration Rule`，非动作语义场景）。
- 反例：用 `Cmd+K` Command Palette 提供"set shortcut for X"——Command Palette 是动作快捷入口，不是配置面板，把配置混入会破坏 palette 的"do something"心智。

### D-V2（视觉）：录键 widget 三态视觉

**决策**：`KeyRecorderInput.svelte` 提供 idle / recording / conflict 三态，复用 `DESIGN.md::The Border Before Shadow Rule`：

| 状态 | 视觉 | 文案 |
|---|---|---|
| idle | neutral surface + 1px border + mono `⌘K` 显示当前 binding | hover: `Click to change` |
| recording | accent border + neutral bg + spinner（`The Static-vs-Live Shape Rule` 的 secondary spinner 10×10） | `Press a key combination...` |
| conflict | semantic-warning border + warning bg（浅暖色，不是红） + mono 新 binding | `Conflicts with: <other-shortcut-name>` |

**理由**：
- idle 用 neutral，避免 Sidebar Persistent Selection Is Quiet Rule 的反例（不抢主线）。
- recording 用 spinner（不是 dot ping），符合 `The Static-vs-Live Shape Rule` "动态 live 信号用 circular spinner"。
- conflict 用 warning 而非 red，因为冲突是 actionable warning（"你需要解掉冲突"），而非 error（"系统坏了"）；用 red 会过度报警。

## Visual Contract

### Surface Decision

新增 Settings → Keyboard Shortcuts tab 作为唯一入口（详 D-V1）。Tab 入口在 `Settings.svelte::sectionList` 与 General / Notifications / Connection 平级，左侧导航文本 `键盘快捷键`。

链回 `PRODUCT.md`：辅助工具不抢主线 → 快捷键配置不进 dashboard / sidebar / TabBar / Command Palette；仅在用户主动进 Settings 时存在。

链回 `DESIGN.md::The Persistent Selection Is Quiet Rule`：列表行（按 ID 排列的 14+ 条快捷键）的 hover 用 neutral hover bg，禁用 Focus Blue；当前编辑中的行用 neutral selection bg + 1px accent border 区分，不引入彩色填充。

### Visual Layer

新增组件清单 + Named Rule 引用：

| 组件 | 视觉决定 | DESIGN.md Named Rule |
|---|---|---|
| `Settings.svelte` 新 tab 入口 | neutral 行 + active 时 1px border + bg 加深 | `The Border Before Shadow Rule` / `The Persistent Selection Is Quiet Rule` |
| `KeyboardShortcutsPanel.svelte` 列表头 | category 分组用 14px medium + 行间距 16px | `The Tool Density Rule` |
| `ShortcutRow.svelte` 单行 | 左：description（自然语言）/ 右：mono `formatShortcut` 输出 + `KeyRecorderInput` | `The Machine Information Rule`（mono 用于按键展示） |
| `KeyRecorderInput.svelte` idle | neutral surface + border + mono `⌘K` | `The Border Before Shadow Rule` |
| `KeyRecorderInput.svelte` recording | accent 1px border + 10×10 secondary spinner | `The Static-vs-Live Shape Rule`（secondary spinner 缩档） |
| `KeyRecorderInput.svelte` conflict | warning border + warning bg + mono 新 binding | 需新增 Named Rule（详 delta plan） |
| 重置按钮（行级） | 弱化 ghost 按钮 + icon "RotateCcw" + tooltip "Reset to default" | `The Status Owns the Color Rule`（不加色） |

### State Coverage

每个新组件覆盖以下状态：

| 组件 | loading | empty | error | disabled | hover | active |
|---|---|---|---|---|---|---|
| `KeyboardShortcutsPanel` | `Skeleton` 14 行 | n/a（默认值始终有 fallback） | "Failed to load config: <reason> [Retry]" | n/a | n/a | n/a |
| `ShortcutRow` | n/a（来自 panel） | n/a | inline error: "Save failed: <reason>" | 整行 muted（仅当 IPC 异常时） | bg neutral hover | accent border 1px |
| `KeyRecorderInput` idle | n/a | n/a | n/a | grayed mono + cursor not-allowed | hover bg | n/a |
| `KeyRecorderInput` recording | spinner | n/a | n/a | n/a | n/a | accent border 持续 |
| `KeyRecorderInput` conflict | n/a | n/a | warning border + warning bg | n/a | n/a | n/a |
| 重置按钮 | n/a | n/a | n/a | grayed（已是 default 时） | hover bg | n/a |

**关键无障碍**：

- 录键时焦点 trap 在录入器内（避免用户按 Tab / Esc 误触全局快捷键）；Esc 退出录键回到 idle。
- 所有冲突 / 错误用 `aria-live="polite"` 通报屏幕阅读器。

### DESIGN.md delta plan

archive 前跑 `/impeccable extract` 提取以下进 `DESIGN.md`：

1. **新 Named Rule 候选**：
   - `The Conflict Is Warning Not Error Rule.` 用户输入冲突的快捷键、表单未通过校验、可解决的临时阻塞 SHALL 用 warning（暖色 border + bg）而非 error red；error red 仅用于"系统已坏 / 操作不可恢复"。归属 Color §Named Rules 末尾。
   - `The Recorder Idle State Rule.` 录入类组件（录键 / 录手势 / 录笔画）idle 态 SHALL 是 neutral surface + 弱 border + mono 当前值；不允许常驻 accent 边框或动画——避免在 Settings 类长列表中持续抢眼。归属 Components §Inputs and search。
2. **新 token 候选**：
   - `--surface-recording-bg` / `--border-recording`（accent low-saturation）
   - `--surface-conflict-bg` / `--border-conflict`（warm warning，区别于 error red）
3. **新组件规则**：
   - `KeyRecorderInput` 三态切换的 motion timing（idle ↔ recording 80ms ease-out、recording ↔ conflict 60ms instant）

extract 由设计师 teammate 在 apply 阶段 archive 前一刻执行，作为本 PR 一部分提交。

## Risks / Trade-offs

- **Risk: 全量迁移引入回归**（14+ 条任一条端到端行为变了用户立刻感知）→ Mitigation：D5 顺序里第 5 步显式跑 playwright 全套用户故事 + 每条快捷键 vitest 单测；codex 二审 prompt 必列"是否每条迁移都端到端等价"。
- **Risk: macOS Cmd+W 与系统关闭窗口冲突**（用户改键场景）→ Mitigation：D2 双 binding 支持 + Settings UI 行内 tooltip 提示"Cmd+W 在 macOS 与系统关闭窗口可能冲突，建议改键"；冲突检测仅检测应用内冲突，OS 级冲突给 hint 不给阻断。
- **Risk: IME composition 期间误触发**（已知踩过坑）→ Mitigation：D1 dispatcher 内置 `event.isComposing` / `event.keyCode === 229` guard + 单测覆盖。
- **Risk: 用户自定义把所有快捷键删空**（清空所有 binding）→ Mitigation：UI "重置全部"按钮 + 单条"重置默认"；config 字段为空 HashMap 时启动按 builtin defaults 跑，不会卡死。
- **Risk: 跨平台 keyboard layout 差异**（QWERTY / AZERTY / Dvorak）→ Mitigation（部分）：D2 `event.code` 兜底 `[` / `]` / `\\` / `/` 等物理位置受影响键；完全不同布局用户走"自定义"自己改。Non-goal 不实现自动布局补偿。
- **Trade-off: 不支持 chord 快捷键** → 影响：无法做 "`Ctrl+K Ctrl+S` 打开快捷键设置" 这类 vscode 风格序列。本期 14+ 条全是单键，无诉求。如未来需要，dispatcher 的 normalize 函数易扩成"序列状态机"。
- **Trade-off: 不引入第三方库**（mousetrap / hotkeys-js） → bundle size 不增；但 normalize 边界 case（如 `Numpad*` / `F1-F12` / 国际键盘字符）需自己测。覆盖通过单测兜底。
- **Trade-off: 局部 keydown 不并入 registry**（D6） → 心智上用户改键时只能改"全局"那部分；局部 Escape / Enter 改不了。这是有意识权衡——避免把 modal close 也卷入冲突检测让维护成本爆炸。

## Migration Plan

**部署策略**（apply 阶段 commit 顺序，详 tasks.md）：

1. 创建 `ui/src/lib/keyboard/` 模块 + 单测，registry 空跑（dispatcher 不接 keydown listener）
2. 后端 `cdt-config::keyboard_shortcuts` 字段 + IPC contract test
3. dispatcher 接管 `App.svelte::handleGlobalKeydown` 9 条全局快捷键（同一 commit 删旧 listener）
4. 迁 `SessionDetail` / `DashboardView` 的剩余 5+ 条
5. playwright 用户故事跑全套，本地手动 smoke
6. Settings UI 上线（KeyboardShortcutsPanel + KeyRecorderInput + ShortcutRow）
7. archive 前 `/impeccable extract` 提 DESIGN.md delta（新 Named Rule + token + 组件规则）

**回退策略**：

- 单 PR revert 即可全回退；`cdt-config::keyboard_shortcuts` 字段 `#[serde(default)]` 保证回退后老 config 反序列化不报错（HashMap 字段被忽略）。
- 用户已存的覆盖在 revert 后保留在 config 文件里但不被消费；如果再 revert-revert（即恢复本 change），覆盖自动生效。

## Open Questions

1. ~~~~录键时如果用户只按了 modifier（如只按 Shift 没按主键）应该怎样响应？~~~~ → 决策：D4 在 `keyup` 把"修饰+主键"凑齐之后才 commit；纯 modifier 不 commit、idle 态不变。
2. ~~~~`Cmd+W` 是否需要专门 OS 级 capture 防止默认关闭窗口？~~~~ → 决策：D1 dispatcher 调 `event.preventDefault()`，Tauri webview 已可截获。Linux/Wayland 边缘 case 走 follow-up。
3. **是否同步给 Command Palette 加 "Keyboard Shortcuts: Edit binding" 项？** → 留 Open。当前倾向"是"（Command Palette 应能直达 Settings 任意 tab），但需要确认 Command Palette 现有 schema 是否支持"action-with-arg"（跳转到 Settings 的某 tab 而非根）。如不支持，本期不做、follow-up。
4. **i18n description 的字段**（`ShortcutSpec.description`）现在直接用中文字面量还是预留 i18n key？仓库目前未启用 i18n（全中文）。决策建议：本期直接中文字面量；如未来引入 i18n，做一次性扫描替换。
5. **录键时 `<input>` 焦点怎么处理？** → 决策：录键 widget 自身就是 `<input>`-like 组件，但内部劫持 keydown + `event.preventDefault()`，不让字符落进 input；录键期间 dispatcher 应跳过全局 dispatch（`registry.suspend()`），录完 resume。

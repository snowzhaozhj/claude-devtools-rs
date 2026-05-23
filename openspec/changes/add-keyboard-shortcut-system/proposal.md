## Why

当前键盘快捷键散落在 8+ 个 Svelte 组件中（`App.svelte` / `SessionDetail.svelte` / `DashboardView.svelte` / `SearchBar.svelte` / `CommandPalette.svelte` / `Modal.svelte` / `Dropdown.svelte` / `ImageBlock.svelte` 等），各自调用 `document.addEventListener("keydown", ...)` 并独立判断 `e.metaKey || e.ctrlKey`，无统一注册中心、无冲突检测、无用户自定义入口。跨平台抽象目前只有 `ui/src/lib/platform.ts::isMac()` 一个工具函数（PR #218 引入）；当前已注册的快捷键 14+ 条（`Cmd/Ctrl+K` / `Cmd/Ctrl+B` / `Cmd/Ctrl+1~9` / `Cmd/Ctrl+W` / `Cmd/Ctrl+[]` / `Cmd/Ctrl+\\` / `Cmd/Ctrl+Alt+←/→` / `/` / `Cmd↓` & `Ctrl+End` / 各组件 Escape），平台映射、文档展示、冲突排查全靠人工对齐，长期演进风险高。

现在做的动机：(a) 已积累 14+ 条快捷键，再不收口未来每个新组件都会复制 `metaKey || ctrlKey` 模板碎片；(b) 用户已多次反馈希望某些键能改键（如 `Cmd+W` 与系统关闭窗口冲突）；(c) 视觉契约（Settings tab + 录键 widget + 冲突高亮）一次性沉淀进 `DESIGN.md` token 体系成本最低。

## What Changes

- 新增 `keyboard-shortcuts` capability：集中式快捷键注册中心 + 跨平台修饰键抽象 + 用户可自定义。注册中心承担 single source of truth，不再允许散落 keydown listener 自行判定全局快捷键。
- UI 端新增 `ui/src/lib/keyboard/` 模块：`registry.ts`（registerShortcut / dispatcher / keymap snapshot）/ `platform.ts`（扩展 `isMac()` → 增加 `modKey()` / `formatShortcut(spec)` / `parseShortcut(str)` / `matchEvent(spec, event)`）/ `defaults.ts`（内置快捷键清单与默认绑定）/ `customization.ts`（自定义绑定 store + 冲突检测）。
- 全量迁移 14+ 条现有全局快捷键到 registry（`App.svelte` 全局表 + `SessionDetail` 跳到最新 + `DashboardView` `/` 聚焦搜索 + `CommandPalette` / `SearchBar` / `Modal` / `Dropdown` / `ImageBlock` 等组件级 Escape/Enter 仍允许局部 keydown，但 mod-key 组合一律走 registry）。
- 新增 Settings → Keyboard Shortcuts tab：列出所有内置快捷键、按 category 分组（Global / Tabs / Sidebar / Search / Session）、支持录键修改 / 重置为默认 / 实时冲突高亮 / "重置全部" 操作。
- 后端 `cdt-config`：新增 `keyboard_shortcuts` 字段持久化用户覆盖（仅存 diff，不存默认值），通过 `LocalDataApi::get_config` / `set_config` IPC 已有通道下发。
- IPC contract test（`cdt-api/tests/ipc_contract`）：覆盖 `keyboardShortcuts` 字段 camelCase 序列化与默认值。
- **BREAKING**（仅对内部代码）：`App.svelte::handleGlobalKeydown` 内联 `if/else` 全局快捷键被替换为 registry dispatcher；任何下游若依赖该 handler 内部行为需迁移到 `registerShortcut`。用户层无 BREAKING。

## Capabilities

### New Capabilities
- `keyboard-shortcuts`: 集中注册 + 跨平台修饰键抽象 + 用户自定义覆盖 + 冲突检测；定义 ShortcutSpec 数据模型（id / category / defaultBinding / handler 契约）、dispatcher 行为（pre-match guards、`<input>` / `<textarea>` 焦点跳过、preventDefault 语义）、覆盖优先级（user override > default）、IPC 持久化字段契约。

### Modified Capabilities
- `settings-ui`: 新增 "Keyboard Shortcuts" tab 的 SHALL（tab 入口、列表渲染分组、录键交互、冲突视觉、重置语义）。
- `sidebar-navigation`: 现有 `Cmd+B` 折叠快捷键的 SHALL 从"App.svelte 直接监听"修订为"注册中心 dispatch + 用户可自定义"，行为契约 ID 化（`sidebar.toggle`）。
- `ui-search`: `Cmd+K` Command Palette / `Cmd+F` 会话搜索两条 SHALL 改写为通过 registry 触发 + 可自定义；`/` 聚焦搜索同步纳入。
- `tab-management`: `Cmd+1~9` / `Cmd+W` / `Cmd+[]` / `Cmd+\\` / `Cmd+Alt+←/→` 五组 SHALL 改写为通过 registry 触发 + 可自定义；明确 `Cmd+W` 与系统关闭窗口冲突时用户改键的回退路径。
- `session-display`: PR #218 的"跳到最新消息" `Cmd+↓` / `Ctrl+End` 快捷键 SHALL 改写为通过 PaneView 顶层注册的 shared handler 在 registry 中 dispatch + 可自定义（多 instance 共享单 spec 1:1，详 design.md::D8）。
- `configuration-management`: ADDED 新字段 `keyboard_shortcuts: HashMap<String, String>` 持久化契约（IPC `keyboardShortcuts` camelCase / 旧 config 兼容性 / **empty `HashMap` 仍序列化为 `{}`**——与 configuration-management 既有契约对齐，让前端 `{}` vs `undefined` 区分"重置全部"与"老 config 缺字段"，详 design.md::D3c）。

## Impact

- **代码触达**：
  - `ui/src/lib/keyboard/`（新增 4 文件 + 测试）
  - `ui/src/App.svelte`（迁出全局快捷键 dispatcher，保留 `<input>` 焦点判定与 IME composition guard）
  - `ui/src/routes/SessionDetail.svelte` / `DashboardView.svelte` / `ui/src/components/CommandPalette.svelte` 等 14+ 处迁移
  - `ui/src/routes/Settings.svelte`（新增 Keyboard tab）+ 新组件 `ui/src/components/settings/KeyboardShortcutsPanel.svelte` + `KeyRecorderInput.svelte` + `ShortcutRow.svelte`
  - `crates/cdt-config/src/lib.rs`（`Config` 结构加 `keyboard_shortcuts: HashMap<String, String>` 字段，serde camelCase）
  - `cdt-api/tests/ipc_contract.rs`（新 case）
- **依赖**：本次不引入新 cargo / npm 依赖；不依赖 `tauri-plugin-global-shortcut`（OS 级 GlobalShortcut 留作 future capability）。
- **DESIGN.md**：archive 前跑 `/impeccable extract` 提取录键 widget / 冲突高亮 / category 分组的 token 与组件规则进 `DESIGN.md`，命名 Rule 候选：The Shortcut Conflict Color Rule、The Recorder Idle State Rule。
- **followups**：本次不动 OS 级全局热键（应用未聚焦也响应）—— 留 `openspec/followups.md::keyboard-shortcuts` 段记录"future: 引入 tauri-plugin-global-shortcut 让 `Cmd+Shift+/` 可全局唤起 CommandPalette"。
- **测试**：14+ 条迁移每条都需 vitest 单测 + 至少 1 条 playwright 用户故事（详见 frontend-test-pyramid 四层分工）。
- **性能**：keydown dispatcher 必须 O(1) 命中（按 normalized key string 走 Map），不允许 O(N) 遍历注册表（详见 design.md D2）。

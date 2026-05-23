## Why

桌面端在 macOS WKWebView 下右键弹出的多数是浏览器/系统默认菜单（Reload / Look Up / Translate / Search with Baidu / Speech / Services 等），破坏 app 一体感；Sidebar 会话项与 TabBar 标签项虽各自实现了自定义菜单，但缺统一组件 + 通用挂载机制，且**右键 mousedown 阶段的 WebKit smart-selection 行为**未被防护——用户在会话标题等元素上右键时会莫名选中一小段文案（用户截图证实）。当前两个独立菜单组件（`SessionContextMenu` / `TabContextMenu`）样式与生命周期管理已重复一遍，下一步要在多 surface 加菜单（用户/AI chunk、工具结果、worktree chip 等，见 design Phase 2）时会扩散成 5+ 个重复实现。先把基础设施 + 全局兜底立起来（Phase 1），让"右键 = app 自家菜单 / 不弹"成为默认；后续会话再按 surface 增量补菜单 items（Phase 2）。

## What Changes

- **新增** `AppContextMenu.svelte` items-driven 通用浮层组件：受 `{ label, icon?, action, disabled?, danger?, separator? }[]` 驱动，承载视觉、定位 clamp、外点/Esc/scroll/window-blur 关闭、键盘 ↑↓ Enter Esc 导航、focus trap、a11y `role="menu"` / `role="menuitem"`。
- **新增** `use:contextMenu={items}` Svelte action：封装 `oncontextmenu` 监听 + smart-select 防护（右键 `mousedown` 时无选区则 `preventDefault`）+ 键盘 `contextmenu` 事件（Menu 键 / Shift+F10）+ 菜单生命周期。
- **新增** 全局 `contextmenu` 兜底（`main.ts` 注册 window-level listener）：未挂 `use:contextMenu` 的元素一律 `preventDefault`，仅 `<input>` / `<textarea>` / `[contenteditable]` / `[data-allow-native-context]` 例外放行系统菜单（保留输入便利）。
- **重构** `SessionContextMenu.svelte` 与 `TabContextMenu.svelte` 改用 `AppContextMenu` + `use:contextMenu`；外部 API（onClose / onAction 等 props）保持兼容，菜单项文案/顺序/动作不变。
- **行为契约**：Phase 1 内"选中文字 + 右键"= 不弹（暂无文本菜单——比当前漏到 OS 菜单已是改善）；文本菜单作为 Phase 2 范围记录到 design.md Future Scope，留下个会话实现。
- **不改动** 现有 `sidebar-navigation` / `tab-management` spec 的 Requirement——两个菜单的菜单项、动作语义、触发位置维持不变；变化只发生在底层组件实现，按 OpenSpec "implementation details belong in design.md" 原则不修主 spec。

## Capabilities

### New Capabilities

- `frontend-context-menu`：定义全应用右键菜单的"三态决策"（自定义菜单 / 系统菜单例外 / 不弹）、`AppContextMenu` 浮层视觉与状态规范、`use:contextMenu` action 契约、键盘可达性、WKWebView smart-select 防护规则。后续 Phase 2 各 surface 加菜单 items 时，仍统一从本 capability 引用 `use:contextMenu` 与 `AppContextMenu`，不再各自实现菜单组件。

### Modified Capabilities

无。`sidebar-navigation` 与 `tab-management` 的右键菜单行为契约（菜单项、动作语义）不变，仅底层实现重构。

## Impact

- **代码**：
  - 新增 `ui/src/lib/components/AppContextMenu.svelte`（约 200 行，承载视觉 + 键盘导航 + clamp + a11y）。
  - 新增 `ui/src/lib/contextMenu.svelte.ts`（`use:contextMenu` action + 全局监听初始化）。
  - 修改 `ui/src/main.ts`：启动时注册全局 `contextmenu` 兜底监听。
  - 修改 `ui/src/components/SessionContextMenu.svelte` / `TabContextMenu.svelte` 改用 `AppContextMenu`；外部消费者（`Sidebar.svelte` / `TabBar.svelte`）调用面尽量保持不变。
  - CSS：`.session-item` 加 `user-select: none`（修截图同款 bug 的兜底——即使 use:contextMenu 已防 smart-select，CSS 提供二重防线）。
- **测试**：vitest 单测覆盖 `use:contextMenu` 三态决策与 smart-select 防护；Playwright e2e 覆盖键盘 ↑↓ Enter Esc + Menu 键触发；现有 sidebar / tab 右键菜单回归测试不应改动。
- **依赖**：纯前端改动，无新依赖、无 IPC 字段改动、无 Rust crate 改动。
- **风险**：全局 `preventDefault contextmenu` 是较激进的兜底——任何未来新加可点击元素若需保留系统菜单（罕见），需显式打 `data-allow-native-context` 标签；这一点写进 spec 让契约清晰。

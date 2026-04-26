## Why

当前 UI 字体写死在 `ui/src/app.css` 的 `--font-sans` / `--font-mono` token 中，且 fallback 栈比 TS 原版（`../claude-devtools/src/renderer/index.css:393-395`）短。用户无法按个人偏好切换 UI 字体（典型诉求：换 JetBrains Mono / Fira Code 用于 mono，换中文字体用于 sans）。同时 `ui/src/components/TeammateMessageItem.svelte:277` 仍硬编码 mono 字体串没接 token，是 token 化遗漏。

## What Changes

- **后端 schema 扩展**：`DisplayConfig` 新增 `font_sans: Option<String>` + `font_mono: Option<String>`。`None` 表示使用 app.css 默认 token，`Some(s)` 表示用户覆盖；空白字符串视同 None
- **IPC 透传**：`getConfig` / `updateConfig` 透传新字段（camelCase: `fontSans` / `fontMono`），`crates/cdt-api/tests/ipc_contract.rs` 加序列化字段断言
- **前端运行时应用**：新增 `ui/src/lib/fonts.ts::applyFonts(config)`，类比 `lib/theme.ts::applyTheme`，把 `display.fontSans/fontMono` 写到 `:root` 的 `--font-sans` / `--font-mono`；空值则 `removeProperty` 让 app.css 默认 token 生效
- **Settings UI**：`SettingsView` 「显示」段新增两个 `<input type="text">`（sans / mono）+ 「恢复默认」按钮，乐观更新模式
- **默认值对齐原版**：把 `app.css` 的 `--font-sans` / `--font-mono` 默认值改成 TS 原版字体栈
- **token 化清理**：把 `TeammateMessageItem.svelte:277` 的硬编码改成 `var(--font-mono)`

## Capabilities

### New Capabilities
（无）

### Modified Capabilities
- `configuration-management`：新增「Display config exposes user-customizable font family」requirement；`Persist application configuration` requirement 的字段清单覆盖 `display.fontSans` / `display.fontMono`

## Impact

- **代码**：
  - `crates/cdt-config/src/types.rs::DisplayConfig`、`crates/cdt-config/src/defaults.rs`、`crates/cdt-config/src/manager.rs`（update_field 路径）
  - `crates/cdt-api/tests/ipc_contract.rs`（contract test）
  - `ui/src/lib/fonts.ts`（新增）、`ui/src/lib/api.ts`（DisplayConfig 字段补齐）、`ui/src/App.svelte`（启动时调 applyFonts）、`ui/src/routes/SettingsView.svelte`、`ui/src/app.css`、`ui/src/components/TeammateMessageItem.svelte`
- **API**：`getConfig` / `updateConfig` 字段扩展；新字段对老前端透明（序列化 omit None 时不出现）
- **依赖**：无新增（前端 / 后端均不引入新 cargo / npm 包）
- **数据迁移**：无；老配置文件读取时新字段缺失通过 serde default → None
- **首屏 UX**：用户已设非默认字体时，首帧仍吃 app.css 默认 token 直至 onMount 调 applyFonts，存在 FOUT；与 `applyTheme` 现有路径一致，本 change 显式接受（详见 design D5b）

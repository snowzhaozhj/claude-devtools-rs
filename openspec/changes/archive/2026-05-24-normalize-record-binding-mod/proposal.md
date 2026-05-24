## Why

跨平台快捷键 config 同步在 mac → Windows 路径上**完全失效但 UI 默不作声**：用户在 mac 录 `command-palette.toggle = meta+shift+p` 后 cdt-config 持久化字面量 `meta+shift+p`，cdt-config 文件经用户手动复制 / iCloud / 第三方同步工具搬到 Windows 设备（应用本身不引入跨设备同步路径，本 PR 也不增删该路径），`bootstrapOverrides → applyOverrides` 原样写入 registry；但 Windows 平台 `normalize(event)` SHALL NOT 把 `event.metaKey` 加入 modifier（spec 既有决策，防 Win 键误触发），dispatcher 永不命中该 binding。同时 Settings panel 行调 `formatShortcut("meta+shift+p")` 在 Windows 渲染为 `Win+Shift+P`，让用户以为快捷键已生效——实际永远不会触发。

Windows 录键路径同样错位：用户在 Windows 按 `Win+B` 录键，`normalize(event)` 在 non-mac 忽略 `metaKey` 后产出字面量 `b`，`KeyRecorderInput` 静默 commit `b` 单字符 binding；用户以为录的是组合键，实际持久化的是按一下 `B` 就触发——这既是数据污染，也是录键体验断裂。

GitHub issue #247（来源 PR #244 windows-compat-reviewer §11.2 followup）。

## What Changes

- **MODIFIED** capability `keyboard-shortcuts`：录键产出与 cdt-config 持久化的 binding 字面量 SHALL 用跨平台 `mod` token 作为主修饰键 source-of-truth，存量 `meta+x` / `ctrl+x` 字面量 SHALL 在 bootstrap 阶段迁移为 `mod+x`，dispatcher 行为零回归（仍由 `normalizeBinding(mod+...)` 平台特化展开）
- **MODIFIED** capability `keyboard-shortcuts`：`KeyRecorderInput` 在 non-mac 平台 + `event.metaKey === true` 时 SHALL NOT commit、SHALL NOT blur，停留 recording 态 + aria-live 警告"Windows 不支持 Win 键作为修饰键"
- nit：`formatMainKey` 对 `Space` 在 mac 显示 `␣` 符号；`register-app-shortcuts.ts` 顶部注释"9 条 / 17 specs"修正

不在 scope（独立 followup）：
- `tab.close` advisoryHints Windows + Tauri WebView2 提示（涉及 src-tauri 配置变更）
- `WIN_TEXT.meta` 删除（保留作为防御性 fallback；走 mod 归一后正常路径不触发）
- `formatShortcut` 在 Windows 遇到 `meta` 显示"(不可用)"标注（迁移后 binding 不再含字面 `meta`，自然规避）

## Capabilities

### New Capabilities
（无）

### Modified Capabilities
- `keyboard-shortcuts`：扩展 D2 录键归一化语义为"录键产出 / 持久化 binding SHALL 用 mod 字面量"；新增 bootstrap 字面量迁移规则；扩展录键守卫覆盖"non-mac + metaKey" 场景

## Impact

- **代码**：
  - `ui/src/components/settings/KeyRecorderInput.svelte`（handleKeyDown：mod 反写 + Win 键守卫 + warning aria-live）
  - `ui/src/lib/platform.ts`（新增 `recordBindingFromEvent(event)` 工厂、`normalizeBindingToMod(binding)` 字面量迁移；`formatMainKey` Space mac 特化）
  - `ui/src/lib/keyboard/customization.ts`（`mergeOverrides` 调 `normalizeBindingToMod`）
  - `ui/src/lib/keyboard/register-app-shortcuts.ts`（顶部注释修正）
- **测试**：vitest 单测覆盖 mod 反写 / 字面量迁移 / Win 键守卫 / Space mac ␣ 渲染
- **数据**：cdt-config `keyboardShortcuts` HashMap 字段含义不变（仍是 `Map<id, binding>`），但 binding 字面量从平台特化（`meta+x` / `ctrl+x`）迁移为跨平台（`mod+x`）；老 config 加载时自动迁移，无需用户操作、无需 schema 版本号
- **dispatcher 零回归**：现有 `normalizeBinding(mod+...)` 平台展开逻辑不变；现有 spec D2"non-mac 不识 metaKey" Scenario 不变；现有 `mod+k` defaults 工作流不变
- **依赖**：无新增依赖

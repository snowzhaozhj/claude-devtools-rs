/*
 * Context menu 用 Settings 快照（per-app 单例）。
 *
 * 三字段（externalEditor / searchEngine / terminalApp）从 `getConfig()` 拷贝
 * 到本模块本地状态，供 `selectionMenu` ctxProvider + 各 surface 组件 build
 * `MenuItemContext.settings` 时**同步**读取（避免每次右键都走 IPC）。
 *
 * 写入路径：
 * - `App.svelte` 启动期 getConfig() 后 → `setMenuSettings(config.general)`
 * - `SettingsView.svelte` 用户改 dropdown 后 → `setMenuSettings(updatedConfig.general)`
 *
 * 读取路径：
 * - `main.ts::installSelectionContextMenu` 的 ctxProvider 调 `getMenuSettings()`
 * - 各 surface 组件 `oncontextmenu` 内 `getMenuSettings()` 拿 settings 注入 ctx
 *
 * Default fallback：老后端 / 启动期 `getConfig()` 还没 resolve 时返回安全默认值
 * （`system` 编辑器 / Google 搜索 / Terminal app）。
 */

import type { GeneralConfig } from "../api";
import type {
  ExternalEditorSetting,
  SearchEngineSetting,
  TerminalAppSetting,
} from "./menu-items";

interface MenuSettings {
  externalEditor: ExternalEditorSetting;
  searchEngine: SearchEngineSetting;
  terminalApp: TerminalAppSetting;
}

const DEFAULT_SETTINGS: MenuSettings = {
  externalEditor: "system",
  searchEngine: { type: "google" },
  terminalApp: "terminal",
};

// 模块级 plain object（非 $state）——本模块由 .ts 模块形式被各处 import，
// 同步 get/set 已足够；不需要 Svelte 响应性（消费者都在 oncontextmenu /
// installSelectionContextMenu 里现读，不订阅变化）
let cachedSettings: MenuSettings = { ...DEFAULT_SETTINGS };

/**
 * 写入快照（App / SettingsView 调用）。从 GeneralConfig 中提取三字段，
 * 缺字段时 fallback 到默认值。
 */
export function setMenuSettings(general: GeneralConfig | undefined): void {
  if (!general) {
    cachedSettings = { ...DEFAULT_SETTINGS };
    return;
  }
  cachedSettings = {
    externalEditor: general.externalEditor ?? DEFAULT_SETTINGS.externalEditor,
    searchEngine: general.searchEngine ?? DEFAULT_SETTINGS.searchEngine,
    terminalApp: general.terminalApp ?? DEFAULT_SETTINGS.terminalApp,
  };
}

/**
 * 同步读取当前快照——返回浅拷贝避免调用方 mutate 污染。
 */
export function getMenuSettings(): MenuSettings {
  return { ...cachedSettings };
}

/** 测试 helper：reset 到默认值 */
export function resetMenuSettingsForTesting(): void {
  cachedSettings = { ...DEFAULT_SETTINGS };
}

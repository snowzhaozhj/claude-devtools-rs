/**
 * 内置快捷键清单（meta 维度）。
 *
 * 仅提供 `id / category / description / defaultBinding / allowInInput / preventDefault`
 * 等无 handler 字段；handler 由各注册点（App.svelte / PaneView / DashboardView 等）
 * 在 `registerShortcut(spec)` 时供给。这样 defaults.ts 与 UI 解耦，单测无需 mock 业务函数。
 *
 * **D8 单 binding 单 spec 1:1 关系**：每条 id SHALL 唯一。多 instance 注册同 ID 会触发
 * registry "重复 ID 抛错"；典型如 `session.jump-to-latest` 仅在 PaneView 注册一次。
 *
 * 详 `openspec/specs/keyboard-shortcuts/spec.md::内置快捷键清单`。
 */

import type { ShortcutCategory } from "./registry";
import type { ShortcutBinding } from "../platform";

export interface ShortcutMeta {
  id: string;
  category: ShortcutCategory;
  description: string;
  defaultBinding: ShortcutBinding;
  /** 默认 false：input/textarea/contenteditable 焦点时跳过；命令面板等需 true。 */
  allowInInput?: boolean;
  /** 默认 true：handler 不返回 false 时调用 event.preventDefault()。 */
  preventDefault?: boolean;
}

/**
 * 18 条内置快捷键 across 5 category：
 * - global   : command-palette.toggle
 * - sidebar  : sidebar.toggle
 * - search   : search.focus / search.in-session
 * - tabs     : tab.switch.1-9 (9 条) / tab.close / tab.next / tab.prev /
 *              pane.split / pane.focus.next / pane.focus.prev
 * - session  : session.jump-to-latest
 *
 * spec scenario "内置 ≥ 14 条 across 5 category" 验证此清单上限。
 */
export const SHORTCUT_DEFAULTS: ReadonlyArray<ShortcutMeta> = [
  // ---- global ------------------------------------------------------------
  {
    id: "command-palette.toggle",
    category: "global",
    description: "打开 / 关闭命令面板",
    defaultBinding: "mod+k",
    allowInInput: true, // 命令面板可在搜索框 focus 时唤起
  },
  // ---- sidebar -----------------------------------------------------------
  {
    id: "sidebar.toggle",
    category: "sidebar",
    description: "切换侧栏折叠 / 展开",
    defaultBinding: "mod+b",
  },
  // ---- search ------------------------------------------------------------
  {
    id: "search.focus",
    category: "search",
    description: "聚焦 Dashboard 搜索框",
    defaultBinding: "/",
    // allowInInput 默认 false：input 焦点时浏览器原生 / 字符直接输入
  },
  {
    id: "search.in-session",
    category: "search",
    description: "在当前会话内查找",
    defaultBinding: "mod+f",
    // allowInInput 默认 false：input 焦点时浏览器原生（如复制 / 选词）
  },
  // ---- tabs --------------------------------------------------------------
  ...Array.from({ length: 9 }, (_, i) => i + 1).map<ShortcutMeta>((n) => ({
    id: `tab.switch.${n}`,
    category: "tabs",
    description: `切换到第 ${n} 个 tab`,
    defaultBinding: `mod+${n}`,
  })),
  {
    id: "tab.close",
    category: "tabs",
    description: "关闭当前 tab",
    defaultBinding: "mod+w",
  },
  {
    id: "tab.next",
    category: "tabs",
    description: "下一个 tab（循环）",
    defaultBinding: "mod+]",
  },
  {
    id: "tab.prev",
    category: "tabs",
    description: "上一个 tab（循环）",
    defaultBinding: "mod+[",
  },
  {
    id: "pane.split",
    category: "tabs",
    description: "拆分当前 pane（向右）",
    defaultBinding: "mod+\\",
  },
  {
    id: "pane.focus.next",
    category: "tabs",
    description: "聚焦下一个 pane（循环）",
    defaultBinding: "mod+alt+ArrowRight",
  },
  {
    id: "pane.focus.prev",
    category: "tabs",
    description: "聚焦上一个 pane（循环）",
    defaultBinding: "mod+alt+ArrowLeft",
  },
  // ---- session -----------------------------------------------------------
  {
    id: "session.jump-to-latest",
    category: "session",
    description: "跳到当前 session 最新消息",
    defaultBinding: { mac: "mod+ArrowDown", other: "ctrl+End" },
  },
];

/** 根据 id 拿 meta；缺失返回 undefined（spec scenario "幽灵 ID 跳过"）。 */
export function getShortcutMeta(id: string): ShortcutMeta | undefined {
  return SHORTCUT_DEFAULTS.find((m) => m.id === id);
}

/** 按 category 分组（Settings panel 渲染用）。顺序保留 SHORTCUT_DEFAULTS 内出现序。 */
export function groupByCategory(): Record<ShortcutCategory, ShortcutMeta[]> {
  const groups: Record<ShortcutCategory, ShortcutMeta[]> = {
    global: [],
    tabs: [],
    sidebar: [],
    search: [],
    session: [],
  };
  for (const m of SHORTCUT_DEFAULTS) groups[m.category].push(m);
  return groups;
}

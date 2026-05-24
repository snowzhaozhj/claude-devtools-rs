/**
 * App.svelte 全局快捷键批量注册器（§5 迁移产物）。
 *
 * 把"曾内联在 `App.svelte::handleGlobalKeydown` 内的 17 条 App-owned 全局快捷键"
 * （其中 `tab.switch.1` ~ `tab.switch.9` 占 9 条，外加 `command-palette.toggle` / `sidebar.toggle` /
 * `tab.close` / `tab.next` / `tab.prev` / `pane.split` / `pane.focus.next` / `pane.focus.prev` 8 条 = 17）
 * 抽出为可单测的纯函数：调用方按 id 给 handler，本模块按 `SHORTCUT_DEFAULTS`
 * 提供 spec meta（description / defaultBinding / allowInInput / preventDefault）
 * 调 `registerShortcut` 并返回汇总 unregister 闭包。
 *
 * **App-owned 范围**：
 * - global  : `command-palette.toggle`
 * - sidebar : `sidebar.toggle`
 * - tabs    : `tab.switch.1` ~ `tab.switch.9` / `tab.close` / `tab.next` / `tab.prev`
 *             / `pane.split` / `pane.focus.next` / `pane.focus.prev`
 *
 * **不归 App 管**（参见各自 owner，避免重复注册抛 D8 错误）：
 * - `search.focus`            → DashboardView
 * - `session.jump-to-latest`  → PaneView shared handler（D8 单 binding 单 spec 1:1 关系）
 *
 * 调用约束：SHALL 在 `bootstrapOverrides()` 完成后调用，确保 user override 已就绪。
 */

import { registerShortcut, type ShortcutSpec } from "./registry";
import { getShortcutMeta } from "./defaults";

/** App.svelte 负责注册的全部 spec ID（17 条；按 SHORTCUT_DEFAULTS 顺序）。 */
export const APP_OWNED_SHORTCUT_IDS: ReadonlyArray<string> = [
  "command-palette.toggle",
  "sidebar.toggle",
  ...Array.from({ length: 9 }, (_, i) => `tab.switch.${i + 1}`),
  "tab.close",
  "tab.next",
  "tab.prev",
  "pane.split",
  "pane.focus.next",
  "pane.focus.prev",
];

export type AppShortcutHandlers = Record<string, ShortcutSpec["handler"]>;

/**
 * 用 caller 提供的 handler 映射表批量调 `registerShortcut`。
 *
 * - 每个 ID SHALL 在 handlers 中有对应函数；缺失 → console.warn 并跳过（不抛错，
 *   保留 graceful degrade：UI 半残总比挂掉好）。
 * - meta 缺失（defaults.ts 与本表 drift）→ 同样 console.warn 跳过。
 *
 * @param handlers id → handler；handler 返回 `false` = 不消费、不 preventDefault
 * @returns 批量 unregister 闭包；onDestroy 调一次即可
 */
export function registerAppShortcuts(handlers: AppShortcutHandlers): () => void {
  const unregisters: Array<() => void> = [];
  for (const id of APP_OWNED_SHORTCUT_IDS) {
    const meta = getShortcutMeta(id);
    if (!meta) {
      // eslint-disable-next-line no-console
      console.warn(`[keyboard] App-owned shortcut id missing in defaults: ${id}`);
      continue;
    }
    const handler = handlers[id];
    if (!handler) {
      // eslint-disable-next-line no-console
      console.warn(`[keyboard] App-owned shortcut handler missing: ${id}`);
      continue;
    }
    unregisters.push(registerShortcut({ ...meta, handler }));
  }
  return () => {
    for (const u of unregisters) u();
  };
}

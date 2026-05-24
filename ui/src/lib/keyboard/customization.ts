/**
 * 用户自定义 overrides 与 cdt-config IPC 桥接层。
 *
 * 责任：
 * - `bootstrapOverrides()`：启动期一次性调 `getConfig()`，把 `keyboardShortcuts`
 *   字段写进 registry 的 `pendingOverrides`；IPC 失败则保持 builtin defaults +
 *   `setConfigLoadError(reason)` 暴露错误条 store。
 * - `persistOverrides(overrides)`：Settings Save 触发，调 `updateConfig("keyboardShortcuts", ...)`
 *   单次整体替换；spec D3b（codex 二审 #3 修订）：**不**做 debounce，commit 仅在 Save 时落 IPC。
 * - `mergeOverrides(defaults, overrides)`：纯函数，把 overrides 套到 defaults 上，
 *   过滤"幽灵 ID"（overrides 里有但 defaults 里没有的 id——前一版本删过的快捷键）。
 * - `retryBootstrap()`：错误条上的"重试"按钮调用，重读 IPC + 应用。
 *
 * 详 `openspec/specs/keyboard-shortcuts/spec.md::用户自定义覆盖`。
 */

import { getConfig, updateConfig } from "../api";
import { normalizeBindingToMod } from "../platform";
import {
  applyOverrides,
  setPendingOverrides,
  setConfigLoadError,
} from "./registry";
import type { ShortcutMeta } from "./defaults";
import { SHORTCUT_DEFAULTS } from "./defaults";

/**
 * 把 overrides 映射套到 defaults 之上。返回 `{id → effectiveBinding}`：
 * - overrides[id] 存在且 id 在 defaults 中：用 override（已是 binding string）
 * - overrides[id] 不在 defaults：跳过（"幽灵 ID"——定义已删除的旧快捷键）
 * - 每条 override 的 binding 字面量经 `normalizeBindingToMod` 迁移：把存量平台特化
 *   字面量（mac 录入的 `meta+x`、win 录入的 `ctrl+x`）转为跨平台 `mod+x`，确保
 *   cdt-config 跨设备同步后在异平台启动时正确归一。该迁移幂等且无信息丢失。
 *
 * 注意返回值类型是 `Record<id, string>`——只含 override 的项；调用方按需 fallback
 * 到 defaults 的 defaultBinding。
 */
export function mergeOverrides(
  defaults: ReadonlyArray<ShortcutMeta>,
  overrides: Record<string, string>,
): Record<string, string> {
  const known = new Set(defaults.map((m) => m.id));
  const result: Record<string, string> = {};
  for (const [id, binding] of Object.entries(overrides)) {
    if (!known.has(id)) continue; // 幽灵 ID 跳过
    if (typeof binding !== "string" || binding.length === 0) continue;
    result[id] = normalizeBindingToMod(binding);
  }
  return result;
}

/**
 * 启动期 bootstrap：调 IPC `getConfig()` 拿 `keyboardShortcuts`；失败 → 保持 builtin
 * defaults（即 setPendingOverrides({})）+ 暴露错误条 reason。
 *
 * 调用时机：App.svelte onMount 早期，**registerShortcut 之前** —— 这样 register
 * 时 pendingOverrides 已就绪，自动用 override 替代 defaultBinding。
 */
export async function bootstrapOverrides(): Promise<void> {
  try {
    const config = await getConfig();
    const raw = config.keyboardShortcuts ?? {};
    const merged = mergeOverrides(SHORTCUT_DEFAULTS, raw);
    setPendingOverrides(merged);
    setConfigLoadError(null);
  } catch (err) {
    const reason = err instanceof Error ? err.message : String(err);
    setPendingOverrides({});
    setConfigLoadError(reason);
  }
}

/**
 * Settings Save：调 `updateConfig("keyboardShortcuts", overrides)` 整体替换 +
 * registry batch update（applyOverrides 全量 rebuild keymap）。
 *
 * 失败 → throw 给调用方（KeyboardShortcutsPanel SHALL 捕获并显示 inline 错误 +
 * 回滚 UI 录键 overlay；registry 内存 keymap 已未变 / 需要回滚）。
 */
export async function persistOverrides(
  overrides: Record<string, string>,
): Promise<void> {
  // 净化：剥离幽灵 ID + 空串
  const sanitized = mergeOverrides(SHORTCUT_DEFAULTS, overrides);
  await updateConfig("keyboardShortcuts", sanitized);
  // IPC 写入成功 → registry 批量 apply
  applyOverrides(sanitized);
  setConfigLoadError(null);
}

/**
 * 错误条上"重试"按钮：重新尝试 bootstrap；成功后 applyOverrides 触发已注册 spec
 * 的 keymap rebuild（没注册的 spec 后续 register 时自动走 pendingOverrides）。
 */
export async function retryBootstrap(): Promise<void> {
  try {
    const config = await getConfig();
    const raw = config.keyboardShortcuts ?? {};
    const merged = mergeOverrides(SHORTCUT_DEFAULTS, raw);
    applyOverrides(merged);
    setConfigLoadError(null);
  } catch (err) {
    const reason = err instanceof Error ? err.message : String(err);
    setConfigLoadError(reason);
  }
}

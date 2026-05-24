/*
 * window-level 文本选区菜单（Layer 2 / Task 8 / design.md::D10）。
 *
 * spec: openspec/specs/frontend-context-menu/spec.md
 *   ::Requirement 文本选区菜单（window-level handler）
 *
 * 三层级联：
 *   Layer 1: surface-level `use:contextMenu` action（stopPropagation 拦截）
 *   Layer 2: 本模块 — window contextmenu listener（检测选区→弹选区菜单）
 *   Layer 3: `installGlobalContextMenuFallback` — 兜底 preventDefault 不弹菜单
 *
 * 注册顺序硬约束：main.ts 内 `installSelectionContextMenu` SHALL 先于
 * `installGlobalContextMenuFallback` 调用——bubble 阶段同一 phase 注册先后顺序
 * 决定执行顺序，Layer 2 先跑才能在 Layer 3 preventDefault 之前判断选区。
 *
 * 触发规则：
 * 1. 跳过 `e.defaultPrevented`（Layer 1 已处理）
 * 2. 跳过 `target.closest('input, textarea, [contenteditable], [data-allow-native-context]')`
 *    （让浏览器原生菜单接管输入便利）
 * 3. 检测 `window.getSelection()?.toString().length > 0`
 * 4. 满足条件：`e.preventDefault()` + 调 `openMenu(buildSelectionItems(...))`
 *
 * HMR 幂等：window sentinel + import.meta.hot.dispose 双保险。
 */

import { openMenu, ensureGlobalCloseListeners } from "../contextMenu.svelte";
import { buildSelectionItems, type MenuItemContext } from "./menu-items";

declare global {
  interface Window {
    __cdtSelectionMenuInstalled?: boolean;
    /** 调用方 main.ts 注入：让 selection 菜单 factory 拿到当前 settings + dispatch */
    __cdtSelectionMenuCtxProvider?: () => MenuItemContext | null;
  }
}

/**
 * 注册 window-level contextmenu listener。
 *
 * @param ctxProvider 提供 `MenuItemContext` 的回调——selection 菜单 factory
 *   需要 `settings.searchEngine` / `dispatch.copyToClipboard` 等字段。lazy
 *   读取（每次右键时调用）让 settings 变更可即时生效。返回 `null` 时菜单
 *   不弹（典型场景：app 启动早期 settings 未就绪）。
 */
export function installSelectionContextMenu(
  ctxProvider: () => MenuItemContext | null,
): void {
  if (typeof window === "undefined") return;
  if (window.__cdtSelectionMenuInstalled) return;
  window.__cdtSelectionMenuInstalled = true;
  window.__cdtSelectionMenuCtxProvider = ctxProvider;

  // bubble 阶段注册（与 Layer 3 相同），让 Layer 1 的 surface action
  // stopPropagation 能阻止冒泡——本 listener 仅对未被 surface 处理的事件触发
  window.addEventListener("contextmenu", selectionContextMenuHandler, false);

  // 共享 Layer 1 的全局关闭触发（外点 / Esc / blur / scroll / resize），
  // 避免选区菜单弹出后无法用同样手势关闭
  ensureGlobalCloseListeners();
}

function selectionContextMenuHandler(e: Event): void {
  // 类型守卫：仅处理 MouseEvent（contextmenu 由 Menu 键 / Shift+F10 触发时
  // 是 KeyboardEvent，那条路径由 Layer 1 / 浏览器原生处理，本 layer 不接管）
  if (!(e instanceof MouseEvent)) return;

  // Layer 1 已处理（surface action stopPropagation + preventDefault），跳过
  if (e.defaultPrevented) return;

  const target = e.target as HTMLElement | null;
  if (!target) return;

  // 输入元素 / contenteditable / 显式 opt-in 跳过——保留浏览器原生输入菜单
  // （粘贴 / 拼写检查 / 朗读等）。判断逻辑与 Layer 3 一致——但这里直接 return
  // **不** preventDefault，让 Layer 3 自己再判一次按相同规则放行
  if (target.isContentEditable) return;
  const editableAncestor = target.closest("[contenteditable]") as HTMLElement | null;
  if (editableAncestor && editableAncestor.getAttribute("contenteditable") !== "false") {
    return;
  }
  if (target.closest("input, textarea, [data-allow-native-context]")) return;

  // 选区检测——无选区时跳过让 Layer 3 兜底 preventDefault（不弹任何菜单）
  const selection = window.getSelection();
  const selectionText = selection?.toString() ?? "";
  if (!selectionText) return;

  // 拿 ctx——provider 缺失或返回 null 时跳过（main.ts 启动期可能 settings 未就绪）
  const provider = window.__cdtSelectionMenuCtxProvider;
  const baseCtx = provider?.();
  if (!baseCtx) return;

  // 把当前选区 text 注入 ctx——provider 返回的 baseCtx 可能携带空 selectionText
  // （main.ts 的 provider 仅暴露 settings + dispatch，selection 由本 listener
  // 当场读取最权威）
  const ctx: MenuItemContext = { ...baseCtx, selectionText };

  const items = buildSelectionItems(selectionText, ctx);
  if (items.length === 0) return;

  e.preventDefault();
  openMenu(target, items, e.clientX, e.clientY);
}

if (typeof import.meta !== "undefined" && import.meta.hot) {
  import.meta.hot.dispose(() => {
    if (typeof window === "undefined") return;
    window.removeEventListener("contextmenu", selectionContextMenuHandler, false);
    delete window.__cdtSelectionMenuInstalled;
    delete window.__cdtSelectionMenuCtxProvider;
  });
}

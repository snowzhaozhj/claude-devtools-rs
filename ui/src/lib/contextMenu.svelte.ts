/*
 * Context menu 基础设施 — 全应用右键菜单系统。
 *
 * spec: openspec/specs/frontend-context-menu/spec.md
 * design.md D1 全局兜底 / D2 provider 函数 / D5 smart-select 防护 / D7 portal
 *
 * 三个公开 API：
 * - `contextMenu` Svelte action：挂在元素上接管右键
 * - `installGlobalContextMenuFallback()`：在 main.ts 启动时调一次，全局拦截
 *   未被 use:contextMenu 处理的 contextmenu 事件
 * - `ContextMenuItem`、`ContextMenuProvider` 类型
 */

import { mount, unmount } from "svelte";
import AppContextMenu, {
  type ContextMenuItem,
} from "./components/AppContextMenu.svelte";

export type { ContextMenuItem } from "./components/AppContextMenu.svelte";
export type ContextMenuProvider =
  | ContextMenuItem[]
  | ((event: MouseEvent | KeyboardEvent) => ContextMenuItem[] | null);

// HMR 持久化 sentinel 挂在 window 全局，避免 vite 模块重载时模块级 flag 归零
// 导致 listener 重复注册（详 ensureGlobalCloseListeners / installGlobalContextMenuFallback）。
declare global {
  interface Window {
    __cdtContextMenuFallbackInstalled?: boolean;
    __cdtContextMenuCloseListenersInstalled?: boolean;
  }
}

// ---------- portal mount 实例管理 ----------

interface MenuInstance {
  component: ReturnType<typeof mount>;
  host: HTMLElement;
  trigger: HTMLElement;
}

let activeInstance: MenuInstance | null = null;

function closeActive(): void {
  if (!activeInstance) return;
  const { component, host, trigger } = activeInstance;
  activeInstance = null;
  try {
    unmount(component);
  } catch {
    // 已 unmount 或挂载失败时忽略——避免清理路径报错连带其它事件处理
  }
  host.remove();
  // focus 还回 trigger，让 a11y 焦点链不丢（spec: AppContextMenu 关闭触发条件）
  if (trigger.isConnected) trigger.focus({ preventScroll: true });
}

function openMenu(
  trigger: HTMLElement,
  items: ContextMenuItem[],
  x: number,
  y: number,
): void {
  // 同一刻仅允许一个菜单实例（spec: portal 多次右键替换 instance）
  closeActive();

  if (items.length === 0) return;

  const host = document.createElement("div");
  // 让 host 不参与布局影响 — 仅作为挂载点；AppContextMenu 内部 position: fixed
  host.style.position = "absolute";
  host.style.top = "0";
  host.style.left = "0";
  document.body.appendChild(host);

  const component = mount(AppContextMenu, {
    target: host,
    props: {
      x,
      y,
      items,
      onClose: closeActive,
    },
  });

  activeInstance = { component, host, trigger };
}

// ---------- 全局关闭触发（在所有 instance 上共享）----------
// 注意：这些 listener **常驻** document/window，避免每个菜单实例 attach/detach
// 的开销，也避免菜单关闭瞬间 listener race。handler 内若 activeInstance===null 直接 return。
//
// HMR 持久化：模块级 flag 在 vite HMR 重载后归零，会让旧 listener 残留 + 新 listener
// 叠加。把 sentinel 挂到 window 全局后再判幂等——HMR 时 window 不重建，旧 handler
// 因模块作用域消失变成无效引用 + 新 handler 跳过注册，避免叠加。


function onDocumentMouseDown(e: MouseEvent): void {
  if (!activeInstance) return;
  const target = e.target as Node | null;
  if (!target) return;
  // 点击发生在菜单 host 内 → 不关；其它位置（包括 trigger 元素自身）→ 关
  if (activeInstance.host.contains(target)) return;
  closeActive();
}

function onDocumentKeyDown(e: KeyboardEvent): void {
  if (!activeInstance) return;
  if (e.key === "Escape") {
    e.preventDefault();
    closeActive();
  }
}

function onWindowBlur(): void {
  if (activeInstance) closeActive();
}

function onWindowResize(): void {
  if (activeInstance) closeActive();
}

function onAnyScroll(): void {
  if (activeInstance) closeActive();
}

function ensureGlobalCloseListeners(): void {
  // window 全局 sentinel 让 vite HMR 模块重载后仍幂等（详上方注释）
  if (window.__cdtContextMenuCloseListenersInstalled) return;
  window.__cdtContextMenuCloseListenersInstalled = true;
  document.addEventListener("mousedown", onDocumentMouseDown, true);
  document.addEventListener("keydown", onDocumentKeyDown, true);
  window.addEventListener("blur", onWindowBlur);
  window.addEventListener("resize", onWindowResize);
  // capture=true 让任意祖先 scroll 都能捕获到
  window.addEventListener("scroll", onAnyScroll, true);
}

// ---------- Svelte action: use:contextMenu ----------

export function contextMenu(node: HTMLElement, provider: ContextMenuProvider) {
  ensureGlobalCloseListeners();

  let currentProvider = provider;

  function resolveItems(e: MouseEvent | KeyboardEvent): ContextMenuItem[] {
    if (typeof currentProvider === "function") {
      return currentProvider(e) ?? [];
    }
    return currentProvider;
  }

  function handleContextMenu(e: MouseEvent): void {
    e.preventDefault();
    e.stopPropagation();
    const items = resolveItems(e);
    if (items.length === 0) return;
    openMenu(node, items, e.clientX, e.clientY);
  }

  // smart-select 防护（design.md D5 / spec WKWebView smart-select 防护）：
  // 右键 mousedown 阶段 WebKit 会 smart-select 光标下的"词"；contextmenu 事件
  // 里再 preventDefault 已晚。在 mousedown 阶段判断"无选区时阻止默认"，
  // 保留"已有选区"路径让 Phase 2 文本菜单可消费。
  function handleMouseDown(e: MouseEvent): void {
    if (e.button !== 2) return;
    const sel = window.getSelection();
    if (sel && sel.toString().length > 0) return;
    e.preventDefault();
  }

  // 键盘 contextmenu（Menu 键 / Shift+F10）：浏览器原生派发 contextmenu 事件
  // 时 e.button === 0 / clientX === 0 / clientY === 0，定位到 trigger bbox 中心。
  // 现代 WebKit 在 keyboard contextmenu 时仍走 contextmenu listener，所以下面
  // 这个分支主要是定位修正。
  function handleKeyDown(e: KeyboardEvent): void {
    if (e.key !== "ContextMenu" && !(e.key === "F10" && e.shiftKey)) return;
    e.preventDefault();
    const items = resolveItems(e);
    if (items.length === 0) return;
    const rect = node.getBoundingClientRect();
    openMenu(node, items, rect.left + rect.width / 2, rect.top + rect.height / 2);
  }

  node.addEventListener("contextmenu", handleContextMenu);
  node.addEventListener("mousedown", handleMouseDown);
  node.addEventListener("keydown", handleKeyDown);

  return {
    update(newProvider: ContextMenuProvider) {
      currentProvider = newProvider;
    },
    destroy() {
      node.removeEventListener("contextmenu", handleContextMenu);
      node.removeEventListener("mousedown", handleMouseDown);
      node.removeEventListener("keydown", handleKeyDown);
      // 若菜单仍挂着且 trigger 是当前 instance 的 trigger，关掉避免残留
      if (activeInstance?.trigger === node) closeActive();
    },
  };
}

// ---------- 全局兜底（main.ts 启动时调）----------

function globalContextMenuHandler(e: Event): void {
  // contextmenu 事件触发；input/textarea/contenteditable/data-allow-native-context
  // 的元素放行系统菜单（保留输入便利）；defaultPrevented 表示元素自身已处理。
  if (e.defaultPrevented) return;
  const target = e.target as HTMLElement | null;
  if (!target) return;
  // contenteditable 检测：用 isContentEditable（HTMLElement 属性，覆盖任意
  // truthy 值 + 继承）作为主路径；同时用 selector `[contenteditable]:not(="false")`
  // 作为 fallback——jsdom 部分实现 isContentEditable 不返回 true，靠选择器在
  // 测试环境兜底，同时也能正确放行显式 `="false"` 关闭可编辑的元素。
  if (target.isContentEditable) return;
  if (
    target.closest(
      'input, textarea, [contenteditable]:not([contenteditable="false"]), [data-allow-native-context]',
    )
  ) {
    return;
  }
  e.preventDefault();
}

/**
 * 在 main.ts 启动序列内调用一次，注册全局 contextmenu 兜底。
 * 幂等：HMR 重复调用不重复注册（避免 listener 叠加）。
 *
 * 用 window 全局 sentinel 而非模块级 flag——vite HMR 模块重载时模块级
 * `let` 会归零；window 全局在 HMR 间保持，旧 handler 因模块卸载变野指针
 * 但事件监听仍在（无害——activeInstance 也在 window 不到的模块作用域，
 * 不会被新 handler 触发），新 handler 跳过注册避免叠加。
 */
export function installGlobalContextMenuFallback(): void {
  if (window.__cdtContextMenuFallbackInstalled) return;
  window.__cdtContextMenuFallbackInstalled = true;
  // bubble 阶段，让元素自身 listener 先有机会 preventDefault；
  // 兜底仅在 e.defaultPrevented === false 时执行 preventDefault。
  window.addEventListener("contextmenu", globalContextMenuHandler, false);
}

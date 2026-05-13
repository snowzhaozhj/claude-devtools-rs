// 全局拦截 markdown 渲染产出的 `<a href>` 点击：把 http/https/mailto 链接交给
// 系统默认浏览器（走 tauri-plugin-opener），避免 webview 在窗口内导航且无回退。
//
// 设计取舍：Tauri webview 不支持多标签页，Cmd/Ctrl+click 与中键 click 走 webview
// 默认行为同样陷入窗口内导航且无回退键 —— 所以一律拦截，统一交给系统浏览器。
// 右键（button >= 2）不触发 click event（触发 contextmenu），此处放行属保险。
//
// mockIPC 浏览器调试：tauriMock 已 stub `plugin:opener|open_url` 走
// `window.open`，所以 mock 环境下也无需特判 —— 走同一条路径。

import { openUrl } from "@tauri-apps/plugin-opener";

// 用原始 href 字符串前缀判断，而非 `new URL(href, location)` 解析后 protocol——
// 后者会把页内 `#anchor` / 相对路径 `./foo` 也归到 `http:`，导致误拦。
const EXTERNAL_PREFIX_RE = /^(https?:\/\/|mailto:)/i;

function openExternal(url: string): void {
  void openUrl(url).catch((err) => {
    console.error("[externalLinks] openUrl failed:", err);
  });
}

function onDocumentClick(e: MouseEvent): void {
  if (e.defaultPrevented) return;
  if (e.button >= 2) return;

  const target = e.target;
  if (!(target instanceof Element)) return;
  const anchor = target.closest("a[href]") as HTMLAnchorElement | null;
  if (!anchor) return;
  const href = anchor.getAttribute("href");
  if (!href) return;
  if (!EXTERNAL_PREFIX_RE.test(href)) return;

  e.preventDefault();
  openExternal(href);
}

/**
 * 注册全局 capture-phase click 监听，拦截外部链接到系统浏览器。
 * 返回 cleanup 函数。
 */
export function attachExternalLinkInterceptor(): () => void {
  document.addEventListener("click", onDocumentClick, true);
  return () => document.removeEventListener("click", onDocumentClick, true);
}

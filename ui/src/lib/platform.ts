/**
 * 平台检测 helper（前端，运行时检测）。
 *
 * 用 `navigator.userAgentData.platform`（Chromium 90+）优先，fallback `navigator.platform`
 * （deprecated 但 Tauri WKWebView / 老 WebView2 仍可读）。SSR / Node 环境兜底返回 false。
 */

let cached: boolean | null = null;

export function isMac(): boolean {
  if (cached !== null) return cached;
  if (typeof navigator === "undefined") {
    cached = false;
    return cached;
  }
  // 优先 userAgentData（Chromium）；userAgentData.platform 是同步可读的 brand info
  const uaData = (navigator as { userAgentData?: { platform?: string } }).userAgentData;
  if (uaData?.platform) {
    cached = /mac/i.test(uaData.platform);
    return cached;
  }
  // Fallback：navigator.platform 在 macOS 上返回 "MacIntel" / "MacPPC" / "Mac68K"
  // Tauri WKWebView 上仍可用；Safari / 旧 Chrome 也仍 expose
  cached = /mac/i.test(navigator.platform || "") || /mac/i.test(navigator.userAgent || "");
  return cached;
}

/**
 * 测试 hook：重置 cache，让下次 isMac() 重新检测。
 * 仅用于 vitest / playwright spec 注入 navigator 平台后强制重读。
 */
export function _resetPlatformCache(): void {
  cached = null;
}

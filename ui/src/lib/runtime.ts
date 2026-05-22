export function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/**
 * 浏览器 `?http=1` 模式下 BrowserTransport / SSE 用的 API base URL。
 *
 * 优先级：
 * 1. URL `?apiBase=<encoded origin>` —— Tauri 内置 HTTP server dev redirect 时
 *    把当前 server origin（如 `http://localhost:4000`）编进 query，让前端在
 *    vite domain 仍能定位真实 API server，不被 vite proxy 写死的 `:3456`
 *    target 错连（用户自定 server-mode 端口场景）。
 * 2. `window.location.origin` —— 默认场景（直接访问 server 而非经 redirect）
 *    或 server 跑在与 UI 同 origin（如 cdt-cli + ServeDir）时走同源。
 *
 * 安全：apiBase 必须是 `http://` 或 `https://` + localhost / 127.0.0.1 主机，
 * 否则忽略——避免 redirect 被注入指向第三方 origin。
 */
export function getServerBaseUrl(): string {
  if (typeof window === "undefined") return "";
  const apiBase = readAndValidateApiBase(window.location.search);
  if (apiBase) return apiBase;
  return window.location.origin;
}

/**
 * 从 query string 提 `apiBase` 并校验，通过返回 normalize 后的 origin（不含
 * path / query / fragment），防止类似 `?apiBase=http://localhost:3456/evil?x=1`
 * 注入污染 `${base}/api/...` 的拼接。失败返 `null`，调用方走 fallback。
 *
 * exported for vitest 单测覆盖（合法 / 注入 / IPv6 / 非 localhost / 非 http）。
 */
export function readAndValidateApiBase(search: string): string | null {
  try {
    const params = new URLSearchParams(search);
    const raw = params.get("apiBase");
    if (!raw) return null;
    const url = new URL(raw);
    if (url.protocol !== "http:" && url.protocol !== "https:") return null;
    if (url.hostname !== "localhost" && url.hostname !== "127.0.0.1") return null;
    return url.origin;
  } catch {
    return null;
  }
}

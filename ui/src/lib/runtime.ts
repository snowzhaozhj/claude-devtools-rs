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
  try {
    const params = new URLSearchParams(window.location.search);
    const apiBase = params.get("apiBase");
    if (apiBase && isAllowedApiBase(apiBase)) {
      return apiBase.replace(/\/$/, "");
    }
  } catch {
    // URLSearchParams 不会抛；防御性 catch 万一以后 location 类型变化
  }
  return window.location.origin;
}

function isAllowedApiBase(value: string): boolean {
  let url: URL;
  try {
    url = new URL(value);
  } catch {
    return false;
  }
  if (url.protocol !== "http:" && url.protocol !== "https:") return false;
  return url.hostname === "localhost" || url.hostname === "127.0.0.1";
}

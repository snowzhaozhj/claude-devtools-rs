/**
 * 提取 IPC / 异常的人类可读 message，兼容三种形态：
 * - `ApiError` shape `{ code, message }`（桌面 Tauri runtime 的 IPC 拒绝形态——
 *   直接 `String(e)` 会得到 `"[object Object]"`，丢失失败原因）
 * - `Error` 实例
 * - 字符串 / 其它 unknown
 *
 * 任何把错误**渲染给用户**的路径都 SHALL 用这个而非 `String(e)`，否则桌面端
 * 错误提示会退化成 `[object Object]`（HTTP 浏览器模式恰好抛真 `Error` 不受影响）。
 */
export function errorMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e && typeof e === "object") {
    const obj = e as Record<string, unknown>;
    // ApiError shape: { code, message }
    if (typeof obj.message === "string" && obj.message.length > 0) {
      return obj.message;
    }
    if (typeof obj.toString === "function") {
      const s = obj.toString();
      if (s !== "[object Object]") return s;
    }
  }
  return String(e);
}

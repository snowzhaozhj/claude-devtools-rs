/*
 * Deeplink hash route（Task 6 / design.md::D9）。
 *
 * spec: openspec/specs/session-display/spec.md
 *   ::Requirement 消息 chunk DOM 锚点 `data-chunk-id`
 *
 * 格式：`#/session/<sessionId>/chunk/<chunkId>`
 *
 * 设计取舍（design.md::D9）：
 * - **不**引入 svelte-spa-router 库（仅一个 deeplink 功能 overkill；本仓
 *   tabStore 管 page state 无 route → component 映射）
 * - **不**用 URL search params（dev 模式 ?mock=1&fixture=... 已占用）
 * - hash route 不影响 Tauri WKWebView navigate 行为（无页面重载）
 *
 * 跨 surface 流程：
 * 1. 用户在某 chunk 右键 → "复制 Deeplink" → clipboard 写入完整 URL
 * 2. 粘贴到 App 或 dev 浏览器 URL 栏访问 → hashchange 触发
 * 3. installDeeplinkWatcher 检测 → onNavigate(target)
 * 4. onNavigate 调 openSessionTab + 设 tabStore.pendingScrollChunkId
 * 5. SessionDetail mount + chunks 加载完毕后消费 pendingScrollChunkId
 *
 * 生命周期（spec session-display "pendingScrollChunkId 绑定 tab lifecycle"）：
 * - 用户始终未激活 tab 时 SHALL **不**超时清除
 * - tab 关闭时随 tabUIState 一起清
 * - chunk 不存在时弹 toast + clear（避免后续重试）
 */

export interface DeeplinkTarget {
  sessionId: string;
  chunkId: string;
}

/**
 * 解析 hash 字符串为 DeeplinkTarget。
 * @param hash 含 `#` 前缀的 hash 字符串；不传时取 `window.location.hash`
 * @returns 匹配则返回 target，否则返回 null
 */
export function parseDeeplink(hash?: string): DeeplinkTarget | null {
  const raw = hash ?? (typeof window !== "undefined" ? window.location.hash : "");
  if (!raw) return null;
  // 去前导 # （hashchange event 给的 location.hash 含 #；外部 build 函数返回值也含 #）
  const stripped = raw.startsWith("#") ? raw.slice(1) : raw;
  // 期望：/session/<sessionId>/chunk/<chunkId>
  const match = /^\/session\/([^/]+)\/chunk\/(.+)$/.exec(stripped);
  if (!match) return null;
  const sessionId = decodeURIComponent(match[1]);
  const chunkId = decodeURIComponent(match[2]);
  if (!sessionId || !chunkId) return null;
  return { sessionId, chunkId };
}

/**
 * 生成 deeplink hash 字符串（含 `#` 前缀，可直接拼到 URL）。
 *
 * sessionId / chunkId 都过 encodeURIComponent——chunkId 形如 `<uuid>:0`
 * 可能含 `:`，必须 encode 才能和 URL fragment 安全互操作。
 */
export function buildDeeplinkHash(sessionId: string, chunkId: string): string {
  return `#/session/${encodeURIComponent(sessionId)}/chunk/${encodeURIComponent(chunkId)}`;
}

declare global {
  interface Window {
    __cdtDeeplinkWatcherInstalled?: boolean;
  }
}

/**
 * 注册 hashchange 监听 + 启动时检查当前 hash。匹配时调 callback。
 *
 * HMR 幂等：window sentinel + import.meta.hot.dispose 双保险（与 Phase 1
 * `installGlobalContextMenuFallback` 同策略）。
 *
 * @param onNavigate 解析到 deeplink 时的回调
 * @returns cleanup 函数，调用后注销监听
 */
export function installDeeplinkWatcher(
  onNavigate: (target: DeeplinkTarget) => void,
): () => void {
  if (typeof window === "undefined") return () => {};

  if (window.__cdtDeeplinkWatcherInstalled) {
    // 已注册过——返回 noop cleanup，避免幂等调用重复 install
    return () => {};
  }
  window.__cdtDeeplinkWatcherInstalled = true;

  const handler = () => {
    const target = parseDeeplink();
    if (target) onNavigate(target);
  };

  window.addEventListener("hashchange", handler);

  // 启动时检查当前 hash（覆盖"应用启动后用户从外部 URL 直接打开"场景）
  // 异步派发避免在 caller 还没写完 onNavigate 闭包前就触发
  queueMicrotask(handler);

  return () => {
    window.removeEventListener("hashchange", handler);
    delete window.__cdtDeeplinkWatcherInstalled;
  };
}

if (typeof import.meta !== "undefined" && import.meta.hot) {
  import.meta.hot.dispose(() => {
    delete window.__cdtDeeplinkWatcherInstalled;
  });
}

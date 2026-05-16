import { getCurrentWindow } from "@tauri-apps/api/window";
import { getNotifications } from "./api";
import { setUnreadCount } from "./tabStore.svelte";

let inflightUnreadRefresh: Promise<number> | null = null;

export function refreshUnreadCount(): Promise<number> {
  if (inflightUnreadRefresh) return inflightUnreadRefresh;

  let request: Promise<number>;
  request = (async () => {
    try {
      const result = await getNotifications(1, 0);
      setUnreadCount(result.unreadCount);
      try {
        await getCurrentWindow().setBadgeCount(result.unreadCount > 0 ? result.unreadCount : undefined);
      } catch {
        // 非 macOS 平台静默
      }
      return result.unreadCount;
    } finally {
      inflightUnreadRefresh = null;
    }
  })();

  inflightUnreadRefresh = request;
  return request;
}

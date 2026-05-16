import { getCurrentWindow } from "@tauri-apps/api/window";
import { getNotifications } from "./api";
import { setUnreadCount } from "./tabStore.svelte";

let inflightUnreadRefresh: Promise<number> | null = null;
let refreshAfterInflight = false;

async function fetchUnreadCount(): Promise<number> {
  const result = await getNotifications(1, 0);
  setUnreadCount(result.unreadCount);
  try {
    await getCurrentWindow().setBadgeCount(result.unreadCount > 0 ? result.unreadCount : undefined);
  } catch {
    // 非 macOS 平台静默
  }
  return result.unreadCount;
}

export function refreshUnreadCount(): Promise<number> {
  if (inflightUnreadRefresh) {
    refreshAfterInflight = true;
    return inflightUnreadRefresh;
  }

  inflightUnreadRefresh = (async () => {
    try {
      return await fetchUnreadCount();
    } finally {
      inflightUnreadRefresh = null;
      if (refreshAfterInflight) {
        refreshAfterInflight = false;
        void refreshUnreadCount();
      }
    }
  })();

  return inflightUnreadRefresh;
}

<script lang="ts">
  import { onMount } from "svelte";
  import { getNotifications, markNotificationRead, type StoredNotification, type GetNotificationsResult } from "../lib/api";
  import { openTab, setUnreadCount } from "../lib/tabStore.svelte";

  let notifications: StoredNotification[] = $state([]);
  let loading = $state(true);
  let error: string | null = $state(null);

  onMount(async () => {
    try {
      const result: GetNotificationsResult = await getNotifications(100, 0);
      notifications = result.notifications;
      setUnreadCount(result.unreadCount);
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  });

  async function handleMarkRead(e: MouseEvent, notif: StoredNotification) {
    e.stopPropagation();
    try {
      await markNotificationRead(notif.id);
      notifications = notifications.map((n) =>
        n.id === notif.id ? { ...n, isRead: true } : n
      );
      setUnreadCount(notifications.filter((n) => !n.isRead).length);
    } catch (err) {
      console.error("Failed to mark notification read:", err);
    }
  }

  function handleNavigate(notif: StoredNotification) {
    openTab(notif.sessionId, notif.projectId, notif.message.slice(0, 50) || notif.sessionId.slice(0, 12));
    if (!notif.isRead) {
      markNotificationRead(notif.id).then(() => {
        notifications = notifications.map((n) =>
          n.id === notif.id ? { ...n, isRead: true } : n
        );
        setUnreadCount(notifications.filter((n) => !n.isRead).length);
      });
    }
  }

  function formatTime(ts: number): string {
    if (!ts) return "";
    const d = new Date(ts);
    const now = new Date();
    const diffMs = now.getTime() - d.getTime();
    const diffMins = Math.floor(diffMs / 60000);
    if (diffMins < 1) return "刚刚";
    if (diffMins < 60) return `${diffMins}m`;
    const diffHours = Math.floor(diffMins / 60);
    if (diffHours < 24) return `${diffHours}h`;
    const diffDays = Math.floor(diffHours / 24);
    if (diffDays < 7) return `${diffDays}d`;
    return d.toLocaleDateString("zh-CN", { month: "short", day: "numeric" });
  }
</script>

<div class="notifications-view">
  <div class="notifications-header">
    <h2 class="notifications-title">通知</h2>
  </div>

  <div class="notifications-body">
    {#if loading}
      <div class="state-msg">加载中...</div>
    {:else if error}
      <div class="state-msg state-err">{error}</div>
    {:else if notifications.length === 0}
      <div class="empty-state">
        <div class="empty-icon">🔔</div>
        <div class="empty-title">暂无通知</div>
        <div class="empty-desc">
          通知由触发器规则自动生成。当 Claude Code 会话中出现工具执行错误、
          匹配关键词或 token 超限时，对应的触发器会产生通知。
        </div>
        <div class="empty-hint">
          在 <strong>设置 → 通知</strong> 中查看和管理触发器规则。
        </div>
      </div>
    {:else}
      <div class="notification-list">
        {#each notifications as notif (notif.id)}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="notification-row"
            class:notification-unread={!notif.isRead}
            onclick={() => handleNavigate(notif)}
          >
            <span
              class="notif-color"
              style:background={notif.triggerColor || "var(--color-text-muted)"}
            ></span>
            <div class="notif-content">
              {#if notif.triggerName}
                <span class="notif-trigger">{notif.triggerName}</span>
              {/if}
              <span class="notif-message">
                {notif.message.length > 100 ? notif.message.slice(0, 100) + "…" : notif.message}
              </span>
            </div>
            <span class="notif-time">{formatTime(notif.createdAt)}</span>
            {#if !notif.isRead}
              <button class="notif-mark-btn" onclick={(e) => handleMarkRead(e, notif)} title="标记已读">✓</button>
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  </div>
</div>

<style>
  .notifications-view {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }

  .notifications-header {
    padding: 16px 24px;
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .notifications-title {
    font-size: 16px;
    font-weight: 600;
    color: var(--color-text);
    margin: 0;
  }

  .notifications-body {
    flex: 1;
    overflow-y: auto;
    padding: 8px;
  }

  .state-msg {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 200px;
    color: var(--color-text-muted);
    font-size: 14px;
  }
  .state-err { color: var(--tool-result-error-text); }

  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    gap: 8px;
    padding: 48px 24px;
    text-align: center;
  }

  .empty-icon {
    font-size: 36px;
    opacity: 0.4;
  }

  .empty-title {
    font-size: 15px;
    font-weight: 600;
    color: var(--color-text);
  }

  .empty-desc {
    font-size: 13px;
    color: var(--color-text-muted);
    max-width: 400px;
    line-height: 1.5;
  }

  .empty-hint {
    font-size: 12px;
    color: var(--color-text-muted);
    margin-top: 8px;
  }

  .empty-hint :global(strong) {
    color: var(--color-text-secondary);
  }

  .notification-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .notification-row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
    border-radius: 6px;
    cursor: pointer;
    transition: background 0.1s;
  }

  .notification-row:hover {
    background: var(--tool-item-hover-bg);
  }

  .notification-unread {
    background: var(--color-surface-raised, var(--color-surface));
  }

  .notif-color {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .notif-content {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .notif-trigger {
    font-size: 11px;
    font-weight: 600;
    color: var(--color-text-secondary);
  }

  .notif-message {
    font-size: 13px;
    color: var(--color-text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .notif-time {
    font-size: 11px;
    color: var(--color-text-muted);
    flex-shrink: 0;
  }

  .notif-mark-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: transparent;
    color: var(--color-text-muted);
    font-size: 12px;
    cursor: pointer;
    flex-shrink: 0;
    transition: background 0.1s, color 0.1s;
  }

  .notif-mark-btn:hover {
    background: var(--color-border);
    color: var(--color-text);
  }
</style>

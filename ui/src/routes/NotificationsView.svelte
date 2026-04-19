<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import {
    getNotifications,
    markNotificationRead,
    deleteNotification,
    markAllNotificationsRead,
    clearNotifications,
    type StoredNotification,
    type GetNotificationsResult,
  } from "../lib/api";
  import { openTab, setUnreadCount } from "../lib/tabStore.svelte";
  import { CHECK_CHECK_SVG, CHECK_SVG, TRASH2_SVG, X_SVG } from "../lib/icons";

  let notifications: StoredNotification[] = $state([]);
  let loading = $state(true);
  let error: string | null = $state(null);
  let actionError: string | null = $state(null);
  let clearPending = $state(false);
  let clearTimer: ReturnType<typeof setTimeout> | null = null;
  let unlistenAdded: UnlistenFn | null = null;
  let unlistenUpdate: UnlistenFn | null = null;

  async function reload() {
    try {
      const result: GetNotificationsResult = await getNotifications(100, 0);
      notifications = result.notifications;
      setUnreadCount(result.unreadCount);
      error = null;
    } catch (e) {
      error = String(e);
    }
  }

  onMount(async () => {
    await reload();
    loading = false;
    unlistenAdded = await listen("notification-added", () => { void reload(); });
    unlistenUpdate = await listen("notification-update", () => { void reload(); });
  });

  onDestroy(() => {
    unlistenAdded?.();
    unlistenUpdate?.();
    if (clearTimer) {
      clearTimeout(clearTimer);
      clearTimer = null;
    }
  });

  const unreadCount = $derived(notifications.filter((n) => !n.isRead).length);

  async function handleNavigate(notif: StoredNotification) {
    openTab(notif.sessionId, notif.projectId, notif.message.slice(0, 50) || notif.sessionId.slice(0, 12));
    if (!notif.isRead) {
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
  }

  async function handleMarkOneRead(e: MouseEvent, notif: StoredNotification) {
    e.stopPropagation();
    actionError = null;
    try {
      await markNotificationRead(notif.id);
      notifications = notifications.map((n) =>
        n.id === notif.id ? { ...n, isRead: true } : n
      );
      setUnreadCount(notifications.filter((n) => !n.isRead).length);
    } catch (err) {
      actionError = `标记失败: ${err}`;
    }
  }

  async function handleDelete(e: MouseEvent, notif: StoredNotification) {
    e.stopPropagation();
    actionError = null;
    try {
      await deleteNotification(notif.id);
      await reload();
    } catch (err) {
      actionError = `删除失败: ${err}`;
    }
  }

  async function handleMarkAllRead() {
    actionError = null;
    try {
      await markAllNotificationsRead();
      await reload();
    } catch (err) {
      actionError = `标记失败: ${err}`;
    }
  }

  async function handleClearAll() {
    if (!clearPending) {
      clearPending = true;
      if (clearTimer) clearTimeout(clearTimer);
      clearTimer = setTimeout(() => {
        clearPending = false;
        clearTimer = null;
      }, 3000);
      return;
    }
    if (clearTimer) {
      clearTimeout(clearTimer);
      clearTimer = null;
    }
    clearPending = false;
    actionError = null;
    try {
      await clearNotifications(undefined);
      await reload();
    } catch (err) {
      actionError = `清空失败: ${err}`;
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
    <div class="header-title-row">
      <h2 class="notifications-title">通知</h2>
      {#if notifications.length > 0}
        <span class="notifications-count">
          {unreadCount > 0 ? `${unreadCount} 条未读` : `共 ${notifications.length} 条`}
        </span>
      {/if}
    </div>

    {#if notifications.length > 0}
      <div class="header-actions">
        <button
          class="header-action-btn"
          onclick={handleMarkAllRead}
          disabled={unreadCount === 0}
          title={unreadCount > 0 ? "全部标记为已读" : "当前无未读通知"}
        >
          <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            {@html CHECK_CHECK_SVG}
          </svg>
          <span>全部已读</span>
        </button>
        <button
          class="header-action-btn"
          class:header-action-danger={clearPending}
          onclick={handleClearAll}
          title={clearPending ? "再次点击确认" : "清空全部通知"}
        >
          <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            {@html TRASH2_SVG}
          </svg>
          <span>{clearPending ? "再次点击确认" : "清空"}</span>
        </button>
      </div>
    {/if}
  </div>

  {#if actionError}
    <div class="action-error">{actionError}</div>
  {/if}

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
              <button
                class="notif-row-btn notif-row-btn-mark"
                onclick={(e) => handleMarkOneRead(e, notif)}
                title="标记为已读"
                aria-label="标记为已读"
              >
                <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  {@html CHECK_SVG}
                </svg>
              </button>
            {/if}
            <button
              class="notif-row-btn notif-row-btn-delete"
              onclick={(e) => handleDelete(e, notif)}
              title="删除此通知"
              aria-label="删除此通知"
            >
              <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                {@html X_SVG}
              </svg>
            </button>
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
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 24px;
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
    gap: 12px;
  }
  .header-title-row {
    display: flex;
    align-items: baseline;
    gap: 10px;
    min-width: 0;
  }
  .notifications-title {
    font-size: 16px;
    font-weight: 600;
    color: var(--color-text);
    margin: 0;
  }
  .notifications-count {
    font-size: 12px;
    color: var(--color-text-muted);
  }
  .header-actions {
    display: flex;
    align-items: center;
    gap: 4px;
    flex-shrink: 0;
  }
  .header-action-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 5px 10px;
    border: 1px solid transparent;
    border-radius: 6px;
    background: transparent;
    color: var(--color-text-muted);
    font: inherit;
    font-size: 12px;
    cursor: pointer;
    transition: background 0.1s, color 0.1s, border-color 0.15s;
  }
  .header-action-btn:hover:not(:disabled) {
    background: var(--tool-item-hover-bg);
    color: var(--color-text);
  }
  .header-action-btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
  .header-action-danger {
    background: rgba(229, 62, 62, 0.18);
    border-color: rgba(229, 62, 62, 0.4);
    color: #e53e3e;
  }
  .header-action-danger:hover {
    background: rgba(229, 62, 62, 0.28);
    color: #e53e3e;
  }

  .action-error {
    padding: 8px 24px;
    background: rgba(229, 62, 62, 0.1);
    color: var(--tool-result-error-text);
    font-size: 13px;
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
    position: relative;
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

  .notif-row-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    padding: 0;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
    flex-shrink: 0;
    opacity: 0;
    transition: background 0.1s, color 0.1s, opacity 0.1s;
  }
  .notification-row:hover .notif-row-btn,
  .notif-row-btn:focus-visible {
    opacity: 1;
  }
  .notif-row-btn-mark:hover {
    background: var(--color-border);
    color: var(--color-text);
  }
  .notif-row-btn-delete:hover {
    background: rgba(229, 62, 62, 0.15);
    color: #e53e3e;
  }
</style>

<script lang="ts">
  import { getTabs, getActiveTabId, setActiveTab, closeTab, openSettingsTab, openNotificationsTab, getUnreadCount } from "../lib/tabStore.svelte";

  const tabs = $derived(getTabs());
  const activeTabId = $derived(getActiveTabId());
  const unreadCount = $derived(getUnreadCount());

  function handleClose(e: MouseEvent, tabId: string) {
    e.stopPropagation();
    closeTab(tabId);
  }

  function tabIcon(type: string): string {
    if (type === "settings") return "⚙";
    if (type === "notifications") return "🔔";
    return "";
  }
</script>

<div class="tab-bar">
  <div class="tab-list">
    {#each tabs as tab (tab.id)}
      <button
        class="tab-item"
        class:tab-item-active={tab.id === activeTabId}
        onclick={() => setActiveTab(tab.id)}
        title={tab.label}
      >
        {#if tab.type !== "session"}
          <span class="tab-icon">{tabIcon(tab.type)}</span>
        {/if}
        <span class="tab-label">{tab.label}</span>
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <span
          class="tab-close"
          role="button"
          tabindex="-1"
          onclick={(e) => handleClose(e, tab.id)}
          onkeydown={(e) => { if (e.key === 'Enter') handleClose(e as unknown as MouseEvent, tab.id); }}
          aria-label="关闭标签"
        >×</span>
      </button>
    {/each}
  </div>

  <div class="tab-actions">
    <button class="tab-action-btn" onclick={() => openNotificationsTab()} title="通知">
      <span class="bell-icon">🔔</span>
      {#if unreadCount > 0}
        <span class="badge">{unreadCount > 99 ? "99+" : unreadCount}</span>
      {/if}
    </button>
    <button class="tab-action-btn" onclick={() => openSettingsTab()} title="设置">
      ⚙
    </button>
  </div>
</div>

<style>
  .tab-bar {
    height: 36px;
    min-height: 36px;
    display: flex;
    align-items: stretch;
    background: var(--color-bg-tertiary, var(--color-surface-sidebar));
    border-bottom: 1px solid var(--color-border);
    overflow: hidden;
  }

  .tab-list {
    display: flex;
    align-items: stretch;
    overflow-x: auto;
    overflow-y: hidden;
    flex: 1;
    scrollbar-width: none;
  }

  .tab-list::-webkit-scrollbar {
    display: none;
  }

  .tab-item {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 12px;
    min-width: 0;
    max-width: 200px;
    border: none;
    border-right: 1px solid var(--color-border);
    background: transparent;
    color: var(--color-text-muted);
    font: inherit;
    font-size: 12px;
    cursor: pointer;
    transition: background 0.1s, color 0.1s;
    flex-shrink: 0;
  }

  .tab-item:hover {
    background: var(--tool-item-hover-bg);
    color: var(--color-text-secondary);
  }

  .tab-item-active {
    background: var(--color-bg-primary, var(--color-surface));
    color: var(--color-text);
    border-bottom: 2px solid var(--color-border-emphasis);
  }

  .tab-icon {
    font-size: 13px;
    flex-shrink: 0;
  }

  .tab-label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
    text-align: left;
  }

  .tab-close {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--color-text-muted);
    font-size: 14px;
    line-height: 1;
    cursor: pointer;
    flex-shrink: 0;
    padding: 0;
    transition: background 0.1s, color 0.1s;
  }

  .tab-close:hover {
    background: var(--color-border);
    color: var(--color-text);
  }

  .tab-actions {
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 0 8px;
    flex-shrink: 0;
    border-left: 1px solid var(--color-border);
  }

  .tab-action-btn {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--color-text-muted);
    font-size: 14px;
    cursor: pointer;
    transition: background 0.1s, color 0.1s;
  }

  .tab-action-btn:hover {
    background: var(--tool-item-hover-bg);
    color: var(--color-text);
  }

  .bell-icon {
    font-size: 14px;
  }

  .badge {
    position: absolute;
    top: 2px;
    right: 0;
    min-width: 16px;
    height: 16px;
    border-radius: 8px;
    background: #e53e3e;
    color: #fff;
    font-size: 10px;
    font-weight: 600;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0 4px;
    line-height: 1;
  }
</style>

<script lang="ts">
  import { getTabs, getActiveTabId, setActiveTab, closeTab, type Tab } from "../lib/tabStore.svelte";

  const tabs = $derived(getTabs());
  const activeTabId = $derived(getActiveTabId());

  function handleClose(e: MouseEvent, tabId: string) {
    e.stopPropagation();
    closeTab(tabId);
  }
</script>

{#if tabs.length > 0}
  <div class="tab-bar">
    <div class="tab-list">
      {#each tabs as tab (tab.id)}
        <button
          class="tab-item"
          class:tab-item-active={tab.id === activeTabId}
          onclick={() => setActiveTab(tab.id)}
          title={tab.label}
        >
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
  </div>
{/if}

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
</style>

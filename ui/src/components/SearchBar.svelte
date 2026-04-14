<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { highlightMatches, clearHighlights, scrollToMatch } from "../lib/searchHighlight";

  interface Props {
    visible: boolean;
    containerEl: HTMLElement | null;
    onClose: () => void;
  }

  let { visible, containerEl, onClose }: Props = $props();

  let inputEl: HTMLInputElement | undefined = $state();
  let query = $state("");
  let totalMatches = $state(0);
  let currentIndex = $state(0);
  let debounceTimer: ReturnType<typeof setTimeout> | undefined;

  function doSearch() {
    if (!containerEl) return;
    clearHighlights(containerEl);
    if (!query) {
      totalMatches = 0;
      currentIndex = 0;
      return;
    }
    totalMatches = highlightMatches(containerEl, query);
    currentIndex = totalMatches > 0 ? 0 : -1;
    if (totalMatches > 0) {
      scrollToMatch(containerEl, 0);
    }
  }

  function onInput() {
    clearTimeout(debounceTimer);
    debounceTimer = setTimeout(doSearch, 300);
  }

  function nextMatch() {
    if (totalMatches === 0 || !containerEl) return;
    currentIndex = (currentIndex + 1) % totalMatches;
    scrollToMatch(containerEl, currentIndex);
  }

  function prevMatch() {
    if (totalMatches === 0 || !containerEl) return;
    currentIndex = (currentIndex - 1 + totalMatches) % totalMatches;
    scrollToMatch(containerEl, currentIndex);
  }

  function close() {
    if (containerEl) clearHighlights(containerEl);
    query = "";
    totalMatches = 0;
    currentIndex = 0;
    onClose();
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      close();
    } else if (e.key === "Enter") {
      e.preventDefault();
      clearTimeout(debounceTimer);
      if (!totalMatches) {
        doSearch();
      }
      if (e.shiftKey) prevMatch(); else nextMatch();
    }
  }

  $effect(() => {
    if (visible && inputEl) {
      inputEl.focus();
      inputEl.select();
    }
  });

  onDestroy(() => {
    clearTimeout(debounceTimer);
  });
</script>

{#if visible}
  <div class="search-bar">
    <input
      bind:this={inputEl}
      class="search-input"
      type="text"
      placeholder="搜索…"
      bind:value={query}
      oninput={onInput}
      onkeydown={onKeydown}
    />

    <span class="search-count">
      {#if totalMatches > 0}
        {currentIndex + 1} / {totalMatches}
      {:else if query}
        无结果
      {/if}
    </span>

    <button class="search-nav-btn" onclick={prevMatch} disabled={totalMatches === 0} title="上一个 (Shift+Enter)">▲</button>
    <button class="search-nav-btn" onclick={nextMatch} disabled={totalMatches === 0} title="下一个 (Enter)">▼</button>
    <button class="search-close-btn" onclick={close} title="关闭 (Esc)">✕</button>
  </div>
{/if}

<style>
  .search-bar {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 16px;
    background: var(--color-surface-raised);
    border-bottom: 1px solid var(--color-border);
  }

  .search-input {
    flex: 1;
    min-width: 0;
    font-size: 13px;
    font-family: inherit;
    color: var(--color-text);
    background: var(--color-surface);
    border: 1px solid var(--color-border);
    border-radius: 6px;
    padding: 4px 10px;
    outline: none;
    transition: border-color 0.15s;
  }

  .search-input:focus {
    border-color: var(--color-border-emphasis);
  }

  .search-input::placeholder {
    color: var(--color-text-muted);
  }

  .search-count {
    font-size: 12px;
    color: var(--color-text-muted);
    font-family: var(--font-mono);
    flex-shrink: 0;
    min-width: 56px;
    text-align: center;
  }

  .search-nav-btn,
  .search-close-btn {
    background: none;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    color: var(--color-text-secondary);
    font-size: 10px;
    width: 24px;
    height: 24px;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    flex-shrink: 0;
    transition: background 0.1s, color 0.1s;
  }

  .search-nav-btn:hover,
  .search-close-btn:hover {
    background: var(--tool-item-hover-bg);
    color: var(--color-text);
  }

  .search-nav-btn:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .search-close-btn {
    font-size: 12px;
  }
</style>

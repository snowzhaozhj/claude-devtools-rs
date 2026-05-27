<script lang="ts">
  import { onDestroy } from "svelte";
  import { highlightMatches, clearHighlights, scrollToMatch, type VirtualMatch } from "../lib/searchHighlight";
  import { ARROW_UP_SVG, ARROW_DOWN_SVG, X_SVG } from "../lib/icons";

  export type { VirtualMatch };

  interface Props {
    visible: boolean;
    containerEl: HTMLElement | null;
    onClose: () => void;
    onBeforeSearch?: (query: string) => void;
    contentVersion?: number;
    focusRequestVersion?: number;
    virtualMatches?: VirtualMatch[];
    onNavigateVirtual?: (match: VirtualMatch) => Promise<void>;
  }

  let {
    visible,
    containerEl,
    onClose,
    onBeforeSearch,
    contentVersion = 0,
    focusRequestVersion = 0,
    virtualMatches = [],
    onNavigateVirtual,
  }: Props = $props();

  let inputEl: HTMLInputElement | undefined = $state();
  let query = $state("");
  let totalMatches = $state(0);
  let domMatches = $state(0);
  let currentIndex = $state(0);
  let debounceTimer: ReturnType<typeof setTimeout> | undefined;
  let navigating = $state(false);

  function doSearch() {
    if (!containerEl) return;
    clearHighlights(containerEl);
    if (!query) {
      totalMatches = 0;
      domMatches = 0;
      currentIndex = 0;
      return;
    }
    onBeforeSearch?.(query);
    domMatches = highlightMatches(containerEl, query);
    totalMatches = domMatches + virtualMatches.length;
    currentIndex = totalMatches > 0 ? 0 : -1;
    if (totalMatches > 0 && domMatches > 0) {
      scrollToMatch(containerEl, 0);
    }
  }

  function onInput() {
    clearTimeout(debounceTimer);
    debounceTimer = setTimeout(doSearch, 300);
  }

  async function nextMatch() {
    if (totalMatches === 0 || !containerEl || navigating) return;
    currentIndex = (currentIndex + 1) % totalMatches;
    await navigateToCurrentIndex();
  }

  async function prevMatch() {
    if (totalMatches === 0 || !containerEl || navigating) return;
    currentIndex = (currentIndex - 1 + totalMatches) % totalMatches;
    await navigateToCurrentIndex();
  }

  async function navigateToCurrentIndex() {
    if (currentIndex < domMatches) {
      scrollToMatch(containerEl!, currentIndex);
    } else {
      const vIdx = currentIndex - domMatches;
      if (vIdx >= 0 && vIdx < virtualMatches.length && onNavigateVirtual) {
        navigating = true;
        try {
          await onNavigateVirtual(virtualMatches[vIdx]);
        } finally {
          navigating = false;
        }
      }
    }
  }

  function close() {
    if (containerEl) clearHighlights(containerEl);
    query = "";
    totalMatches = 0;
    domMatches = 0;
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

  export function triggerResearch() {
    doSearch();
  }

  $effect(() => {
    // 无条件读取 focusRequestVersion 建立依赖——若放进下面 if 内会被
    // `visible` 为 false 时短路，nonce 递增便无法触发 effect 重跑（Svelte 5
    // effect 依赖集是动态的，只追踪上次实际读到的响应式变量）。
    // void 前缀明确"刻意读取以建立依赖"的意图，避免被误判为无用语句。
    void focusRequestVersion;
    if (visible && inputEl) {
      inputEl.focus();
      inputEl.select();
    }
  });

  $effect(() => {
    contentVersion;
    if (visible && query) doSearch();
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
      type="search"
      placeholder="搜索…"
      bind:value={query}
      oninput={onInput}
      onkeydown={onKeydown}
      autocomplete="off"
      autocorrect="off"
      autocapitalize="off"
      spellcheck="false"
      enterkeyhint="search"
      aria-label="会话内搜索"
    />

    <span class="search-count">
      {#if totalMatches > 0}
        {currentIndex + 1} / {totalMatches}
      {:else if query}
        无结果
      {/if}
    </span>

    <button
      class="search-nav-btn"
      onclick={prevMatch}
      disabled={totalMatches === 0}
      aria-label="上一个匹配"
      title="上一个 (Shift+Enter)"
    >
      <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">{@html ARROW_UP_SVG}</svg>
    </button>
    <button
      class="search-nav-btn"
      onclick={nextMatch}
      disabled={totalMatches === 0}
      aria-label="下一个匹配"
      title="下一个 (Enter)"
    >
      <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">{@html ARROW_DOWN_SVG}</svg>
    </button>
    <button
      class="search-close-btn"
      onclick={close}
      aria-label="关闭搜索"
      title="关闭 (Esc)"
    >
      <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">{@html X_SVG}</svg>
    </button>
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

  /* 隐藏 WebKit 原生 clear 按钮，UI 自带关闭按钮。 */
  .search-input::-webkit-search-cancel-button,
  .search-input::-webkit-search-decoration {
    appearance: none;
    -webkit-appearance: none;
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
    width: 24px;
    height: 24px;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    flex-shrink: 0;
    transition: background 0.1s, color 0.1s;
    padding: 0;
  }

  .search-nav-btn svg,
  .search-close-btn svg {
    width: 12px;
    height: 12px;
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
</style>

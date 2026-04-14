<script lang="ts">
  import type { SessionDetail } from "../lib/api";
  import { extractContext, groupByCategory, categoryLabel, type ContextCategory, type ContextEntry } from "../lib/contextExtractor";

  interface Props {
    detail: SessionDetail;
    onClose: () => void;
  }

  let { detail, onClose }: Props = $props();

  let collapsedCategories: Set<ContextCategory> = $state(new Set());

  const entries = $derived(extractContext(detail));
  const grouped = $derived(groupByCategory(entries));
  const totalTokens = $derived(entries.reduce((sum, e) => sum + e.estimatedTokens, 0));

  const categories: ContextCategory[] = ["user", "claudemd", "tools", "system", "thinking"];

  function toggleCategory(cat: ContextCategory) {
    const n = new Set(collapsedCategories);
    if (n.has(cat)) n.delete(cat); else n.add(cat);
    collapsedCategories = n;
  }

  function fk(n: number): string {
    if (n >= 1e6) return (n / 1e6).toFixed(1) + "M";
    if (n >= 1e3) return (n / 1e3).toFixed(1) + "k";
    return String(n);
  }
</script>

<aside class="context-panel">
  <div class="cp-header">
    <span class="cp-title">Context</span>
    <span class="cp-stats">{entries.length} items · ~{fk(totalTokens)} tokens</span>
    <button class="cp-close" onclick={onClose} title="关闭">✕</button>
  </div>

  <div class="cp-body">
    {#each categories as cat}
      {@const catEntries = grouped.get(cat)}
      {#if catEntries && catEntries.length > 0}
        {@const collapsed = collapsedCategories.has(cat)}
        <div class="cp-section">
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div class="cp-section-header" onclick={() => toggleCategory(cat)}>
            <span class="cp-chevron" class:cp-chevron-open={!collapsed}>▸</span>
            <span class="cp-section-label">{categoryLabel(cat)}</span>
            <span class="cp-section-count">{catEntries.length}</span>
          </div>

          {#if !collapsed}
            <div class="cp-section-items">
              {#each catEntries as entry, i}
                <div class="cp-item">
                  <div class="cp-item-row">
                    <span class="cp-item-label">{entry.label}</span>
                    <span class="cp-item-tokens">~{fk(entry.estimatedTokens)}</span>
                  </div>
                  <span class="cp-item-preview">{entry.preview}</span>
                </div>
              {/each}
            </div>
          {/if}
        </div>
      {/if}
    {/each}
  </div>
</aside>

<style>
  .context-panel {
    width: 320px;
    min-width: 320px;
    height: 100%;
    display: flex;
    flex-direction: column;
    border-left: 1px solid var(--color-border);
    background: var(--color-surface-sidebar);
    overflow: hidden;
  }

  .cp-header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 14px;
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .cp-title {
    font-size: 13px;
    font-weight: 600;
    color: var(--color-text);
  }

  .cp-stats {
    flex: 1;
    font-size: 11px;
    color: var(--color-text-muted);
    font-family: var(--font-mono);
  }

  .cp-close {
    background: none;
    border: none;
    color: var(--color-text-muted);
    font-size: 14px;
    cursor: pointer;
    padding: 2px 4px;
    border-radius: 4px;
    transition: background 0.1s, color 0.1s;
  }

  .cp-close:hover {
    background: var(--tool-item-hover-bg);
    color: var(--color-text);
  }

  .cp-body {
    flex: 1;
    overflow-y: auto;
    padding: 8px 0;
  }

  .cp-section {
    margin-bottom: 4px;
  }

  .cp-section-header {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 14px;
    cursor: pointer;
    transition: background 0.1s;
  }

  .cp-section-header:hover {
    background: var(--tool-item-hover-bg);
  }

  .cp-chevron {
    font-size: 10px;
    color: var(--color-text-muted);
    width: 12px;
    transition: transform 0.15s ease;
  }

  .cp-chevron-open {
    transform: rotate(90deg);
  }

  .cp-section-label {
    font-size: 11px;
    font-weight: 600;
    color: var(--color-text-muted);
    letter-spacing: 0.3px;
    text-transform: uppercase;
  }

  .cp-section-count {
    font-size: 10px;
    color: var(--color-text-muted);
    background: var(--badge-neutral-bg);
    padding: 0 5px;
    border-radius: 8px;
  }

  .cp-section-items {
    padding: 0 8px;
  }

  .cp-item {
    padding: 6px 10px;
    border-radius: 6px;
    cursor: pointer;
    transition: background 0.1s;
  }

  .cp-item:hover {
    background: var(--tool-item-hover-bg);
  }

  .cp-item-row {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .cp-item-label {
    flex: 1;
    font-size: 12px;
    font-weight: 500;
    color: var(--color-text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }

  .cp-item-tokens {
    font-size: 10px;
    color: var(--color-text-muted);
    font-family: var(--font-mono);
    flex-shrink: 0;
  }

  .cp-item-preview {
    display: block;
    font-size: 11px;
    color: var(--color-text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    margin-top: 2px;
  }

</style>

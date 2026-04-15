<script lang="ts">
  import type { SessionDetail } from "../lib/api";
  import { extractContext, groupByCategory, categoryLabel, CATEGORY_COLORS, type ContextCategory, type ContextEntry } from "../lib/contextExtractor";
  import DirectoryTree from "./DirectoryTree.svelte";

  interface Props {
    detail: SessionDetail;
    onClose: () => void;
  }

  let { detail, onClose }: Props = $props();

  type ViewMode = "category" | "ranked";
  let viewMode: ViewMode = $state("category");
  let collapsedCategories: Set<ContextCategory> = $state(new Set());

  const entries = $derived(extractContext(detail));
  const grouped = $derived(groupByCategory(entries));
  const totalTokens = $derived(entries.reduce((sum, e) => sum + e.estimatedTokens, 0));
  const rankedEntries = $derived([...entries].sort((a, b) => b.estimatedTokens - a.estimatedTokens));

  // claude-md 条目单独提取用于 DirectoryTree
  const claudeMdEntries = $derived(
    entries.filter(e => e.categoryKey === "claude-md")
  );
  const mentionedFileEntries = $derived(
    entries.filter(e => e.categoryKey === "mentioned-file")
  );

  const categories: ContextCategory[] = ["user", "claudemd", "tools", "system", "thinking", "task"];

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
  <!-- Header -->
  <div class="cp-header">
    <div class="cp-header-top">
      <span class="cp-title">Context</span>
      <span class="cp-stats">{entries.length} items · ~{fk(totalTokens)} tokens</span>
      <button class="cp-close" onclick={onClose} title="关闭">✕</button>
    </div>
    <div class="cp-mode-bar">
      <button
        class="cp-mode-btn"
        class:cp-mode-active={viewMode === "category"}
        onclick={() => viewMode = "category"}
      >Category</button>
      <button
        class="cp-mode-btn"
        class:cp-mode-active={viewMode === "ranked"}
        onclick={() => viewMode = "ranked"}
      >Ranked</button>
    </div>
  </div>

  <div class="cp-body">
    {#if viewMode === "category"}
      <!-- Category 视图 -->
      {#each categories as cat}
        {@const catEntries = grouped.get(cat)}
        {#if catEntries && catEntries.length > 0}
          {@const isCollapsed = collapsedCategories.has(cat)}
          <div class="cp-section">
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div class="cp-section-header" onclick={() => toggleCategory(cat)}>
              <span class="cp-chevron" class:cp-chevron-open={!isCollapsed}>▸</span>
              <span class="cp-section-label">{categoryLabel(cat)}</span>
              <span class="cp-section-count">{catEntries.length}</span>
              <span class="cp-section-tokens">~{fk(catEntries.reduce((s, e) => s + e.estimatedTokens, 0))}</span>
            </div>

            {#if !isCollapsed}
              <div class="cp-section-items">
                {#if cat === "claudemd" && claudeMdEntries.length > 0}
                  <!-- CLAUDE.md 用 DirectoryTree -->
                  <DirectoryTree entries={claudeMdEntries} />
                  {#if mentionedFileEntries.length > 0}
                    <div class="cp-sub-label">Mentioned Files</div>
                    {#each mentionedFileEntries as entry}
                      <div class="cp-item">
                        <div class="cp-item-row">
                          <span class="cp-item-label">{entry.label}</span>
                          <span class="cp-item-tokens">~{fk(entry.estimatedTokens)}</span>
                        </div>
                        <span class="cp-item-preview">{entry.preview}</span>
                      </div>
                    {/each}
                  {/if}
                {:else}
                  {#each catEntries as entry}
                    <div class="cp-item">
                      <div class="cp-item-row">
                        <span class="cp-item-label">{entry.label}</span>
                        <span class="cp-item-tokens">~{fk(entry.estimatedTokens)}</span>
                      </div>
                      <span class="cp-item-preview">{entry.preview}</span>
                    </div>
                  {/each}
                {/if}
              </div>
            {/if}
          </div>
        {/if}
      {/each}

    {:else}
      <!-- Ranked 视图 -->
      <div class="cp-ranked-list">
        {#each rankedEntries as entry}
          {@const color = CATEGORY_COLORS[entry.categoryKey]}
          <div class="cp-ranked-item">
            {#if color}
              <span class="cp-cat-tag" style:background={color.bg} style:color={color.text}>
                {color.label}
              </span>
            {/if}
            <span class="cp-ranked-label">{entry.label}</span>
            <span class="cp-ranked-tokens">~{fk(entry.estimatedTokens)}</span>
          </div>
        {/each}
      </div>
    {/if}
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
    flex-shrink: 0;
    border-bottom: 1px solid var(--color-border);
  }

  .cp-header-top {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 14px 6px;
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

  .cp-mode-bar {
    display: flex;
    gap: 4px;
    padding: 4px 14px 8px;
  }

  .cp-mode-btn {
    font-size: 11px;
    font-family: inherit;
    color: var(--color-text-muted);
    background: none;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    padding: 2px 10px;
    cursor: pointer;
    transition: background 0.1s, color 0.1s, border-color 0.1s;
  }

  .cp-mode-btn:hover {
    background: var(--tool-item-hover-bg);
  }

  .cp-mode-active {
    background: var(--color-surface-raised);
    color: var(--color-text);
    border-color: var(--color-border-emphasis);
  }

  .cp-body {
    flex: 1;
    overflow-y: auto;
    padding: 8px 0;
  }

  /* ── Category 视图 ── */

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

  .cp-section-tokens {
    font-size: 10px;
    color: var(--color-text-muted);
    font-family: var(--font-mono);
    margin-left: auto;
  }

  .cp-section-items {
    padding: 0 8px 0 14px;
  }

  .cp-sub-label {
    font-size: 10px;
    font-weight: 600;
    color: var(--color-text-muted);
    padding: 8px 0 2px;
    letter-spacing: 0.2px;
  }

  .cp-item {
    padding: 6px 10px;
    border-radius: 6px;
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

  /* ── Ranked 视图 ── */

  .cp-ranked-list {
    padding: 0 8px;
  }

  .cp-ranked-item {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 8px;
    border-radius: 4px;
    transition: background 0.1s;
  }

  .cp-ranked-item:hover {
    background: var(--tool-item-hover-bg);
  }

  .cp-cat-tag {
    font-size: 9px;
    font-weight: 600;
    padding: 1px 6px;
    border-radius: 3px;
    flex-shrink: 0;
    letter-spacing: 0.2px;
  }

  .cp-ranked-label {
    flex: 1;
    font-size: 12px;
    color: var(--color-text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }

  .cp-ranked-tokens {
    font-size: 10px;
    color: var(--color-text-muted);
    font-family: var(--font-mono);
    flex-shrink: 0;
  }
</style>

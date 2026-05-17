<script lang="ts">
  import { groupByCategory, categoryLabel, CATEGORY_COLORS, type ContextCategory, type ContextEntry } from "../lib/contextExtractor";
  import DirectoryTree from "./DirectoryTree.svelte";

  interface Props {
    entries: ContextEntry[];
    onClose: () => void;
  }

  let { entries, onClose }: Props = $props();

  type ViewMode = "category" | "ranked";
  let viewMode: ViewMode = $state("category");
  let collapsedCategories: Set<ContextCategory> = $state(new Set());

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

  function categoryTokens(items: ContextEntry[]): number {
    return items.reduce((sum, e) => sum + e.estimatedTokens, 0);
  }
</script>

<aside class="context-panel">
  <!-- Header -->
  <div class="cp-header">
    <div class="cp-title-row">
      <div class="cp-title-wrap">
        <svg class="cp-title-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z" />
          <path d="M14 2v4a2 2 0 0 0 2 2h4" />
          <path d="M10 9H8" />
          <path d="M16 13H8" />
          <path d="M16 17H8" />
        </svg>
        <span class="cp-title">Visible Context</span>
        <span class="cp-count-badge">{entries.length}</span>
      </div>
      <button class="cp-close" onclick={onClose} title="关闭" aria-label="关闭 Context 面板">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <path d="M18 6 6 18" />
          <path d="m6 6 12 12" />
        </svg>
      </button>
    </div>
    <div class="cp-token-row">
      <span class="cp-token-muted">Visible:</span>
      <span class="cp-token-value">~{fk(totalTokens)}</span>
    </div>
    <div class="cp-mode-row">
      <span class="cp-mode-label">View:</span>
      <button
        class="cp-mode-btn"
        class:cp-mode-active={viewMode === "category"}
        onclick={() => viewMode = "category"}
      >
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M3 6h18" /><path d="M3 12h18" /><path d="M3 18h18" /></svg>
        Category
      </button>
      <button
        class="cp-mode-btn"
        class:cp-mode-active={viewMode === "ranked"}
        onclick={() => viewMode = "ranked"}
      >
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="m3 16 4 4 4-4" /><path d="M7 20V4" /><path d="M11 4h10" /><path d="M11 8h7" /><path d="M11 12h4" /></svg>
        By Size
      </button>
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
            <button class="cp-section-header" onclick={() => toggleCategory(cat)} aria-expanded={!isCollapsed}>
              <span class="cp-section-main">
                <span class="cp-chevron" class:cp-chevron-open={!isCollapsed}>
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="m9 18 6-6-6-6" /></svg>
                </span>
                <span class="cp-section-label">{categoryLabel(cat)}</span>
                <span class="cp-section-count">{catEntries.length}</span>
              </span>
              <span class="cp-section-tokens">~{fk(categoryTokens(catEntries))} tokens</span>
            </button>

            {#if !isCollapsed}
              <div class="cp-section-items">
                {#if cat === "claudemd" && claudeMdEntries.length > 0}
                  <!-- CLAUDE.md 用 DirectoryTree -->
                  <div class="cp-context-group">
                    <div class="cp-sub-label">Loaded instruction files</div>
                    <DirectoryTree entries={claudeMdEntries} />
                  </div>
                  {#if mentionedFileEntries.length > 0}
                    <div class="cp-context-group cp-context-group-spaced">
                      <div class="cp-sub-label">Mentioned files</div>
                      {#each mentionedFileEntries as entry}
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
    position: absolute;
    top: 0;
    right: 0;
    bottom: 0;
    z-index: 30;
    width: min(320px, 100%);
    min-width: 0;
    max-width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    border-left: 1px solid var(--color-border);
    background: var(--color-surface);
    box-shadow: -8px 0 24px rgba(0, 0, 0, 0.08);
    overflow: hidden;
  }

  .cp-header {
    flex-shrink: 0;
    border-bottom: 1px solid var(--color-border);
    padding: 12px 14px 10px;
  }

  .cp-title-row,
  .cp-title-wrap,
  .cp-token-row,
  .cp-mode-row,
  .cp-mode-btn,
  .cp-section-main {
    display: flex;
    align-items: center;
  }

  .cp-title-row {
    justify-content: space-between;
    gap: 12px;
  }

  .cp-title-wrap {
    gap: 8px;
    min-width: 0;
  }

  .cp-title-icon {
    width: 16px;
    height: 16px;
    color: var(--color-text-secondary);
    flex-shrink: 0;
  }

  .cp-title {
    font-size: 14px;
    font-weight: 650;
    color: var(--color-text);
    letter-spacing: 0.01em;
  }

  .cp-count-badge,
  .cp-section-count {
    border-radius: 5px;
    background: var(--color-surface-overlay, var(--badge-neutral-bg));
    color: var(--color-text-secondary);
    font-size: 12px;
    line-height: 1;
    padding: 3px 6px;
  }

  .cp-close {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    background: transparent;
    border: none;
    color: var(--color-text-secondary);
    cursor: pointer;
    padding: 0;
    border-radius: 6px;
    transition: background-color 0.12s ease, color 0.1s;
  }

  .cp-close svg {
    width: 16px;
    height: 16px;
  }

  .cp-close:hover {
    background: var(--tool-item-hover-bg);
    color: var(--color-text);
  }

  .cp-token-row {
    gap: 4px;
    justify-content: flex-start;
    margin-top: 9px;
    padding-top: 9px;
    border-top: 1px solid var(--color-border-subtle, var(--color-border));
    font-size: 12px;
  }

  .cp-token-muted {
    color: var(--color-text-muted);
  }

  .cp-token-value {
    color: var(--color-text-secondary);
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }

  .cp-mode-row {
    gap: 6px;
    margin-top: 9px;
    padding-top: 9px;
    border-top: 1px solid var(--color-border-subtle, var(--color-border));
  }

  .cp-mode-label {
    margin-right: 2px;
    font-size: 10px;
    color: var(--color-text-muted);
  }

  .cp-mode-btn {
    gap: 4px;
    font-size: 10px;
    font-family: inherit;
    color: var(--color-text-muted);
    background: var(--color-surface-overlay, var(--badge-neutral-bg));
    border: 1px solid transparent;
    border-radius: 5px;
    padding: 3px 8px;
    cursor: pointer;
    transition: background-color 0.12s ease, color 0.1s, border-color 0.1s;
  }

  .cp-mode-btn svg {
    width: 10px;
    height: 10px;
  }

  .cp-mode-btn:hover {
    color: var(--color-text-secondary);
    border-color: var(--color-border-subtle, var(--color-border));
  }

  .cp-mode-active {
    /* rgba fallback 给旧 WebKitGTK（< 2.40）；现代浏览器走 color-mix。 */
    background: rgba(99, 102, 241, 0.18);
    background: color-mix(in oklch, var(--color-accent-indigo) 18%, transparent);
    color: var(--color-accent-indigo);
    border-color: rgba(99, 102, 241, 0.24);
    border-color: color-mix(in oklch, var(--color-accent-indigo) 24%, transparent);
  }

  .cp-body {
    flex: 1;
    overflow-y: auto;
    overflow-x: hidden;
    scrollbar-gutter: stable;
    padding: 12px 10px 18px 14px;
  }

  /* ── Category 视图 ── */

  .cp-section {
    margin-bottom: 8px;
    overflow: hidden;
    border: 1px solid var(--color-border-subtle, var(--color-border));
    border-radius: 8px;
    background: var(--color-surface-raised);
  }

  .cp-section-header {
    width: 100%;
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 4px;
    padding: 8px 10px;
    cursor: pointer;
    border: none;
    background: transparent;
    color: inherit;
    font-family: inherit;
    text-align: left;
    transition: background-color 0.12s ease;
  }

  .cp-section-header[aria-expanded="true"] {
    background: var(--color-surface-overlay, var(--tool-item-hover-bg));
  }

  .cp-section-header:hover {
    background: var(--tool-item-hover-bg);
  }

  .cp-section-main {
    display: grid;
    grid-template-columns: 14px minmax(0, 1fr) auto;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }

  .cp-chevron {
    display: inline-flex;
    width: 14px;
    height: 14px;
    color: var(--color-text-secondary);
    flex-shrink: 0;
    transition: transform 0.15s ease;
  }

  .cp-chevron svg {
    width: 14px;
    height: 14px;
  }

  .cp-chevron-open {
    transform: rotate(90deg);
  }

  .cp-section-label {
    font-size: 13px;
    font-weight: 600;
    color: var(--color-text);
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    line-height: 1.25;
  }

  .cp-section-count {
    font-size: 11px;
    flex-shrink: 0;
  }

  .cp-section-tokens {
    padding-left: 22px;
    font-size: 11px;
    color: var(--color-text-muted);
    font-family: var(--font-mono);
    white-space: nowrap;
    line-height: 1.2;
  }

  .cp-section-items {
    padding: 8px 10px 10px 28px;
    border-top: 1px solid var(--color-border-subtle, var(--color-border));
    /* surface 42% 半透铺底；旧 WebKitGTK 用 surface-raised 兜底（视觉接近） */
    background: var(--color-surface-raised);
    background: color-mix(in srgb, var(--color-surface) 42%, transparent);
  }

  .cp-context-group {
    min-width: 0;
  }

  .cp-context-group-spaced {
    margin-top: 10px;
    padding-top: 10px;
    border-top: 1px solid var(--color-border-subtle, var(--color-border));
  }

  .cp-sub-label {
    margin-bottom: 5px;
    font-size: 10px;
    font-weight: 600;
    color: var(--color-text-muted);
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .cp-item {
    padding: 6px 8px;
    border-radius: 6px;
    transition: background-color 0.12s ease;
  }

  .cp-item:hover {
    background: var(--tool-item-hover-bg);
  }

  .cp-item-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: start;
    gap: 8px;
  }

  .cp-item-label {
    font-size: 12px;
    font-weight: 600;
    color: var(--color-text);
    min-width: 0;
    overflow-wrap: anywhere;
    line-height: 1.3;
  }

  .cp-item-tokens {
    font-size: 11px;
    color: var(--color-text-muted);
    font-family: var(--font-mono);
    flex-shrink: 0;
  }

  .cp-item-preview {
    display: block;
    font-size: 11px;
    color: var(--color-text-muted);
    overflow-wrap: anywhere;
    line-height: 1.35;
    margin-top: 4px;
  }

  /* ── Ranked 视图 ── */

  .cp-ranked-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .cp-ranked-item {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: 8px;
    padding: 7px 9px;
    border: 1px solid var(--color-border-subtle, var(--color-border));
    border-radius: 7px;
    background: var(--color-surface-raised);
    transition: background-color 0.12s ease, border-color 0.1s;
  }

  .cp-ranked-item:hover {
    background: var(--tool-item-hover-bg);
    border-color: var(--color-border-emphasis);
  }

  .cp-cat-tag {
    font-size: 9px;
    font-weight: 600;
    padding: 2px 6px;
    border-radius: 4px;
    flex-shrink: 0;
    letter-spacing: 0.2px;
  }

  .cp-ranked-label {
    font-size: 12px;
    color: var(--color-text);
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    line-height: 1.3;
  }

  .cp-ranked-tokens {
    font-size: 11px;
    color: var(--color-text-muted);
    font-family: var(--font-mono);
    flex-shrink: 0;
  }
</style>

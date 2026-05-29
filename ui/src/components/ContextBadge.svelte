<script lang="ts">
  import {
    type TurnContextStats,
    type ContextInjection,
    shouldShowBadge,
    getCategoryBreakdown,
    formatTokens,
  } from "../lib/contextExtractor";

  interface Props {
    stats: TurnContextStats | null;
    injections: ContextInjection[];
    popoverId: string;
    openPopoverId: string | null;
    onToggle: (id: string) => void;
  }

  let { stats, injections: _injections, popoverId, openPopoverId, onToggle }: Props = $props();

  let isOpen = $derived(openPopoverId === popoverId);
  let show = $derived(shouldShowBadge(stats));
  let breakdown = $derived(stats ? getCategoryBreakdown(stats) : []);

  function handleClick(e: MouseEvent) {
    e.stopPropagation();
    onToggle(popoverId);
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape" && isOpen) {
      e.stopPropagation();
      onToggle(popoverId);
    }
  }
</script>

{#if show && stats}
  <button
    class="context-badge"
    class:context-badge-active={isOpen}
    aria-expanded={isOpen}
    aria-label="Context injected this turn: {stats.newCount} items, ~{formatTokens(stats.newTokens)} tokens"
    onclick={handleClick}
    onkeydown={handleKeydown}
  >
    <span class="context-badge-label">Context</span>
    <span class="context-badge-count">+{stats.newCount}</span>
    {#if isOpen}
      <span class="context-badge-popover" role="dialog" aria-label="New Context Injected This Turn">
        <span class="cbp-title">New Context Injected This Turn</span>
        <span class="cbp-list">
          {#each breakdown as item}
            <span class="cbp-row">
              <span class="cbp-cat">{item.label} ({item.count})</span>
              <span class="cbp-tokens">~{formatTokens(item.tokens)} tokens</span>
            </span>
          {/each}
        </span>
        <span class="cbp-footer">
          <span class="cbp-footer-label">Total new tokens</span>
          <span class="cbp-footer-value">~{formatTokens(stats.newTokens)} tokens</span>
        </span>
      </span>
    {/if}
  </button>
{/if}

<style>
  .context-badge {
    position: relative;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 11px;
    font-family: var(--font-mono);
    font-weight: 500;
    color: var(--color-text-muted);
    background: var(--color-surface-raised);
    border: 1px solid var(--color-border);
    border-radius: 9999px;
    padding: 2px 8px;
    cursor: pointer;
    flex-shrink: 0;
    transition:
      background 120ms ease,
      border-color 120ms ease;
  }

  .context-badge:hover {
    background: var(--color-surface-overlay);
    border-color: var(--color-border-emphasis);
  }

  .context-badge-active {
    background: var(--color-surface-overlay);
    border-color: var(--color-border-emphasis);
  }

  .context-badge:focus-visible {
    outline: none;
    box-shadow: 0 0 0 2px rgba(59, 130, 246, 0.15);
  }

  .context-badge-label {
    color: var(--color-text-muted);
  }

  .context-badge-count {
    font-weight: 600;
    color: var(--color-text-secondary);
  }

  .context-badge-popover {
    position: absolute;
    top: calc(100% + 8px);
    right: 0;
    z-index: 20;
    min-width: 240px;
    padding: 10px 12px;
    border-radius: 10px;
    background: var(--color-surface);
    border: 1px solid var(--color-border-emphasis);
    box-shadow:
      0 12px 32px rgba(0, 0, 0, 0.14),
      0 0 0 1px color-mix(in oklch, var(--color-accent-blue) 0%, transparent);
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 11.5px;
    font-family: var(--font-mono);
    text-align: left;
    cursor: default;
  }

  .cbp-title {
    font-weight: 600;
    font-size: 12px;
    color: var(--color-text-secondary);
    font-family: var(--font-sans);
    padding-bottom: 4px;
    border-bottom: 1px solid var(--color-border);
  }

  .cbp-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .cbp-row {
    display: flex;
    justify-content: space-between;
    gap: 12px;
  }

  .cbp-cat {
    color: var(--color-text-muted);
  }

  .cbp-tokens {
    color: var(--color-text-secondary);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .cbp-footer {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    padding-top: 4px;
    border-top: 1px solid var(--color-border);
    font-weight: 600;
  }

  .cbp-footer-label {
    color: var(--color-text-muted);
  }

  .cbp-footer-value {
    color: var(--color-text-secondary);
    font-variant-numeric: tabular-nums;
  }
</style>

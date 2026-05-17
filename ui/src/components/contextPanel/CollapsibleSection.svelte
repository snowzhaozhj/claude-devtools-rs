<script lang="ts">
  /**
   * 共享 Section header：chevron + label + count + tokens；点击切换展开折叠。
   * 各 Section 通过 children snippet 渲染自身内容。
   */
  import type { Snippet } from "svelte";

  interface Props {
    label: string;
    count: number;
    tokens: number;
    expanded: boolean;
    onToggle: () => void;
    children: Snippet;
  }

  let { label, count, tokens, expanded, onToggle, children }: Props = $props();

  function fk(n: number): string {
    if (n >= 1e6) return (n / 1e6).toFixed(1) + "M";
    if (n >= 1e3) return (n / 1e3).toFixed(1) + "k";
    return String(n);
  }
</script>

<div class="cp-section">
  <button
    type="button"
    class="cp-section-header"
    onclick={onToggle}
    aria-expanded={expanded}
  >
    <span class="cp-section-main">
      <span class="cp-chevron" class:cp-chevron-open={expanded}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="m9 18 6-6-6-6" /></svg>
      </span>
      <span class="cp-section-label">{label}</span>
      <span class="cp-section-count">{count}</span>
    </span>
    <span class="cp-section-tokens">~{fk(tokens)} tokens</span>
  </button>

  {#if expanded}
    <div class="cp-section-items">
      {@render children()}
    </div>
  {/if}
</div>

<style>
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
    transition: background 0.1s;
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
    border-radius: 5px;
    background: var(--color-surface-overlay, var(--badge-neutral-bg));
    color: var(--color-text-secondary);
    font-size: 11px;
    line-height: 1;
    padding: 3px 6px;
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
    background: var(--color-surface-raised);
    background: color-mix(in srgb, var(--color-surface) 42%, transparent);
  }
</style>

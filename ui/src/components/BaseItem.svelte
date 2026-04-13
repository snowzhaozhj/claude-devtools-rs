<script lang="ts">
  import type { Snippet } from "svelte";
  import StatusDot from "./StatusDot.svelte";

  interface Props {
    icon?: string;
    label: string;
    summary?: string;
    tokenCount?: number;
    status?: "ok" | "error" | "pending" | "orphaned";
    durationMs?: number;
    isExpanded: boolean;
    onclick: () => void;
    children?: Snippet;
  }

  let { icon, label, summary, tokenCount, status, durationMs, isExpanded, onclick, children }: Props = $props();

  function formatTokens(n: number): string {
    if (n >= 1e6) return (n / 1e6).toFixed(1) + "M";
    if (n >= 1e3) return (n / 1e3).toFixed(1) + "k";
    return String(n);
  }

  function formatDuration(ms: number): string {
    if (ms < 1000) return `${ms}ms`;
    return `${(ms / 1000).toFixed(1)}s`;
  }
</script>

<div class="base-item">
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="base-item-header" onclick={onclick}>
    {#if icon}
      <span class="base-item-icon">{icon}</span>
    {/if}

    <span class="base-item-label">{label}</span>

    {#if summary}
      <span class="base-item-sep">-</span>
      <span class="base-item-summary">{summary}</span>
    {:else}
      <span class="base-item-spacer"></span>
    {/if}

    {#if tokenCount != null && tokenCount > 0}
      <span class="base-item-tokens">~{formatTokens(tokenCount)}</span>
    {/if}

    {#if status}
      <StatusDot {status} />
    {/if}

    {#if durationMs != null}
      <span class="base-item-duration">{formatDuration(durationMs)}</span>
    {/if}

    <span class="base-item-chevron" class:base-item-chevron-open={isExpanded}>▸</span>
  </div>

  {#if isExpanded && children}
    <div class="base-item-content">
      {@render children()}
    </div>
  {/if}
</div>

<style>
  .base-item {
    border-radius: 4px;
    transition: background-color 0.2s;
  }

  .base-item-header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    border-radius: 4px;
    cursor: pointer;
    transition: background-color 0.1s;
  }

  .base-item-header:hover {
    background: var(--tool-item-hover-bg);
  }

  .base-item-icon {
    font-size: 14px;
    width: 16px;
    text-align: center;
    flex-shrink: 0;
    color: var(--tool-item-muted);
  }

  .base-item-label {
    font-size: 14px;
    font-weight: 500;
    color: var(--tool-item-name);
    flex-shrink: 0;
  }

  .base-item-sep {
    font-size: 14px;
    color: var(--tool-item-muted);
    flex-shrink: 0;
  }

  .base-item-summary {
    flex: 1;
    font-size: 14px;
    color: var(--tool-item-summary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }

  .base-item-spacer {
    flex: 1;
  }

  .base-item-tokens {
    font-size: 12px;
    color: var(--tool-item-muted);
    background: var(--badge-neutral-bg);
    padding: 1px 6px;
    border-radius: 4px;
    flex-shrink: 0;
    font-family: var(--font-mono);
  }

  .base-item-duration {
    font-size: 12px;
    color: var(--tool-item-muted);
    flex-shrink: 0;
    font-family: var(--font-mono);
  }

  .base-item-chevron {
    font-size: 10px;
    color: var(--tool-item-muted);
    flex-shrink: 0;
    width: 12px;
    transition: transform 0.15s ease;
  }

  .base-item-chevron-open {
    transform: rotate(90deg);
  }

  .base-item-content {
    margin-left: 8px;
    margin-top: 8px;
    padding-left: 24px;
    border-left: 2px solid var(--color-border);
  }
</style>

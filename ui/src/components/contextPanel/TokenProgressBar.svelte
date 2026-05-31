<script lang="ts">
  import { getLastAssistantUsage, getUsageLevel, type ContextWindowUsage } from "../../lib/modelLimits";
  import { formatTokens } from "../../lib/contextExtractor";
  import type { Chunk } from "../../lib/api";

  interface Props {
    chunks: Chunk[];
  }

  let { chunks }: Props = $props();

  const usage: ContextWindowUsage | null = $derived(getLastAssistantUsage(chunks));
  const level = $derived(usage ? getUsageLevel(usage.ratio) : null);
  const percent = $derived(usage ? Math.min(Math.round(usage.ratio * 100), 100) : 0);
</script>

{#if usage}
  <div class="tp-container" data-level={level}>
    <div class="tp-header">
      <span class="tp-label">Context Window</span>
      <span class="tp-value">
        <span class="tp-tokens">{formatTokens(usage.inputTokens)}</span>
        <span class="tp-separator">/</span>
        <span class="tp-limit">{formatTokens(usage.contextLimit)}</span>
        <span class="tp-percent" class:tp-percent-high={level === "high"}>({percent}%)</span>
      </span>
    </div>
    <div class="tp-track">
      <div class="tp-fill" style:width="{percent}%"></div>
    </div>
  </div>
{/if}

<style>
  .tp-container {
    padding: 0;
    margin-top: 9px;
    padding-top: 9px;
    border-top: 1px solid var(--color-border-subtle, var(--color-border));
  }

  .tp-header {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    margin-bottom: 6px;
  }

  .tp-label {
    font-size: 11px;
    font-weight: 600;
    color: var(--color-text-secondary);
    letter-spacing: 0.02em;
  }

  .tp-value {
    font-size: 11px;
    font-family: var(--font-mono);
    color: var(--color-text-muted);
    font-variant-numeric: tabular-nums;
  }

  .tp-tokens {
    color: var(--color-text-secondary);
    font-weight: 500;
  }

  .tp-separator {
    margin: 0 2px;
    opacity: 0.5;
  }

  .tp-percent {
    margin-left: 4px;
    font-weight: 500;
  }

  .tp-percent-high {
    color: var(--color-danger, #dc2626);
  }

  .tp-track {
    height: 4px;
    border-radius: 9999px;
    background: var(--color-surface-raised);
    overflow: hidden;
  }

  .tp-fill {
    height: 100%;
    border-radius: 9999px;
    transition: width 0.3s ease-out, background-color 0.3s ease-out;
  }

  .tp-container[data-level="low"] .tp-fill {
    background-color: var(--color-success, #15803d);
  }

  .tp-container[data-level="medium"] .tp-fill {
    background-color: var(--color-warning, #f59e0b);
  }

  .tp-container[data-level="high"] .tp-fill {
    background-color: var(--color-danger, #dc2626);
  }
</style>

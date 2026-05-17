<script lang="ts">
  import CollapsibleSection from "./CollapsibleSection.svelte";
  import { sumTokens, formatTokens, type ThinkingTextInjection } from "../../lib/contextExtractor";

  interface Props {
    injections: ThinkingTextInjection[];
    expanded: boolean;
    onToggle: () => void;
    onNavigate: (aiGroupId: string) => void;
  }

  let { injections, expanded, onToggle, onNavigate }: Props = $props();

  const sorted = $derived([...injections].sort((a, b) => a.turnIndex - b.turnIndex));
  const tokens = $derived(sumTokens(injections));
</script>

{#if injections.length > 0}
  <CollapsibleSection label="Thinking + Text" count={injections.length} {tokens} {expanded} {onToggle}>
    {#each sorted as inj (inj.id)}
      <button type="button" class="tt-row" onclick={() => onNavigate(inj.aiGroupId)}>
        <span class="tt-turn">Turn {inj.turnIndex + 1}</span>
        <span class="tt-breakdown">
          {#each inj.breakdown as bd, i (i)}
            {#if bd.tokenCount > 0}
              <span class="tt-bd-item" class:tt-bd-thinking={bd.type === "thinking"}>
                {bd.type}
                <span class="tt-bd-tokens">~{formatTokens(bd.tokenCount)}</span>
              </span>
            {/if}
          {/each}
        </span>
        <span class="tt-total">~{formatTokens(inj.estimatedTokens)}</span>
      </button>
    {/each}
  </CollapsibleSection>
{/if}

<style>
  .tt-row {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px 8px;
    background: transparent;
    border: none;
    border-radius: 6px;
    color: inherit;
    font-family: inherit;
    text-align: left;
    cursor: pointer;
    transition: background 0.1s;
  }

  .tt-row:hover {
    background: var(--tool-item-hover-bg);
  }

  .tt-turn {
    font-size: 10px;
    font-weight: 600;
    color: var(--color-accent-purple, #a78bfa);
    background: color-mix(in srgb, var(--color-accent-purple, #a78bfa) 14%, transparent);
    border-radius: 4px;
    padding: 2px 6px;
    flex-shrink: 0;
    font-variant-numeric: tabular-nums;
  }

  .tt-breakdown {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
    min-width: 0;
  }

  .tt-bd-item {
    font-size: 11px;
    color: var(--color-text-muted);
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }

  .tt-bd-thinking {
    color: var(--color-accent-purple, #a78bfa);
  }

  .tt-bd-tokens {
    font-family: var(--font-mono);
    font-size: 10px;
  }

  .tt-total {
    font-size: 11px;
    color: var(--color-text-muted);
    font-family: var(--font-mono);
    flex-shrink: 0;
  }
</style>

<script lang="ts">
  import CollapsibleSection from "./CollapsibleSection.svelte";
  import { sumTokens, formatTokens, type UserMessageInjection } from "../../lib/contextExtractor";

  interface Props {
    injections: UserMessageInjection[];
    expanded: boolean;
    onToggle: () => void;
    onNavigate: (aiGroupId: string) => void;
  }

  let { injections, expanded, onToggle, onNavigate }: Props = $props();

  const sorted = $derived([...injections].sort((a, b) => a.turnIndex - b.turnIndex));
  const tokens = $derived(sumTokens(injections));
</script>

{#if injections.length > 0}
  <CollapsibleSection label="User Messages" count={injections.length} {tokens} {expanded} {onToggle}>
    {#each sorted as inj (inj.id)}
      <button type="button" class="user-msg-row" onclick={() => onNavigate(inj.aiGroupId)}>
        <span class="user-msg-turn">Turn {inj.turnIndex + 1}</span>
        <span class="user-msg-preview">{inj.textPreview}</span>
        <span class="user-msg-tokens">~{formatTokens(inj.estimatedTokens)}</span>
      </button>
    {/each}
  </CollapsibleSection>
{/if}

<style>
  .user-msg-row {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: start;
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

  .user-msg-row:hover {
    background: var(--tool-item-hover-bg);
  }

  .user-msg-turn {
    font-size: 10px;
    font-weight: 600;
    color: var(--color-accent-blue, var(--color-text-secondary));
    background: color-mix(in srgb, var(--color-accent-blue) 12%, transparent);
    border-radius: 4px;
    padding: 2px 6px;
    flex-shrink: 0;
    font-variant-numeric: tabular-nums;
  }

  .user-msg-preview {
    font-size: 12px;
    color: var(--color-text);
    min-width: 0;
    overflow-wrap: anywhere;
    line-height: 1.35;
  }

  .user-msg-tokens {
    font-size: 11px;
    color: var(--color-text-muted);
    font-family: var(--font-mono);
    flex-shrink: 0;
  }
</style>

<script lang="ts">
  import CollapsibleSection from "./CollapsibleSection.svelte";
  import { sumTokens, formatTokens, type MentionedFileInjection } from "../../lib/contextExtractor";

  interface Props {
    injections: MentionedFileInjection[];
    expanded: boolean;
    onToggle: () => void;
    onNavigate: (aiGroupId: string) => void;
  }

  let { injections, expanded, onToggle, onNavigate }: Props = $props();

  const sorted = $derived([...injections].sort((a, b) => b.estimatedTokens - a.estimatedTokens));
  const tokens = $derived(sumTokens(injections));

  function shortenPath(p: string): string {
    return p.replace(/^\/Users\/[^/]+/, "~");
  }
</script>

{#if injections.length > 0}
  <CollapsibleSection label="Mentioned Files" count={injections.length} {tokens} {expanded} {onToggle}>
    {#each sorted as inj (inj.id)}
      <button
        type="button"
        class="mf-row"
        class:mf-missing={!inj.exists}
        onclick={() => onNavigate(inj.firstSeenInGroup)}
        title={inj.path}
      >
        <span class="mf-name">{inj.displayName}</span>
        <span class="mf-tokens">~{formatTokens(inj.estimatedTokens)}</span>
        <span class="mf-path">{shortenPath(inj.path)}</span>
        {#if !inj.exists}
          <span class="mf-missing-chip">missing</span>
        {/if}
      </button>
    {/each}
  </CollapsibleSection>
{/if}

<style>
  .mf-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto auto;
    grid-template-areas:
      "name tokens chip"
      "path path path";
    column-gap: 8px;
    row-gap: 2px;
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

  .mf-row:hover {
    background: var(--tool-item-hover-bg);
  }

  .mf-name {
    grid-area: name;
    font-size: 12px;
    font-weight: 600;
    color: var(--color-text);
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .mf-tokens {
    grid-area: tokens;
    font-size: 11px;
    color: var(--color-text-muted);
    font-family: var(--font-mono);
    flex-shrink: 0;
  }

  .mf-path {
    grid-area: path;
    font-size: 10px;
    color: var(--color-text-muted);
    font-family: var(--font-mono);
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .mf-missing-chip {
    grid-area: chip;
    font-size: 9px;
    font-weight: 600;
    color: var(--color-danger);
    background: color-mix(in srgb, var(--color-danger) 15%, transparent);
    border-radius: 4px;
    padding: 2px 6px;
    flex-shrink: 0;
    letter-spacing: 0.2px;
  }

  .mf-missing .mf-name {
    color: var(--color-text-muted);
    text-decoration: line-through;
  }
</style>

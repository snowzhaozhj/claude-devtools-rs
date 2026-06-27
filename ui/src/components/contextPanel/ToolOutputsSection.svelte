<script lang="ts">
  import CollapsibleSection from "./CollapsibleSection.svelte";
  import { sumTokens, formatTokens, type ToolOutputInjection } from "../../lib/contextExtractor";

  interface Props {
    injections: ToolOutputInjection[];
    expanded: boolean;
    onToggle: () => void;
    onNavigateTool: (aiGroupId: string, toolUseId: string) => void;
  }

  let { injections, expanded, onToggle, onNavigateTool }: Props = $props();

  // 按 turn index 升序展示，让"Turn 1 / Turn 2 / ..."顺序自然
  const sorted = $derived([...injections].sort((a, b) => a.turnIndex - b.turnIndex));
  const tokens = $derived(sumTokens(injections));

  let openTurns: Set<string> = $state(new Set());
  function toggleTurn(id: string) {
    const n = new Set(openTurns);
    if (n.has(id)) n.delete(id);
    else n.add(id);
    openTurns = n;
  }
</script>

{#if injections.length > 0}
  <CollapsibleSection label="Tool Outputs" count={injections.length} {tokens} {expanded} {onToggle}>
    {#each sorted as inj (inj.id)}
      {@const open = openTurns.has(inj.id)}
      <div class="to-group">
        <button type="button" class="to-turn-row" onclick={() => toggleTurn(inj.id)} aria-expanded={open}>
          <span class="to-chevron" class:to-chevron-open={open} aria-hidden="true">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m9 18 6-6-6-6" /></svg>
          </span>
          <span class="to-turn">Turn {inj.turnIndex + 1}</span>
          <span class="to-tools-count">{inj.toolCount} tool{inj.toolCount === 1 ? "" : "s"}</span>
          <span class="to-tokens">~{formatTokens(inj.estimatedTokens)}</span>
        </button>

        {#if open && inj.toolBreakdown.length > 0}
          <div class="to-breakdown">
            {#each inj.toolBreakdown as bd, i (bd.toolUseId ?? `${inj.id}-${i}`)}
              {#if bd.toolUseId}
                <button
                  type="button"
                  class="to-tool-row"
                  class:to-tool-error={bd.isError}
                  onclick={() => onNavigateTool(inj.aiGroupId, bd.toolUseId!)}
                  title="跳转到 {bd.toolName}（{bd.toolUseId}）"
                >
                  <span class="to-tool-name">{bd.toolName}</span>
                  {#if bd.isError}
                    <span class="to-error-chip">error</span>
                  {/if}
                  <span class="to-tool-tokens">~{formatTokens(bd.tokenCount)}</span>
                </button>
              {:else}
                <div class="to-tool-row to-tool-disabled" class:to-tool-error={bd.isError}>
                  <span class="to-tool-name">{bd.toolName}</span>
                  {#if bd.isError}
                    <span class="to-error-chip">error</span>
                  {/if}
                  <span class="to-tool-tokens">~{formatTokens(bd.tokenCount)}</span>
                </div>
              {/if}
            {/each}
          </div>
        {/if}
      </div>
    {/each}
  </CollapsibleSection>
{/if}

<style>
  .to-group {
    border-radius: 6px;
    margin-bottom: 4px;
    overflow: hidden;
  }

  .to-group:last-child {
    margin-bottom: 0;
  }

  .to-turn-row {
    display: grid;
    grid-template-columns: 14px auto minmax(0, 1fr) auto;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 5px 8px;
    background: transparent;
    border: none;
    border-radius: 6px;
    color: inherit;
    font-family: inherit;
    text-align: left;
    cursor: pointer;
    transition: background 0.1s;
  }

  .to-turn-row:hover {
    background: var(--tool-item-hover-bg);
  }

  .to-chevron {
    display: inline-flex;
    width: 14px;
    height: 14px;
    color: var(--color-text-muted);
    transition: transform 0.15s ease;
  }

  .to-chevron svg {
    width: 13px;
    height: 13px;
  }

  .to-chevron-open {
    transform: rotate(90deg);
  }

  .to-turn {
    font-size: 10px;
    font-weight: 600;
    color: var(--color-warning-text);
    background: color-mix(in srgb, var(--color-warning) 14%, transparent);
    border-radius: 4px;
    padding: 2px 6px;
    flex-shrink: 0;
    font-variant-numeric: tabular-nums;
  }

  .to-tools-count {
    font-size: 11px;
    color: var(--color-text-muted);
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .to-tokens {
    font-size: 11px;
    color: var(--color-text-muted);
    font-family: var(--font-mono);
    flex-shrink: 0;
  }

  .to-breakdown {
    padding: 4px 0 4px 28px;
    border-left: 1px solid var(--color-border-subtle, var(--color-border));
    margin-left: 8px;
  }

  .to-tool-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto auto;
    align-items: center;
    gap: 6px;
    width: 100%;
    padding: 4px 8px;
    background: transparent;
    border: none;
    border-radius: 5px;
    color: inherit;
    font-family: inherit;
    text-align: left;
    cursor: pointer;
    transition: background 0.1s;
  }

  .to-tool-row.to-tool-disabled {
    cursor: default;
  }

  .to-tool-row:hover:not(.to-tool-disabled) {
    background: var(--tool-item-hover-bg);
  }

  .to-tool-name {
    font-size: 11px;
    font-weight: 500;
    color: var(--color-text);
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-mono);
  }

  .to-tool-error .to-tool-name {
    color: var(--color-danger);
  }

  .to-tool-tokens {
    font-size: 10px;
    color: var(--color-text-muted);
    font-family: var(--font-mono);
    flex-shrink: 0;
  }

  .to-error-chip {
    font-size: 9px;
    font-weight: 600;
    color: var(--color-danger);
    background: color-mix(in srgb, var(--color-danger) 15%, transparent);
    border-radius: 3px;
    padding: 1px 5px;
    flex-shrink: 0;
    letter-spacing: 0.2px;
  }
</style>

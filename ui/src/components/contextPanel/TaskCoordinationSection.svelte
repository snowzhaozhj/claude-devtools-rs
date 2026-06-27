<script lang="ts">
  import CollapsibleSection from "./CollapsibleSection.svelte";
  import {
    sumTokens,
    formatTokens,
    type TaskCoordinationInjection,
    type TaskCoordinationKind,
  } from "../../lib/contextExtractor";

  interface Props {
    injections: TaskCoordinationInjection[];
    expanded: boolean;
    onToggle: () => void;
    onNavigate: (aiGroupId: string) => void;
  }

  let { injections, expanded, onToggle, onNavigate }: Props = $props();

  const sorted = $derived([...injections].sort((a, b) => a.turnIndex - b.turnIndex));
  const tokens = $derived(sumTokens(injections));

  const KIND_LABEL: Record<TaskCoordinationKind, string> = {
    "send-message": "SendMessage",
    "task-tool": "Task",
    "teammate-message": "Teammate",
  };
</script>

{#if injections.length > 0}
  <CollapsibleSection label="Task Coordination" count={injections.length} {tokens} {expanded} {onToggle}>
    {#each sorted as inj (inj.id)}
      <div class="tc-group">
        <button type="button" class="tc-turn-row" onclick={() => onNavigate(inj.aiGroupId)}>
          <span class="tc-turn">Turn {inj.turnIndex + 1}</span>
          <span class="tc-summary">{inj.breakdown.length} item{inj.breakdown.length === 1 ? "" : "s"}</span>
          <span class="tc-tokens">~{formatTokens(inj.estimatedTokens)}</span>
        </button>
        <div class="tc-breakdown">
          {#each inj.breakdown as bd, i (i)}
            <div class="tc-item">
              <span class="tc-kind">{KIND_LABEL[bd.type]}</span>
              <span class="tc-label">{bd.label}</span>
              <span class="tc-item-tokens">~{formatTokens(bd.tokenCount)}</span>
            </div>
          {/each}
        </div>
      </div>
    {/each}
  </CollapsibleSection>
{/if}

<style>
  .tc-group {
    margin-bottom: 6px;
  }

  .tc-group:last-child {
    margin-bottom: 0;
  }

  .tc-turn-row {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
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

  .tc-turn-row:hover {
    background: var(--tool-item-hover-bg);
  }

  .tc-turn {
    font-size: 10px;
    font-weight: 600;
    color: var(--color-warning-text);
    background: color-mix(in srgb, var(--color-warning) 14%, transparent);
    border-radius: 4px;
    padding: 2px 6px;
    flex-shrink: 0;
    font-variant-numeric: tabular-nums;
  }

  .tc-summary {
    font-size: 11px;
    color: var(--color-text-muted);
  }

  .tc-tokens {
    font-size: 11px;
    color: var(--color-text-muted);
    font-family: var(--font-mono);
    flex-shrink: 0;
  }

  .tc-breakdown {
    padding: 4px 0 4px 28px;
    border-left: 1px solid var(--color-border-subtle, var(--color-border));
    margin-left: 8px;
  }

  .tc-item {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: 6px;
    padding: 3px 8px;
    border-radius: 5px;
  }

  .tc-kind {
    font-size: 9px;
    font-weight: 600;
    color: var(--color-warning-text);
    background: color-mix(in srgb, var(--color-warning) 14%, transparent);
    border-radius: 3px;
    padding: 1px 5px;
    flex-shrink: 0;
    letter-spacing: 0.2px;
  }

  .tc-label {
    font-size: 11px;
    color: var(--color-text);
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .tc-item-tokens {
    font-size: 10px;
    color: var(--color-text-muted);
    font-family: var(--font-mono);
    flex-shrink: 0;
  }
</style>

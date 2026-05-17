<script lang="ts">
  import type { ContextPhase } from "../../lib/api";

  interface Props {
    phases: ContextPhase[];
    selected: number | null; // null = Latest
    onChange: (selected: number | null) => void;
  }

  let { phases, selected, onChange }: Props = $props();

  // 当 phases.length <= 1 时父组件不应渲染 PhaseSelector；这里再 guard 一次。
  const showSelector = $derived(phases.length > 1);

  function handleChange(e: Event) {
    const v = (e.target as HTMLSelectElement).value;
    onChange(v === "latest" ? null : Number(v));
  }
</script>

{#if showSelector}
  <div class="ps-row">
    <label class="ps-label" for="phase-selector">Phase:</label>
    <select id="phase-selector" class="ps-select" onchange={handleChange}>
      <option value="latest" selected={selected === null}>Latest</option>
      {#each phases as p (p.phaseNumber)}
        <option value={String(p.phaseNumber)} selected={selected === p.phaseNumber}>
          Phase {p.phaseNumber}
        </option>
      {/each}
    </select>
  </div>
{/if}

<style>
  .ps-row {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 9px;
    padding-top: 9px;
    border-top: 1px solid var(--color-border-subtle, var(--color-border));
  }

  .ps-label {
    font-size: 10px;
    color: var(--color-text-muted);
  }

  .ps-select {
    flex: 1;
    background: var(--color-surface-overlay, var(--badge-neutral-bg));
    color: var(--color-text-secondary);
    border: 1px solid transparent;
    border-radius: 5px;
    padding: 3px 6px;
    font-size: 11px;
    font-family: inherit;
    cursor: pointer;
    transition: border-color 0.1s;
  }

  .ps-select:hover,
  .ps-select:focus {
    border-color: var(--color-border, var(--color-border-subtle));
    outline: none;
  }
</style>

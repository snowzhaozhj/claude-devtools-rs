<script lang="ts">
  import type { ContextPhase } from "../../lib/api";
  import Dropdown, { type DropdownOption } from "../../lib/components/Dropdown.svelte";

  interface Props {
    phases: ContextPhase[];
    selected: number | null; // null = Latest
    onChange: (selected: number | null) => void;
  }

  let { phases, selected, onChange }: Props = $props();

  const showSelector = $derived(phases.length > 1);

  const options = $derived<DropdownOption[]>([
    { value: "latest", label: "Latest" },
    ...phases.map((p) => ({
      value: String(p.phaseNumber),
      label: `Phase ${p.phaseNumber}`,
    })),
  ]);

  const currentValue = $derived(selected === null ? "latest" : String(selected));

  function handleChange(v: string) {
    onChange(v === "latest" ? null : Number(v));
  }
</script>

{#if showSelector}
  <div class="ps-row">
    <span class="ps-label">Phase:</span>
    <div class="ps-control">
      <Dropdown
        value={currentValue}
        {options}
        onChange={handleChange}
        ariaLabel="Phase selector"
        size="sm"
      />
    </div>
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

  .ps-control {
    flex: 1;
    display: flex;
  }

  .ps-control :global(.dd-anchor) {
    flex: 1;
  }
</style>

<script lang="ts">
  import { tick } from "svelte";

  export interface ChipOption {
    value: string;
    label: string;
  }

  interface Props {
    value: string;
    options: ChipOption[];
    onChange: (v: string) => void;
    ariaLabel?: string;
  }

  let { value, options, onChange, ariaLabel }: Props = $props();

  let chipEls: HTMLButtonElement[] = $state([]);

  function selectAt(i: number) {
    const opt = options[i];
    if (!opt) return;
    if (opt.value !== value) onChange(opt.value);
  }

  async function focusAt(i: number) {
    await tick();
    chipEls[i]?.focus();
  }

  function onKeydown(e: KeyboardEvent, i: number) {
    if (e.key === "ArrowRight") {
      if (i >= options.length - 1) return;
      e.preventDefault();
      selectAt(i + 1);
      void focusAt(i + 1);
    } else if (e.key === "ArrowLeft") {
      if (i <= 0) return;
      e.preventDefault();
      selectAt(i - 1);
      void focusAt(i - 1);
    } else if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      selectAt(i);
    }
  }
</script>

<div class="worktree-chip-cluster" role="radiogroup" aria-label={ariaLabel}>
  {#each options as opt, i (opt.value)}
    <button
      type="button"
      class="worktree-chip"
      class:worktree-chip-active={opt.value === value}
      role="radio"
      aria-checked={opt.value === value}
      tabindex={opt.value === value ? 0 : -1}
      onclick={() => selectAt(i)}
      onkeydown={(e) => onKeydown(e, i)}
      bind:this={chipEls[i]}
    >{opt.label}</button>
  {/each}
</div>

<style>
  .worktree-chip-cluster {
    display: flex;
    flex-wrap: nowrap;
    align-items: center;
    gap: 4px;
    overflow-x: auto;
    overflow-y: hidden;
    scrollbar-width: none;
    -webkit-mask-image: linear-gradient(to right, black calc(100% - 16px), transparent);
    mask-image: linear-gradient(to right, black calc(100% - 16px), transparent);
  }
  .worktree-chip-cluster::-webkit-scrollbar {
    display: none;
  }

  .worktree-chip {
    flex-shrink: 0;
    height: 24px;
    padding: 3px 10px;
    border-radius: 6px;
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--color-text-secondary);
    border: 1px solid transparent;
    background: transparent;
    cursor: pointer;
    white-space: nowrap;
    user-select: none;
    outline: none;
    transition: background-color 0.12s, color 0.12s, border-color 0.12s;
  }
  .worktree-chip:hover {
    background: var(--tool-item-hover-bg, var(--color-surface-overlay));
  }
  .worktree-chip-active {
    background: var(--color-surface-overlay);
    color: var(--color-text);
    border-color: var(--color-border-emphasis, var(--color-border));
  }
  .worktree-chip:focus-visible {
    outline: 2px solid var(--color-accent-blue);
    outline-offset: 1px;
  }
</style>

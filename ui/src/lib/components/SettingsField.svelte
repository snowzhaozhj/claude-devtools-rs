<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    label: string;
    description?: string;
    /** stack: label/desc 顶部，control 单独成行（适合 path 输入、宽控件） */
    layout?: "inline" | "stack";
    /** for=labelFor 绑定 input id，提升可达性 */
    labelFor?: string;
    control?: Snippet;
    children?: Snippet;
  }

  let { label, description, layout = "inline", labelFor, control, children }: Props = $props();
</script>

<div class="field" class:field-stack={layout === "stack"}>
  <div class="field-info">
    {#if labelFor}
      <label class="field-label" for={labelFor}>{label}</label>
    {:else}
      <span class="field-label">{label}</span>
    {/if}
    {#if description}
      <span class="field-desc">{description}</span>
    {/if}
  </div>
  {#if control}
    <div class="field-control">
      {@render control()}
    </div>
  {/if}
  {#if children}
    <div class="field-extra">
      {@render children()}
    </div>
  {/if}
</div>

<style>
  .field {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 14px 16px;
    background: var(--color-surface);
    transition: background-color 0.1s;
  }
  .field-stack {
    flex-direction: column;
    align-items: stretch;
    gap: 10px;
  }
  .field-info {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
    flex: 1;
  }
  .field-label {
    font-size: 14px;
    font-weight: 500;
    color: var(--color-text);
    line-height: 1.35;
  }
  .field-desc {
    font-size: 12px;
    color: var(--color-text-secondary);
    line-height: 1.5;
  }
  .field-desc :global(code) {
    padding: 1px 5px;
    border-radius: 3px;
    background: var(--color-surface-overlay);
    font-family: var(--font-mono);
    font-size: 11px;
  }
  .field-control {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .field-stack .field-control {
    width: 100%;
  }
  .field-extra {
    flex-basis: 100%;
  }
</style>

<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    title?: string;
    description?: string;
    children: Snippet;
    footer?: Snippet;
  }

  let { title, description, children, footer }: Props = $props();
</script>

<section class="settings-group">
  {#if title}
    <header class="group-header">
      <h3 class="group-title">{title}</h3>
      {#if description}
        <p class="group-desc">{description}</p>
      {/if}
    </header>
  {/if}
  <div class="group-body">
    {@render children()}
  </div>
  {#if footer}
    <div class="group-footer">
      {@render footer()}
    </div>
  {/if}
</section>

<style>
  .settings-group {
    display: flex;
    flex-direction: column;
  }
  .group-header {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 0 4px 12px;
  }
  .group-title {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
    color: var(--color-text);
    letter-spacing: -0.005em;
  }
  .group-desc {
    margin: 0;
    font-size: 13px;
    color: var(--color-text-secondary);
    line-height: 1.5;
  }
  .group-body {
    display: flex;
    flex-direction: column;
    border: 1px solid var(--color-border);
    border-radius: 8px;
    background: var(--color-surface);
    overflow: hidden;
  }
  .group-body :global(> *:not(:last-child)) {
    border-bottom: 1px solid var(--color-border-subtle);
  }
  .group-footer {
    padding: 12px 4px 0;
  }
</style>

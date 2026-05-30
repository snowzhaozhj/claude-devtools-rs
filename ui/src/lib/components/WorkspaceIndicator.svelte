<script lang="ts">
  import { onMount } from "svelte";
  import { contextStore } from "../stores/context.svelte";
  import { CHECK_SVG, CHEVRON_DOWN, WIFI_OFF_SVG, WIFI_SVG } from "../icons";
  import ConnectionStatusBadge from "./ConnectionStatusBadge.svelte";

  let open = $state(false);
  let root: HTMLDivElement | null = $state(null);

  const activeContext = $derived(
    contextStore.availableContexts.find((ctx) => ctx.id === contextStore.activeContextId)
      ?? contextStore.availableContexts[0]
      ?? { id: "local", kind: "local", label: "Local", status: "connected" },
  );
  const activeLabel = $derived(activeContext.label ?? activeContext.host ?? activeContext.id.replace(/^ssh-/, ""));

  onMount(() => {
    void contextStore.initialize();
    void contextStore.startListening();
    const onPointerDown = (event: PointerEvent) => {
      if (root && !root.contains(event.target as Node)) open = false;
    };
    const onKeydown = (event: KeyboardEvent) => {
      if (event.key === "Escape") open = false;
    };
    document.addEventListener("pointerdown", onPointerDown);
    document.addEventListener("keydown", onKeydown);
    return () => {
      document.removeEventListener("pointerdown", onPointerDown);
      document.removeEventListener("keydown", onKeydown);
    };
  });

  function choose(id: string) {
    open = false;
    void contextStore.switchContext(id);
  }
</script>

{#if contextStore.availableContexts.length > 1}
  <div class="workspace-indicator" bind:this={root}>
    {#if open && !contextStore.switching}
      <div class="menu" role="menu" aria-label="切换工作区">
        <div class="menu-title">工作区</div>
        {#each contextStore.availableContexts as ctx (ctx.id)}
          {@const selected = ctx.id === contextStore.activeContextId}
          {@const label = ctx.label ?? ctx.host ?? ctx.id.replace(/^ssh-/, "")}
          <button
            type="button"
            class="menu-item"
            class:selected
            role="menuitemradio"
            aria-checked={selected}
            onclick={() => choose(ctx.id)}
          >
            <ConnectionStatusBadge contextId={ctx.id} status={ctx.status} showText={false} />
            <span class="item-label">{label}</span>
            {#if selected}
              <span class="check" aria-hidden="true"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{@html CHECK_SVG}</svg></span>
            {/if}
          </button>
        {/each}
      </div>
    {/if}

    <button
      type="button"
      class="pill"
      disabled={contextStore.switching}
      aria-expanded={open}
      aria-haspopup="menu"
      onclick={() => (open = !open)}
    >
      <span class="pill-icon" aria-hidden="true">
        {#if activeContext.kind === "ssh"}
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{@html WIFI_SVG}</svg>
        {:else}
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{@html WIFI_OFF_SVG}</svg>
        {/if}
      </span>
      <span class="pill-label">{activeLabel}</span>
      <span class="chevron" class:open aria-hidden="true">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d={CHEVRON_DOWN} /></svg>
      </span>
    </button>
  </div>
{/if}

<style>
  .workspace-indicator {
    position: fixed;
    right: 16px;
    bottom: 16px;
    z-index: 35;
  }
  .pill {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    min-height: 32px;
    padding: 0 12px;
    border: 1px solid var(--color-border-emphasis);
    border-radius: 999px;
    background: var(--color-surface-raised);
    color: var(--color-text);
    box-shadow: 0 4px 14px rgba(0, 0, 0, 0.12);
    font: inherit;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
  }
  .pill:hover:not(:disabled) {
    background: var(--color-surface-overlay);
  }
  .pill:focus-visible {
    outline: 2px solid var(--color-switch-on);
    outline-offset: 2px;
  }
  .pill:disabled {
    opacity: 0.6;
    cursor: wait;
  }
  .pill-icon,
  .chevron,
  .check {
    display: inline-flex;
    width: 14px;
    height: 14px;
  }
  .pill-icon {
    color: var(--color-success);
  }
  .chevron {
    color: var(--color-text-muted);
    transition: transform 0.15s ease;
  }
  .chevron.open {
    transform: rotate(180deg);
  }
  .pill-icon :global(svg),
  .chevron :global(svg),
  .check :global(svg) {
    width: 14px;
    height: 14px;
  }
  .pill-label {
    max-width: 160px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .menu {
    position: absolute;
    right: 0;
    bottom: calc(100% + 8px);
    width: 240px;
    max-height: 260px;
    overflow-y: auto;
    /* scrollbar-gutter-exempt: 浮层打开即定尺寸，滚动条首帧即在，无生命周期内宽度跳变 */
    padding: 6px;
    border: 1px solid var(--color-border);
    border-radius: 10px;
    background: var(--color-surface-sidebar);
    box-shadow: 0 8px 20px rgba(0, 0, 0, 0.16);
  }
  .menu-title {
    padding: 6px 8px 8px;
    color: var(--color-text-muted);
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  .menu-item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    min-height: 32px;
    padding: 6px 8px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: var(--color-text-secondary);
    font: inherit;
    font-size: 13px;
    text-align: left;
    cursor: pointer;
  }
  .menu-item:hover {
    background: var(--tool-item-hover-bg);
    color: var(--color-text);
  }
  .menu-item:focus-visible {
    outline: 2px solid var(--color-switch-on);
    outline-offset: -2px;
  }
  .menu-item.selected {
    background: var(--color-surface-raised);
    color: var(--color-text);
  }
  .item-label {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .check {
    color: var(--color-text-secondary);
  }
</style>

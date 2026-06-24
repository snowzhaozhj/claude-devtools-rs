<script lang="ts">
  import { resizePanes, getPaneLayout } from "../../lib/tabStore.svelte";
  import { MIN_FRACTION } from "../../lib/paneTypes";

  interface Props {
    leftPaneId: string;
    containerEl: HTMLElement | null;
  }

  let { leftPaneId, containerEl }: Props = $props();

  let active = $state(false);

  const leftPane = $derived(
    getPaneLayout().panes.find((p) => p.id === leftPaneId),
  );
  const paneCount = $derived(getPaneLayout().panes.length);
  const ariaValueNow = $derived(
    leftPane ? Math.round(leftPane.widthFraction * 100) : 50,
  );
  const ariaValueMin = $derived(Math.round(MIN_FRACTION * 100));
  const ariaValueMax = $derived(
    Math.round((1 - MIN_FRACTION * (paneCount - 1)) * 100),
  );

  function onPointerDown(e: PointerEvent) {
    if (e.button !== 0 || !containerEl) return;
    e.preventDefault();
    active = true;
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";

    const rect = containerEl.getBoundingClientRect();
    const startLeft = rect.left;
    const width = rect.width;

    function onMove(ev: PointerEvent) {
      if (width <= 0) return;
      const relFraction = Math.max(0, Math.min(1, (ev.clientX - startLeft) / width));
      resizePanesFromContainerFraction(leftPaneId, relFraction);
    }

    function onUp() {
      active = false;
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
      document.removeEventListener("pointermove", onMove);
      document.removeEventListener("pointerup", onUp);
    }

    document.addEventListener("pointermove", onMove);
    document.addEventListener("pointerup", onUp);
  }

  function resizePanesFromContainerFraction(paneId: string, containerFraction: number): void {
    const layout = getPaneLayout();
    const idx = layout.panes.findIndex((p) => p.id === paneId);
    if (idx === -1) return;
    let cumulative = 0;
    for (let i = 0; i < idx; i++) cumulative += layout.panes[i].widthFraction;
    const newFraction = containerFraction - cumulative;
    resizePanes(paneId, newFraction);
  }

  function onKeyDown(e: KeyboardEvent) {
    if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(e.key)) return;
    e.preventDefault();

    const step = e.shiftKey ? 0.15 : 0.05;
    const layout = getPaneLayout();
    const idx = layout.panes.findIndex((p) => p.id === leftPaneId);
    if (idx === -1 || idx >= layout.panes.length - 1) return;
    const current = layout.panes[idx].widthFraction;
    const combined = current + layout.panes[idx + 1].widthFraction;

    if (e.key === "ArrowLeft") {
      resizePanes(leftPaneId, current - step);
    } else if (e.key === "ArrowRight") {
      resizePanes(leftPaneId, current + step);
    } else if (e.key === "Home") {
      resizePanes(leftPaneId, MIN_FRACTION);
    } else if (e.key === "End") {
      resizePanes(leftPaneId, combined - MIN_FRACTION);
    }
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  class="pane-resize-handle"
  class:active
  role="separator"
  tabindex="0"
  aria-orientation="vertical"
  aria-label="拖动调整面板宽度"
  aria-valuemin={ariaValueMin}
  aria-valuemax={ariaValueMax}
  aria-valuenow={ariaValueNow}
  onpointerdown={onPointerDown}
  onkeydown={onKeyDown}
></div>

<style>
  .pane-resize-handle {
    width: 5px;
    flex-shrink: 0;
    background: transparent;
    cursor: col-resize;
    transition: background 0.15s;
    position: relative;
  }
  .pane-resize-handle::after {
    content: "";
    position: absolute;
    top: 0;
    bottom: 0;
    left: 2px;
    width: 1px;
    background: var(--color-border-emphasis);
    transition: opacity 0.15s;
  }
  .pane-resize-handle:hover,
  .active,
  .pane-resize-handle:focus-visible {
    background: color-mix(in oklch, var(--color-border-emphasis) 60%, transparent);
    outline: none;
  }
  .pane-resize-handle:hover::after,
  .active::after,
  .pane-resize-handle:focus-visible::after {
    opacity: 0;
  }
</style>

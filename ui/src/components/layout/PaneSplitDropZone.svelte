<script lang="ts">
  import { MAX_PANES } from "../../lib/paneTypes";
  import { getDragSource, getHit, isDragging } from "../../lib/dragSession.svelte";
  import { getPaneLayout } from "../../lib/tabStore.svelte";

  interface Props {
    paneId: string;
    side: "left" | "right";
  }

  let { paneId, side }: Props = $props();

  const dragging = $derived(isDragging() && !!getDragSource());
  const canSplit = $derived(getPaneLayout().panes.length < MAX_PANES);
  const hit = $derived.by(() => {
    const h = getHit();
    return !!h && h.kind === "drop-zone" && h.paneId === paneId && h.side === side;
  });
</script>

<!-- 只在拖拽期间且未达 MAX_PANES 时显示；命中时高亮 -->
<div
  class="pane-drop-zone"
  class:visible={dragging && canSplit}
  class:hit
  data-pane-id={paneId}
  data-side={side}
></div>

<style>
  .pane-drop-zone {
    position: absolute;
    top: 36px; /* TabBar 高度下方 */
    bottom: 0;
    width: 48px;
    pointer-events: none;
    z-index: 10;
    transition: background 0.1s;
  }
  .pane-drop-zone.visible {
    pointer-events: auto;
  }
  .pane-drop-zone[data-side="left"] {
    left: 0;
  }
  .pane-drop-zone[data-side="right"] {
    right: 0;
  }
  .pane-drop-zone.visible.hit {
    background: color-mix(in srgb, var(--color-text) 10%, transparent);
    border-left: 2px solid var(--color-border-emphasis);
  }
  .pane-drop-zone[data-side="right"].visible.hit {
    border-left: none;
    border-right: 2px solid var(--color-border-emphasis);
  }
</style>

<script lang="ts">
  import { resizePanes } from "../../lib/tabStore.svelte";

  interface Props {
    leftPaneId: string;
    /** 容器整体宽度（px），用于把 rel-x 转成 fraction */
    containerEl: HTMLElement | null;
  }

  let { leftPaneId, containerEl }: Props = $props();

  let active = $state(false);

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
      // 找到 leftPane 在容器内的起始 X（累加前面所有 pane 的 widthFraction）
      // 近似做法：rel = (ev.clientX - startLeft) / width 作为新"累积 fraction"；
      // 再减去 leftPane 之前所有 pane 的 fraction 得到 leftPane 的新 fraction
      // 简化：假设 ResizeHandle 放在两个 pane 之间，用户的拖动作用于
      // 直接 sibling。这里直接把 "手柄当前位置相对 leftPane 起始点" 的占比
      // 作为新 leftPane.widthFraction。
      const handleEl = (ev.target as HTMLElement).closest<HTMLElement>(
        ".pane-resize-handle",
      );
      void handleEl;
      // 近似：用相对整个容器的 X - leftPane 左侧起始的 fraction
      // 因调用方传的是 containerEl，rel 即 leftPane 起点前的 cumulative fraction + leftPane.fraction
      // 这里直接算从容器左到 pointer 的 fraction，由 tabStore.resizePanes 内部做 clamp
      const relFraction = Math.max(0, Math.min(1, (ev.clientX - startLeft) / width));
      // 交给 resizeAdjacent；实际的"leftPane 起点前 fraction"由调用方通过 paneLayout 已知，
      // 但 resizeAdjacent 只管相邻两 pane 的重分配，传入的 newFraction 是 **leftPane 独占** 的新值。
      // 简化：取 relFraction 作为"容器左侧到 handle 的 fraction"，减去前序 fraction 交给 resizePanes。
      // 这里就把 relFraction 交给 resizePanes，依赖 clamp 防越界。调用方需保证 handle 放在 leftPane 右侧。
      // （TODO: 若后续发现拖拽不直观，再引入精确的 cumulative fraction 计算。）
      const deltaFromLeft = relFraction; // 0..1 of container
      // resizePanes 的语义：newFraction 是 leftPane 单独的 widthFraction
      // 故从 deltaFromLeft 减去 leftPane 之前所有 pane 的 fraction 累积值
      resizePanesFromContainerFraction(leftPaneId, deltaFromLeft);
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

  // resizePanes 需要的是 leftPane 自己的 newFraction；containerFraction 包含
  // 前序 pane 的宽度之和，所以调用前需要减去 "前序 pane fraction 累积"。
  // 这个计算需要 paneLayout 信息，我们通过 tabStore 的公共查询获取。
  import { getPaneLayout } from "../../lib/tabStore.svelte";

  function resizePanesFromContainerFraction(paneId: string, containerFraction: number): void {
    const layout = getPaneLayout();
    const idx = layout.panes.findIndex((p) => p.id === paneId);
    if (idx === -1) return;
    let cumulative = 0;
    for (let i = 0; i < idx; i++) cumulative += layout.panes[i].widthFraction;
    const newFraction = containerFraction - cumulative;
    resizePanes(paneId, newFraction);
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="pane-resize-handle"
  class:active
  onpointerdown={onPointerDown}
></div>

<style>
  .pane-resize-handle {
    width: 6px;
    flex-shrink: 0;
    background: transparent;
    cursor: col-resize;
    transition: background 0.15s;
    position: relative;
  }
  .pane-resize-handle:hover,
  .active {
    background: var(--color-border-emphasis);
  }
</style>

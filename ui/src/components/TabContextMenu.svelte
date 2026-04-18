<script lang="ts">
  import { onMount, onDestroy } from "svelte";

  interface Props {
    x: number;
    y: number;
    canSplit: boolean;
    canCloseOthers: boolean;
    onClose: () => void;
    onCloseTab: () => void;
    onCloseOthers: () => void;
    onSplitLeft: () => void;
    onSplitRight: () => void;
  }

  let {
    x, y, canSplit, canCloseOthers,
    onClose, onCloseTab, onCloseOthers, onSplitLeft, onSplitRight,
  }: Props = $props();

  let menuEl: HTMLDivElement | undefined = $state(undefined);

  const MENU_WIDTH = 200;
  const MENU_HEIGHT = 180;
  const clampedX = $derived(Math.min(x, window.innerWidth - MENU_WIDTH - 8));
  const clampedY = $derived(Math.min(y, window.innerHeight - MENU_HEIGHT - 8));

  function handleMouseDown(e: MouseEvent) {
    if (menuEl && !menuEl.contains(e.target as Node)) onClose();
  }
  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === "Escape") onClose();
  }

  function doAction(fn: () => void) {
    fn();
    onClose();
  }

  onMount(() => {
    document.addEventListener("mousedown", handleMouseDown);
    document.addEventListener("keydown", handleKeyDown);
  });
  onDestroy(() => {
    document.removeEventListener("mousedown", handleMouseDown);
    document.removeEventListener("keydown", handleKeyDown);
  });
</script>

<div class="context-menu" bind:this={menuEl} style="left: {clampedX}px; top: {clampedY}px;">
  <button class="cm-item" onclick={() => doAction(onCloseTab)}>关闭</button>
  <button
    class="cm-item"
    class:cm-item-disabled={!canCloseOthers}
    disabled={!canCloseOthers}
    onclick={() => doAction(onCloseOthers)}
  >
    关闭其他
  </button>
  <div class="cm-sep"></div>
  <button
    class="cm-item"
    class:cm-item-disabled={!canSplit}
    disabled={!canSplit}
    onclick={() => doAction(onSplitLeft)}
    title={canSplit ? "拆分到当前 Pane 左侧" : "已达 Pane 上限"}
  >
    Split Left
  </button>
  <button
    class="cm-item"
    class:cm-item-disabled={!canSplit}
    disabled={!canSplit}
    onclick={() => doAction(onSplitRight)}
    title={canSplit ? "拆分到当前 Pane 右侧（Cmd+\\）" : "已达 Pane 上限"}
  >
    Split Right
  </button>
</div>

<style>
  .context-menu {
    position: fixed;
    z-index: 100;
    min-width: 180px;
    background: var(--color-surface);
    border: 1px solid var(--color-border-emphasis);
    border-radius: 8px;
    padding: 4px;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.15);
  }
  .cm-item {
    display: block;
    width: 100%;
    padding: 7px 12px;
    background: none;
    border: none;
    border-radius: 4px;
    font: inherit;
    font-size: 13px;
    color: var(--color-text);
    text-align: left;
    cursor: pointer;
    transition: background 0.1s;
  }
  .cm-item:hover:not(.cm-item-disabled) {
    background: var(--tool-item-hover-bg);
  }
  .cm-item-disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
  .cm-sep {
    height: 1px;
    margin: 4px 8px;
    background: var(--color-border);
  }
</style>

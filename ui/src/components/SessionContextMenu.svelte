<script lang="ts">
  import { onMount, onDestroy } from "svelte";

  interface Props {
    x: number;
    y: number;
    sessionId: string;
    isPinned: boolean;
    isHidden: boolean;
    onOpenInNewTab: () => void;
    onTogglePin: () => void;
    onToggleHide: () => void;
    onClose: () => void;
  }

  let {
    x, y, sessionId, isPinned, isHidden,
    onOpenInNewTab, onTogglePin, onToggleHide, onClose,
  }: Props = $props();

  let menuEl: HTMLDivElement | undefined = $state(undefined);
  let copyFeedback: string | null = $state(null);

  const MENU_WIDTH = 220;
  const MENU_HEIGHT = 240;
  const clampedX = $derived(Math.min(x, window.innerWidth - MENU_WIDTH - 8));
  const clampedY = $derived(Math.min(y, window.innerHeight - MENU_HEIGHT - 8));

  function handleMouseDown(e: MouseEvent) {
    if (menuEl && !menuEl.contains(e.target as Node)) {
      onClose();
    }
  }

  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === "Escape") onClose();
  }

  function copyText(text: string, label: string) {
    navigator.clipboard.writeText(text);
    copyFeedback = label;
    setTimeout(() => {
      copyFeedback = null;
      onClose();
    }, 600);
  }

  function doAction(action: () => void) {
    action();
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

<div
  class="context-menu"
  bind:this={menuEl}
  style="left: {clampedX}px; top: {clampedY}px;"
>
  <button class="cm-item" onclick={() => doAction(onOpenInNewTab)}>
    在新标签页打开
  </button>
  <div class="cm-sep"></div>
  <button class="cm-item" onclick={() => doAction(onTogglePin)}>
    {isPinned ? "取消置顶" : "置顶会话"}
  </button>
  <button class="cm-item" onclick={() => doAction(onToggleHide)}>
    {isHidden ? "取消隐藏" : "隐藏会话"}
  </button>
  <div class="cm-sep"></div>
  <button
    class="cm-item"
    onclick={() => copyText(sessionId, "id")}
  >
    {copyFeedback === "id" ? "已复制!" : "复制 Session ID"}
  </button>
  <button
    class="cm-item"
    onclick={() => copyText(`claude --resume ${sessionId}`, "cmd")}
  >
    {copyFeedback === "cmd" ? "已复制!" : "复制恢复命令"}
  </button>
</div>

<style>
  .context-menu {
    position: fixed;
    z-index: 100;
    min-width: 200px;
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

  .cm-item:hover {
    background: var(--tool-item-hover-bg);
  }

  .cm-sep {
    height: 1px;
    margin: 4px 8px;
    background: var(--color-border);
  }
</style>

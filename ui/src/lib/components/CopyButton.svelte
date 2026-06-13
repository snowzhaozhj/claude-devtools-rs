<script lang="ts">
  import { COPY_SVG, CHECK } from "../icons";
  import { onDestroy } from "svelte";

  interface Props {
    text: string;
  }

  let { text }: Props = $props();
  let copied = $state(false);
  let timeoutId: ReturnType<typeof setTimeout> | undefined;

  onDestroy(() => {
    if (timeoutId !== undefined) clearTimeout(timeoutId);
  });

  async function copy() {
    const copyText = text ?? "";
    if (!copyText) return;
    try {
      await navigator.clipboard.writeText(copyText);
      copied = true;
      if (timeoutId !== undefined) clearTimeout(timeoutId);
      timeoutId = setTimeout(() => (copied = false), 2000);
    } catch (e) {
      console.warn("[CopyButton] clipboard write failed:", e);
    }
  }
</script>

<button
  class="copy-btn"
  class:copied
  onmousedown={(e) => { if (e.button === 0) e.preventDefault(); }}
  onclick={copy}
  aria-label={copied ? "已复制" : "复制"}
>
  <svg
    width="14"
    height="14"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="2"
    stroke-linecap="round"
    stroke-linejoin="round"
  >
    {#if copied}
      <path d={CHECK} />
    {:else}
      {@html COPY_SVG}
    {/if}
  </svg>
</button>

<style>
  .copy-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: none;
    border-radius: 4px;
    cursor: pointer;
    padding: 4px;
    color: var(--color-text-muted);
    background: transparent;
    flex-shrink: 0;
    transition:
      color 0.15s,
      background 0.15s;
  }

  .copy-btn:hover {
    color: var(--color-text);
    background: var(--color-hover-bg, rgba(127, 127, 127, 0.1));
  }

  .copy-btn.copied {
    color: var(--color-success, #22c55e);
  }
</style>

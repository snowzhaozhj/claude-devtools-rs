<script lang="ts">
  import { COPY_SVG, CHECK } from "../icons";

  interface Props {
    text: string;
    mode?: "inline" | "overlay";
  }

  let { text, mode = "inline" }: Props = $props();
  let copied = $state(false);

  async function copy() {
    try {
      await navigator.clipboard.writeText(text);
      copied = true;
      setTimeout(() => (copied = false), 2000);
    } catch {
      /* silent */
    }
  }
</script>

<button
  class="copy-btn"
  class:copy-btn-overlay={mode === "overlay"}
  class:copy-btn-inline={mode === "inline"}
  class:copied
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
    transition:
      color 0.15s,
      opacity 0.15s,
      background 0.15s;
  }

  .copy-btn:hover {
    color: var(--color-text);
    background: var(--color-hover-bg, rgba(127, 127, 127, 0.1));
  }

  .copy-btn.copied {
    color: var(--color-success, #22c55e);
  }

  .copy-btn-overlay {
    position: absolute;
    top: 6px;
    right: 6px;
    opacity: 0;
    pointer-events: none;
  }

  :global(.copy-host:hover) .copy-btn-overlay,
  .copy-btn-overlay:focus-visible {
    opacity: 1;
    pointer-events: auto;
  }
</style>

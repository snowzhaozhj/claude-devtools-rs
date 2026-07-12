<script lang="ts">
  import { COPY_SVG, CHECK } from "../icons";
  import { onDestroy } from "svelte";

  interface Props {
    text: string;
    /** 可选可见文字标签（如"复制全文"）；省略则仅图标。 */
    label?: string;
    /** 禁用态（完整原文未就绪 / 失败）：不响应点击，SHALL NOT 复制可见片段。 */
    disabled?: boolean;
    /** 自定义 aria-label / title；省略则用默认"复制"。 */
    ariaLabel?: string;
  }

  let { text, label, disabled = false, ariaLabel }: Props = $props();
  let copied = $state(false);
  let timeoutId: ReturnType<typeof setTimeout> | undefined;

  onDestroy(() => {
    if (timeoutId !== undefined) clearTimeout(timeoutId);
  });

  async function copy() {
    if (disabled) return;
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
  class:has-label={label}
  {disabled}
  onmousedown={(e) => { if (e.button === 0) e.preventDefault(); }}
  onclick={copy}
  aria-label={ariaLabel ?? (copied ? "已复制" : label ?? "复制")}
  title={ariaLabel ?? label}
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
  {#if label}<span class="copy-btn-label">{copied ? "已复制" : label}</span>{/if}
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
    background: var(--color-surface-hover);
  }

  .copy-btn:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .copy-btn:disabled:hover {
    color: var(--color-text-muted);
    background: transparent;
  }

  .copy-btn-label {
    font-size: 11px;
    margin-left: 4px;
  }

  .copy-btn.copied {
    color: var(--color-success, #15803d);
  }
</style>

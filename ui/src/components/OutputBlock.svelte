<script lang="ts">
  import { highlightCode } from "../lib/render";
  import CopyButton from "../lib/components/CopyButton.svelte";

  const highlightCache = new Map<string, string>();
  const HIGHLIGHT_CACHE_CAP = 128;

  interface Props {
    code: string;
    lang?: string;
    isError?: boolean;
    maxHeight?: number;
    label?: string;
  }

  let { code, lang = "json", isError = false, maxHeight = 384, label }: Props = $props();

  function cachedHighlight(value: string, language: string): string {
    const key = `${language}\0${value.length}\0${value}`;
    const hit = highlightCache.get(key);
    if (hit !== undefined) {
      highlightCache.delete(key);
      highlightCache.set(key, hit);
      return hit;
    }
    const result = highlightCode(value, language);
    if (highlightCache.size >= HIGHLIGHT_CACHE_CAP) {
      const first = highlightCache.keys().next().value;
      if (first !== undefined) highlightCache.delete(first);
    }
    highlightCache.set(key, result);
    return result;
  }

  const highlighted = $derived(cachedHighlight(code, lang));
</script>

<div class="output-block" class:output-block-err={isError}>
  {#if label}
    <div class="output-header">
      <span class="output-label">{label}</span>
      <CopyButton text={code} />
    </div>
    <pre class="output-pre" style:max-height="{maxHeight}px"><code>{@html highlighted}</code></pre>
  {:else}
    <div class="output-block-inline">
      <pre class="output-pre output-pre-standalone" style:max-height="{maxHeight}px"><code>{@html highlighted}</code></pre>
      <div class="copy-float">
        <CopyButton text={code} />
      </div>
    </div>
  {/if}
</div>

<style>
  .output-block {
    min-width: 0;
  }

  .output-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 4px 12px 0;
    background: var(--code-bg);
    border: 1px solid var(--code-border);
    border-bottom: none;
    border-radius: 6px 6px 0 0;
  }

  .output-block-inline {
    position: relative;
  }

  .output-pre-standalone {
    border-top: 1px solid var(--code-border);
    border-radius: 6px;
  }

  .copy-float {
    position: absolute;
    top: 4px;
    right: 4px;
    opacity: 0;
    transition: opacity 0.15s;
  }

  .output-block-inline:hover .copy-float {
    opacity: 1;
  }

  .output-label {
    font-size: 9px;
    font-weight: 600;
    color: var(--color-text-muted);
    letter-spacing: 1px;
    text-transform: uppercase;
  }

  .output-pre {
    min-width: 0;
    font-size: 12px;
    font-family: var(--font-mono);
    color: var(--color-text-secondary);
    background: var(--code-bg);
    border: 1px solid var(--code-border);
    border-top: none;
    border-radius: 0 0 6px 6px;
    padding: 10px 12px;
    margin: 0;
    white-space: pre;
    overflow-x: auto;
    overflow-y: auto;
    /* scrollbar-gutter-exempt: 等宽输出块首帧定型 + 横向滚动为主，竖向滚动条不影响可读性 */
    line-height: 1.5;
  }

  .output-pre :global(code) {
    display: block;
    background: none;
    padding: 0;
    color: inherit;
    font: inherit;
    border-radius: 0;
  }

  .output-block-err .output-label {
    color: var(--tool-result-error-text);
  }

  .output-block-err .output-pre {
    color: var(--tool-result-error-text);
    background: var(--tool-result-error-bg);
    border-color: rgba(239, 68, 68, 0.2);
    border-color: color-mix(in oklch, var(--color-danger-bright) 20%, transparent);
  }

  .output-block-err .output-header {
    background: var(--tool-result-error-bg);
    border-color: rgba(239, 68, 68, 0.2);
    border-color: color-mix(in oklch, var(--color-danger-bright) 20%, transparent);
  }

  .output-block-err .output-pre-standalone {
    border-color: rgba(239, 68, 68, 0.2);
    border-color: color-mix(in oklch, var(--color-danger-bright) 20%, transparent);
  }
</style>

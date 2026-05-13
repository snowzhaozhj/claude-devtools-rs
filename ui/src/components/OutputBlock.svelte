<script lang="ts">
  import { highlightCode } from "../lib/render";

  const highlightCache = new Map<string, string>();
  const HIGHLIGHT_CACHE_CAP = 128;

  interface Props {
    code: string;
    lang?: string;
    isError?: boolean;
    maxHeight?: number;
  }

  let { code, lang = "json", isError = false, maxHeight = 384 }: Props = $props();

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
  <pre class="output-pre" style:max-height="{maxHeight}px"><code>{@html highlighted}</code></pre>
</div>

<style>
  .output-pre {
    font-size: 12px;
    font-family: var(--font-mono);
    color: var(--color-text-secondary);
    background: var(--code-bg);
    border: 1px solid var(--code-border);
    border-radius: 6px;
    padding: 10px 12px;
    margin: 0;
    white-space: pre;
    overflow-x: auto;
    overflow-y: auto;
    line-height: 1.5;
  }

  .output-pre :global(code) {
    background: none;
    padding: 0;
    color: inherit;
    font: inherit;
    border-radius: 0;
  }

  .output-block-err .output-pre {
    color: var(--tool-result-error-text);
    background: var(--tool-result-error-bg);
    border-color: rgba(239, 68, 68, 0.2);
  }

  /* 语法高亮 token 颜色统一在 app.css 的 .hljs-* 全局规则里 */
</style>

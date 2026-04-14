<script lang="ts">
  import { highlightCode } from "../lib/render";

  interface Props {
    code: string;
    lang?: string;
    isError?: boolean;
    maxHeight?: number;
  }

  let { code, lang = "json", isError = false, maxHeight = 384 }: Props = $props();
</script>

<div class="output-block" class:output-block-err={isError}>
  <pre class="output-pre" style:max-height="{maxHeight}px"><code>{@html highlightCode(code, lang)}</code></pre>
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

  /* 语法高亮 token */
  .output-pre :global(.hljs-string) { color: var(--syntax-string); }
  .output-pre :global(.hljs-number) { color: var(--syntax-number); }
  .output-pre :global(.hljs-keyword),
  .output-pre :global(.hljs-literal) { color: var(--syntax-keyword); }
  .output-pre :global(.hljs-attr) { color: var(--code-filename); }
  .output-pre :global(.hljs-comment) { color: var(--syntax-comment); }
  .output-pre :global(.hljs-function),
  .output-pre :global(.hljs-title) { color: var(--syntax-function); }
  .output-pre :global(.hljs-built_in) { color: var(--syntax-type); }
  .output-pre :global(.hljs-type) { color: var(--syntax-type); }
  .output-pre :global(.hljs-punctuation) { color: var(--color-text-muted); }
</style>

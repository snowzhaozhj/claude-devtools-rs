<script lang="ts">
  import type { ToolExecution } from "../../lib/api";
  import { toolOutputText, truncate } from "../../lib/toolHelpers";
  import { highlightCode } from "../../lib/render";

  interface Props {
    exec: ToolExecution;
  }

  let { exec }: Props = $props();
  let fullOutputExpanded = $state(false);

  const inputStr = $derived(JSON.stringify(exec.input, null, 2));
  const outputStr = $derived(toolOutputText(exec.output));
  const outputTrunc = $derived(truncate(outputStr, 2000));
</script>

<div class="default-viewer">
  <div class="viewer-section">
    <span class="viewer-label">INPUT</span>
    <pre class="code-block"><code>{@html highlightCode(inputStr, "json")}</code></pre>
  </div>

  {#if outputStr}
    <div class="viewer-section">
      <span class="viewer-label" class:viewer-label-err={exec.isError}>
        {exec.isError ? "ERROR" : "OUTPUT"}
      </span>
      <pre class="code-block" class:code-block-err={exec.isError}><code>{@html highlightCode(outputTrunc.text)}</code></pre>

      {#if outputTrunc.truncated}
        <button class="expand-btn" onclick={() => fullOutputExpanded = !fullOutputExpanded}>
          {fullOutputExpanded ? "收起" : `展开全部 (${outputStr.length} chars)`}
        </button>
        {#if fullOutputExpanded}
          <pre class="code-block"><code>{@html highlightCode(outputStr)}</code></pre>
        {/if}
      {/if}
    </div>
  {/if}
</div>

<style>
  .default-viewer {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .viewer-section {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .viewer-label {
    font-size: 9px;
    font-weight: 600;
    color: var(--color-text-muted);
    letter-spacing: 1px;
    text-transform: uppercase;
  }

  .viewer-label-err {
    color: var(--tool-result-error-text);
  }

  .code-block {
    font-size: 12px;
    font-family: var(--font-mono);
    color: var(--color-text-secondary);
    background: var(--code-bg);
    border: 1px solid var(--code-border);
    border-radius: 6px;
    padding: 10px 12px;
    margin: 0;
    white-space: pre-wrap;
    overflow-x: auto;
    max-height: 300px;
    overflow-y: auto;
    line-height: 1.5;
  }

  .code-block :global(code) {
    background: none;
    padding: 0;
    color: inherit;
    font: inherit;
    border-radius: 0;
  }

  .code-block-err {
    color: var(--tool-result-error-text);
    background: var(--tool-result-error-bg);
    border-color: rgba(239, 68, 68, 0.2);
  }

  /* Syntax tokens */
  .code-block :global(.hljs-string) { color: var(--syntax-string); }
  .code-block :global(.hljs-number) { color: var(--syntax-number); }
  .code-block :global(.hljs-keyword),
  .code-block :global(.hljs-literal) { color: var(--syntax-keyword); }
  .code-block :global(.hljs-attr) { color: var(--code-filename); }
  .code-block :global(.hljs-comment) { color: var(--syntax-comment); }
  .code-block :global(.hljs-function),
  .code-block :global(.hljs-title) { color: var(--syntax-function); }
  .code-block :global(.hljs-built_in) { color: var(--syntax-type); }
  .code-block :global(.hljs-type) { color: var(--syntax-type); }
  .code-block :global(.hljs-punctuation) { color: var(--color-text-muted); }

  .expand-btn {
    background: none;
    border: none;
    color: var(--prose-link);
    font-size: 12px;
    cursor: pointer;
    padding: 2px 0;
    align-self: flex-start;
  }

  .expand-btn:hover {
    text-decoration: underline;
  }
</style>

<script lang="ts">
  import type { ToolExecution } from "../../lib/api";
  import { toolOutputText, truncate } from "../../lib/toolHelpers";
  import { highlightCode } from "../../lib/render";

  interface Props {
    exec: ToolExecution;
  }

  let { exec }: Props = $props();
  let fullOutputExpanded = $state(false);

  const input = $derived(exec.input as Record<string, unknown>);
  const command = $derived(String(input?.command ?? ""));
  const outputStr = $derived(toolOutputText(exec.output));
  const outputTrunc = $derived(truncate(outputStr, 2000));
</script>

<div class="bash-viewer">
  <!-- Command -->
  <div class="bash-command">
    <span class="bash-prompt">$</span>
    <code class="bash-cmd">{command}</code>
  </div>

  <!-- Output -->
  {#if outputStr}
    <div class="bash-output-section">
      <span class="output-label" class:output-label-err={exec.isError}>
        {exec.isError ? "ERROR" : "OUTPUT"}
      </span>
      <pre class="bash-output" class:bash-output-err={exec.isError}><code>{@html highlightCode(outputTrunc.text, "bash")}</code></pre>

      {#if outputTrunc.truncated}
        <button class="expand-btn" onclick={() => fullOutputExpanded = !fullOutputExpanded}>
          {fullOutputExpanded ? "收起" : `展开全部 (${outputStr.length} chars)`}
        </button>
        {#if fullOutputExpanded}
          <pre class="bash-output"><code>{@html highlightCode(outputStr, "bash")}</code></pre>
        {/if}
      {/if}
    </div>
  {/if}
</div>

<style>
  .bash-viewer {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .bash-command {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    padding: 10px 12px;
    background: var(--code-bg);
    border: 1px solid var(--code-border);
    border-radius: 6px;
  }

  .bash-prompt {
    color: var(--syntax-string);
    font-family: var(--font-mono);
    font-size: 12px;
    font-weight: 600;
    flex-shrink: 0;
    user-select: none;
  }

  .bash-cmd {
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--color-text);
    background: none;
    padding: 0;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .bash-output-section {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .output-label {
    font-size: 9px;
    font-weight: 600;
    color: var(--color-text-muted);
    letter-spacing: 1px;
    text-transform: uppercase;
  }

  .output-label-err {
    color: var(--tool-result-error-text);
  }

  .bash-output {
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

  .bash-output :global(code) {
    background: none;
    padding: 0;
    color: inherit;
    font: inherit;
    border-radius: 0;
  }

  .bash-output-err {
    color: var(--tool-result-error-text);
    background: var(--tool-result-error-bg);
    border-color: rgba(239, 68, 68, 0.2);
  }

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

<script lang="ts">
  import type { ToolExecution } from "../../lib/api";
  import { shortenPath, getLanguageFromPath } from "../../lib/toolHelpers";
  import { highlightCode } from "../../lib/render";

  interface Props {
    exec: ToolExecution;
  }

  let { exec }: Props = $props();

  const input = $derived(exec.input as Record<string, unknown>);
  const filePath = $derived(String(input?.file_path ?? input?.filePath ?? ""));
  const content = $derived(String(input?.content ?? ""));
  const language = $derived(getLanguageFromPath(filePath));
</script>

<div class="write-viewer">
  <div class="write-header">
    <span class="write-icon">W</span>
    <span class="write-path">{shortenPath(filePath)}</span>
    <span class="write-badge">NEW</span>
  </div>

  {#if content}
    <pre class="write-code"><code>{@html highlightCode(content, language)}</code></pre>
  {/if}
</div>

<style>
  .write-viewer {
    border: 1px solid var(--diff-added-border);
    border-radius: 8px;
    overflow: hidden;
    background: var(--diff-added-bg);
  }

  .write-header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    border-bottom: 1px solid rgba(34, 197, 94, 0.2);
  }

  .write-icon {
    font-size: 12px;
  }

  .write-path {
    font-size: 12px;
    font-family: var(--font-mono);
    color: var(--diff-added-text);
    font-weight: 500;
  }

  .write-badge {
    font-size: 9px;
    font-weight: 600;
    color: var(--diff-added-text);
    background: rgba(34, 197, 94, 0.2);
    padding: 1px 6px;
    border-radius: 4px;
    letter-spacing: 0.5px;
  }

  .write-code {
    font-size: 12px;
    font-family: var(--font-mono);
    color: var(--diff-added-text);
    padding: 10px 12px;
    margin: 0;
    white-space: pre;
    overflow-x: auto;
    max-height: 400px;
    overflow-y: auto;
    line-height: 1.5;
  }

  .write-code :global(code) {
    background: none;
    padding: 0;
    color: inherit;
    font: inherit;
    border-radius: 0;
  }
</style>

<script lang="ts">
  import type { ToolExecution } from "../../lib/api";
  import { shortenPath } from "../../lib/toolHelpers";
  import { highlightCode } from "../../lib/render";

  interface Props {
    exec: ToolExecution;
  }

  let { exec }: Props = $props();

  const input = $derived(exec.input as Record<string, unknown>);
  const filePath = $derived(String(input?.file_path ?? input?.filePath ?? ""));
  const oldString = $derived(String(input?.old_string ?? input?.oldString ?? ""));
  const newString = $derived(String(input?.new_string ?? input?.newString ?? ""));
</script>

<div class="edit-viewer">
  <div class="edit-file-path">{shortenPath(filePath)}</div>

  {#if oldString}
    <div class="diff-section diff-removed">
      <span class="diff-label">- REMOVED</span>
      <pre class="diff-code"><code>{@html highlightCode(oldString)}</code></pre>
    </div>
  {/if}

  {#if newString}
    <div class="diff-section diff-added">
      <span class="diff-label">+ ADDED</span>
      <pre class="diff-code"><code>{@html highlightCode(newString)}</code></pre>
    </div>
  {/if}
</div>

<style>
  .edit-viewer {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .edit-file-path {
    font-size: 12px;
    font-family: var(--font-mono);
    color: var(--code-filename);
  }

  .diff-section {
    border-radius: 6px;
    overflow: hidden;
  }

  .diff-label {
    display: block;
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 1px;
    padding: 4px 10px;
  }

  .diff-removed {
    background: var(--diff-removed-bg);
    border: 1px solid rgba(239, 68, 68, 0.2);
  }

  .diff-removed .diff-label {
    color: var(--diff-removed-text);
  }

  .diff-added {
    background: var(--diff-added-bg);
    border: 1px solid rgba(34, 197, 94, 0.2);
  }

  .diff-added .diff-label {
    color: var(--diff-added-text);
  }

  .diff-code {
    font-size: 12px;
    font-family: var(--font-mono);
    padding: 8px 10px;
    margin: 0;
    white-space: pre;
    overflow-x: auto;
    line-height: 1.5;
    max-height: 300px;
    overflow-y: auto;
  }

  .diff-removed .diff-code {
    color: var(--diff-removed-text);
  }

  .diff-removed .diff-code :global(code) {
    background: none;
    padding: 0;
    color: inherit;
    font: inherit;
  }

  .diff-added .diff-code {
    color: var(--diff-added-text);
  }

  .diff-added .diff-code :global(code) {
    background: none;
    padding: 0;
    color: inherit;
    font: inherit;
  }
</style>

<script lang="ts">
  import type { ToolExecution } from "../../lib/api";
  import { shortenPath, getLanguageFromPath } from "../../lib/toolHelpers";
  import { highlightCode, renderMarkdown } from "../../lib/render";

  interface Props {
    exec: ToolExecution;
  }

  let { exec }: Props = $props();

  const input = $derived(exec.input as Record<string, unknown>);
  const filePath = $derived(String(input?.file_path ?? input?.filePath ?? ""));
  const content = $derived(String(input?.content ?? ""));
  const language = $derived(getLanguageFromPath(filePath));
  const isMarkdown = $derived(language === "markdown");

  // .md 默认 preview，可切 code（对齐原版 WriteToolViewer.tsx 第 59-62 行）
  let viewMode = $state<"preview" | "code">("preview");
</script>

<div class="write-viewer">
  <div class="write-header">
    <span class="write-icon">W</span>
    <span class="write-path">{shortenPath(filePath)}</span>
    <span class="write-badge">NEW</span>
    {#if isMarkdown}
      <span class="write-spacer"></span>
      <button
        class="view-toggle"
        onclick={() => (viewMode = viewMode === "preview" ? "code" : "preview")}
      >
        {viewMode === "preview" ? "源码" : "预览"}
      </button>
    {/if}
  </div>

  {#if content}
    {#if isMarkdown && viewMode === "preview"}
      <div class="md-preview">{@html renderMarkdown(content)}</div>
    {:else}
      <pre class="write-code"><code>{@html highlightCode(content, language)}</code></pre>
    {/if}
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

  .write-spacer {
    flex: 1;
  }

  .view-toggle {
    font-size: 11px;
    color: var(--color-text-muted);
    background: none;
    border: 1px solid var(--color-border-emphasis);
    border-radius: 4px;
    padding: 2px 8px;
    cursor: pointer;
    font-family: inherit;
    transition:
      color 0.15s,
      border-color 0.15s;
  }

  .view-toggle:hover {
    color: var(--color-text);
    border-color: var(--color-text-muted);
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

  .md-preview {
    padding: 12px 16px;
    max-height: 500px;
    overflow: auto;
    background: var(--code-bg);
    color: var(--color-text);
    font-size: 13px;
    line-height: 1.6;
  }

  .md-preview :global(h1),
  .md-preview :global(h2),
  .md-preview :global(h3),
  .md-preview :global(h4) {
    margin: 0.8em 0 0.4em;
    font-weight: 600;
  }

  .md-preview :global(p) {
    margin: 0.5em 0;
  }

  .md-preview :global(code) {
    background: var(--code-inline-bg, rgba(127, 127, 127, 0.15));
    padding: 1px 4px;
    border-radius: 3px;
    font-family: var(--font-mono);
    font-size: 12px;
  }

  .md-preview :global(pre code) {
    background: none;
    padding: 0;
  }

  .md-preview :global(pre) {
    background: var(--code-bg, #1e1e1e);
    border: 1px solid var(--code-border);
    border-radius: 4px;
    padding: 8px 12px;
    overflow-x: auto;
  }

  .md-preview :global(ul),
  .md-preview :global(ol) {
    padding-left: 1.5em;
    margin: 0.5em 0;
  }

  .md-preview :global(blockquote) {
    border-left: 3px solid var(--color-border-emphasis);
    padding-left: 12px;
    margin: 0.5em 0;
    color: var(--color-text-muted);
  }

  .md-preview :global(a) {
    color: var(--color-link, #4a9eff);
    text-decoration: none;
  }

  .md-preview :global(a:hover) {
    text-decoration: underline;
  }

  .md-preview :global(table) {
    border-collapse: collapse;
    margin: 0.5em 0;
  }

  .md-preview :global(th),
  .md-preview :global(td) {
    border: 1px solid var(--code-border);
    padding: 4px 8px;
  }
</style>

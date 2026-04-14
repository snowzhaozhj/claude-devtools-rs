<script lang="ts">
  import type { ToolExecution } from "../../lib/api";
  import { toolOutputText, shortenPath, getLanguageFromPath, getFileName } from "../../lib/toolHelpers";
  import { highlightCode } from "../../lib/render";

  interface Props {
    exec: ToolExecution;
  }

  let { exec }: Props = $props();
  let copied = $state(false);

  const input = $derived(exec.input as Record<string, unknown>);
  const filePath = $derived(String(input?.file_path ?? input?.filePath ?? ""));
  const fileName = $derived(getFileName(filePath));
  const language = $derived(getLanguageFromPath(filePath));
  const outputText = $derived(toolOutputText(exec.output));
  const lines = $derived(outputText.split("\n"));

  async function copyContent() {
    try {
      await navigator.clipboard.writeText(outputText);
      copied = true;
      setTimeout(() => copied = false, 2000);
    } catch { /* ignore */ }
  }
</script>

<div class="read-viewer">
  <!-- File header -->
  <div class="file-header">
    <span class="file-icon">F</span>
    <span class="file-name">{shortenPath(filePath)}</span>
    <span class="file-lang">{language}</span>
    <span class="file-spacer"></span>
    <button class="copy-btn" onclick={copyContent}>
      {copied ? "✓ 已复制" : "复制"}
    </button>
  </div>

  <!-- Code with line numbers -->
  <div class="code-container">
    <pre class="code-content"><code>{#each lines as line, i}<span class="line-number">{i + 1}</span><span class="line-content">{@html highlightCode(line, language)}</span>
{/each}</code></pre>
  </div>
</div>

<style>
  .read-viewer {
    border: 1px solid var(--code-border);
    border-radius: 8px;
    overflow: hidden;
  }

  .file-header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    background: var(--code-header-bg);
    border-bottom: 1px solid var(--code-border);
  }

  .file-icon {
    font-size: 12px;
  }

  .file-name {
    font-size: 12px;
    font-family: var(--font-mono);
    color: var(--code-filename);
    font-weight: 500;
  }

  .file-lang {
    font-size: 10px;
    color: var(--tag-text);
    background: var(--tag-bg);
    border: 1px solid var(--tag-border);
    padding: 1px 6px;
    border-radius: 4px;
  }

  .file-spacer {
    flex: 1;
  }

  .copy-btn {
    font-size: 11px;
    color: var(--color-text-muted);
    background: none;
    border: 1px solid var(--color-border-emphasis);
    border-radius: 4px;
    padding: 2px 8px;
    cursor: pointer;
    font-family: inherit;
    transition: color 0.15s, border-color 0.15s;
  }

  .copy-btn:hover {
    color: var(--color-text);
    border-color: var(--color-text-muted);
  }

  .code-container {
    max-height: 400px;
    overflow: auto;
    background: var(--code-bg);
  }

  .code-content {
    margin: 0;
    padding: 10px 0;
    font-size: 12px;
    font-family: var(--font-mono);
    line-height: 1.5;
    color: var(--color-text-secondary);
  }

  .code-content :global(code) {
    background: none;
    padding: 0;
    color: inherit;
    font: inherit;
    border-radius: 0;
  }

  .line-number {
    display: inline-block;
    width: 48px;
    padding-right: 12px;
    text-align: right;
    color: var(--code-line-number);
    user-select: none;
    flex-shrink: 0;
  }

  .line-content {
    white-space: pre;
  }

  /* Syntax tokens */
  .code-content :global(.hljs-string) { color: var(--syntax-string); }
  .code-content :global(.hljs-number) { color: var(--syntax-number); }
  .code-content :global(.hljs-keyword),
  .code-content :global(.hljs-literal) { color: var(--syntax-keyword); }
  .code-content :global(.hljs-attr) { color: var(--code-filename); }
  .code-content :global(.hljs-comment) { color: var(--syntax-comment); }
  .code-content :global(.hljs-function),
  .code-content :global(.hljs-title) { color: var(--syntax-function); }
  .code-content :global(.hljs-built_in) { color: var(--syntax-type); }
  .code-content :global(.hljs-type) { color: var(--syntax-type); }
</style>

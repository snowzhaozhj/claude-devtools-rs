<script lang="ts">
  import type { ToolExecution } from "../../lib/api";
  import { toolOutputText, shortenPath, getLanguageFromPath, getFileName } from "../../lib/toolHelpers";
  import { highlightCode, renderMarkdown } from "../../lib/render";

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
  const parsedLines = $derived(parseReadLines(outputText));
  const isMarkdown = $derived(language === "markdown");

  /**
   * 解析 Read 工具输出为 `{num, text}[]`。
   *
   * Claude Read 工具的 raw `tool_result.content` 是 cat -n 风格的 `<num>\t<text>` 前缀
   * （见 ../../../docs JSONL fixtures），如果直接渲染会和 CSS `::before data-line` 双重显示
   * 行号。本函数：
   * - 严格检测每行是否匹配 `^\s*\d+\t.*$`（trailing newline 产生的尾部空字符串忽略）
   * - 全部匹配 → strip 前缀，行号取自前缀本身（保留真实文件行号，对齐原版 CodeBlockViewer 的
   *   `startLine` 语义）
   * - 任一行不匹配 → fallback 用 i+1（兼容非 Read 路径或后端 enriched 后的干净内容）
   */
  function parseReadLines(raw: string): { num: number; text: string }[] {
    if (raw.length === 0) return [];
    // 去掉单一 trailing newline 避免 split 末尾产生空字符串干扰检测
    const cleaned = raw.endsWith("\n") ? raw.slice(0, -1) : raw;
    const rawLines = cleaned.split("\n");
    const catN = /^\s*(\d+)\t(.*)$/;
    const parsed: { num: number; text: string }[] = [];
    for (const l of rawLines) {
      const m = catN.exec(l);
      if (!m) {
        return rawLines.map((text, i) => ({ num: i + 1, text }));
      }
      parsed.push({ num: Number(m[1]), text: m[2] });
    }
    return parsed;
  }

  // .md 默认 preview，可切 code（对齐原版 ReadToolViewer.tsx 第 90-98 行）
  let viewMode = $state<"preview" | "code">("preview");

  /** 取 strip 后的纯文本（无 cat -n 前缀）；空内容时退回 outputText。 */
  const cleanText = $derived(
    parsedLines.length > 0 ? parsedLines.map((p) => p.text).join("\n") : outputText
  );

  /** 复制按钮：用 strip 后的纯文本。 */
  async function copyContent() {
    try {
      await navigator.clipboard.writeText(cleanText);
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
    {#if isMarkdown}
      <button
        class="view-toggle"
        onclick={() => (viewMode = viewMode === "preview" ? "code" : "preview")}
      >
        {viewMode === "preview" ? "源码" : "预览"}
      </button>
    {/if}
    <button class="copy-btn" onclick={copyContent}>
      {copied ? "✓ 已复制" : "复制"}
    </button>
  </div>

  {#if isMarkdown && viewMode === "preview"}
    <!-- 用 strip 后的纯文本渲染：raw outputText 含 cat -n 前缀会让 markdown 标记失效 -->
    <div class="md-preview">{@html renderMarkdown(cleanText)}</div>
  {:else}
    <!-- Code with line numbers (line numbers are CSS ::before, not part of clipboard text) -->
    <div class="code-container">
      <pre class="code-content"><code>{#each parsedLines as p (p.num)}<span class="line" data-line={p.num}>{@html highlightCode(p.text, language)}
</span>{/each}</code></pre>
    </div>
  {/if}
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

  .copy-btn,
  .view-toggle {
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

  .copy-btn:hover,
  .view-toggle:hover {
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

  .line {
    display: block;
    position: relative;
    padding-left: 60px;
    white-space: pre;
  }

  .line::before {
    content: attr(data-line);
    position: absolute;
    left: 0;
    width: 48px;
    padding-right: 12px;
    text-align: right;
    color: var(--code-line-number);
    user-select: none;
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

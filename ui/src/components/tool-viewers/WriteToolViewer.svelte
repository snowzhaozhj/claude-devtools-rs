<script lang="ts">
  import type { ToolExecution } from "../../lib/api";
  import { shortenPath, getLanguageFromPath } from "../../lib/toolHelpers";
  import { highlightCode, renderMarkdown } from "../../lib/render";
  import { lightHighlightLine } from "../../lib/lightSyntax";
  import { contextMenu } from "../../lib/contextMenu.svelte";
  import { buildFileToolItems, type MenuItemContext } from "../../lib/contextMenu/menu-items";
  import { getMenuSettings } from "../../lib/contextMenu/settings.svelte";
  import { getMenuItemDispatch } from "../../lib/contextMenu/dispatch";
  import CopyButton from "../../lib/components/CopyButton.svelte";
  import { formatBytes } from "../../lib/formatters";
  import { adaptiveScrollViewport } from "../../lib/adaptiveViewport";
  import { classifyText, countLines, utf8ByteLength, sliceLineIndices } from "../../lib/outputSizing";

  interface Props {
    exec: ToolExecution;
    sessionId?: string;
    projectId?: string;
  }

  let { exec, sessionId = "", projectId = "" }: Props = $props();

  const input = $derived(exec.input as Record<string, unknown>);
  const filePath = $derived(String(input?.file_path ?? input?.filePath ?? ""));
  const content = $derived(String(input?.content ?? ""));
  const language = $derived(getLanguageFromPath(filePath));
  const isMarkdown = $derived(language === "markdown");
  const lines = $derived(content.split("\n"));
  const useLightHighlight = $derived(lines.length > 250 || content.length > 40_000);
  const highlightLine = $derived(useLightHighlight ? lightHighlightLine : highlightCode);

  // .md 默认 preview，可切 code（对齐原版 WriteToolViewer.tsx 第 59-62 行）
  let viewMode = $state<"preview" | "code">("preview");

  // 三档分级：内容面 = 待写入文件内容（input.content），不按成功回执分档
  // （spec tool-viewer-routing::写入型工具按输入内容规模分档）。
  // markdown 预览是富文本不切片；code 模式行导向允许 top/tail 切片。
  const totalLines = $derived(countLines(content));
  const totalBytes = $derived(utf8ByteLength(content));
  const allowSlice = $derived(!(isMarkdown && viewMode === "preview"));
  const tier = $derived(classifyText(content, allowSlice));
  const sliceIdx = $derived(
    tier === "oversized" ? sliceLineIndices(lines.map((l) => utf8ByteLength(l))) : null
  );
  const effectiveTier = $derived(tier === "oversized" && sliceIdx === null ? "bounded" : tier);
  const headLines = $derived(sliceIdx ? lines.slice(0, sliceIdx.headCount) : []);
  const tailStart = $derived(sliceIdx ? lines.length - sliceIdx.tailCount : 0);
  const tailLines = $derived(sliceIdx ? lines.slice(tailStart) : []);
  const scent = $derived(`${totalLines} 行 · ${formatBytes(totalBytes)} · 预览`);

  function buildCtx(): MenuItemContext {
    return {
      sessionId,
      projectId,
      settings: getMenuSettings(),
      selectionText: window.getSelection()?.toString() ?? "",
      dispatch: getMenuItemDispatch(),
    };
  }
</script>

<div class="write-viewer" use:contextMenu={() => buildFileToolItems(exec, buildCtx())}>
  <div class="write-header">
    <span class="write-icon">W</span>
    <span class="write-path">{shortenPath(filePath)}</span>
    <span class="write-badge">NEW</span>
    {#if effectiveTier !== "inline"}
      <span class="write-scent">{scent}</span>
    {/if}
    {#if isMarkdown}
      <span class="write-spacer"></span>
      <button
        class="view-toggle"
        onclick={() => (viewMode = viewMode === "preview" ? "code" : "preview")}
      >
        {viewMode === "preview" ? "源码" : "预览"}
      </button>
    {/if}
    <CopyButton text={content} ariaLabel="复制全文" />
  </div>

  {#if content}
    {#if isMarkdown && viewMode === "preview"}
      <div
        class="md-preview"
        class:bounded={effectiveTier !== "inline"}
        {@attach adaptiveScrollViewport(() => `Write ${shortenPath(filePath)}（${scent}，可滚动）`)}
      >{@html renderMarkdown(content)}</div>
    {:else}
      <div
        class="write-code-container"
        class:bounded={effectiveTier !== "inline"}
        {@attach adaptiveScrollViewport(() => `Write ${shortenPath(filePath)}（${scent}，可滚动）`)}
      >
        {#if effectiveTier === "oversized" && sliceIdx}
          <pre class="write-code"><code>{#each headLines as line, i}<span class="line" data-line={i + 1}>{@html highlightLine(line, language)}
</span>{/each}</code></pre>
          <div class="write-seam" role="separator">
            已省略 {sliceIdx.omittedLines} 行 · {formatBytes(sliceIdx.omittedBytes)}
          </div>
          <pre class="write-code"><code>{#each tailLines as line, i}<span class="line" data-line={tailStart + i + 1}>{@html highlightLine(line, language)}
</span>{/each}</code></pre>
        {:else}
          <pre class="write-code"><code>{#each lines as line, i}<span class="line" data-line={i + 1}>{@html highlightLine(line, language)}
</span>{/each}</code></pre>
        {/if}
      </div>
    {/if}
  {/if}
</div>

<style>
  .write-viewer {
    min-width: 0;
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
    flex: 1 1 auto;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12px;
    font-family: var(--font-mono);
    color: var(--diff-added-text);
    font-weight: 500;
  }

  .write-badge {
    flex-shrink: 0;
    font-size: 9px;
    font-weight: 600;
    color: var(--diff-added-text);
    background: rgba(34, 197, 94, 0.2);
    padding: 1px 6px;
    border-radius: 4px;
    letter-spacing: 0.5px;
  }

  .write-spacer {
    flex: 0 0 auto;
  }

  /* 信息气味：总行数 · 总字节数 · 预览（mono metadata，中性色）。 */
  .write-scent {
    flex-shrink: 0;
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--color-text-secondary);
    white-space: nowrap;
  }

  .view-toggle {
    flex-shrink: 0;
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

  .write-code-container {
    overflow: auto;
    scrollbar-gutter: stable;
    background: var(--code-bg);
  }

  /* bounded / oversized：响应式限高（共享 token），inline 不限高。 */
  .write-code-container.bounded {
    max-block-size: var(--ao-preview-max-block);
  }

  .write-code-container:focus-visible,
  .md-preview:focus-visible {
    outline: 2px solid var(--color-accent-blue, #3b82f6);
    outline-offset: -2px;
  }

  /* 省略接缝：中性文字 + 细分隔线，显式标注省略量（不用渐隐遮罩）。 */
  .write-seam {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--color-text-secondary);
    background: var(--code-bg);
    padding: 4px 12px 4px 60px;
    border-block: 1px dashed var(--color-border);
    white-space: normal;
  }

  .write-code {
    font-size: 12px;
    font-family: var(--font-mono);
    color: var(--color-text-secondary);
    padding: 10px 0;
    margin: 0;
    line-height: 1.5;
  }

  .write-code :global(code) {
    background: none;
    padding: 0;
    color: inherit;
    font: inherit;
    border-radius: 0;
  }

  .write-code .line {
    display: block;
    position: relative;
    padding-left: 60px;
    white-space: pre;
  }

  .write-code .line::before {
    content: attr(data-line);
    position: absolute;
    left: 0;
    width: 48px;
    padding-right: 12px;
    text-align: right;
    color: var(--code-line-number);
    user-select: none;
  }

  /* hljs token 颜色统一在 app.css 的 .hljs-* 全局规则里 */

  .md-preview {
    padding: 12px 16px;
    overflow: auto;
    scrollbar-gutter: stable;
    background: var(--code-bg);
    color: var(--color-text);
    font-size: 13px;
    line-height: 1.6;
  }

  /* markdown 富文本不切片：bounded 顶格为限高预览（完整内容留 DOM）。 */
  .md-preview.bounded {
    max-block-size: var(--ao-preview-max-block);
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

<script lang="ts">
  import type { ToolExecution } from "../../lib/api";
  import { toolOutputText, shortenPath, getLanguageFromPath, getFileName } from "../../lib/toolHelpers";
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
    /** 完整输出懒加载中：以限高档稳定占位渲染，复制禁用。 */
    outputLoading?: boolean;
    /** 懒加载失败：显式失败态。 */
    outputLoadFailed?: boolean;
  }

  let { exec, sessionId = "", projectId = "", outputLoading = false, outputLoadFailed = false }: Props = $props();

  function buildCtx(): MenuItemContext {
    return {
      sessionId,
      projectId,
      settings: getMenuSettings(),
      selectionText: window.getSelection()?.toString() ?? "",
      dispatch: getMenuItemDispatch(),
    };
  }

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
  const useLightHighlight = $derived(parsedLines.length > 250 || cleanText.length > 40_000);
  const highlightLine = $derived(useLightHighlight ? lightHighlightLine : highlightCode);

  // 三档分级（spec tool-viewer-routing::工具查看器按内容规模自适应展示）：
  // 内容面 = Read 输出（strip 后主内容）。markdown 预览是富文本不切片
  // （oversized 降级 bounded）；code 模式行导向允许 top/tail 切片。
  const totalLines = $derived(countLines(cleanText));
  const totalBytes = $derived(utf8ByteLength(cleanText));
  const allowSlice = $derived(!(isMarkdown && viewMode === "preview"));
  const tier = $derived(classifyText(cleanText, allowSlice));
  const sliceIdx = $derived(
    tier === "oversized" ? sliceLineIndices(parsedLines.map((p) => utf8ByteLength(p.text))) : null
  );
  // 行数不足以切片（sliceLineIndices 返回 null）→ 退回限高预览。
  const effectiveTier = $derived(tier === "oversized" && sliceIdx === null ? "bounded" : tier);
  const headLines = $derived(sliceIdx ? parsedLines.slice(0, sliceIdx.headCount) : []);
  const tailLines = $derived(sliceIdx ? parsedLines.slice(parsedLines.length - sliceIdx.tailCount) : []);
  const scent = $derived(`${totalLines} 行 · ${formatBytes(totalBytes)} · 预览`);
</script>

<div class="read-viewer" use:contextMenu={() => buildFileToolItems(exec, buildCtx())}>
  <!-- File header -->
  <div class="file-header">
    <span class="file-icon">F</span>
    <span class="file-name">{shortenPath(filePath)}</span>
    <span class="file-lang">{language}</span>
    <span class="file-spacer"></span>
    {#if outputLoadFailed}
      <span class="file-scent">加载失败</span>
    {:else if !outputLoading && effectiveTier !== "inline"}
      <span class="file-scent">{scent}</span>
    {/if}
    {#if isMarkdown}
      <button
        class="view-toggle"
        onclick={() => (viewMode = viewMode === "preview" ? "code" : "preview")}
      >
        {viewMode === "preview" ? "源码" : "预览"}
      </button>
    {/if}
    <CopyButton
      text={outputLoading || outputLoadFailed ? "" : cleanText}
      disabled={outputLoading || outputLoadFailed}
      ariaLabel={outputLoadFailed
        ? "完整内容加载失败，暂不可复制"
        : outputLoading
          ? "完整内容加载中，暂不可复制"
          : "复制全文"}
    />
  </div>

  {#if outputLoadFailed}
    <div class="read-loading">完整内容加载失败，收起后重新展开可重试</div>
  {:else if outputLoading}
    <div class="read-loading" aria-busy="true">正在载入完整内容…</div>
  {:else if isMarkdown && viewMode === "preview"}
    <!-- 用 strip 后的纯文本渲染：raw outputText 含 cat -n 前缀会让 markdown 标记失效 -->
    <div
      class="md-preview"
      class:bounded={effectiveTier !== "inline"}
      {@attach adaptiveScrollViewport(() => `Read ${fileName}（${scent}，可滚动）`)}
    >{@html renderMarkdown(cleanText)}</div>
  {:else}
    <!-- Code with line numbers (line numbers are CSS ::before, not part of clipboard text) -->
    <div
      class="code-container"
      class:bounded={effectiveTier !== "inline"}
      {@attach adaptiveScrollViewport(() => `Read ${fileName}（${scent}，可滚动）`)}
    >
      {#if effectiveTier === "oversized" && sliceIdx}
        <pre class="code-content"><code>{#each headLines as p (p.num)}<span class="line" data-line={p.num}>{@html highlightLine(p.text, language)}
</span>{/each}</code></pre>
        <div class="read-seam" role="separator">
          已省略 {sliceIdx.omittedLines} 行 · {formatBytes(sliceIdx.omittedBytes)}
        </div>
        <pre class="code-content"><code>{#each tailLines as p (p.num)}<span class="line" data-line={p.num}>{@html highlightLine(p.text, language)}
</span>{/each}</code></pre>
      {:else}
        <pre class="code-content"><code>{#each parsedLines as p (p.num)}<span class="line" data-line={p.num}>{@html highlightLine(p.text, language)}
</span>{/each}</code></pre>
      {/if}
    </div>
  {/if}
</div>

<style>
  .read-viewer {
    min-width: 0;
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
    flex: 1 1 auto;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12px;
    font-family: var(--font-mono);
    color: var(--code-filename);
    font-weight: 500;
  }

  .file-lang {
    flex-shrink: 0;
    font-size: 10px;
    color: var(--tag-text);
    background: var(--tag-bg);
    border: 1px solid var(--tag-border);
    padding: 1px 6px;
    border-radius: 4px;
  }

  .file-spacer {
    flex: 0 0 auto;
  }

  /* 信息气味：总行数 · 总字节数 · 预览（mono metadata，中性色）。 */
  .file-scent {
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
    transition: color 0.15s, border-color 0.15s;
  }

  .view-toggle:hover {
    color: var(--color-text);
    border-color: var(--color-text-muted);
  }

  .code-container {
    overflow: auto;
    scrollbar-gutter: stable;
    background: var(--code-bg);
  }

  /* bounded / oversized：响应式限高（共享 token），inline 不限高。 */
  .code-container.bounded {
    max-block-size: var(--ao-preview-max-block);
  }

  .code-container:focus-visible,
  .md-preview:focus-visible {
    outline: 2px solid var(--color-accent-blue, #3b82f6);
    outline-offset: -2px;
  }

  .read-loading {
    display: flex;
    align-items: center;
    justify-content: center;
    min-block-size: var(--ao-preview-max-block);
    color: var(--color-text-muted);
    font-size: 12px;
    background: var(--code-bg);
  }

  /* 省略接缝：中性文字 + 细分隔线，显式标注省略量（不用渐隐遮罩）。 */
  .read-seam {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--color-text-secondary);
    background: var(--code-bg);
    padding: 4px 12px 4px 60px;
    border-block: 1px dashed var(--color-border);
    white-space: normal;
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

  /* hljs token 颜色统一在 app.css 的 .hljs-* 全局规则里 */
</style>

<script lang="ts">
  import { highlightCode } from "../lib/render";
  import { ByteCappedCache } from "../lib/byteCappedCache";
  import AdaptiveOutputFrame from "./AdaptiveOutputFrame.svelte";
  import CopyButton from "../lib/components/CopyButton.svelte";
  import { formatBytes } from "../lib/formatters";
  import {
    countLines,
    utf8ByteLength,
    sliceHeadTail,
    sizingForToolOutput,
    trimBlankEdgeLines,
    type OutputTier,
  } from "../lib/outputSizing";

  // key 含完整源码 + 高亮后 HTML，单条可达数 MB → count + byte 双闸门（见 byteCappedCache）。
  const highlightCache = new ByteCappedCache<string>({
    maxEntries: 128,
    maxBytes: 4 * 1024 * 1024,
    sizeOf: (key, value) => key.length + value.length,
  });

  interface Props {
    code: string;
    lang?: string;
    isError?: boolean;
    label?: string;
    /** 完整输出懒加载中：以限高档稳定占位渲染，复制禁用（spec 工具输出懒加载态的稳定分档）。 */
    loading?: boolean;
    /** 懒加载失败：显式失败态（复制禁用 + 可重试提示），不停留在假加载占位。 */
    loadFailed?: boolean;
    /** 懒加载前已知的输出字节量（exec.outputBytes），loading 态信息气味 + 占位分档用。 */
    bytesHint?: number;
  }

  let { code: rawCode, lang = "json", isError = false, label, loading = false, loadFailed = false, bytesHint }: Props = $props();

  // 终端 / API 输出常以空行开头 / 结尾（cargo / git / kbase fetch 等）；inline 档以
  // white-space:pre 忠实渲染会在框顶 / 框底留出空白行，配常驻复制 icon 就成了
  // "空框"（图1 回归）。仅**显示 / 分档**修剪首尾整条空白行（单遍扫描，见
  // trimBlankEdgeLines）；**复制仍用完整原文** rawCode（spec copy-to-clipboard::
  // 复制该输出面的完整原文——显示可规范化但复制不删原文）。
  const code = $derived(trimBlankEdgeLines(rawCode));
  // 修剪后为纯空（原文只有空白行 / 纯空格）：不渲染带边框 padding 的空 `<pre>`，
  // 改极简占位；但按 spec copy-to-clipboard 空内容仍提供**禁用**复制入口（说明原因），
  // 不移除入口。
  const isEmpty = $derived(!loading && !loadFailed && code.length === 0);

  function cachedHighlight(value: string, language: string): string {
    const key = `${language}\0${value.length}\0${value}`;
    const hit = highlightCache.get(key);
    if (hit !== undefined) return hit;
    const result = highlightCode(value, language);
    highlightCache.set(key, result);
    return result;
  }

  const lines = $derived(countLines(code));
  const bytes = $derived(utf8ByteLength(code));
  // 工具输出为行导向纯文本 / 代码，允许 oversized top/tail 切片。
  // 分档走懒加载稳定状态机（spec 工具输出懒加载态的稳定分档）：
  // 未加载时按 outputBytes 保守占位（不判 inline），加载后按真实内容校正。
  const tier = $derived<OutputTier>(
    sizingForToolOutput({
      loadedText: loading || loadFailed ? null : code,
      outputBytes: bytesHint,
      omitted: loading || loadFailed,
      allowOversized: true,
    }).tier,
  );

  // oversized 切片；行数不足以切片时 sliceHeadTail 返回 null → 退回完整渲染。
  const sliced = $derived(tier === "oversized" ? sliceHeadTail(code) : null);
  const effectiveTier = $derived<OutputTier>(
    tier === "oversized" && sliced === null ? "bounded" : tier,
  );

  const fullHighlighted = $derived(
    loading || loadFailed || effectiveTier === "oversized" ? "" : cachedHighlight(code, lang),
  );
  const headHighlighted = $derived(sliced ? cachedHighlight(sliced.head, lang) : "");
  const tailHighlighted = $derived(sliced ? cachedHighlight(sliced.tail, lang) : "");
</script>

{#if isEmpty}
  <!-- 纯空白 / 空输出：不渲染带边框空 <pre>，仅极简占位；复制入口按 spec
       copy-to-clipboard 保留但禁用（说明原因），不移除入口、不复制空白。 -->
  <div class="output-empty">
    <span class="output-empty-label">{label ? `${label}·空` : "空输出"}</span>
    <CopyButton text="" disabled={true} ariaLabel="输出为空，暂无可复制内容" />
  </div>
{:else}
  <AdaptiveOutputFrame
    tier={effectiveTier}
    lines={loading || loadFailed ? 0 : lines}
    bytes={loading || loadFailed ? (bytesHint ?? 0) : bytes}
    copyText={loading || loadFailed ? "" : rawCode}
    {loading}
    failed={loadFailed}
    {label}
    {isError}
    viewportLabel={label ?? "输出"}
  >
    {#snippet children()}
      {#if effectiveTier === "oversized" && sliced}
        <pre class="output-pre output-pre-slice"><code>{@html headHighlighted}</code></pre>
        <div class="output-seam" role="separator">
          已省略 {sliced.omittedLines > 0 ? `${sliced.omittedLines} 行` : ""}{sliced.omittedBytes >
          0
            ? ` · ${formatBytes(sliced.omittedBytes)}`
            : ""}
        </div>
        <pre class="output-pre output-pre-slice"><code>{@html tailHighlighted}</code></pre>
      {:else}
        <pre class="output-pre"><code>{@html fullHighlighted}</code></pre>
      {/if}
    {/snippet}
  </AdaptiveOutputFrame>
{/if}

<style>
  /* 纯空白 / 空输出：不套边框框，仅极简次级文案 + 禁用复制入口，不留空框。 */
  .output-empty {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--color-text-muted);
    padding: 4px 2px;
  }

  .output-empty-label {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .output-pre {
    min-width: 0;
    font-size: 12px;
    font-family: var(--font-mono);
    color: var(--color-text-secondary);
    background: var(--code-bg);
    padding: 10px 12px;
    margin: 0;
    white-space: pre;
    line-height: 1.5;
  }

  .output-pre-slice {
    padding-block: 6px;
  }

  .output-pre :global(code) {
    display: block;
    background: none;
    padding: 0;
    color: inherit;
    font: inherit;
    border-radius: 0;
  }

  .output-seam {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--color-text-secondary);
    background: var(--code-bg);
    padding: 4px 12px;
    border-top: 1px dashed var(--color-border);
    white-space: normal;
  }

  /* 错误态：正文用 danger 语义色（header 由 frame 的 ao-err 承载）。 */
  :global(.ao-err) .output-pre {
    color: var(--tool-result-error-text);
    background: var(--tool-result-error-bg);
  }

  :global(.ao-err) .output-seam {
    background: var(--tool-result-error-bg);
  }
</style>

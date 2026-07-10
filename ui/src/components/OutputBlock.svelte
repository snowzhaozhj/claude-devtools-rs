<script lang="ts">
  import { highlightCode } from "../lib/render";
  import { ByteCappedCache } from "../lib/byteCappedCache";
  import AdaptiveOutputFrame from "./AdaptiveOutputFrame.svelte";
  import { formatBytes } from "../lib/formatters";
  import {
    classifyText,
    countLines,
    utf8ByteLength,
    sliceHeadTail,
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
  }

  let { code, lang = "json", isError = false, label }: Props = $props();

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
  const tier = $derived<OutputTier>(classifyText(code, true));

  // oversized 切片；行数不足以切片时 sliceHeadTail 返回 null → 退回完整渲染。
  const sliced = $derived(tier === "oversized" ? sliceHeadTail(code) : null);
  const effectiveTier = $derived<OutputTier>(
    tier === "oversized" && sliced === null ? "bounded" : tier,
  );

  const fullHighlighted = $derived(
    effectiveTier === "oversized" ? "" : cachedHighlight(code, lang),
  );
  const headHighlighted = $derived(sliced ? cachedHighlight(sliced.head, lang) : "");
  const tailHighlighted = $derived(sliced ? cachedHighlight(sliced.tail, lang) : "");
</script>

<AdaptiveOutputFrame
  tier={effectiveTier}
  {lines}
  {bytes}
  copyText={code}
  {label}
  {isError}
  viewportLabel={label ?? "输出"}
>
  {#snippet children()}
    {#if effectiveTier === "oversized" && sliced}
      <pre class="output-pre output-pre-slice"><code>{@html headHighlighted}</code></pre>
      <div class="output-seam" role="separator">
        已省略中段 {sliced.omittedLines > 0 ? `${sliced.omittedLines} 行` : ""}
        {sliced.omittedBytes > 0 ? `· ${formatBytes(sliced.omittedBytes)}` : ""} — 复制全文查看完整内容
      </div>
      <pre class="output-pre output-pre-slice"><code>{@html tailHighlighted}</code></pre>
    {:else}
      <pre class="output-pre"><code>{@html fullHighlighted}</code></pre>
    {/if}
  {/snippet}
</AdaptiveOutputFrame>

<style>
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
    color: var(--color-text-muted);
    background: var(--code-bg);
    padding: 4px 12px;
    border-block: 1px dashed var(--color-border);
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

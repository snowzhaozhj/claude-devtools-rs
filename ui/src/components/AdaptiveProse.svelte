<script lang="ts">
  import type { Snippet } from "svelte";
  import AdaptiveOutputFrame from "./AdaptiveOutputFrame.svelte";
  import { classifyText, countLines, utf8ByteLength } from "../lib/outputSizing";

  interface Props {
    /** markdown 源文本，用于规模判定与复制全文。 */
    text: string;
    /** 可滚动 viewport 的可访问名前缀。 */
    viewportLabel?: string;
    /** 实际的 prose 渲染节点（含 use:contextMenu / lazy markdown attach）。 */
    body: Snippet;
  }

  let { text, viewportLabel = "输出", body }: Props = $props();

  // prose 参与 Cmd+F 全文搜索：只有 inline / bounded 两档，不切片（allowOversized=false）。
  const tier = $derived(classifyText(text, false));
  const lines = $derived(countLines(text));
  const bytes = $derived(utf8ByteLength(text));
</script>

{#if tier === "bounded"}
  <AdaptiveOutputFrame
    tier="bounded"
    {lines}
    {bytes}
    copyText={text}
    {viewportLabel}
    variant="prose"
  >
    {#snippet children()}{@render body()}{/snippet}
  </AdaptiveOutputFrame>
{:else}
  {@render body()}
{/if}

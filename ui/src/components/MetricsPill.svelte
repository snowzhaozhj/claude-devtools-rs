<script lang="ts">
  import { formatTokensCompact } from "../lib/formatters";

  interface Props {
    /** 父 session 的 token 贡献（call+result 之和）；null/0 时不渲染该槽 */
    mainTokens?: number | null;
    /** subagent 内部最后一轮 usage 合计；null/0 时不渲染 */
    isolatedTokens?: number | null;
    /** 非 team 默认 "Subagent Context"，team 成员传 "Context Window" */
    isolatedLabel?: string;
  }

  let {
    mainTokens = null,
    isolatedTokens = null,
    isolatedLabel = "Subagent Context",
  }: Props = $props();

  const showMain = $derived(mainTokens != null && mainTokens > 0);
  const showIsolated = $derived(isolatedTokens != null && isolatedTokens > 0);
</script>

{#if showMain || showIsolated}
  <div class="metrics-pill">
    {#if showMain}
      <span class="slot slot-main" title="Main Context: {mainTokens} tokens">{formatTokensCompact(mainTokens)}</span>
    {/if}
    {#if showMain && showIsolated}
      <span class="sep">|</span>
    {/if}
    {#if showIsolated}
      <span class="slot slot-iso" title="{isolatedLabel}: {isolatedTokens} tokens">{formatTokensCompact(isolatedTokens)}</span>
    {/if}
  </div>
{/if}

<style>
  /* 对齐原版 MetricsPill.tsx：仅显示数字 + `|` 分隔，语义靠 hover tooltip */
  .metrics-pill {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 1px 8px;
    border-radius: 4px;
    background: var(--badge-neutral-bg);
    border: 1px solid var(--card-border);
    font-size: 11px;
    font-family: var(--font-mono);
    flex-shrink: 0;
    color: var(--color-text-secondary);
    font-variant-numeric: tabular-nums;
    cursor: default;
  }

  .slot { display: inline-flex; align-items: center; }

  .sep { color: var(--card-separator); }
</style>

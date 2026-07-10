<script lang="ts">
  import type { Snippet } from "svelte";
  import CopyButton from "../lib/components/CopyButton.svelte";
  import { formatBytes } from "../lib/formatters";
  import type { OutputTier } from "../lib/outputSizing";

  interface Props {
    /** 规模档位。inline 不限高；bounded / oversized 进限高滚动 viewport。 */
    tier: OutputTier;
    /** 总行数（信息气味）。 */
    lines: number;
    /** 总 UTF-8 字节数（信息气味）。 */
    bytes: number;
    /** 完整原文（复制全文用）；未就绪时传空串并置 loading。 */
    copyText: string;
    /** 完整原文是否正在加载 / 不可得——复制禁用、显示加载占位。 */
    loading?: boolean;
    /** 可选左上标签（如 "OUTPUT" / 文件名）。 */
    label?: string;
    /** 错误态样式。 */
    isError?: boolean;
    /** 可访问名前缀（如工具名），用于可滚动 viewport 的 aria-label。 */
    viewportLabel?: string;
    /** 内容槽。oversized 时由调用方组合 head + 省略接缝 + tail。 */
    children: Snippet;
  }

  let {
    tier,
    lines,
    bytes,
    copyText,
    loading = false,
    label,
    isError = false,
    viewportLabel = "输出",
    children,
  }: Props = $props();

  const isBounded = $derived(tier === "bounded" || tier === "oversized");
  const scent = $derived(`${lines} 行 · ${formatBytes(bytes)}`);
  const previewTag = $derived(isBounded ? "预览" : "");

  // 仅当内容沿任一轴实际溢出时，viewport 才进入 Tab 序列（spec a11y 契约）。
  let overflowing = $state(false);

  function watchOverflow(el: HTMLElement) {
    const measure = () => {
      overflowing = el.scrollHeight > el.clientHeight + 1 || el.scrollWidth > el.clientWidth + 1;
    };
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    // 内容懒加载 / 高亮完成后尺寸会变，观察子树变化重测。
    const mo = new MutationObserver(measure);
    mo.observe(el, { childList: true, subtree: true, characterData: true });
    return () => {
      ro.disconnect();
      mo.disconnect();
    };
  }
</script>

<div class="ao" class:ao-err={isError}>
  <div class="ao-header">
    <span class="ao-meta">
      {#if label}<span class="ao-label">{label}</span>{/if}
      <span class="ao-scent">{scent}</span>
      {#if previewTag}<span class="ao-preview">{previewTag}</span>{/if}
    </span>
    <CopyButton
      text={copyText}
      label="复制全文"
      disabled={loading || !copyText}
      ariaLabel={loading ? "完整内容加载中，暂不可复制" : "复制全文"}
    />
  </div>

  {#if loading}
    <div class="ao-body ao-viewport ao-loading" aria-busy="true">正在载入完整内容…</div>
  {:else if isBounded}
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <!-- 可滚动 region 需可键盘聚焦以滚动（WCAG 2.1.1）；仅在实际溢出时才可聚焦。 -->
    <div
      class="ao-body ao-viewport"
      tabindex={overflowing ? 0 : undefined}
      role={overflowing ? "region" : undefined}
      aria-label={overflowing ? `${viewportLabel}（${scent}，可滚动）` : undefined}
      {@attach watchOverflow}
    >
      {@render children()}
    </div>
  {:else}
    <div class="ao-body">
      {@render children()}
    </div>
  {/if}
</div>

<style>
  .ao {
    min-width: 0;
  }

  .ao-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 3px 8px 3px 10px;
    background: var(--code-bg);
    border: 1px solid var(--code-border);
    border-bottom: none;
    border-radius: 6px 6px 0 0;
  }

  .ao-meta {
    display: flex;
    align-items: baseline;
    gap: 8px;
    min-width: 0;
    overflow: hidden;
  }

  .ao-label {
    font-size: 9px;
    font-weight: 600;
    color: var(--color-text-muted);
    letter-spacing: 1px;
    text-transform: uppercase;
    flex-shrink: 0;
  }

  .ao-scent {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--color-text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .ao-preview {
    font-size: 10px;
    color: var(--color-text-muted);
    border: 1px solid var(--color-border);
    border-radius: 4px;
    padding: 0 4px;
    flex-shrink: 0;
  }

  .ao-body {
    border: 1px solid var(--code-border);
    border-top: none;
    border-radius: 0 0 6px 6px;
  }

  .ao-viewport {
    max-block-size: clamp(12rem, 42dvh, 30rem);
    overflow: auto;
    scrollbar-gutter: stable;
  }

  .ao-viewport:focus-visible {
    outline: 2px solid var(--color-accent-blue, #3b82f6);
    outline-offset: -2px;
  }

  .ao-loading {
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--color-text-muted);
    font-size: 12px;
    min-block-size: clamp(12rem, 42dvh, 30rem);
  }

  .ao-err .ao-header {
    background: var(--tool-result-error-bg);
    border-color: color-mix(in oklch, var(--color-danger-bright) 20%, transparent);
  }

  .ao-err .ao-body {
    border-color: color-mix(in oklch, var(--color-danger-bright) 20%, transparent);
  }
</style>

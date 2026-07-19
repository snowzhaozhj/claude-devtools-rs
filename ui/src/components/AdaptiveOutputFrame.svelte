<script lang="ts">
  import type { Snippet } from "svelte";
  import CopyButton from "../lib/components/CopyButton.svelte";
  import { formatBytes } from "../lib/formatters";
  import { adaptiveScrollViewport } from "../lib/adaptiveViewport";
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
    /** 懒加载失败：显式失败态（复制禁用 + 可重试提示），不停留在 aria-busy 假占位。 */
    failed?: boolean;
    /** 可选左上标签（如 "OUTPUT" / 文件名）。 */
    label?: string;
    /** 错误态样式。 */
    isError?: boolean;
    /** 可访问名前缀（如工具名），用于可滚动 viewport 的 aria-label。 */
    viewportLabel?: string;
    /** 视觉变体：code（工具输出，code-bg 框）/ prose（AI 文本，轻量框）。 */
    variant?: "code" | "prose";
    /** 内容槽。oversized 时由调用方组合 head + 省略接缝 + tail。 */
    children: Snippet;
  }

  let {
    tier,
    lines,
    bytes,
    copyText,
    loading = false,
    failed = false,
    label,
    isError = false,
    viewportLabel = "输出",
    variant = "code",
    children,
  }: Props = $props();

  const isBounded = $derived(tier === "bounded" || tier === "oversized");
  // header 仅在 bounded/oversized/loading 出现，故 scent 恒为"预览"语境；
  // "预览"折进纯文本，取代此前的带框 badge（无状态装饰违反 Status Owns the Color）。
  // loading 时行数未知（未加载），只显示已知字节量（outputBytes）+ 载入中。
  const scent = $derived(
    failed
      ? "加载失败"
      : loading
        ? bytes > 0
          ? `${formatBytes(bytes)} · 载入中`
          : "载入中"
        : `${lines} 行 · ${formatBytes(bytes)} · 预览`,
  );
</script>

<div class="ao" class:ao-err={isError} class:ao-prose={variant === "prose"}>
  {#if failed}
    <div class="ao-header">
      <span class="ao-meta">
        {#if label}<span class="ao-label">{label}</span>{/if}
        <span class="ao-scent">{scent}</span>
      </span>
      <CopyButton text="" disabled={true} ariaLabel="完整内容加载失败，暂不可复制" />
    </div>
    <!-- 与 loading 占位同几何（min-block-size 共用），失败↔重试切换不跳变。 -->
    <div class="ao-body ao-loading">完整内容加载失败，收起后重新展开可重试</div>
  {:else if loading}
    <div class="ao-header">
      <span class="ao-meta">
        {#if label}<span class="ao-label">{label}</span>{/if}
        <span class="ao-scent">{scent}</span>
      </span>
      <CopyButton text="" disabled={true} ariaLabel="完整内容加载中，暂不可复制" />
    </div>
    <div class="ao-body ao-viewport ao-loading" aria-busy="true">正在载入完整内容…</div>
  {:else if isBounded}
    <div class="ao-header">
      <span class="ao-meta">
        {#if label}<span class="ao-label">{label}</span>{/if}
        <span class="ao-scent">{scent}</span>
      </span>
      <CopyButton text={copyText} disabled={!copyText} ariaLabel="复制全文" />
    </div>
    <!-- 可滚动 region 需可键盘聚焦以滚动（WCAG 2.1.1）；仅实际溢出时才进
         Tab 序列（tabindex/role/aria-label 由 attachment 按溢出实测切换）。 -->
    <div
      class="ao-body ao-viewport"
      {@attach adaptiveScrollViewport(() => `${viewportLabel}（${scent}，可滚动）`)}
    >
      {@render children()}
    </div>
  {:else}
    <!-- inline（短内容）：不渲 metadata 带——chrome 不得压过内容。
         复制全文入口降为右上角常驻低调 icon（满足 spec"常驻可发现"，不用 hover-only）。
         内层滚动容器承载长单行横向滚动（main 既有行为）+ 仅实际溢出时键盘可达；
         copy 挂外层不随横向滚动漂移。 -->
    <div class="ao-body ao-inline">
      <div
        class="ao-inline-scroll"
        {@attach adaptiveScrollViewport(() => `${viewportLabel}（${lines} 行 · ${formatBytes(bytes)}，可滚动）`)}
      >
        {@render children()}
      </div>
      {#if copyText}
        <span class="ao-inline-copy">
          <CopyButton text={copyText} ariaLabel="复制全文" />
        </span>
      {/if}
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
    padding: 4px 8px 4px 10px;
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
    font-size: 11px;
    font-weight: 600;
    color: var(--color-text-secondary);
    letter-spacing: 1px;
    text-transform: uppercase;
    flex-shrink: 0;
  }

  .ao-scent {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--color-text-secondary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .ao-body {
    border: 1px solid var(--code-border);
    border-top: none;
    border-radius: 0 0 6px 6px;
  }

  /* inline：无 header 带，body 四角自成一体；相对定位承载常驻 copy。 */
  .ao-inline {
    position: relative;
    border-radius: 6px;
  }

  .ao-inline-scroll {
    overflow-x: auto;
    /* scrollbar-gutter-exempt: 横向滚动为主，inline 档内容不限高、无竖向滚动条 */
    border-radius: inherit;
  }

  .ao-inline-scroll:focus-visible {
    outline: 2px solid var(--color-accent-blue, #3b82f6);
    outline-offset: -2px;
  }

  .ao-inline-copy {
    position: absolute;
    top: 4px;
    inset-inline-end: 4px;
    /* 常驻但低调：与 code-bg 同底融入，不抢内容；hover 由 CopyButton 自身加深。 */
    background: var(--code-bg);
    border-radius: 4px;
    opacity: 0.65;
    transition: opacity 0.15s;
  }

  .ao-inline:hover .ao-inline-copy,
  .ao-inline-copy:focus-within {
    opacity: 1;
  }

  .ao-viewport {
    max-block-size: var(--ao-preview-max-block);
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
    min-block-size: var(--ao-preview-max-block);
  }

  .ao-err .ao-header {
    background: var(--tool-result-error-bg);
    border-color: color-mix(in oklch, var(--color-danger-bright) 20%, transparent);
  }

  .ao-err .ao-body {
    border-color: color-mix(in oklch, var(--color-danger-bright) 20%, transparent);
  }

  /* prose 轻量变体：不套 code-bg 框，透明 header + 细下边框，避免把正文显示得像代码。 */
  .ao-prose .ao-header {
    background: transparent;
    border: none;
    border-bottom: 1px solid var(--color-border);
    border-radius: 0;
    padding-inline: 2px;
  }

  .ao-prose .ao-body {
    border: none;
    border-radius: 0;
  }
</style>

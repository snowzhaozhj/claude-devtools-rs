<!--
  通用 loading 骨架占位。带 shimmer 横扫动画，对齐原版 claude-devtools
  `.skeleton-shimmer` 视觉模式——避免静态色块给用户"挂起"的感觉。

  variant="row" → sidebar/notification 列表条目；"card" → dashboard 项目卡 /
  setting row；"text" → 段落行。shimmer 走 transform GPU 加速；
  `prefers-reduced-motion: reduce` 下退化为静态色块。
-->

<script lang="ts">
  interface Props {
    variant?: "row" | "card" | "text";
    height?: number | string;
    width?: number | string;
  }
  let { variant = "row", height, width }: Props = $props();

  const resolvedHeight = $derived(
    height ?? (variant === "card" ? 96 : variant === "text" ? 14 : 32)
  );
  const resolvedWidth = $derived(width ?? "100%");

  const heightStyle = $derived(
    typeof resolvedHeight === "number" ? `${resolvedHeight}px` : resolvedHeight
  );
  const widthStyle = $derived(
    typeof resolvedWidth === "number" ? `${resolvedWidth}px` : resolvedWidth
  );
</script>

<div
  class="skel"
  class:skel-row={variant === "row"}
  class:skel-card={variant === "card"}
  class:skel-text={variant === "text"}
  style:height={heightStyle}
  style:width={widthStyle}
  aria-hidden="true"
></div>

<style>
  .skel {
    position: relative;
    overflow: hidden;
    background: var(--skel-base, var(--color-border));
    flex-shrink: 0;
  }
  .skel-row {
    border-radius: 4px;
  }
  .skel-card {
    border-radius: 8px;
  }
  .skel-text {
    border-radius: 3px;
  }

  .skel::after {
    content: "";
    position: absolute;
    inset: 0;
    border-radius: inherit;
    transform: translateX(-100%);
    background: linear-gradient(
      90deg,
      transparent 0%,
      var(--skel-shimmer, rgba(0, 0, 0, 0.06)) 50%,
      transparent 100%
    );
    animation: skel-shimmer 1.4s ease-in-out infinite;
    pointer-events: none;
  }

  @media (prefers-reduced-motion: reduce) {
    .skel::after {
      animation: none;
    }
  }
</style>

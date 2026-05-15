<!--
  通用 loading 骨架占位。模式对齐 SessionDetailSkeleton：
  静态背景 + opacity 0.5、无 shimmer 动画（避免与 OngoingIndicator 等 ping 视觉竞争）。
  variant="row" → sidebar/notification 列表条目；"card" → dashboard 项目卡 / setting row；"text" → 段落行。
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
    background: var(--color-border);
    opacity: 0.5;
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
</style>

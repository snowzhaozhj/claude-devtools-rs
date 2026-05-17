<!--
  OngoingIndicator：sidebar 行首的 "session 进行中" 静态状态点。

  设计取舍（vs 原版 ping 涟漪）：sidebar 可能同时多条 ongoing session 并存，
  N 个 infinite 动画叠加是用户感知最强的视觉噪音源。改为静态实心绿点 +
  静态光环 box-shadow（"亮着的灯"语义），保留 "在线" 信号但不抢注意力。
  实时刷新仍由列表内容驱动（in-place patch），动画不承担信号职责。
-->
<script lang="ts">
  interface Props {
    size?: "sm" | "md";
    showLabel?: boolean;
    label?: string;
  }

  let { size = "sm", showLabel = false, label = "Session in progress" }: Props = $props();
</script>

<span class="ongoing" class:ongoing-md={size === "md"} title="Session in progress">
  <span class="dot" aria-hidden="true"></span>
  {#if showLabel}
    <span class="label">{label}</span>
  {/if}
</span>

<style>
  .ongoing {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--color-success-bright);
    box-shadow: 0 0 0 2px color-mix(in oklch, var(--color-success-bright) 22%, transparent);
    flex-shrink: 0;
  }

  .ongoing-md .dot {
    width: 10px;
    height: 10px;
    box-shadow: 0 0 0 3px color-mix(in oklch, var(--color-success-bright) 22%, transparent);
  }

  .label {
    font-size: 12px;
    color: var(--color-accent-blue);
  }
</style>

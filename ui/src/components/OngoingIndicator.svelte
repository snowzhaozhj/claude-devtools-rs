<!--
  OngoingIndicator + OngoingBanner。

  对齐原版 `../claude-devtools/src/renderer/components/common/OngoingIndicator.tsx`：
  - 绿点脉冲：sidebar 行首展示 session 进行中
  - 蓝色底部横幅：SessionDetail 尾部展示 "Session is in progress..."

  CSS 变量使用 `app.css` 已声明的 `--color-info*` / `--color-success`
  token；没有再 fallback 到硬编码蓝/绿。
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
  <span class="dot-wrap">
    <span class="dot-ping"></span>
    <span class="dot-core"></span>
  </span>
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

  .dot-wrap {
    position: relative;
    display: inline-flex;
    width: 8px;
    height: 8px;
    flex-shrink: 0;
  }

  .ongoing-md .dot-wrap {
    width: 10px;
    height: 10px;
  }

  .dot-ping {
    position: absolute;
    inset: 0;
    border-radius: 50%;
    background: #4ade80; /* green-400 */
    opacity: 0.75;
    animation: ongoing-ping 1.4s cubic-bezier(0, 0, 0.2, 1) infinite;
  }

  .dot-core {
    position: relative;
    width: 100%;
    height: 100%;
    border-radius: 50%;
    background: #22c55e; /* green-500 */
  }

  .label {
    font-size: 12px;
    color: var(--color-info-text, #3b82f6);
  }

  @keyframes ongoing-ping {
    75%,
    100% {
      transform: scale(2);
      opacity: 0;
    }
  }
</style>

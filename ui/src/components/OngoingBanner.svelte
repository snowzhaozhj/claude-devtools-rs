<!--
  OngoingBanner：嵌入最后一个 AIChunk 的 lastOutput 槽位，表达"流仍在进行"。
  不再使用旋转 spinner（与 IDE 工具的稳态质感不符），改用 IDE-style
  shimmer bar（横扫 1.6s）+ mono uppercase label，对齐 product register
  下的"调试器进度指示器"语言，同时降低视觉噪音。
-->
<script lang="ts">
</script>

<div class="ongoing" role="status" aria-live="polite">
  <div class="ongoing-row">
    <span class="ongoing-pulse" aria-hidden="true"></span>
    <span class="ongoing-label">STREAMING</span>
    <span class="ongoing-hint">Session is still in progress…</span>
  </div>
  <div class="ongoing-track" aria-hidden="true">
    <span class="ongoing-sweep"></span>
  </div>
</div>

<style>
  .ongoing {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px 14px 12px;
    border-radius: 8px;
    background: color-mix(in oklch, var(--color-accent-blue) 5%, transparent);
    border: 1px solid color-mix(in oklch, var(--color-accent-blue) 22%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in oklch, var(--color-accent-blue) 6%, transparent);
    width: 100%;
  }

  .ongoing-row {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .ongoing-pulse {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--color-accent-blue);
    flex-shrink: 0;
    /* 静态光环替代 ping scale 动画——sweep bar 已表达 streaming
       进度，避免与 pulse 重复传递同一信号导致视觉过载。 */
    box-shadow: 0 0 0 2px color-mix(in oklch, var(--color-accent-blue) 24%, transparent);
  }

  .ongoing-label {
    font-family: var(--font-mono);
    font-size: 10.5px;
    font-weight: 700;
    letter-spacing: 0.14em;
    color: var(--color-accent-blue);
    flex-shrink: 0;
  }

  .ongoing-hint {
    font-size: 12px;
    color: var(--color-text-muted);
    line-height: 1.2;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }

  .ongoing-track {
    position: relative;
    height: 2px;
    border-radius: 2px;
    overflow: hidden;
    background: color-mix(in oklch, var(--color-accent-blue) 12%, transparent);
  }

  .ongoing-sweep {
    position: absolute;
    inset: 0;
    background: linear-gradient(
      90deg,
      transparent 0%,
      color-mix(in oklch, var(--color-accent-blue) 75%, transparent) 50%,
      transparent 100%
    );
    animation: ongoing-sweep 1.6s cubic-bezier(0.4, 0, 0.6, 1) infinite;
    transform: translateX(-100%);
  }

  @keyframes ongoing-sweep {
    0% {
      transform: translateX(-100%);
    }
    100% {
      transform: translateX(100%);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .ongoing-sweep {
      animation: none;
      transform: translateX(0);
      opacity: 0.5;
    }
  }
</style>

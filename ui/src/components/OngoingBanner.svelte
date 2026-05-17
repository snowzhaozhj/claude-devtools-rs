<!--
  OngoingBanner：嵌入最后一个 AIChunk 的 lastOutput 槽位，表达"流仍在进行"。

  色彩：success 绿（与 sidebar OngoingIndicator / 顶栏 LIVE 一致）。原先用
  accent-blue 会与 timeline node / 顶栏 LIVE 在同屏堆叠成多块蓝色——按
  「ongoing 全应用统一一种颜色」原则收回到绿色。
  动效：track 内 sweep 横扫 2.4s（比原 1.6s 更慢更克制），给用户最小的
  "系统在跑"心跳，避免静态进度条带来的"卡死"感。
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
    background: color-mix(in oklch, var(--color-success) 5%, transparent);
    border: 1px solid color-mix(in oklch, var(--color-success) 22%, transparent);
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
    background: var(--color-success);
    flex-shrink: 0;
    box-shadow: 0 0 0 2px color-mix(in oklch, var(--color-success) 22%, transparent);
  }

  .ongoing-label {
    font-family: var(--font-mono);
    font-size: 10.5px;
    font-weight: 700;
    letter-spacing: 0.14em;
    color: var(--color-success);
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

  /* track + sweep：2.4s 横扫（比原 1.6s 慢），让用户感知"在跑"但不抢注意力。
     完全静态会让人感觉"卡死"，这是用户反馈直接驱动的取舍。 */
  .ongoing-track {
    position: relative;
    height: 2px;
    border-radius: 2px;
    overflow: hidden;
    background: color-mix(in oklch, var(--color-success) 10%, transparent);
  }

  .ongoing-sweep {
    position: absolute;
    inset: 0;
    background: linear-gradient(
      90deg,
      transparent 0%,
      color-mix(in oklch, var(--color-success) 70%, transparent) 50%,
      transparent 100%
    );
    animation: ongoing-sweep 2.4s ease-in-out infinite;
    transform: translateX(-100%);
  }

  @keyframes ongoing-sweep {
    0% { transform: translateX(-100%); }
    100% { transform: translateX(100%); }
  }

  @media (prefers-reduced-motion: reduce) {
    .ongoing-sweep {
      animation: none;
      transform: translateX(0);
      opacity: 0.45;
    }
  }
</style>

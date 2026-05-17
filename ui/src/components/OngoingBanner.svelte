<!--
  OngoingBanner：嵌入最后一个 AIChunk 的 lastOutput 槽位，作为 SessionDetail
  的 primary "session 仍在进行" 指示器。

  视觉决策（详见 DESIGN.md `The Ongoing Owns Blue Rule` 与 `One Live Signal Rule`）：
  - 详情页一屏只允许一个动态 live 信号；该信号属于 dot ping
  - 去掉早期版本的 shimmer sweep bar：bar 横扫 + dot ping 双层动画
    会让眼睛被持续吸到 banner 上，违反 product register 的 "实时但不闪烁" 原则
  - 仅保留 dot ping + STREAMING label + 文案，整体仍保持 IDE-style 稳态质感
-->
<script lang="ts">
</script>

<div class="ongoing" role="status" aria-live="polite">
  <span class="ongoing-pulse" aria-hidden="true"></span>
  <span class="ongoing-label">STREAMING</span>
  <span class="ongoing-hint">Session is still in progress…</span>
</div>

<style>
  .ongoing {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 14px;
    border-radius: 8px;
    background: color-mix(in oklch, var(--color-accent-blue) 5%, transparent);
    border: 1px solid color-mix(in oklch, var(--color-accent-blue) 22%, transparent);
    width: 100%;
  }

  .ongoing-pulse {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--color-accent-blue);
    flex-shrink: 0;
    box-shadow: 0 0 0 0 color-mix(in oklch, var(--color-accent-blue) 50%, transparent);
    animation: ongoing-ping 1.8s cubic-bezier(0.16, 1, 0.3, 1) infinite;
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

  @keyframes ongoing-ping {
    0% {
      transform: scale(0.85);
      box-shadow: 0 0 0 0 color-mix(in oklch, var(--color-accent-blue) 55%, transparent);
    }
    70% {
      transform: scale(1);
      box-shadow: 0 0 0 6px color-mix(in oklch, var(--color-accent-blue) 0%, transparent);
    }
    100% {
      transform: scale(0.85);
      box-shadow: 0 0 0 0 color-mix(in oklch, var(--color-accent-blue) 0%, transparent);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .ongoing-pulse {
      animation: none;
    }
  }
</style>

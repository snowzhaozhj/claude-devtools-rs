<!--
  OngoingBanner：嵌入最后一个 AIChunk 的 lastOutput 槽位，作为 SessionDetail
  的 primary "session 仍在进行" 指示器。

  视觉决策（详见 DESIGN.md `The Static-vs-Live Shape Rule` 与
  `The One Live Signal Rule`）：
  - 详情页一屏只允许一个动态 live 信号；该信号属于 circular spinner
  - 改用 CSS border spinner（顶边 accent-blue + 其余 border 浅蓝 mask）+
    1.2s linear infinite rotate——稳态恒速旋转是 IDE/调试器工具的
    "白噪音"型 live 语言（VS Code / IntelliJ / GitHub Actions 同款），
    眼睛会快速适应不再持续抢戏，比周期性 dot ping 的 attention spike
    更适合"克制工作台"的 product register
-->
<script lang="ts">
</script>

<div class="ongoing" role="status" aria-live="polite">
  <span class="ongoing-spinner" aria-hidden="true"></span>
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

  /* 双层 border circular spinner：浅蓝静态环 + 蓝色顶弧旋转。
     形态：圆环 + 旋转（白噪音），与所有其它位置的 outline 静态空心圆
     在形态上自然分开。 */
  .ongoing-spinner {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    border: 2px solid color-mix(in oklch, var(--color-accent-blue) 18%, transparent);
    border-top-color: var(--color-accent-blue);
    flex-shrink: 0;
    box-sizing: border-box;
    animation: ongoing-spin 1.2s linear infinite;
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

  @keyframes ongoing-spin {
    to {
      transform: rotate(360deg);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .ongoing-spinner {
      animation: none;
      /* reduced-motion 下保留可识别静态形态：顶弧蓝色仍可见，仅不旋转 */
    }
  }
</style>

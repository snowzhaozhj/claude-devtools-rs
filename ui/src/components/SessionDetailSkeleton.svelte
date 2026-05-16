<!--
  SessionDetail loading 骨架。对齐原版 `ChatHistoryLoadingState.tsx`：
  organic 多行 line widths + shimmer 横扫；用户消息靠右、AI 响应靠左带
  border-accent，模拟真实对话流——比单纯色块更贴近最终内容，让用户更早
  形成布局预期。
-->

<script lang="ts">
  // 3 个对话回合 × (user 行 + ai 行)。宽度组合学原版 `ChatHistoryLoadingState`，
  // 用 const 而非随机：保证刷新视觉稳定（不会因为重 mount 改变 layout）。
  const rows = [
    { user: ["85%", "60%"], ai: ["92%", "70%", "82%", "45%"] },
    { user: ["75%", "92%", "40%"], ai: ["88%", "65%", "78%"] },
    { user: ["95%", "55%"], ai: ["72%", "85%", "60%", "92%", "35%"] },
  ];
</script>

<div class="skel-root" role="status" aria-busy="true" aria-label="加载中">
  <div class="skel-inner">
    {#each rows as row, i (i)}
      <div class="skel-turn">
        <!-- User: 右对齐 -->
        <div class="skel-user-wrap">
          <div class="skel-user-col">
            {#each row.user as w, j (j)}
              <div class="skel-line" style:width={w}></div>
            {/each}
          </div>
        </div>
        <!-- AI: 左对齐 + 左侧 border accent -->
        <div class="skel-ai">
          {#each row.ai as w, j (j)}
            <div class="skel-line" style:width={w}></div>
          {/each}
        </div>
      </div>
    {/each}
  </div>
</div>

<style>
  .skel-root {
    display: flex;
    flex: 1 1 auto;
    align-items: center;
    justify-content: center;
    overflow: hidden;
    background: var(--color-surface);
  }

  .skel-inner {
    width: 100%;
    max-width: 880px;
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 32px;
  }

  .skel-turn {
    display: flex;
    flex-direction: column;
    gap: 24px;
  }

  .skel-user-wrap {
    display: flex;
    justify-content: flex-end;
  }

  .skel-user-col {
    width: 66%;
    display: flex;
    flex-direction: column;
    gap: 8px;
    align-items: flex-end;
  }

  .skel-ai {
    border-left: 2px solid var(--color-border);
    padding-left: 12px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .skel-line {
    height: 12px;
    border-radius: 3px;
    position: relative;
    overflow: hidden;
    background: var(--skel-base, var(--color-border));
  }

  .skel-user-col .skel-line {
    margin-left: auto;
  }

  .skel-line::after {
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
    .skel-line::after {
      animation: none;
    }
  }
</style>

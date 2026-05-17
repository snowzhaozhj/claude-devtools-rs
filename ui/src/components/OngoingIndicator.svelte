<!--
  OngoingIndicator：Sidebar 行首的 "session 进行中" 静态指示器。

  视觉决策：ongoing 状态全局统一使用 Focus Blue（详见 DESIGN.md
  `The Ongoing Owns Blue Rule`）。Sidebar 多个进行中会话同时出现时，
  N 个独立脉冲会形成视觉噪音；这里**只**用蓝色填充 + 蓝色 halo
  ring，保持稳态指示，不再脉冲。仅 SessionDetail 的 OngoingBanner
  保留单一动态信号（dot ping），作为详情页 primary live signal。
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
    align-items: center;
    justify-content: center;
  }

  .ongoing-md .dot-wrap {
    width: 10px;
    height: 10px;
  }

  /* 形态分化：静态指示器用 outline 空心圆，与 OngoingBanner 的 filled
     dot ping 形态完全区分——避免相同颜色 + halo 让大脑把两类点归一组、
     被脉冲源"感染"产生节律错觉。详见 DESIGN.md `The Static-vs-Live
     Shape Rule`。 */
  .dot-core {
    width: 100%;
    height: 100%;
    border-radius: 50%;
    background: transparent;
    border: 1.5px solid var(--color-accent-blue);
    box-sizing: border-box;
  }

  .label {
    font-size: 12px;
    color: var(--color-accent-blue);
  }
</style>

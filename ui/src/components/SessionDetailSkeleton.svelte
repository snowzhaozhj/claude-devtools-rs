<!--
  Loading 骨架占位。配合 lazy markdown 一起把首屏从纯文本"加载中..."升级到
  布局级反馈。静态背景、无动画——避免与 OngoingIndicator ping 视觉竞争。
  设计依据：openspec/changes/session-detail-lazy-render/design.md decision 4。
-->

<div class="skel-root" aria-busy="true" aria-label="加载中">
  <div class="skel-card skel-user" style="height: 80px"></div>
  <div class="skel-card skel-ai" style="height: 200px"></div>
  <div class="skel-card skel-user" style="height: 60px"></div>
  <div class="skel-card skel-ai" style="height: 240px"></div>
  <div class="skel-card skel-system" style="height: 100px"></div>
</div>

<style>
  .skel-root {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 16px 24px;
  }

  .skel-card {
    border-radius: 12px;
    background: var(--color-border);
    opacity: 0.5;
  }

  .skel-user {
    align-self: flex-end;
    width: 60%;
  }

  .skel-ai {
    align-self: flex-start;
    width: 90%;
  }

  .skel-system {
    align-self: stretch;
    width: 100%;
  }
</style>

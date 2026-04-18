<!--
  OngoingBanner：SessionDetail 尾部蓝色胶囊横幅。
  对齐原版 OngoingIndicator.tsx 中的 OngoingBanner 组件。
-->
<script lang="ts">
</script>

<div class="ongoing-banner" role="status" aria-live="polite">
  <span class="spinner" aria-hidden="true"></span>
  <span class="banner-text">Session is in progress...</span>
</div>

<style>
  .ongoing-banner {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 10px;
    padding: 12px 16px;
    border-radius: 10px;
    background: var(--color-info-bg, rgba(59, 130, 246, 0.1));
    border: 1px solid var(--color-info-border, rgba(59, 130, 246, 0.3));
    color: var(--color-info-text, #3b82f6);
    font-size: 13px;
    width: 100%;
  }

  .spinner {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    border: 2px solid currentColor;
    border-right-color: transparent;
    animation: ongoing-spin 0.9s linear infinite;
    flex-shrink: 0;
  }

  .banner-text {
    line-height: 1.2;
  }

  @keyframes ongoing-spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>

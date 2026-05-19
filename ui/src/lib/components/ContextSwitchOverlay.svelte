<script lang="ts">
  import { contextStore } from "../stores/context.svelte";

  const targetLabel = $derived(
    contextStore.switchingTo === "local"
      ? "Local"
      : (contextStore.availableContexts.find((ctx) => ctx.id === contextStore.switchingTo)?.label
        ?? contextStore.switchingTo?.replace(/^ssh-/, "")
        ?? "workspace"),
  );
</script>

{#if contextStore.switching}
  <div class="overlay" role="status" aria-live="polite" aria-label="正在切换工作区">
    <div class="panel">
      <span class="spinner" aria-hidden="true"></span>
      <div class="text">
        <p>正在切换到 {targetLabel}</p>
        <span>加载工作区数据</span>
      </div>
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 9999;
    display: flex;
    align-items: center;
    justify-content: center;
    background: color-mix(in oklch, var(--color-surface) 88%, transparent);
  }
  .panel {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 18px 20px;
    border: 1px solid var(--color-border-emphasis);
    border-radius: 10px;
    background: var(--color-surface-raised);
    box-shadow: 0 12px 36px rgba(0, 0, 0, 0.14);
  }
  .spinner {
    width: 20px;
    height: 20px;
    border: 2px solid color-mix(in oklch, var(--color-accent-blue) 22%, transparent);
    border-top-color: var(--color-accent-blue);
    border-radius: 999px;
    animation: spin 0.9s linear infinite;
  }
  .text {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .text p {
    margin: 0;
    color: var(--color-text);
    font-size: 14px;
    font-weight: 600;
  }
  .text span {
    color: var(--color-text-secondary);
    font-size: 12px;
  }
  @keyframes spin {
    to { transform: rotate(360deg); }
  }
  @media (prefers-reduced-motion: reduce) {
    .spinner { animation: none; }
  }
</style>

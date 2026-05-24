<script lang="ts">
  import { toastStore } from "../lib/toastStore.svelte";
</script>

<div class="toast-container" role="region" aria-live="polite" aria-atomic="false">
  {#each toastStore.toasts as toast (toast.id)}
    <div
      class="toast"
      class:toast-error={toast.level === "error"}
      class:toast-info={toast.level === "info"}
      role="status"
    >
      <span class="toast-message">{toast.message}</span>
      <button
        type="button"
        class="toast-close"
        aria-label="关闭"
        onclick={() => toastStore.dismiss(toast.id)}
      >
        ×
      </button>
    </div>
  {/each}
</div>

<style>
  .toast-container {
    position: fixed;
    bottom: 16px;
    right: 16px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    z-index: 9999;
    pointer-events: none;
  }

  .toast {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    padding: 10px 12px;
    border-radius: 6px;
    border: 1px solid var(--color-border-emphasis);
    background: var(--color-surface);
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.15);
    max-width: 360px;
    font-size: 12px;
    line-height: 1.4;
    color: var(--color-text);
    pointer-events: auto;
    animation: toast-in 150ms ease-out;
  }

  .toast-error {
    border-color: var(--color-danger, #d33);
    color: var(--color-danger, #d33);
  }

  /* .toast-info uses default surface tone (no extra rules) */

  .toast-message {
    flex: 1;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .toast-close {
    background: transparent;
    border: 0;
    cursor: pointer;
    color: inherit;
    font-size: 16px;
    line-height: 1;
    padding: 0 2px;
    opacity: 0.7;
  }

  .toast-close:hover {
    opacity: 1;
  }

  @keyframes toast-in {
    from {
      opacity: 0;
      transform: translateY(8px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
</style>

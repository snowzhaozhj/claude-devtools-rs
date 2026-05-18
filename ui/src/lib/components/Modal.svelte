<script lang="ts">
  import { tick, type Snippet } from "svelte";
  import SettingsButton from "./SettingsButton.svelte";

  interface Props {
    open: boolean;
    title?: string;
    primaryLabel?: string;
    primaryDisabled?: boolean;
    cancelLabel?: string;
    onPrimary?: () => void;
    onClose: () => void;
    children?: Snippet;
  }

  let {
    open,
    title = "",
    primaryLabel = "确定",
    primaryDisabled = false,
    cancelLabel = "取消",
    onPrimary,
    onClose,
    children,
  }: Props = $props();

  let dialogEl: HTMLDivElement | null = $state(null);
  let primaryBtnEl: HTMLButtonElement | null = $state(null);
  let cancelBtnEl: HTMLButtonElement | null = $state(null);

  $effect(() => {
    if (open) {
      tick().then(() => {
        (primaryBtnEl ?? cancelBtnEl ?? dialogEl)?.focus();
      });
    }
  });

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      onClose();
      return;
    }
    if (e.key === "Tab") {
      const focusables = Array.from(
        dialogEl?.querySelectorAll<HTMLElement>(
          'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [contenteditable="true"], [tabindex]:not([tabindex="-1"])',
        ) ?? [],
      ).filter((el) => !el.hasAttribute("disabled"));
      if (focusables.length === 0) return;
      const first = focusables[0];
      const last = focusables[focusables.length - 1];
      const active = document.activeElement as HTMLElement | null;
      if (e.shiftKey && active === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && active === last) {
        e.preventDefault();
        first.focus();
      }
    }
  }

  function handleOverlayClick(e: MouseEvent) {
    if (e.target === e.currentTarget) onClose();
  }
</script>

{#if open}
  <div
    class="modal-overlay"
    role="presentation"
    onclick={handleOverlayClick}
    onkeydown={handleKeydown}
  >
    <div
      class="modal"
      role="dialog"
      aria-modal="true"
      aria-labelledby={title ? "cdt-modal-title" : undefined}
      bind:this={dialogEl}
      tabindex="-1"
    >
      {#if title}
        <h2 id="cdt-modal-title" class="modal-title">{title}</h2>
      {/if}
      <div class="modal-body">
        {@render children?.()}
      </div>
      <div class="modal-actions">
        <SettingsButton
          variant="ghost"
          onClick={onClose}
          buttonRef={(el) => {
            cancelBtnEl = el;
          }}
        >
          {cancelLabel}
        </SettingsButton>
        {#if onPrimary}
          <SettingsButton
            variant="primary"
            disabled={primaryDisabled}
            onClick={onPrimary}
            buttonRef={(el) => {
              primaryBtnEl = el;
            }}
          >
            {primaryLabel}
          </SettingsButton>
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .modal-overlay {
    position: fixed;
    inset: 0;
    background: color-mix(in oklch, var(--color-bg) 70%, transparent);
    backdrop-filter: blur(2px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }
  .modal {
    background: var(--color-surface);
    border: 1px solid var(--color-border);
    border-radius: 10px;
    padding: 20px 22px;
    min-width: 360px;
    max-width: min(640px, 90vw);
    max-height: 80vh;
    display: flex;
    flex-direction: column;
    gap: 14px;
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.18);
  }
  .modal:focus {
    outline: none;
  }
  .modal-title {
    font-size: 14px;
    font-weight: 600;
    color: var(--color-text);
    margin: 0;
  }
  .modal-body {
    flex: 1 1 auto;
    overflow-y: auto;
    color: var(--color-text-secondary);
    font-size: 13px;
  }
  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
</style>

<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    variant?: "ghost" | "primary" | "danger";
    size?: "sm" | "md";
    /** icon-only: 隐藏文字仅显图标（需要 aria-label） */
    iconOnly?: boolean;
    disabled?: boolean;
    type?: "button" | "submit";
    ariaLabel?: string;
    title?: string;
    onClick?: (e: MouseEvent) => void;
    icon?: Snippet;
    children?: Snippet;
  }

  let {
    variant = "ghost",
    size = "md",
    iconOnly = false,
    disabled = false,
    type = "button",
    ariaLabel,
    title,
    onClick,
    icon,
    children,
  }: Props = $props();
</script>

<button
  {type}
  {disabled}
  {title}
  aria-label={ariaLabel}
  class="btn btn-{variant} btn-{size}"
  class:btn-icon-only={iconOnly}
  onclick={(e) => onClick?.(e)}
>
  {#if icon}
    <span class="btn-icon" aria-hidden="true">{@render icon()}</span>
  {/if}
  {#if children && !iconOnly}
    <span class="btn-label">{@render children()}</span>
  {/if}
</button>

<style>
  .btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    border: 1px solid transparent;
    border-radius: 6px;
    font: inherit;
    font-weight: 500;
    cursor: pointer;
    white-space: nowrap;
    transition:
      background-color 0.12s,
      border-color 0.12s,
      color 0.12s;
  }
  .btn:focus-visible {
    outline: 2px solid var(--color-switch-on);
    outline-offset: 1px;
  }
  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn-md {
    height: 30px;
    padding: 0 12px;
    font-size: 13px;
  }
  .btn-sm {
    height: 26px;
    padding: 0 10px;
    font-size: 12px;
  }

  .btn-icon-only.btn-md {
    width: 30px;
    padding: 0;
  }
  .btn-icon-only.btn-sm {
    width: 26px;
    padding: 0;
  }

  .btn-ghost {
    background: transparent;
    border-color: var(--color-border);
    color: var(--color-text-secondary);
  }
  .btn-ghost:hover:not(:disabled) {
    background: var(--tool-item-hover-bg);
    color: var(--color-text);
  }

  .btn-primary {
    background: var(--color-switch-on);
    border-color: var(--color-switch-on);
    color: #ffffff;
  }
  .btn-primary:hover:not(:disabled) {
    filter: brightness(1.08);
  }

  .btn-danger {
    background: transparent;
    border-color: transparent;
    color: var(--color-text-muted);
  }
  .btn-danger:hover:not(:disabled) {
    background: color-mix(in oklch, var(--color-danger-bright) 14%, transparent);
    color: var(--color-danger);
  }

  .btn-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 14px;
    height: 14px;
  }
  .btn-icon :global(svg) {
    width: 14px;
    height: 14px;
  }
  .btn-label {
    line-height: 1;
  }
</style>

<script lang="ts">
  import type { SshStatus } from "../types/ssh";
  import { ALERT_CIRCLE_SVG, MONITOR_SVG, WIFI_OFF_SVG, WIFI_SVG } from "../icons";

  interface Props {
    status?: SshStatus | string | null;
    contextId?: string | null;
    error?: string | null;
    showText?: boolean;
  }

  let { status = "disconnected", contextId = null, error = null, showText = true }: Props = $props();

  const effectiveStatus = $derived(contextId === "local" ? "connected" : (status ?? "disconnected"));
  const label = $derived(
    contextId === "local"
      ? "Local"
      : effectiveStatus === "connected"
        ? "Connected"
        : effectiveStatus === "connecting"
          ? "Connecting"
          : effectiveStatus === "error"
            ? "Error"
            : "Disconnected",
  );
</script>

<span class="badge badge-{effectiveStatus}" title={error ?? label} aria-label={error ? `${label}: ${error}` : label}>
  <span class="badge-icon" aria-hidden="true">
    {#if effectiveStatus === "connecting"}
      <span class="spinner"></span>
    {:else if contextId === "local"}
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{@html MONITOR_SVG}</svg>
    {:else if effectiveStatus === "connected"}
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{@html WIFI_SVG}</svg>
    {:else if effectiveStatus === "error"}
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{@html ALERT_CIRCLE_SVG}</svg>
    {:else}
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{@html WIFI_OFF_SVG}</svg>
    {/if}
  </span>
  {#if showText}
    <span>{label}</span>
  {/if}
</span>

<style>
  .badge {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 22px;
    padding: 2px 8px;
    border: 1px solid var(--color-border);
    border-radius: 999px;
    background: var(--color-surface-raised);
    color: var(--color-text-secondary);
    font-size: 11px;
    font-weight: 600;
    line-height: 1;
  }
  .badge-icon {
    display: inline-flex;
    width: 13px;
    height: 13px;
  }
  .badge-icon :global(svg) {
    width: 13px;
    height: 13px;
  }
  .badge-connected {
    border-color: color-mix(in oklch, var(--color-success-bright) 35%, var(--color-border));
    background: color-mix(in oklch, var(--color-success-bright) 8%, var(--color-surface));
    color: var(--color-success);
  }
  .badge-connecting {
    border-color: color-mix(in oklch, var(--color-warning) 35%, var(--color-border));
    background: color-mix(in oklch, var(--color-warning) 8%, var(--color-surface));
    color: var(--color-warning);
  }
  .badge-error {
    border-color: color-mix(in oklch, var(--color-danger-bright) 35%, var(--color-border));
    background: color-mix(in oklch, var(--color-danger-bright) 8%, var(--color-surface));
    color: var(--color-danger);
  }
  .spinner {
    width: 13px;
    height: 13px;
    border: 1.5px solid color-mix(in oklch, currentColor 30%, transparent);
    border-top-color: currentColor;
    border-radius: 999px;
    animation: spin 0.9s linear infinite;
  }
  @keyframes spin {
    to { transform: rotate(360deg); }
  }
  @media (prefers-reduced-motion: reduce) {
    .spinner { animation: none; }
  }
</style>

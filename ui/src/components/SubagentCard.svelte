<script lang="ts">
  import type { SubagentProcess } from "../lib/api";
  import { CHEVRON_RIGHT } from "../lib/icons";
  import { openTab } from "../lib/tabStore.svelte";

  interface Props {
    process: SubagentProcess;
  }

  let { process }: Props = $props();

  let isExpanded = $state(false);

  const truncatedDesc = $derived(
    process.rootTaskDescription
      ? process.rootTaskDescription.length > 60
        ? process.rootTaskDescription.slice(0, 60) + "…"
        : process.rootTaskDescription
      : "Subagent"
  );

  const isCompleted = $derived(!!process.endTs);

  const durationText = $derived.by(() => {
    if (!process.endTs || !process.spawnTs) return null;
    const ms = new Date(process.endTs).getTime() - new Date(process.spawnTs).getTime();
    if (ms < 1000) return `${ms}ms`;
    if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
    const min = Math.floor(ms / 60000);
    const sec = Math.floor((ms % 60000) / 1000);
    return sec > 0 ? `${min}m ${sec}s` : `${min}m`;
  });

  // team 用 memberColor，非 team 默认 teal
  const dotColor = $derived(
    process.team?.memberColor ?? "#2dd4bf"
  );

  const badgeLabel = $derived(
    process.team ? process.team.memberName : "Task"
  );

  function toggleExpanded() {
    isExpanded = !isExpanded;
  }

  function navigateToSession() {
    const label = (process.team?.memberName ?? "Subagent") + " — " + (process.rootTaskDescription ?? process.sessionId).slice(0, 40);
    openTab(process.sessionId, "", label);
  }

  function fk(n: number): string {
    if (n >= 1e6) return (n / 1e6).toFixed(1) + "M";
    if (n >= 1e3) return (n / 1e3).toFixed(1) + "k";
    return String(n);
  }
</script>

<div class="sa-card">
  <!-- Header (clickable) -->
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="sa-header" class:sa-header-expanded={isExpanded} onclick={toggleExpanded}>
    <!-- Chevron -->
    <svg class="sa-chevron" class:sa-chevron-open={isExpanded} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d={CHEVRON_RIGHT}/></svg>

    <!-- Colored dot -->
    <span class="sa-dot" style="background-color: {dotColor}"></span>

    <!-- Type badge -->
    <span class="sa-badge" style="background-color: {dotColor}20; color: {dotColor}; border: 1px solid {dotColor}40">
      {badgeLabel}
    </span>

    <!-- Description -->
    <span class="sa-desc">{truncatedDesc}</span>

    <!-- Status indicator -->
    {#if isCompleted}
      <svg class="sa-status-done" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"/><polyline points="22 4 12 14.01 9 11.01"/></svg>
    {:else}
      <svg class="sa-status-running" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12a9 9 0 1 1-6.219-8.56"/></svg>
    {/if}

    <!-- Token metrics -->
    {#if process.metrics}
      <span class="sa-tokens">{fk(process.metrics.inputTokens + process.metrics.outputTokens)}</span>
    {/if}

    <!-- Duration -->
    {#if durationText}
      <span class="sa-duration">{durationText}</span>
    {/if}
  </div>

  <!-- Expanded content -->
  {#if isExpanded}
    <div class="sa-body">
      <!-- Meta row -->
      <div class="sa-meta">
        <span class="sa-meta-label">Type</span>
        <span class="sa-meta-value">{process.team ? "Team" : "Task"}</span>
        <span class="sa-meta-sep">·</span>
        {#if durationText}
          <span class="sa-meta-label">Duration</span>
          <span class="sa-meta-value">{durationText}</span>
          <span class="sa-meta-sep">·</span>
        {/if}
        <span class="sa-meta-label">ID</span>
        <span class="sa-meta-value sa-meta-id" title={process.sessionId}>{process.sessionId.slice(0, 8)}</span>
      </div>

      <!-- Full description -->
      {#if process.rootTaskDescription && process.rootTaskDescription.length > 60}
        <div class="sa-full-desc">{process.rootTaskDescription}</div>
      {/if}

      <!-- Context usage -->
      {#if process.metrics && (process.metrics.inputTokens > 0 || process.metrics.outputTokens > 0)}
        <div class="sa-context">
          <div class="sa-context-title">Context Usage</div>
          <div class="sa-context-row">
            <span class="sa-context-label">Input</span>
            <span class="sa-context-val">{process.metrics.inputTokens.toLocaleString()}</span>
          </div>
          <div class="sa-context-row">
            <span class="sa-context-label">Output</span>
            <span class="sa-context-val">{process.metrics.outputTokens.toLocaleString()}</span>
          </div>
        </div>
      {/if}

      <!-- Navigate button -->
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="sa-navigate" onclick={(e: MouseEvent) => { e.stopPropagation(); navigateToSession(); }}>
        <svg class="sa-nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M7 17l9.2-9.2M17 17V8H8"/></svg>
        <span>Open Session</span>
      </div>
    </div>
  {/if}
</div>

<style>
  .sa-card {
    border-radius: 6px;
    border: 1px solid var(--card-border);
    background: var(--card-bg);
    overflow: hidden;
    transition: box-shadow 0.2s;
  }

  .sa-header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    cursor: pointer;
    transition: background-color 0.1s;
  }

  .sa-header:hover {
    background: var(--card-header-hover);
  }

  .sa-header-expanded {
    background: var(--card-header-bg);
    border-bottom: 1px solid var(--card-border);
  }

  .sa-chevron {
    width: 14px;
    height: 14px;
    flex-shrink: 0;
    color: var(--card-icon-muted);
    transition: transform 0.15s ease;
  }

  .sa-chevron-open {
    transform: rotate(90deg);
  }

  .sa-dot {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .sa-badge {
    font-size: 10px;
    font-weight: 500;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    padding: 1px 6px;
    border-radius: 4px;
    flex-shrink: 0;
  }

  .sa-desc {
    flex: 1;
    font-size: 12px;
    color: var(--card-text-light);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }

  .sa-status-done {
    width: 14px;
    height: 14px;
    flex-shrink: 0;
    color: #22c55e;
  }

  .sa-status-running {
    width: 14px;
    height: 14px;
    flex-shrink: 0;
    color: #3b82f6;
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }

  .sa-tokens {
    font-size: 11px;
    font-family: var(--font-mono);
    color: var(--card-icon-muted);
    flex-shrink: 0;
  }

  .sa-duration {
    font-size: 11px;
    font-family: var(--font-mono);
    color: var(--card-icon-muted);
    flex-shrink: 0;
    font-variant-numeric: tabular-nums;
  }

  /* Expanded body */
  .sa-body {
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .sa-meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 6px;
    font-size: 11px;
  }

  .sa-meta-label {
    color: var(--card-icon-muted);
  }

  .sa-meta-value {
    color: var(--card-text-light);
    font-family: var(--font-mono);
  }

  .sa-meta-id {
    max-width: 120px;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--card-icon-muted);
  }

  .sa-meta-sep {
    color: var(--card-separator);
  }

  .sa-full-desc {
    font-size: 12px;
    color: var(--card-text-light);
    line-height: 1.5;
  }

  .sa-context {
    padding-top: 4px;
  }

  .sa-context-title {
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--card-icon-muted);
    margin-bottom: 6px;
  }

  .sa-context-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 2px 0;
  }

  .sa-context-label {
    font-size: 12px;
    color: var(--tool-item-summary);
  }

  .sa-context-val {
    font-size: 12px;
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
    color: var(--card-text-lighter);
  }

  .sa-navigate {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: var(--card-icon-muted);
    cursor: pointer;
    padding: 4px 0;
    transition: color 0.1s;
  }

  .sa-navigate:hover {
    color: var(--card-text-light);
  }

  .sa-nav-icon {
    width: 14px;
    height: 14px;
  }
</style>

<script lang="ts">
  import type { WorkflowItem } from "../lib/api";
  import { formatDuration } from "../lib/formatters";
  import { CHEVRON_RIGHT } from "../lib/icons";

  interface Props {
    workflow: WorkflowItem;
  }

  let { workflow }: Props = $props();

  let isExpanded = $state(false);
  let isScriptExpanded = $state(false);

  const statusLabel = $derived.by(() => {
    switch (workflow.status) {
      case "completed": return "Done";
      case "partial_failure": {
        const failedCount = workflow.agents.filter(a => a.status === "failed").length;
        return `${failedCount} failed`;
      }
      case "running": return "Running";
      case "pending": return "Pending";
    }
  });

  const phaseSummary = $derived(
    `${workflow.phases.length} phase${workflow.phases.length !== 1 ? "s" : ""} · ${workflow.agents.length} agent${workflow.agents.length !== 1 ? "s" : ""}`,
  );

  const durationText = $derived(formatDuration(workflow.durationMs ?? null));

  const totalTokensText = $derived(
    workflow.totalTokens ? workflow.totalTokens.toLocaleString() : null,
  );

  const agentsByPhase = $derived.by(() => {
    const map = new Map<number, typeof workflow.agents>();
    for (const agent of workflow.agents) {
      const list = map.get(agent.phaseIndex) ?? [];
      list.push(agent);
      map.set(agent.phaseIndex, list);
    }
    return map;
  });

  function toggleExpanded() {
    isExpanded = !isExpanded;
  }

  function toggleScript(e: Event) {
    e.stopPropagation();
    isScriptExpanded = !isScriptExpanded;
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="wf-card">
  <div class="wf-header" onclick={toggleExpanded}>
    <svg class="wf-chevron" class:wf-chevron-open={isExpanded} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d={CHEVRON_RIGHT}/></svg>

    <svg class="wf-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <path d="M12 2L2 7l10 5 10-5-10-5Z" />
      <path d="M2 17l10 5 10-5" />
      <path d="M2 12l10 5 10-5" />
    </svg>

    <span class="wf-name">{workflow.name ?? workflow.runId}</span>
    <span class="wf-summary">{phaseSummary}</span>

    <span class="wf-status" class:wf-status-done={workflow.status === "completed"} class:wf-status-failed={workflow.status === "partial_failure"} class:wf-status-running={workflow.status === "running"}>
      {#if workflow.status === "completed"}
        <svg class="wf-status-check" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"/><polyline points="22 4 12 14.01 9 11.01"/></svg>
      {:else if workflow.status === "running"}
        <span class="wf-spinner"></span>
      {/if}
      {statusLabel}
    </span>

    {#if totalTokensText}
      <span class="wf-tokens">{totalTokensText} tk</span>
    {/if}

    {#if durationText}
      <span class="wf-duration">{durationText}</span>
    {/if}
  </div>

  {#if isExpanded}
    <div class="wf-body">
      {#if workflow.status === "running" && workflow.phases.length === 0}
        <div class="wf-running-minimal">Details available after completion</div>
      {:else if workflow.agents.length === 0}
        <div class="wf-empty">No subagents</div>
      {:else}
        {#each workflow.phases as phase (phase.index)}
          <div class="wf-phase">
            <div class="wf-phase-title">{phase.title}</div>
            <div class="wf-chips">
              {#each agentsByPhase.get(phase.index) ?? [] as agent (agent.label)}
                <div class="wf-chip" class:wf-chip-failed={agent.status === "failed"}>
                  <span class="wf-chip-dot" class:wf-dot-done={agent.status === "done"} class:wf-dot-failed={agent.status === "failed"} class:wf-dot-running={agent.status === "running"} class:wf-dot-queued={agent.status === "queued" || agent.status === "cached"}></span>
                  <span class="wf-chip-label">{agent.label}</span>
                  {#if agent.tokens}
                    <span class="wf-chip-meta">{agent.tokens.toLocaleString()} tk</span>
                  {/if}
                  {#if agent.durationMs}
                    <span class="wf-chip-meta">{formatDuration(agent.durationMs)}</span>
                  {/if}
                </div>
              {/each}
            </div>
          </div>
        {/each}
      {/if}

      {#if workflow.scriptPreview}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="wf-script-toggle" onclick={toggleScript}>
          <svg class="wf-script-chevron" class:wf-chevron-open={isScriptExpanded} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d={CHEVRON_RIGHT}/></svg>
          <span>View script</span>
        </div>
        {#if isScriptExpanded}
          <pre class="wf-script">{workflow.scriptPreview}</pre>
        {/if}
      {/if}
    </div>
  {/if}
</div>

<style>
  .wf-card {
    border-radius: var(--radius-md);
    border: 1px solid var(--card-border);
    background: var(--card-bg);
    overflow: hidden;
  }

  .wf-header {
    display: flex;
    align-items: center;
    gap: var(--bubble-header-gap);
    padding: var(--bubble-header-padding-l1);
    cursor: pointer;
    transition: background-color 0.12s ease;
  }
  .wf-header:hover {
    background: var(--card-header-hover);
  }

  .wf-chevron {
    width: var(--bubble-icon-md);
    height: var(--bubble-icon-md);
    flex-shrink: 0;
    color: var(--card-icon-muted);
    transition: transform 0.15s ease;
  }
  .wf-chevron-open {
    transform: rotate(90deg);
  }

  .wf-icon {
    width: var(--bubble-icon-md);
    height: var(--bubble-icon-md);
    flex-shrink: 0;
    color: var(--color-accent-blue);
  }

  .wf-name {
    font-size: 12px;
    font-weight: 500;
    color: var(--card-text-light);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .wf-summary {
    font-size: 11px;
    color: var(--card-icon-muted);
    white-space: nowrap;
    flex: 1;
    min-width: 0;
  }

  .wf-status {
    font-size: 10px;
    font-weight: 500;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    padding: 1px 6px;
    border-radius: var(--radius-xs);
    white-space: nowrap;
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .wf-status-done {
    color: var(--color-success-bright);
  }
  .wf-status-failed {
    color: var(--color-error);
    background: color-mix(in oklch, var(--color-error) 10%, transparent);
    border: 1px solid color-mix(in oklch, var(--color-error) 20%, transparent);
  }
  .wf-status-running {
    color: var(--color-accent-blue);
  }
  .wf-status-check {
    width: 14px;
    height: 14px;
    flex-shrink: 0;
  }

  .wf-spinner {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    border: 1.5px solid color-mix(in oklch, var(--color-accent-blue) 18%, transparent);
    border-top-color: var(--color-accent-blue);
    box-sizing: border-box;
    animation: wf-spin 1.2s linear infinite;
  }
  @keyframes wf-spin {
    to { transform: rotate(360deg); }
  }
  @media (prefers-reduced-motion: reduce) {
    .wf-spinner { animation: none; }
  }

  .wf-tokens {
    font-size: 11px;
    font-family: var(--font-mono);
    color: var(--card-icon-muted);
    flex-shrink: 0;
    font-variant-numeric: tabular-nums;
  }

  .wf-duration {
    font-size: 11px;
    font-family: var(--font-mono);
    color: var(--card-icon-muted);
    flex-shrink: 0;
    font-variant-numeric: tabular-nums;
  }

  .wf-body {
    padding: 8px 14px 12px;
    border-top: 1px solid var(--card-border);
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .wf-running-minimal, .wf-empty {
    font-size: 12px;
    color: var(--color-text-muted);
    font-style: italic;
    padding: 4px 0;
  }

  .wf-phase {
    padding-left: 12px;
    border-left: 2px solid var(--card-border);
  }

  .wf-phase-title {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--card-icon-muted);
    margin-bottom: 6px;
  }

  .wf-chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .wf-chip {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 10px;
    min-width: 140px;
    border-radius: var(--radius-sm);
    background: var(--card-header-bg);
    border: 1px solid var(--card-border);
    font-size: 12px;
    transition: border-color 0.12s ease;
  }
  .wf-chip-failed {
    border-color: color-mix(in oklch, var(--color-error) 30%, transparent);
  }

  .wf-chip-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }
  .wf-dot-done { background-color: var(--color-success-bright); }
  .wf-dot-failed { background-color: var(--color-error); }
  .wf-dot-running { background-color: var(--color-accent-blue); }
  .wf-dot-queued {
    border: 1.5px solid var(--card-icon-muted);
    background: transparent;
  }

  .wf-chip-label {
    color: var(--card-text-light);
    font-family: var(--font-mono);
    white-space: nowrap;
  }

  .wf-chip-meta {
    font-size: 10px;
    color: var(--card-icon-muted);
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .wf-script-toggle {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    color: var(--color-text-muted);
    cursor: pointer;
    padding: 4px 0;
  }
  .wf-script-toggle:hover {
    color: var(--card-text-light);
  }
  .wf-script-chevron {
    width: 12px;
    height: 12px;
    flex-shrink: 0;
    color: var(--card-icon-muted);
    transition: transform 0.15s ease;
  }

  .wf-script {
    font-size: 11px;
    font-family: var(--font-mono);
    color: var(--card-text-light);
    background: var(--card-header-bg);
    border: 1px solid var(--card-border);
    border-radius: var(--radius-sm);
    padding: 8px 10px;
    margin: 0;
    overflow-x: auto;
    white-space: pre-wrap;
    word-break: break-all;
  }
</style>

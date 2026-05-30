<script lang="ts">
  import type { JobSummary } from "../lib/types/jobs";
  import {
    stateToColor,
    extractProjectId,
    formatAge,
    stopJob,
  } from "../lib/jobsStore.svelte";
  import { openSessionTab } from "../lib/tabStore.svelte";

  interface Props {
    job: JobSummary;
  }

  let { job }: Props = $props();

  const isWorking = $derived(job.state === "working");
  const isTerminal = $derived(job.state === "done" || job.state === "failed" || job.state === "stopped");
  const color = $derived(stateToColor(job.state));
  const age = $derived(formatAge(job.updatedAt));
  const prChild = $derived(job.children.find((c) => c.kind === "pr"));
  const prNumber = $derived(prChild?.href?.match(/\/pull\/(\d+)/)?.[1] ?? null);

  function handleOpenSession() {
    if (!job.sessionId) return;
    const projectId = extractProjectId(job) ?? "";
    openSessionTab(job.sessionId, projectId, job.name);
  }

  function handleOpenPR(e: Event) {
    e.preventDefault();
    if (!prChild?.href) return;
    window.open(prChild.href, "_blank");
  }

  let stopping = $state(false);
  let stopError: string | null = $state(null);

  async function handleStop() {
    stopping = true;
    stopError = null;
    try {
      await stopJob(job.id);
    } catch (err) {
      stopError = err instanceof Error ? err.message : String(err);
    } finally {
      stopping = false;
    }
  }
</script>

<div class="job-row" class:terminal={isTerminal}>
  <div class="row-line-1">
    <div class="indicator" style:--indicator-color={color}>
      {#if isWorking}
        <div class="spinner"></div>
      {:else}
        <div class="dot"></div>
      {/if}
    </div>

    <span class="job-name">{job.name || job.id.slice(0, 8)}</span>

    {#if prNumber}
      <a
        class="pr-chip"
        href={prChild?.href ?? "#"}
        target="_blank"
        rel="noopener"
        onclick={handleOpenPR}
      >#{prNumber}</a>
    {/if}

    <span class="job-age">{age}</span>
  </div>

  {#if job.detail}
    <div class="row-line-2">{job.detail}</div>
  {/if}

  <div class="row-actions">
    {#if job.sessionId}
      <button class="action-link" onclick={handleOpenSession}>打开 session →</button>
    {/if}
    {#if isWorking}
      <button class="action-link danger" onclick={handleStop} disabled={stopping}>
        {stopping ? "Stopping..." : "Stop"}
      </button>
    {/if}
  </div>

  {#if stopError}
    <div class="row-error">{stopError}</div>
  {/if}
</div>

<style>
  .job-row {
    display: flex;
    flex-direction: column;
    padding: 10px 12px;
    border-radius: 6px;
    gap: 3px;
  }

  .job-row:hover {
    background: var(--color-surface-raised);
  }

  .job-row.terminal .job-name {
    color: var(--color-text-muted);
    font-weight: 400;
  }

  .row-line-1 {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }

  .indicator {
    flex-shrink: 0;
    width: 14px;
    height: 14px;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--indicator-color);
  }

  .terminal .dot {
    opacity: 0.5;
  }

  .spinner {
    width: 10px;
    height: 10px;
    border: 1.5px solid color-mix(in srgb, var(--indicator-color) 20%, transparent);
    border-top-color: var(--indicator-color);
    border-radius: 50%;
    animation: job-spin 1.2s linear infinite;
  }

  @keyframes job-spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }

  .job-name {
    font-size: 13px;
    font-weight: 500;
    color: var(--color-text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    flex: 1;
    min-width: 0;
  }

  .pr-chip {
    flex-shrink: 0;
    font-size: 11px;
    font-family: var(--font-mono);
    font-weight: 500;
    padding: 1px 6px;
    border-radius: 4px;
    background: color-mix(in srgb, var(--color-success-bright) 12%, transparent);
    color: var(--color-success, var(--color-success-bright));
    text-decoration: none;
    white-space: nowrap;
    transition: background 0.12s ease-out;
  }

  .pr-chip:hover {
    background: color-mix(in srgb, var(--color-success-bright) 22%, transparent);
    text-decoration: underline;
  }

  .job-age {
    flex-shrink: 0;
    font-size: 11px;
    font-family: var(--font-mono);
    color: var(--color-text-muted);
    white-space: nowrap;
  }

  .row-line-2 {
    padding-left: 22px;
    font-size: 12px;
    color: var(--color-text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    line-height: 1.4;
  }

  .row-actions {
    padding-left: 22px;
    display: flex;
    gap: 12px;
    margin-top: 4px;
  }

  .action-link {
    font-size: 11px;
    color: var(--color-accent-blue);
    background: none;
    border: none;
    padding: 0;
    cursor: pointer;
    text-decoration: none;
    transition: color 150ms ease-out, text-decoration-color 150ms ease-out;
  }

  .action-link:hover {
    color: var(--color-accent-blue);
    text-decoration: underline;
  }

  .action-link.danger {
    color: var(--color-danger);
  }

  .action-link.danger:hover {
    color: var(--color-danger);
    text-decoration: underline;
  }

  .action-link:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .row-error {
    padding-left: 22px;
    font-size: 11px;
    color: var(--color-danger);
    margin-top: 2px;
  }
</style>

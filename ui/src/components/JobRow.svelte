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

  function handleOpenSession(e: Event) {
    e.preventDefault();
    if (!job.sessionId) return;
    const projectId = extractProjectId(job) ?? "";
    openSessionTab(job.sessionId, projectId, job.name);
  }

  function handleOpenPR(e: Event) {
    e.preventDefault();
    e.stopPropagation();
    if (!prChild?.href) return;
    window.open(prChild.href, "_blank");
  }

  let stopping = $state(false);

  async function handleStop(e: Event) {
    e.preventDefault();
    e.stopPropagation();
    stopping = true;
    try {
      await stopJob(job.id);
    } catch {
      // 静默——job 列表会自动刷新反映结果
    } finally {
      stopping = false;
    }
  }
</script>

<div class="job-row">
  <div class="row-line-1">
    <div class="indicator" style:--indicator-color={color}>
      {#if isWorking}
        <div class="spinner"></div>
      {:else}
        <div class="dot"></div>
      {/if}
    </div>

    <!-- svelte-ignore a11y_invalid_attribute -->
    <a class="job-name" class:muted={isTerminal} href="#" onclick={handleOpenSession}>
      {job.name || job.id.slice(0, 8)}
    </a>

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

    {#if isWorking}
      <button class="stop-btn" onclick={handleStop} disabled={stopping}>stop</button>
    {/if}
  </div>

  {#if job.needs}
    <div class="row-line-2 needs">{job.needs}</div>
  {:else if job.detail}
    <div class="row-line-2">{job.detail}</div>
  {/if}
</div>

<style>
  .job-row {
    display: flex;
    flex-direction: column;
    padding: 10px 12px;
    border-radius: 6px;
    gap: 3px;
    transition: background 150ms ease-out;
  }

  .job-row:hover {
    background: var(--color-surface-raised);
  }

  .job-row:hover .stop-btn {
    opacity: 1;
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
    text-decoration: none;
    cursor: pointer;
  }

  .job-name:hover {
    text-decoration: underline;
  }

  .job-name.muted {
    color: var(--color-text-muted);
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
    transition: background 150ms ease-out;
  }

  .pr-chip:hover {
    background: color-mix(in srgb, var(--color-success-bright) 22%, transparent);
  }

  .job-age {
    flex-shrink: 0;
    font-size: 11px;
    font-family: var(--font-mono);
    color: var(--color-text-muted);
    white-space: nowrap;
  }

  .stop-btn {
    flex-shrink: 0;
    font-size: 11px;
    padding: 2px 6px;
    border-radius: 4px;
    border: none;
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
    opacity: 0;
    transition: opacity 150ms ease-out;
  }

  .stop-btn:hover {
    color: var(--color-text);
    background: var(--color-surface-overlay, var(--color-surface-raised));
  }

  .stop-btn:disabled {
    opacity: 0.3;
    cursor: not-allowed;
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

  .row-line-2.needs {
    color: var(--color-warning);
  }
</style>

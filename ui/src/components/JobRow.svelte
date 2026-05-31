<script lang="ts">
  import type { JobSummary } from "../lib/types/jobs";
  import {
    stateToColor,
    extractProjectId,
    formatAge,
    stopJob,
    deleteJob,
  } from "../lib/jobsStore.svelte";
  import { openSessionTab } from "../lib/tabStore.svelte";

  interface Props {
    job: JobSummary;
  }

  let { job }: Props = $props();

  const isWorking = $derived(job.state === "working");
  const isTerminal = $derived(
    job.state === "done" || job.state === "failed" || job.state === "stopped",
  );
  const hasPr = $derived(job.children.some((c) => c.kind === "pr"));
  const isFaded = $derived(isTerminal && !hasPr && job.state !== "failed");
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
  let deleting = $state(false);
  let confirmingDelete = $state(false);
  let confirmTimer: ReturnType<typeof setTimeout> | null = $state(null);

  async function handleStop(e: Event) {
    e.preventDefault();
    e.stopPropagation();
    stopping = true;
    try {
      await stopJob(job.id);
    } catch {
      // 静默
    } finally {
      stopping = false;
    }
  }

  function handleDeleteClick(e: Event) {
    e.preventDefault();
    e.stopPropagation();
    if (confirmingDelete) {
      if (confirmTimer) clearTimeout(confirmTimer);
      confirmTimer = null;
      confirmingDelete = false;
      void doDelete();
    } else {
      confirmingDelete = true;
      confirmTimer = setTimeout(() => {
        confirmingDelete = false;
        confirmTimer = null;
      }, 3000);
    }
  }

  async function doDelete() {
    deleting = true;
    try {
      await deleteJob(job.id);
    } catch {
      // 静默
    } finally {
      deleting = false;
    }
  }
</script>

<div class="job-row" class:faded={isFaded}>
  <div class="row-main">
    <div class="indicator" style:--indicator-color={color}>
      {#if isWorking}
        <div class="spinner"></div>
      {:else}
        <div class="dot"></div>
      {/if}
    </div>

    <!-- svelte-ignore a11y_invalid_attribute -->
    <a class="job-name" href="#" onclick={handleOpenSession}>
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
      <button class="action-btn stop" onclick={handleStop} disabled={stopping} title="Stop">
        <svg viewBox="0 0 16 16" fill="currentColor"><rect x="3" y="3" width="10" height="10" rx="1"/></svg>
      </button>
    {:else if isTerminal}
      <button
        class="action-btn dismiss"
        class:confirming={confirmingDelete}
        onclick={handleDeleteClick}
        disabled={deleting}
        title={confirmingDelete ? "再次点击确认删除" : "删除"}
      >
        {#if confirmingDelete}
          <span class="confirm-text">确认?</span>
        {:else}
          <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M4 4l8 8M12 4l-8 8"/></svg>
        {/if}
      </button>
    {/if}
  </div>

  {#if job.needs}
    <div class="row-detail needs">{job.needs}</div>
  {:else if job.detail}
    <div class="row-detail">{job.detail}</div>
  {/if}
</div>

<style>
  .job-row {
    display: flex;
    flex-direction: column;
    padding: 7px 12px;
    border-radius: 6px;
    gap: 2px;
    transition: background 120ms ease-out;
  }

  .job-row:hover {
    background: var(--color-surface-raised);
  }

  .job-row.faded {
    opacity: 0.55;
  }

  .job-row.faded:hover {
    opacity: 0.85;
  }

  .row-main {
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
    width: 6px;
    height: 6px;
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
    transition: background 120ms ease-out;
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

  .action-btn {
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 20px;
    height: 20px;
    padding: 0 4px;
    border-radius: 4px;
    border: none;
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
    opacity: 0;
    transition: opacity 120ms ease-out, background 120ms ease-out, color 120ms ease-out;
  }

  .action-btn svg {
    width: 12px;
    height: 12px;
  }

  .job-row:hover .action-btn,
  .job-row:focus-within .action-btn {
    opacity: 1;
  }

  .action-btn.confirming {
    opacity: 1;
    color: var(--color-danger);
    background: color-mix(in srgb, var(--color-danger) 8%, transparent);
  }

  .confirm-text {
    font-size: 10px;
    font-weight: 500;
    white-space: nowrap;
  }

  .action-btn:hover {
    background: var(--color-surface-overlay);
    color: var(--color-text);
  }

  .action-btn.stop:hover {
    color: var(--color-danger);
  }

  .action-btn.confirming:hover {
    background: color-mix(in srgb, var(--color-danger) 14%, transparent);
    color: var(--color-danger);
  }

  .action-btn:disabled {
    opacity: 0.3;
    cursor: not-allowed;
  }

  .row-detail {
    padding-left: 22px;
    font-size: 12px;
    color: var(--color-text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    line-height: 1.3;
  }

  .row-detail.needs {
    color: var(--color-warning);
  }
</style>

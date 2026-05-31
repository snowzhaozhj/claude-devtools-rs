<script lang="ts">
  import JobRow from "../components/JobRow.svelte";
  import { onDestroy } from "svelte";
  import {
    getJobs,
    getJobsDirExists,
    getJobsLoading,
    getJobsError,
    refreshJobs,
    groupJobs,
    deleteCompletedJobs,
  } from "../lib/jobsStore.svelte";

  const jobs = $derived(getJobs());
  const jobsDirExists = $derived(getJobsDirExists());
  const loading = $derived(getJobsLoading());
  const error = $derived(getJobsError());
  const grouped = $derived(groupJobs(jobs));

  let confirmingClear = $state(false);
  let clearTimer: ReturnType<typeof setTimeout> | null = $state(null);
  let clearing = $state(false);

  onDestroy(() => {
    if (clearTimer) clearTimeout(clearTimer);
  });

  function handleClearClick() {
    if (confirmingClear) {
      if (clearTimer) clearTimeout(clearTimer);
      clearTimer = null;
      confirmingClear = false;
      void doClear();
    } else {
      confirmingClear = true;
      clearTimer = setTimeout(() => {
        confirmingClear = false;
        clearTimer = null;
      }, 3000);
    }
  }

  async function doClear() {
    clearing = true;
    try {
      await deleteCompletedJobs();
    } catch (err) {
      console.error("[jobs] clear completed failed:", err);
    } finally {
      clearing = false;
    }
  }
</script>

<div class="jobs-view">
  <div class="jobs-header">
    <h2 class="jobs-title">Background Jobs</h2>
    <button
      class="refresh-btn"
      onclick={() => void refreshJobs()}
      disabled={loading}
      title="刷新"
      aria-label="刷新 jobs 列表"
    >
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class:spinning={loading}>
        <path d="M21 12a9 9 0 1 1-3-6.7L21 8"/>
        <path d="M21 3v5h-5"/>
      </svg>
    </button>
  </div>

  {#if error}
    <div class="jobs-error">{error}</div>
  {:else if !jobsDirExists}
    <div class="jobs-empty">
      <span class="empty-text">No background jobs</span>
      <span class="empty-hint">使用 <code>claude --bg</code> 启动后台任务</span>
    </div>
  {:else if jobs.length === 0}
    <div class="jobs-empty">
      <span class="empty-text">No background jobs</span>
    </div>
  {:else}
    <div class="jobs-list">
      {#each grouped as groupData (groupData.group)}
        <div class="job-group">
          <div class="group-header">
            <span class="group-label">{groupData.label}</span>
            <span class="group-count">{groupData.jobs.length}</span>
            {#if groupData.group === "completed" && groupData.jobs.length > 0}
              <button
                class="clear-btn"
                class:confirming={confirmingClear}
                onclick={handleClearClick}
                disabled={clearing}
              >
                {#if confirmingClear}
                  确认清除 {groupData.jobs.length} 项?
                {:else}
                  Clear
                {/if}
              </button>
            {/if}
          </div>
          {#each groupData.jobs as job (job.id)}
            <JobRow {job} />
          {/each}
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .jobs-view {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow-y: auto;
    padding: 20px 16px;
    gap: 16px;
  }

  .jobs-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-shrink: 0;
    padding: 0 4px;
  }

  .jobs-title {
    font-size: 14px;
    font-weight: 600;
    color: var(--color-text);
    margin: 0;
  }

  .refresh-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    padding: 0;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
    transition: background 0.12s ease-out, color 0.12s ease-out;
  }

  .refresh-btn:hover {
    background: var(--color-surface-raised);
    color: var(--color-text);
  }

  .refresh-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .refresh-btn svg {
    width: 14px;
    height: 14px;
  }

  .refresh-btn svg.spinning {
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }

  .jobs-empty {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 48px 0;
  }

  .empty-text {
    font-size: 13px;
    color: var(--color-text-muted);
  }

  .empty-hint {
    font-size: 12px;
    color: var(--color-text-muted);
    opacity: 0.6;
  }

  .empty-hint code {
    font-family: var(--font-mono);
    font-size: 11px;
    background: var(--color-surface-raised);
    padding: 2px 6px;
    border-radius: 4px;
  }

  .jobs-error {
    padding: 10px 12px;
    border-radius: 6px;
    font-size: 12px;
    color: var(--color-danger);
    background: color-mix(in srgb, var(--color-danger) 6%, transparent);
  }

  .jobs-list {
    display: flex;
    flex-direction: column;
    gap: 0;
  }

  .job-group {
    display: flex;
    flex-direction: column;
  }

  .job-group + .job-group {
    margin-top: 20px;
  }

  .group-header {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 16px 6px;
  }

  .group-label {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--color-text-muted);
  }

  .group-count {
    font-size: 10px;
    color: var(--color-text-muted);
    opacity: 0.5;
    font-family: var(--font-mono);
  }

  .clear-btn {
    margin-left: auto;
    font-size: 11px;
    padding: 2px 8px;
    border-radius: 4px;
    border: none;
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
    transition: background 120ms ease-out, color 120ms ease-out;
  }

  .clear-btn:hover {
    background: var(--color-surface-overlay);
    color: var(--color-text);
  }

  .clear-btn.confirming {
    color: var(--color-danger);
    background: color-mix(in srgb, var(--color-danger) 8%, transparent);
  }

  .clear-btn.confirming:hover {
    background: color-mix(in srgb, var(--color-danger) 14%, transparent);
  }

  .clear-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
</style>

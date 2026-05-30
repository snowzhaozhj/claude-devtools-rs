<script lang="ts">
  import JobRow from "../components/JobRow.svelte";
  import {
    getJobs,
    getJobsDirExists,
    getJobsLoading,
    getJobsError,
    refreshJobs,
    groupJobs,
  } from "../lib/jobsStore.svelte";

  const jobs = $derived(getJobs());
  const jobsDirExists = $derived(getJobsDirExists());
  const loading = $derived(getJobsLoading());
  const error = $derived(getJobsError());
  const grouped = $derived(groupJobs(jobs));

  let selectedJobId: string | null = $state(null);

  function handleSelectJob(jobId: string) {
    selectedJobId = selectedJobId === jobId ? null : jobId;
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
    <div class="jobs-error">
      <span class="error-text">加载失败: {error}</span>
    </div>
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
          </div>
          {#each groupData.jobs as job (job.id)}
            <JobRow
              {job}
              selected={selectedJobId === job.id}
              onSelect={() => handleSelectJob(job.id)}
            />
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
    padding: 16px;
    gap: 12px;
  }

  .jobs-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-shrink: 0;
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
    width: 24px;
    height: 24px;
    padding: 0;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
    transition: background 0.12s ease, color 0.12s ease;
  }

  .refresh-btn:hover {
    background: var(--tool-item-hover-bg);
    color: var(--color-text);
  }

  .refresh-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .refresh-btn svg {
    width: 14px;
    height: 14px;
  }

  .refresh-btn svg.spinning {
    animation: spin 1s linear infinite;
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
    gap: 8px;
  }

  .empty-text {
    font-size: 13px;
    color: var(--color-text-muted);
  }

  .empty-hint {
    font-size: 12px;
    color: var(--color-text-muted);
    opacity: 0.7;
  }

  .empty-hint code {
    font-family: var(--font-mono);
    font-size: 11px;
    background: var(--color-surface-raised);
    padding: 1px 4px;
    border-radius: 3px;
  }

  .jobs-error {
    padding: 12px;
    border-radius: 6px;
    background: color-mix(in srgb, var(--color-danger) 10%, transparent);
    border: 1px solid color-mix(in srgb, var(--color-danger) 30%, transparent);
  }

  .error-text {
    font-size: 12px;
    color: var(--color-danger);
  }

  .jobs-list {
    display: flex;
    flex-direction: column;
    gap: 16px;
    border: 1px solid var(--color-border-default);
    border-radius: 8px;
    padding: 8px 0;
  }

  .job-group {
    display: flex;
    flex-direction: column;
  }

  .group-header {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 12px 6px;
  }

  .group-label {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.03em;
    color: var(--color-text-muted);
  }

  .group-count {
    font-size: 10px;
    color: var(--color-text-muted);
    opacity: 0.7;
    font-family: var(--font-mono);
  }
</style>

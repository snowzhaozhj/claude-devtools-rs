<script lang="ts">
  import type { JobSummary } from "../lib/types/jobs";
  import {
    stateToColor,
    extractProjectId,
    formatAge,
    stopJob,
  } from "../lib/jobsStore.svelte";
  import { openSessionTab } from "../lib/tabStore.svelte";
  import { CHEVRON_RIGHT } from "../lib/icons";

  interface Props {
    job: JobSummary;
    selected: boolean;
    onSelect: () => void;
  }

  let { job, selected, onSelect }: Props = $props();

  let expanded = $state(false);

  const isWorking = $derived(job.state === "working");
  const isStopped = $derived(job.state === "stopped");
  const color = $derived(stateToColor(job.state));
  const age = $derived(formatAge(job.updatedAt));
  const prChild = $derived(job.children.find((c) => c.kind === "pr"));

  function handleClick() {
    onSelect();
  }

  function toggleExpand(e: Event) {
    e.stopPropagation();
    expanded = !expanded;
  }

  function handleOpenSession(e: Event) {
    e.stopPropagation();
    if (!job.sessionId) return;
    const projectId = extractProjectId(job) ?? "";
    openSessionTab(job.sessionId, projectId, job.name);
  }

  function handleOpenPR(e: Event) {
    e.stopPropagation();
    if (!prChild?.href) return;
    window.open(prChild.href, "_blank");
  }

  function handleStop(e: Event) {
    e.stopPropagation();
    void stopJob(job.id);
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="job-row"
  class:selected
  class:expanded
  onclick={handleClick}
  onkeydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); handleClick(); } }}
  tabindex="0"
  role="button"
  aria-expanded={expanded}
>
  <div class="row-main">
    <!-- 状态 indicator -->
    <div class="indicator" style:--indicator-color={color}>
      {#if isWorking}
        <div class="spinner"></div>
      {:else}
        <div class="dot" class:stopped={isStopped}></div>
      {/if}
    </div>

    <!-- 名称 -->
    <span class="job-name">{job.name}</span>

    <!-- 详情 -->
    {#if job.detail}
      <span class="job-detail">{job.detail}</span>
    {/if}

    <!-- PR chip -->
    {#if prChild}
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <span
        class="pr-chip"
        role="button"
        tabindex="-1"
        onclick={handleOpenPR}
        title={prChild.href ?? ""}
      >
        PR
      </span>
    {/if}

    <!-- age -->
    <span class="job-age">{age}</span>

    <!-- chevron -->
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <span
      class="chevron"
      class:chevron-expanded={expanded}
      role="button"
      tabindex="-1"
      onclick={toggleExpand}
      aria-label={expanded ? "收起详情" : "展开详情"}
    >
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d={CHEVRON_RIGHT} />
      </svg>
    </span>
  </div>

  <!-- 选中态左侧 indicator -->
  {#if selected}
    <div class="selection-indicator"></div>
  {/if}

  <!-- 展开区 -->
  {#if expanded}
    <div class="expand-area">
      {#if job.intent}
        <div class="intent-line">
          <div class="intent-bar"></div>
          <span class="intent-text">{job.intent}</span>
        </div>
      {/if}

      {#if job.detail}
        <div class="expand-detail">{job.detail}</div>
      {/if}

      <!-- metadata chips -->
      <div class="metadata-row">
        <span class="meta-chip">ID: {job.id.slice(0, 8)}</span>
        {#if job.cwd}
          <span class="meta-chip">{job.cwd}</span>
        {/if}
        <span class="meta-chip">Updated {formatAge(job.updatedAt)} ago</span>
      </div>

      <!-- 操作按钮 -->
      <div class="actions-row">
        {#if prChild?.href}
          <button class="action-btn primary" onclick={handleOpenPR}>
            Review PR →
          </button>
        {/if}
        {#if job.sessionId}
          <button class="action-btn" onclick={handleOpenSession}>
            打开 session →
          </button>
        {/if}
        {#if job.group === "working"}
          <button class="action-btn btn-stop" onclick={handleStop}>
            Stop
          </button>
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .job-row {
    position: relative;
    display: flex;
    flex-direction: column;
    padding: 8px 10px;
    cursor: pointer;
    transition: background 0.15s ease-out;
    border-radius: 6px;
  }

  .job-row:hover {
    background: var(--color-surface-raised);
  }

  .job-row.selected {
    background: var(--color-surface-raised);
  }

  .job-row.expanded {
    background: var(--color-surface-raised);
  }

  .selection-indicator {
    position: absolute;
    left: 0;
    top: 6px;
    bottom: 6px;
    width: 2px;
    border-radius: 1px;
    background: var(--color-accent-blue);
  }

  .row-main {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }

  .indicator {
    flex-shrink: 0;
    width: 16px;
    height: 16px;
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

  .dot.stopped {
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

  .job-detail {
    font-size: 12px;
    color: var(--color-text-secondary, var(--color-text-muted));
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    flex: 0 1 auto;
    max-width: 240px;
    min-width: 0;
  }

  .pr-chip {
    flex-shrink: 0;
    font-size: 10px;
    font-family: var(--font-mono);
    font-weight: 500;
    padding: 2px 6px;
    border-radius: 4px;
    background: color-mix(in srgb, var(--color-success-bright) 12%, transparent);
    color: var(--color-success, var(--color-success-bright));
    cursor: pointer;
    white-space: nowrap;
    transition: background 0.15s ease-out;
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
    opacity: 0.7;
  }

  .chevron {
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    border-radius: 4px;
    color: var(--color-text-muted);
    transition: transform 0.15s ease-out, background 0.15s ease-out;
    cursor: pointer;
    opacity: 0.6;
  }

  .chevron:hover {
    background: var(--color-surface-overlay, var(--color-surface-raised));
    opacity: 1;
  }

  .chevron svg {
    width: 12px;
    height: 12px;
  }

  .chevron-expanded {
    transform: rotate(90deg);
    opacity: 1;
  }

  .expand-area {
    margin-top: 8px;
    margin-left: 26px;
    padding: 10px 14px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    border: 1px solid var(--color-border-default);
    border-radius: 6px;
    background: var(--color-surface);
  }

  .intent-line {
    display: flex;
    align-items: flex-start;
    gap: 10px;
  }

  .intent-bar {
    width: 2px;
    min-height: 16px;
    align-self: stretch;
    background: var(--color-border-emphasis);
    border-radius: 1px;
    flex-shrink: 0;
  }

  .intent-text {
    font-size: 12px;
    font-style: italic;
    color: var(--color-text-secondary, var(--color-text-muted));
    line-height: 1.5;
  }

  .expand-detail {
    font-size: 12px;
    color: var(--color-text-muted);
    line-height: 1.5;
  }

  .metadata-row {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .meta-chip {
    font-size: 10px;
    font-family: var(--font-mono);
    padding: 2px 6px;
    border-radius: 4px;
    background: var(--color-surface-raised);
    color: var(--color-text-muted);
  }

  .actions-row {
    display: flex;
    gap: 8px;
    margin-top: 2px;
  }

  .action-btn {
    font-size: 12px;
    font-weight: 500;
    padding: 4px 10px;
    border-radius: 4px;
    border: 1px solid var(--color-border-default);
    background: var(--color-surface);
    color: var(--color-text);
    cursor: pointer;
    transition: background 0.15s ease-out, border-color 0.15s ease-out;
  }

  .action-btn:hover {
    background: var(--color-surface-raised);
    border-color: var(--color-border-emphasis);
  }

  .action-btn.primary {
    background: color-mix(in srgb, var(--color-accent-blue) 8%, var(--color-surface));
    border-color: color-mix(in srgb, var(--color-accent-blue) 25%, transparent);
    color: var(--color-accent-blue);
  }

  .action-btn.primary:hover {
    background: color-mix(in srgb, var(--color-accent-blue) 15%, var(--color-surface));
  }

  .action-btn.btn-stop {
    border-color: color-mix(in srgb, var(--color-danger) 25%, transparent);
    color: var(--color-danger);
  }

  .action-btn.btn-stop:hover {
    background: color-mix(in srgb, var(--color-danger) 8%, var(--color-surface));
  }
</style>

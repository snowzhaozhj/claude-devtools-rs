<script lang="ts">
  import { onDestroy } from "svelte";
  import type { WorkflowItem, WorkflowAgent, Chunk } from "../lib/api";
  import { getWorkflowAgentTrace, getWorkflowDetail } from "../lib/api";
  import { buildDisplayItemsFromChunks } from "../lib/displayItemBuilder";
  import { formatDuration } from "../lib/formatters";
  import { CHEVRON_RIGHT } from "../lib/icons";
  import { activateOnKey } from "../lib/a11y";
  import ExecutionTrace from "./ExecutionTrace.svelte";

  interface Props {
    workflow: WorkflowItem;
    sessionId: string;
    projectId: string;
  }

  let { workflow, sessionId, projectId }: Props = $props();

  let isExpanded = $state(false);
  let expandedAgentId = $state<string | null>(null);
  let agentTrace = $state<Chunk[] | null>(null);
  let isLoadingAgentTrace = $state(false);
  const agentDisplayItems = $derived(agentTrace ? buildDisplayItemsFromChunks(agentTrace) : []);

  // --- Poll state for running workflows ---
  let fullDetail: WorkflowItem | null = $state(null);
  let pollTimer: ReturnType<typeof setTimeout> | null = null;
  let pollGeneration = 0;

  // fullDetail（来自 poll 的最新数据）优先；否则用 props workflow（来自 session detail）
  const effectiveWorkflow = $derived(fullDetail ?? workflow);

  const phases = $derived(effectiveWorkflow.phases ?? []);
  const agents = $derived(effectiveWorkflow.agents ?? []);

  const statusLabel = $derived.by(() => {
    switch (effectiveWorkflow.status) {
      case "completed": return "Done";
      case "partial_failure": {
        const failedCount = agents.filter(a => a.state === "failed").length;
        return `${failedCount} failed`;
      }
      case "running": return "Running";
      case "pending": return "Pending";
      default: return effectiveWorkflow.status;
    }
  });

  const doneCount = $derived(agents.filter(a => a.state === "completed").length);

  const phaseSummary = $derived.by(() => {
    if (effectiveWorkflow.status === "running") {
      return `${agents.length} agent${agents.length !== 1 ? "s" : ""} (${doneCount} done)`;
    }
    return `${phases.length} phase${phases.length !== 1 ? "s" : ""} · ${agents.length} agent${agents.length !== 1 ? "s" : ""}`;
  });

  const durationText = $derived(formatDuration(effectiveWorkflow.durationMs || null));

  const totalTokensText = $derived(
    effectiveWorkflow.totalTokens ? effectiveWorkflow.totalTokens.toLocaleString() : null,
  );

  const agentsByPhase = $derived.by(() => {
    const map = new Map<number, typeof agents>();
    for (const agent of agents) {
      const list = map.get(agent.phaseIndex) ?? [];
      list.push(agent);
      map.set(agent.phaseIndex, list);
    }
    return map;
  });

  const POLL_INTERVAL_MS = 3000;
  const MAX_POLL_FAILURES = 5;

  // --- Poll logic for running/pending workflows ---
  // 用 setTimeout 链（await 完成后再排下一次）而非 setInterval：IPC 慢于轮询间隔时
  // setInterval 会堆积并发请求 + 响应乱序覆盖。链式调度天然串行、单次在途。
  // 连续失败 MAX_POLL_FAILURES 次即停，避免 workflow 已删除时空 catch 无限重试。
  function startPoll(): void {
    if (pollTimer) return;
    const gen = ++pollGeneration;
    let failures = 0;
    const tick = async () => {
      pollTimer = null;
      if (gen !== pollGeneration) return;
      try {
        const fresh = await getWorkflowDetail(projectId, sessionId, workflow.runId);
        if (gen !== pollGeneration) return;
        failures = 0;
        fullDetail = fresh;
        if (isTerminal(fresh.status)) return;
      } catch {
        if (gen !== pollGeneration) return;
        if (++failures >= MAX_POLL_FAILURES) return; // workflow 可能已删除，停止重试
      }
      pollTimer = setTimeout(tick, POLL_INTERVAL_MS);
    };
    pollTimer = setTimeout(tick, POLL_INTERVAL_MS);
  }

  function stopPoll(): void {
    if (pollTimer) { clearTimeout(pollTimer); pollTimer = null; }
    // 让在途 tick（已过 pollTimer=null 行、正 await）的 gen 检查失效，
    // 杜绝其 await 返回后再次 schedule。
    pollGeneration++;
  }

  function isTerminal(status: string): boolean {
    return status !== "running" && status !== "pending";
  }

  function toggleExpanded() {
    isExpanded = !isExpanded;
    if (isExpanded) {
      // 展开且非终态时开始轮询以获取最新 workflow 状态
      if (!isTerminal(effectiveWorkflow.status)) {
        startPoll();
      }
    } else {
      stopPoll();
    }
  }

  // 当 workflow status 变为终态时停止轮询
  $effect(() => {
    const status = effectiveWorkflow.status;
    if (isTerminal(status)) {
      stopPoll();
    }
  });

  onDestroy(() => {
    stopPoll();
  });

  async function toggleAgentDrilldown(agent: WorkflowAgent) {
    if (!agent.sessionId) return;
    if (expandedAgentId === agent.sessionId) {
      expandedAgentId = null;
      agentTrace = null;
      return;
    }
    expandedAgentId = agent.sessionId;
    agentTrace = null;
    isLoadingAgentTrace = true;
    try {
      const chunks = await getWorkflowAgentTrace(projectId, sessionId, workflow.runId, agent.sessionId);
      if (expandedAgentId === agent.sessionId) {
        agentTrace = chunks;
      }
    } catch {
      if (expandedAgentId === agent.sessionId) {
        agentTrace = null;
      }
    } finally {
      if (expandedAgentId === agent.sessionId) {
        isLoadingAgentTrace = false;
      }
    }
  }
</script>

<div class="wf-card">
  <div
    class="wf-header"
    role="button"
    tabindex="0"
    aria-expanded={isExpanded}
    onclick={toggleExpanded}
    onkeydown={(e) => activateOnKey(e, toggleExpanded)}
  >
    <svg class="wf-chevron" class:wf-chevron-open={isExpanded} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d={CHEVRON_RIGHT}/></svg>

    <svg class="wf-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <path d="M12 2L2 7l10 5 10-5-10-5Z" />
      <path d="M2 17l10 5 10-5" />
      <path d="M2 12l10 5 10-5" />
    </svg>

    <span class="wf-name">{effectiveWorkflow.name ?? effectiveWorkflow.runId}</span>
    <span class="wf-summary">{phaseSummary}</span>

    <span class="wf-status" class:wf-status-done={effectiveWorkflow.status === "completed"} class:wf-status-failed={effectiveWorkflow.status === "partial_failure"} class:wf-status-running={effectiveWorkflow.status === "running"}>
      {#if effectiveWorkflow.status === "completed"}
        <svg class="wf-status-check" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"/><polyline points="22 4 12 14.01 9 11.01"/></svg>
      {:else if effectiveWorkflow.status === "running"}
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

  <!-- index 由调用点传入（运行态 = 全局 agents 顺序；完成态 = phase 内序号，label 恒
       非空不触发 fallback）。避免在 each 内 agents.indexOf() 造成 O(n²) 渲染。 -->
  {#snippet chipInner(agent: WorkflowAgent, index: number)}
    <span class="wf-chip-dot" class:wf-dot-done={agent.state === "completed"} class:wf-dot-failed={agent.state === "failed"} class:wf-dot-running={agent.state === "running"} class:wf-dot-queued={agent.state === "pending"}></span>
    <span class="wf-chip-label">{agent.label || `Agent ${index + 1}`}</span>
    {#if agent.tokens}
      <span class="wf-chip-meta">{agent.tokens.toLocaleString()} tk</span>
    {/if}
    {#if agent.durationMs}
      <span class="wf-chip-meta">{formatDuration(agent.durationMs)}</span>
    {/if}
    {#if agent.sessionId}
      <svg class="wf-chip-expand" class:wf-chip-expand-open={expandedAgentId === agent.sessionId} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d={CHEVRON_RIGHT}/></svg>
    {/if}
  {/snippet}

  {#snippet agentChip(agent: WorkflowAgent, index: number)}
    {#if agent.sessionId}
      <!-- 有 sessionId 才可下钻 → 渲染为可聚焦的 button 语义 -->
      <div
        class="wf-chip wf-chip-clickable"
        class:wf-chip-failed={agent.state === "failed"}
        class:wf-chip-active={expandedAgentId === agent.sessionId}
        role="button"
        tabindex="0"
        aria-expanded={expandedAgentId === agent.sessionId}
        onclick={() => toggleAgentDrilldown(agent)}
        onkeydown={(e) => activateOnKey(e, () => toggleAgentDrilldown(agent))}
      >
        {@render chipInner(agent, index)}
      </div>
    {:else}
      <!-- 无 trace 可展开 → 纯信息 chip，不暴露交互语义 -->
      <div class="wf-chip" class:wf-chip-failed={agent.state === "failed"}>
        {@render chipInner(agent, index)}
      </div>
    {/if}
    {#if expandedAgentId === agent.sessionId}
      <div class="wf-agent-trace">
        {#if isLoadingAgentTrace}
          <div class="wf-trace-loading">Loading trace…</div>
        {:else if agentDisplayItems.length > 0}
          <ExecutionTrace items={agentDisplayItems} rootSessionId={sessionId} />
        {:else}
          <div class="wf-trace-empty">No trace data</div>
        {/if}
      </div>
    {/if}
  {/snippet}

  {#if isExpanded}
    <div class="wf-body">
      {#if agents.length === 0}
        {#if effectiveWorkflow.status === "running"}
          <div class="wf-running-minimal">Running…</div>
        {:else}
          <div class="wf-empty">No subagents</div>
        {/if}
      {:else if effectiveWorkflow.status === "running"}
        <!-- 运行态：合成 agent 无法归属 phase（journal 无 phase 标记）。
             Tier 1 解出 phases 时仅作静态列表展示在 chips 之上；agent 扁平排列。 -->
        {#if phases.length > 0}
          <div class="wf-phase-list">
            {#each phases as phase (phase.index)}
              <span class="wf-phase-pill">{phase.title}</span>
            {/each}
          </div>
        {/if}
        <div class="wf-chips">
          {#each agents as agent, idx (idx)}
            {@render agentChip(agent, idx)}
          {/each}
        </div>
      {:else}
        <!-- 完成态 / 部分失败态：agent 有真实 phaseIndex，按 phase 分组 -->
        {#each phases as phase (phase.index)}
          <div class="wf-phase">
            <div class="wf-phase-title">{phase.title}</div>
            <div class="wf-chips">
              {#each agentsByPhase.get(phase.index) ?? [] as agent, idx (`${phase.index}-${idx}`)}
                {@render agentChip(agent, idx)}
              {/each}
            </div>
          </div>
        {/each}
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
  .wf-header:focus-visible {
    outline: 2px solid var(--color-accent-blue);
    outline-offset: -2px;
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

  /* 运行态 Tier 1：静态 phase 列表（仅标题，无当前 phase 高亮） */
  .wf-phase-list {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .wf-phase-pill {
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--card-icon-muted);
    background: var(--card-header-bg);
    border: 1px solid var(--card-border);
    border-radius: var(--radius-xs);
    padding: 2px 8px;
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

  .wf-chip-clickable {
    cursor: pointer;
  }
  .wf-chip-clickable:hover {
    border-color: var(--color-accent-blue);
    background: color-mix(in oklch, var(--color-accent-blue) 5%, var(--card-header-bg));
  }
  .wf-chip-clickable:focus-visible {
    outline: 2px solid var(--color-accent-blue);
    outline-offset: 1px;
  }
  .wf-chip-active {
    border-color: var(--color-accent-blue);
    background: color-mix(in oklch, var(--color-accent-blue) 8%, var(--card-header-bg));
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

  .wf-chip-expand {
    width: 12px;
    height: 12px;
    flex-shrink: 0;
    color: var(--card-icon-muted);
    margin-left: auto;
    transition: transform 0.15s ease;
  }
  .wf-chip-expand-open {
    transform: rotate(90deg);
  }

  .wf-agent-trace {
    width: 100%;
    border: 1px solid var(--card-border);
    border-radius: var(--radius-sm);
    background: var(--card-bg);
    padding: 8px;
    margin-top: 4px;
    margin-bottom: 4px;
    max-height: 400px;
    overflow-y: auto;
    /* trace 流式增长越过 max-height 时滚动条弹出不得压缩内部 ExecutionTrace 宽度 */
    scrollbar-gutter: stable;
  }

  .wf-trace-loading, .wf-trace-empty {
    font-size: 12px;
    color: var(--color-text-muted);
    font-style: italic;
    padding: 8px 4px;
  }

</style>

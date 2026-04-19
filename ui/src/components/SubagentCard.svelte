<script lang="ts">
  import type { SubagentProcess, ContentBlock, ToolCall, Chunk } from "../lib/api";
  import { getSubagentTrace } from "../lib/api";
  import { CHEVRON_RIGHT, TERMINAL } from "../lib/icons";
  import { getTeamColorSet, getSubagentTypeColorSet, type TeamColorSet } from "../lib/teamColors";
  import { getAgentConfigsByName } from "../lib/agentConfigsStore.svelte";
  import { formatDuration } from "../lib/formatters";
  import { buildDisplayItemsFromChunks, buildSummary } from "../lib/displayItemBuilder";
  import { parseModelString } from "../lib/modelParser";
  import MetricsPill from "./MetricsPill.svelte";
  import ExecutionTrace from "./ExecutionTrace.svelte";

  interface Props {
    process: SubagentProcess;
    /** 顶层 SessionDetail 的 sessionId，用于 getSubagentTrace 懒拉取。
        嵌套 SubagentCard 必须把同值一路传递（不变）。 */
    rootSessionId: string;
    /** 嵌套深度，由 ExecutionTrace 传递；顶层 0 */
    depth?: number;
  }

  let { process, rootSessionId, depth = 0 }: Props = $props();

  let isExpanded = $state(false);
  let isTraceExpanded = $state(false);

  // Lazy trace cache：messages 在 IPC 已被裁空时通过 getSubagentTrace 拉取后填入。
  // 同 SubagentCard 实例多次展开/折叠不重拉。
  let messagesLocal: Chunk[] | null = $state(null);
  let isLoadingTrace = $state(false);

  // 派生 messages：优先用本地缓存，fallback 到 process.messages
  // （后端 OMIT_SUBAGENT_MESSAGES=false 或老后端时直接用 process.messages）。
  const effectiveMessages = $derived<Chunk[]>(messagesLocal ?? process.messages);

  async function ensureMessages(): Promise<void> {
    if (messagesLocal != null) return;
    if (!process.messagesOmitted) {
      // 老后端 / 回滚开关 false：messages 已是完整
      messagesLocal = process.messages;
      return;
    }
    isLoadingTrace = true;
    try {
      messagesLocal = await getSubagentTrace(rootSessionId, process.sessionId);
    } catch (e) {
      console.warn("getSubagentTrace failed:", e);
      messagesLocal = []; // 失败也别一直 loading；显示空 trace
    } finally {
      isLoadingTrace = false;
    }
  }

  // ----------------- 颜色 -----------------
  const colorSet: TeamColorSet = $derived.by(() => {
    if (process.team) return getTeamColorSet(process.team.memberColor);
    if (process.subagentType) {
      return getSubagentTypeColorSet(process.subagentType, getAgentConfigsByName());
    }
    // 完全无类型信息：返回中性色 sentinel（用 badge 空字符串判定不渲染）
    return { border: "transparent", badge: "transparent", text: "" };
  });

  // ----------------- Badge 标签 -----------------
  // 非 team 场景固定显示 `TASK`（对齐原版 SubagentItem.tsx badge 文案），
  // subagentType 仅用于决定圆点/徽章的着色，不再作为徽章文字。
  const badgeLabel = $derived.by(() => {
    if (process.team) return process.team.memberName;
    return "TASK";
  });
  const showBadgeDot = $derived(process.team != null || process.subagentType != null);

  // ----------------- Description -----------------
  const description = $derived(
    process.description ?? process.rootTaskDescription ?? "Subagent",
  );
  const truncatedDesc = $derived(
    description.length > 60 ? description.slice(0, 60) + "…" : description,
  );

  // ----------------- Model 提取 -----------------
  // 优先用后端预算的 headerModel；缺失（老后端）时 fallback 派生。effectiveMessages
  // 在 messagesOmitted=true 且未懒拉时为空数组——派生返回 null 即可。
  const modelName = $derived.by(() => {
    if (process.headerModel) return process.headerModel;
    for (const c of effectiveMessages) {
      if (c.kind !== "ai") continue;
      for (const r of c.responses) {
        const info = parseModelString(r.model);
        if (info) return info.name;
      }
    }
    return null;
  });

  // ----------------- Last usage / context window 合计 -----------------
  const isolatedTokens = $derived.by(() => {
    if ((process.lastIsolatedTokens ?? 0) > 0) return process.lastIsolatedTokens!;
    let last: typeof process.messages[number] extends infer _C
      ? null | { input_tokens: number; output_tokens: number; cache_read_input_tokens: number; cache_creation_input_tokens: number }
      : never = null;
    for (const c of effectiveMessages) {
      if (c.kind !== "ai") continue;
      for (const r of c.responses) {
        if (r.usage) last = r.usage;
      }
    }
    if (!last) return 0;
    return (
      (last.input_tokens ?? 0) +
      (last.output_tokens ?? 0) +
      (last.cache_read_input_tokens ?? 0) +
      (last.cache_creation_input_tokens ?? 0)
    );
  });

  // ----------------- Shutdown-only team 特例 -----------------
  // 优先用后端预算 flag；缺失（老后端）时 fallback 派生（与改造前完全一致）。
  // effectiveMessages 在 messagesOmitted=true 且未懒拉时为空 → fallback 返回 false，
  // 卡片走完整渲染分支——shutdown-only 是 team 极简渲染优化，错过显示完整也安全。
  const isShutdownOnly = $derived.by(() => {
    if (process.isShutdownOnly !== undefined) return process.isShutdownOnly && process.team != null;
    if (!process.team) return false;
    let assistantCount = 0;
    let onlyCall: ToolCall | null = null;
    for (const c of effectiveMessages) {
      if (c.kind !== "ai") continue;
      for (const r of c.responses) {
        assistantCount++;
        if (r.toolCalls.length === 1) {
          onlyCall = r.toolCalls[0];
        }
      }
    }
    if (assistantCount !== 1 || !onlyCall) return false;
    if (onlyCall.name !== "SendMessage") return false;
    const input = onlyCall.input as { type?: string } | null;
    return input?.type === "shutdown_response";
  });

  // ----------------- ExecutionTrace items -----------------
  const traceItems = $derived(
    isExpanded ? buildDisplayItemsFromChunks(effectiveMessages) : [],
  );
  const traceSummary = $derived(
    isExpanded ? buildSummary(traceItems) : "",
  );

  // ----------------- Duration -----------------
  const durationText = $derived(formatDuration(process.durationMs));

  async function toggleExpanded() {
    isExpanded = !isExpanded;
    if (isExpanded) await ensureMessages();
  }
  function toggleTrace(e: Event) {
    e.stopPropagation();
    isTraceExpanded = !isTraceExpanded;
  }
</script>

{#if isShutdownOnly && process.team}
  <!-- 极简 shutdown-only 行 -->
  <div class="sa-shutdown" style="border-color: {colorSet.border}40">
    <span class="sa-dot" style="background-color: {colorSet.border}"></span>
    <span class="sa-badge" style="background-color: {colorSet.badge}; color: {colorSet.text}; border-color: {colorSet.border}40">
      {process.team.memberName}
    </span>
    <span class="sa-shutdown-label">Shutdown confirmed</span>
    <span class="sa-shutdown-spacer"></span>
    {#if durationText}
      <span class="sa-duration">{durationText}</span>
    {/if}
  </div>
{:else}
  <div class="sa-card" class:sa-nested={depth > 0}>
    <!-- Header -->
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="sa-header" class:sa-header-expanded={isExpanded} onclick={toggleExpanded}>
      <svg class="sa-chevron" class:sa-chevron-open={isExpanded} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d={CHEVRON_RIGHT}/></svg>

      {#if showBadgeDot}
        <span class="sa-dot" style="background-color: {colorSet.border}"></span>
        <span class="sa-badge" style="background-color: {colorSet.badge}; color: {colorSet.text}; border-color: {colorSet.border}40">
          {badgeLabel}
        </span>
      {:else}
        <svg class="sa-bot" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <rect x="3" y="11" width="18" height="10" rx="2" />
          <circle cx="12" cy="5" r="2" />
          <path d="M12 7v4" />
          <line x1="8" y1="16" x2="8" y2="16" />
          <line x1="16" y1="16" x2="16" y2="16" />
        </svg>
        <span class="sa-badge sa-badge-neutral">TASK</span>
      {/if}

      {#if modelName}
        <span class="sa-model">{modelName}</span>
      {/if}

      <span class="sa-desc">{truncatedDesc}</span>

      {#if process.isOngoing}
        <svg class="sa-status-running" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12a9 9 0 1 1-6.219-8.56"/></svg>
      {:else}
        <svg class="sa-status-done" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"/><polyline points="22 4 12 14.01 9 11.01"/></svg>
      {/if}

      <MetricsPill
        mainTokens={process.team ? null : (process.mainSessionImpact?.totalTokens ?? null)}
        isolatedTokens={isolatedTokens}
        isolatedLabel={process.team ? "Context Window" : "Subagent Context"}
      />

      {#if durationText}
        <span class="sa-duration">{durationText}</span>
      {/if}
    </div>

    {#if isExpanded}
      <div class="sa-body">
        <!-- Meta 行 -->
        <div class="sa-meta">
          <span class="sa-meta-label">Type</span>
          <span class="sa-meta-value">{process.subagentType ?? (process.team ? "Team" : "Task")}</span>
          <span class="sa-meta-sep">·</span>
          {#if durationText}
            <span class="sa-meta-label">Duration</span>
            <span class="sa-meta-value">{durationText}</span>
            <span class="sa-meta-sep">·</span>
          {/if}
          {#if modelName}
            <span class="sa-meta-label">Model</span>
            <span class="sa-meta-value">{modelName}</span>
            <span class="sa-meta-sep">·</span>
          {/if}
          <span class="sa-meta-label">ID</span>
          <span class="sa-meta-value sa-meta-id" title={process.sessionId}>{process.sessionId.slice(0, 8)}</span>
        </div>

        <!-- Context Usage -->
        {#if (process.mainSessionImpact && process.mainSessionImpact.totalTokens > 0) || isolatedTokens > 0}
          <div class="sa-context">
            <div class="sa-context-title">Context Usage</div>
            {#if !process.team && process.mainSessionImpact && process.mainSessionImpact.totalTokens > 0}
              <div class="sa-context-row">
                <span class="sa-context-label">Main Context</span>
                <span class="sa-context-val">{process.mainSessionImpact.totalTokens.toLocaleString()}</span>
              </div>
            {/if}
            {#if isolatedTokens > 0}
              <div class="sa-context-row">
                <span class="sa-context-label">{process.team ? "Context Window" : "Subagent Context"}</span>
                <span class="sa-context-val">{isolatedTokens.toLocaleString()}</span>
              </div>
            {/if}
          </div>
        {/if}

        <!-- Execution Trace 折叠块 -->
        {#if traceItems.length > 0}
          <div class="sa-trace">
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div class="sa-trace-header" onclick={toggleTrace}>
              <svg class="sa-trace-chevron" class:sa-trace-chevron-open={isTraceExpanded} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d={CHEVRON_RIGHT}/></svg>
              <svg class="sa-trace-terminal" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d={TERMINAL}/></svg>
              <span class="sa-trace-label">Execution Trace</span>
              {#if traceSummary}
                <span class="sa-trace-summary">· {traceSummary}</span>
              {/if}
            </div>
            {#if isTraceExpanded}
              <div class="sa-trace-body">
                {#if isLoadingTrace}
                  <div class="sa-trace-loading">Loading trace…</div>
                {:else}
                  <ExecutionTrace items={traceItems} {rootSessionId} sessionId={process.sessionId} {depth} />
                {/if}
              </div>
            {/if}
          </div>
        {/if}
      </div>
    {/if}
  </div>
{/if}

<style>
  .sa-card {
    border-radius: 6px;
    border: 1px solid var(--card-border);
    background: var(--card-bg);
    overflow: hidden;
  }
  .sa-nested {
    background: var(--card-header-bg);
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
    width: 12px;
    height: 12px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .sa-bot {
    width: 16px;
    height: 16px;
    color: var(--color-text-muted);
    flex-shrink: 0;
  }

  .sa-badge {
    font-size: 10px;
    font-weight: 500;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    padding: 1px 6px;
    border-radius: 4px;
    border: 1px solid;
    flex-shrink: 0;
  }
  .sa-badge-neutral {
    background: var(--badge-neutral-bg);
    color: var(--color-text-secondary);
    border-color: var(--card-border);
  }

  .sa-model {
    font-size: 11px;
    color: var(--color-text-muted);
    font-family: var(--font-mono);
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

  .sa-duration {
    font-size: 11px;
    font-family: var(--font-mono);
    color: var(--card-icon-muted);
    flex-shrink: 0;
    font-variant-numeric: tabular-nums;
  }

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
  .sa-meta-label { color: var(--card-icon-muted); }
  .sa-meta-value { color: var(--card-text-light); font-family: var(--font-mono); }
  .sa-meta-id { max-width: 120px; overflow: hidden; text-overflow: ellipsis; color: var(--card-icon-muted); }
  .sa-meta-sep { color: var(--card-separator); }

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

  .sa-trace {
    border: 1px solid var(--card-border);
    border-radius: 6px;
    overflow: hidden;
    background: var(--card-header-bg);
  }
  .sa-trace-header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
    cursor: pointer;
    transition: background-color 0.1s;
  }
  .sa-trace-header:hover {
    background: var(--card-header-hover);
  }
  .sa-trace-chevron {
    width: 12px;
    height: 12px;
    color: var(--card-icon-muted);
    transition: transform 0.15s ease;
    flex-shrink: 0;
  }
  .sa-trace-chevron-open {
    transform: rotate(90deg);
  }
  .sa-trace-terminal {
    width: 14px;
    height: 14px;
    color: var(--card-icon-muted);
    flex-shrink: 0;
  }
  .sa-trace-label {
    font-size: 12px;
    color: var(--color-text-secondary);
  }
  .sa-trace-summary {
    font-size: 11px;
    color: var(--card-icon-muted);
  }
  .sa-trace-body {
    padding: 8px;
    border-top: 1px solid var(--card-border);
  }
  .sa-trace-loading {
    padding: 8px 4px;
    color: var(--color-text-muted);
    font-size: 12px;
    font-style: italic;
  }

  /* Shutdown-only 特例 */
  .sa-shutdown {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 12px;
    border-radius: 6px;
    border: 1px solid;
    background: var(--card-bg);
    opacity: 0.6;
  }
  .sa-shutdown-label {
    font-size: 12px;
    color: var(--card-icon-muted);
  }
  .sa-shutdown-spacer { flex: 1; }
</style>

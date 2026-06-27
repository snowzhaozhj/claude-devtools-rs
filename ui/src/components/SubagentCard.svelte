<script module lang="ts">
  import { getSubagentTrace as _fetchSubagentTrace } from "../lib/api";
  import type { Chunk as _Chunk } from "../lib/api";

  /**
   * 模块级 inflight 去重：复用 key MUST 为 `${sessionId}|${version}` 联合 key
   * （codex 二审强制约束，spec `session-display` "SubagentCard 在 ongoing 期间
   * 主动重拉 trace" Requirement）。仅按 sessionId 复用会让旧版本 Promise 在版本
   * 递增后被复用，把 stale trace 写入 `messagesLocal`。
   *
   * 测试用：`__resetSubagentTraceInflightForTest()` 清空 Map。
   */
  const inflightTrace = new Map<string, Promise<_Chunk[]>>();

  /** 复用 key = `${sessionId}|${version}`。 */
  function traceKey(sessionId: string, version: string): string {
    return `${sessionId}|${version}`;
  }

  /**
   * 拉取 subagent trace，按 `(sessionId, version)` 联合 key 复用 inflight Promise。
   * 同 version 并发触发 SHALL 复用；跨 version 触发 SHALL 各自独立 Promise。
   */
  export function loadSubagentTrace(
    rootSessionId: string,
    sessionId: string,
    version: string,
  ): Promise<_Chunk[]> {
    const key = traceKey(sessionId, version);
    const existing = inflightTrace.get(key);
    if (existing) return existing;
    const p = (async () => {
      try {
        return await _fetchSubagentTrace(rootSessionId, sessionId);
      } finally {
        inflightTrace.delete(key);
      }
    })();
    inflightTrace.set(key, p);
    return p;
  }

  /** 仅供测试：清空 inflight Map。 */
  export function __resetSubagentTraceInflightForTest(): void {
    inflightTrace.clear();
  }
</script>

<script lang="ts">
  import { untrack } from "svelte";
  import type { SubagentProcess, ContentBlock, ToolCall, Chunk } from "../lib/api";
  import { CHEVRON_RIGHT, TERMINAL } from "../lib/icons";
  import { activateOnKey } from "../lib/a11y";
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
  let messagesLocal: Chunk[] | null = $state(null);
  let isLoadingTrace = $state(false);

  // 派生 messages：优先用本地缓存，fallback 到 process.messages
  // （后端 OMIT_SUBAGENT_MESSAGES=false 或老后端时直接用 process.messages）。
  const effectiveMessages = $derived<Chunk[]>(messagesLocal ?? process.messages);

  /**
   * messages 版本指纹（spec `session-display` "SubagentCard 在 ongoing 期间
   * 主动重拉 trace"）：`isOngoing|endTs|messagesTotalCount`。版本递增 + 已展开
   * + ongoing 时主动重拉；老后端缺 `messagesTotalCount` 时该位为空串，版本指纹
   * 保持常量，主动重拉自然不触发（退化为既有 lazy 路径）。
   */
  const messagesVersion = $derived(
    `${process.isOngoing ? "1" : "0"}|${process.endTs ?? "_"}|${
      process.messagesTotalCount ?? ""
    }`,
  );

  async function ensureMessages(): Promise<void> {
    if (messagesLocal != null) return;
    if (!process.messagesOmitted) {
      // 老后端 / 回滚开关 false：messages 已是完整。**不**写 messagesLocal——
      // `effectiveMessages = messagesLocal ?? process.messages` 的 fallback
      // 自动消费实时 `process.messages`，跟随父 detail 刷新替换 process 实例。
      // 早期版本写 `messagesLocal = process.messages` 会让 rollback 路径下
      // process 替换但 messagesVersion 三元组不变时 UI 永远卡在旧 trace
      // （codex 二审 V2/V3 发现）。
      return;
    }
    const fetchedVersion = untrack(() => messagesVersion);
    isLoadingTrace = true;
    try {
      const chunks = await loadSubagentTrace(
        rootSessionId,
        process.sessionId,
        fetchedVersion,
      );
      // race-check：严格按版本匹配才写入（codex 二审 C1）——若 pending 期间
      // version 已跳变，SHALL NOT 把 stale trace 写进 messagesLocal；effect
      // 会因 messagesVersion 变化、isExpanded=true 而发起新版本的 fetch
      // 接管显示。早期版本用 `|| messagesLocal == null` 兜底会让 first-fetch
      // 的 stale 结果固化（C1 描述的 race）。
      const currentVersion = untrack(() => messagesVersion);
      if (currentVersion === fetchedVersion) {
        messagesLocal = chunks;
      }
    } catch (e) {
      console.warn("getSubagentTrace failed:", e);
      // codex 二审 C3：不要把 messagesLocal 写成空数组——保留 null 让下次
      // toggleExpanded → ensureMessages 能重新尝试（empty array 命中
      // `if (messagesLocal != null) return;` 永久封堵重试入口）。
      // UI 在 messagesLocal=null 时通过 effectiveMessages fallback 到
      // process.messages（已被 OMIT 裁剪为空），视觉上仍是空 trace，
      // 折叠重开即可重试。
    } finally {
      isLoadingTrace = false;
    }
  }

  /**
   * spec `session-display` "SubagentCard 在 ongoing 期间主动重拉 trace"：
   * 用户已展开（`isExpanded=true`）且 messages 被裁剪时，版本指纹变化 SHALL
   * 主动调 `getSubagentTrace` 重拉，无论 `messagesLocal` 当前是 null（首次展开
   * 期间）还是已加载——`isExpanded` 才是"用户期待看到 trace"的真实信号，用
   * `messagesLocal !== null` 判会让首次展开期间 version 跳变后新版本 fetch
   * 不被触发（codex 二审 C1）。未展开时 `isExpanded=false` 自然短路，不发 IPC。
   *
   * 版本指纹翻转到 done（isOngoing=false + endTs 出现）也会命中此分支做一次
   * final 重拉同步收尾态。
   */
  $effect(() => {
    // 显式订阅版本指纹（其依赖：process.isOngoing / endTs / messagesTotalCount）
    const version = messagesVersion;
    untrack(() => {
      if (!isExpanded) return; // 未展开 → 不主动 IPC
      if (!process.messagesOmitted) return; // 完整 payload 不需要重拉
      void refetchOnVersionChange(version);
    });
  });

  async function refetchOnVersionChange(fetchedVersion: string): Promise<void> {
    try {
      const chunks = await loadSubagentTrace(
        rootSessionId,
        process.sessionId,
        fetchedVersion,
      );
      const currentVersion = untrack(() => messagesVersion);
      // race-check：fetch settle 时若版本又变了，丢弃本次结果，等更新版本接管
      if (currentVersion !== fetchedVersion) return;
      messagesLocal = chunks;
    } catch (e) {
      console.warn("subagent trace re-fetch failed:", e);
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
  // 注意：`modelName` 仅用于展开后的 Model 详情行；**始终可见的卡片 header** 的
  // model badge 只读 `process.headerModel`（稳定值），不走 messages 派生——否则嵌套
  // 骨架（headerModel 缺省 + messages 懒拉）展开时会让 header 突然冒出 model badge
  // 造成布局跳动（用户反馈）。骨架不知自身 model（零 IO 不读子文件），header 留空即可，
  // 真实 model 随展开 body 的 Model 详情行一并出现，属正常展开行为。
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
    <div
      class="sa-header"
      class:sa-header-expanded={isExpanded}
      role="button"
      tabindex="0"
      aria-expanded={isExpanded}
      onclick={toggleExpanded}
      onkeydown={(e) => activateOnKey(e, toggleExpanded)}
    >
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

      {#if process.headerModel}
        <span class="sa-model">{process.headerModel}</span>
      {/if}

      <span class="sa-desc">{truncatedDesc}</span>

      {#if process.isOngoing}
        <span class="sa-status-running" aria-label="Subagent running" title="Subagent running">
          <span class="sa-status-running-dot"></span>
        </span>
      {:else}
        <svg class="sa-status-done" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-label="Subagent done"><path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"/><polyline points="22 4 12 14.01 9 11.01"/></svg>
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
          <span class="sa-meta-value sa-meta-id" title={process.sessionId}>{process.sessionId}</span>
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
            <div
              class="sa-trace-header"
              role="button"
              tabindex="0"
              aria-expanded={isTraceExpanded}
              onclick={toggleTrace}
              onkeydown={(e) => activateOnKey(e, () => toggleTrace(e))}
            >
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
    border-radius: var(--radius-md);
    border: 1px solid var(--card-border);
    background: var(--card-bg);
    overflow: hidden;
  }
  .sa-nested {
    background: var(--card-header-bg);
  }

  .sa-header {
    position: relative;
    display: flex;
    align-items: center;
    gap: var(--bubble-header-gap);
    padding: var(--bubble-header-padding-l1);
    cursor: pointer;
    transition: background-color 0.12s ease;
  }
  .sa-header:hover {
    background: var(--card-header-hover);
  }
  .sa-header:focus-visible {
    outline: 2px solid var(--color-accent-blue);
    outline-offset: -2px;
  }
  .sa-header-expanded {
    background: var(--card-header-bg);
  }
  .sa-header-expanded::after {
    content: "";
    position: absolute;
    right: 0;
    bottom: 0;
    left: 0;
    height: 1px;
    background: var(--card-border);
    pointer-events: none;
  }
  .sa-chevron {
    width: var(--bubble-icon-md);
    height: var(--bubble-icon-md);
    flex-shrink: 0;
    color: var(--card-icon-muted);
    transition: transform 0.15s ease;
  }
  .sa-chevron-open {
    transform: rotate(90deg);
  }

  .sa-dot {
    width: var(--bubble-icon-sm);
    height: var(--bubble-icon-sm);
    border-radius: 50%;
    flex-shrink: 0;
  }

  .sa-bot {
    width: var(--bubble-icon-md);
    height: var(--bubble-icon-md);
    color: var(--color-text-muted);
    flex-shrink: 0;
  }

  .sa-badge {
    font-size: 10px;
    font-weight: 500;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    padding: 1px 6px;
    border-radius: var(--radius-xs);
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
    width: var(--bubble-icon-md);
    height: var(--bubble-icon-md);
    flex-shrink: 0;
    color: var(--color-success-bright);
  }
  /* SubagentCard running 标记：与 OngoingBanner 同款 circular spinner，
     但尺寸更小（10×10 vs 14×14）、border 更细（1.5px vs 2px），
     依靠尺寸 + 位置（header inline vs 贴底独立条带）天然分层 hierarchy。
     之前用 outline 静态圆点（"形态对立"防多脉冲源感染），但实测"看不出
     活跃"——和 sa-status-done 静态对勾视觉同一类，缺"事情正在发生"
     语义。旋转是 IDE/调试器工具领域的 running lingua franca（VS Code /
     IntelliJ / GitHub Actions / cargo / pnpm 全是同款），用户零学习成本。
     详见 DESIGN.md `The Static-vs-Live Shape Rule` 与 `One Live Signal Rule`。 */
  .sa-status-running {
    width: var(--bubble-icon-md);
    height: var(--bubble-icon-md);
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }
  .sa-status-running-dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    border: 1.5px solid color-mix(in oklch, var(--color-accent-blue) 18%, transparent);
    border-top-color: var(--color-accent-blue);
    box-sizing: border-box;
    animation: sa-status-spin 1.2s linear infinite;
  }
  @keyframes sa-status-spin {
    to {
      transform: rotate(360deg);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .sa-status-running-dot {
      animation: none;
      /* reduced-motion 下保留可识别静态形态：顶弧蓝色仍可见，仅不旋转 */
    }
  }

  .sa-duration {
    font-size: 11px;
    font-family: var(--font-mono);
    color: var(--card-icon-muted);
    flex-shrink: 0;
    font-variant-numeric: tabular-nums;
  }

  .sa-body {
    padding: var(--bubble-body-padding-l1);
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
  .sa-meta-id { max-width: 120px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--card-icon-muted); }
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
    border-radius: var(--radius-md);
    overflow: hidden;
    background: var(--card-header-bg);
  }
  .sa-trace-header {
    display: flex;
    align-items: center;
    gap: var(--bubble-header-gap);
    padding: var(--bubble-padding-l2);
    cursor: pointer;
    transition: var(--bubble-transition);
  }
  .sa-trace-header:hover {
    background: var(--card-header-hover);
  }
  .sa-trace-header:focus-visible {
    outline: 2px solid var(--color-accent-blue);
    outline-offset: -2px;
  }
  .sa-trace-chevron {
    width: var(--bubble-icon-sm);
    height: var(--bubble-icon-sm);
    color: var(--card-icon-muted);
    transition: transform 0.15s ease;
    flex-shrink: 0;
  }
  .sa-trace-chevron-open {
    transform: rotate(90deg);
  }
  .sa-trace-terminal {
    width: var(--bubble-icon-md);
    height: var(--bubble-icon-md);
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
    gap: var(--bubble-header-gap);
    padding: 6px 12px;
    border-radius: var(--radius-md);
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

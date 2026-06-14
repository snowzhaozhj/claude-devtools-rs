<script lang="ts">
  import type { DisplayItem } from "../lib/displayItemBuilder";
  import { getToolOutput, type ToolExecution, type ToolOutput, type WorkflowItem } from "../lib/api";
  import { renderMarkdown } from "../lib/render";
  import { getToolSummary, getToolStatus, getToolDurationMs, isToolPending, cleanDisplayText, getToolContextTokens, estimateTokens, viewerUsesOutput } from "../lib/toolHelpers";
  import { WRENCH, BRAIN, SLASH, MESSAGE_SQUARE, USER_ICON } from "../lib/icons";
  import BaseItem from "./BaseItem.svelte";
  import SubagentCard from "./SubagentCard.svelte";
  import WorkflowCard from "./WorkflowCard.svelte";
  import DefaultToolViewer from "./tool-viewers/DefaultToolViewer.svelte";
  import ReadToolViewer from "./tool-viewers/ReadToolViewer.svelte";
  import EditToolViewer from "./tool-viewers/EditToolViewer.svelte";
  import WriteToolViewer from "./tool-viewers/WriteToolViewer.svelte";
  import BashToolViewer from "./tool-viewers/BashToolViewer.svelte";

  interface Props {
    items: DisplayItem[];
    /** 顶层 SessionDetail 的 sessionId；嵌套 SubagentCard 用它做 getSubagentTrace 的 root key。 */
    rootSessionId: string;
    /** 本 trace 所属 session 的 sessionId（嵌套 subagent 时是 subagent 自己的 id），
     *  用于 getToolOutput 懒拉。fallback 到 rootSessionId 兼容老调用点。 */
    sessionId?: string;
    depth?: number;
    /** 顶层 workflow items，按 toolExecution.workflowRunId 匹配渲染 WorkflowCard。 */
    workflowItems?: WorkflowItem[];
    /** projectId 透传给 WorkflowCard lazy-loading。嵌套场景可省略（workflow 已是完整数据）。 */
    projectId?: string;
  }

  let { items, rootSessionId, sessionId, depth = 0, workflowItems = [], projectId = "" }: Props = $props();

  const workflowMap = $derived.by(() => {
    const map = new Map<string, WorkflowItem>();
    for (const wf of workflowItems ?? []) {
      map.set(wf.runId, wf);
    }
    return map;
  });
  const traceSessionId = $derived(sessionId ?? rootSessionId);

  const MAX_DEPTH = 8;

  // per-trace 独立的展开状态
  let expandedKeys = $state(new Set<string>());

  // tool output 懒拉缓存：toolUseId → ToolOutput。仅当 exec.outputOmitted=true
  // 且用户首次展开该 tool 时通过 getToolOutput IPC 拉取。
  let outputCache: Map<string, ToolOutput> = $state(new Map());
  const outputLoads = new Map<string, Promise<void>>();

  function cachedOutput(exec: ToolExecution): ToolOutput | undefined {
    const cached = outputCache.get(exec.toolUseId);
    return cached?.kind === "missing" ? undefined : cached;
  }

  function isOutputReady(exec: ToolExecution): boolean {
    return !exec.outputOmitted || !!cachedOutput(exec);
  }

  function effectiveExec(exec: ToolExecution): ToolExecution {
    const cached = cachedOutput(exec);
    if (!cached) return exec;
    return { ...exec, output: cached };
  }

  async function ensureToolOutput(exec: ToolExecution): Promise<void> {
    if (!exec.outputOmitted) return;
    if (cachedOutput(exec)) return;
    const existing = outputLoads.get(exec.toolUseId);
    if (existing) return existing;
    const load = (async () => {
      try {
        const out = await getToolOutput(rootSessionId, traceSessionId, exec.toolUseId);
        if (out.kind === "missing") return;
        const next = new Map(outputCache);
        next.set(exec.toolUseId, out);
        outputCache = next;
      } catch (e) {
        console.warn("[perf] getToolOutput failed", exec.toolUseId, e);
      } finally {
        outputLoads.delete(exec.toolUseId);
      }
    })();
    outputLoads.set(exec.toolUseId, load);
    return load;
  }

  async function toggle(key: string, exec?: ToolExecution) {
    if (expandedKeys.has(key)) {
      const next = new Set(expandedKeys);
      next.delete(key);
      expandedKeys = next;
      return;
    }
    if (exec && viewerUsesOutput(exec) && !isOutputReady(exec)) {
      await ensureToolOutput(exec);
      if (!isOutputReady(exec)) return;
    }
    const next = new Set(expandedKeys);
    next.add(key);
    expandedKeys = next;
  }

  function isReadTool(exec: ToolExecution): boolean {
    return exec.toolName === "Read" && !exec.isError;
  }
  function isEditTool(exec: ToolExecution): boolean {
    return exec.toolName === "Edit";
  }
  function isWriteTool(exec: ToolExecution): boolean {
    return exec.toolName === "Write" && !exec.isError;
  }
  function isBashTool(exec: ToolExecution): boolean {
    return ["Bash", "bash"].includes(exec.toolName);
  }
</script>

<div class="execution-trace" class:nested={depth > 0}>
  {#each items as item, i}
    {#if item.type === "slash"}
      <BaseItem
        svgIcon={SLASH}
        label={"/" + item.slash.name}
        summary={item.slash.args ?? item.slash.message ?? ""}
        isExpanded={false}
        onclick={() => {}}
        collapsible={false}
      />
    {:else if item.type === "tool"}
      {@const exec = item.execution}
      {@const matchedWorkflow = exec.workflowRunId ? workflowMap.get(exec.workflowRunId) : undefined}
      {#if matchedWorkflow}
        <WorkflowCard workflow={matchedWorkflow} sessionId={sessionId ?? rootSessionId} {projectId} />
      {:else}
        {@const key = `tool-${exec.toolUseId}`}
        {@const eff = effectiveExec(exec)}
        <BaseItem
          svgIcon={WRENCH}
          label={exec.toolName}
          summary={getToolSummary(exec.toolName, exec.input)}
          tokenCount={getToolContextTokens(exec)}
          status={getToolStatus(exec)}
          durationMs={getToolDurationMs(exec)}
          pendingLabel={isToolPending(exec) ? "pending" : undefined}
          isExpanded={expandedKeys.has(key)}
          onclick={() => toggle(key, exec)}
        >
          {#snippet children()}
            {#if isReadTool(exec)}
              <ReadToolViewer exec={eff} />
            {:else if isEditTool(exec)}
              <EditToolViewer exec={eff} />
            {:else if isWriteTool(exec)}
              <WriteToolViewer exec={eff} />
            {:else if isBashTool(exec)}
              <BashToolViewer exec={eff} />
            {:else}
              <DefaultToolViewer exec={eff} />
            {/if}
          {/snippet}
        </BaseItem>
      {/if}
    {:else if item.type === "thinking"}
      {@const key = `thinking-${i}`}
      <BaseItem
        svgIcon={BRAIN}
        label="Thinking"
        tokenCount={estimateTokens(item.text)}
        isExpanded={expandedKeys.has(key)}
        onclick={() => toggle(key)}
      >
        {#snippet children()}
          <div class="prose prose-thinking">{@html renderMarkdown(item.text)}</div>
        {/snippet}
      </BaseItem>
    {:else if item.type === "output"}
      {@const key = `output-${i}`}
      {@const cleaned = cleanDisplayText(item.text)}
      <BaseItem
        svgIcon={MESSAGE_SQUARE}
        label="Output"
        summary={cleaned.length > 60 ? cleaned.slice(0, 60) + "…" : cleaned}
        tokenCount={estimateTokens(item.text)}
        isExpanded={expandedKeys.has(key)}
        onclick={() => toggle(key)}
      >
        {#snippet children()}
          <div class="prose">{@html renderMarkdown(item.text)}</div>
        {/snippet}
      </BaseItem>
    {:else if item.type === "user_message"}
      {@const key = `usermsg-${i}`}
      <BaseItem
        svgIcon={USER_ICON}
        label="User"
        summary={item.text.length > 60 ? item.text.slice(0, 60) + "…" : item.text}
        tokenCount={estimateTokens(item.text)}
        isExpanded={expandedKeys.has(key)}
        onclick={() => toggle(key)}
      >
        {#snippet children()}
          <div class="prose">{@html renderMarkdown(item.text)}</div>
        {/snippet}
      </BaseItem>
    {:else if item.type === "subagent"}
      {#if depth < MAX_DEPTH}
        <SubagentCard process={item.process} {rootSessionId} depth={depth + 1} />
      {:else}
        <!-- 达到最大递归深度，只渲染简化头 -->
        <div class="depth-limit">
          <span class="depth-limit-label">Nested subagent (depth limit reached)</span>
          <span class="depth-limit-name">{item.process.subagentType ?? "Task"}</span>
        </div>
      {/if}
    {/if}
  {/each}
</div>

<style>
  .execution-trace {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .nested {
    font-size: 13px;
  }

  .prose {
    font-size: 13px;
    color: var(--prose-body);
    line-height: 1.55;
    word-break: break-word;
  }
  /* Thinking 正文：与 prose 同字体同行高，仅以 13px 略小暗示次级。
     身份完全靠 BRAIN icon 形状 + "Thinking" label 区分，与 Bash / Read /
     Output 同等克制——保持 PRODUCT.md "quiet debugging workbench" 调性。 */
  .prose-thinking {
    color: var(--color-text);
    font-size: 13px;
    line-height: 1.65;
  }

  .depth-limit {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
    border: 1px dashed var(--card-border);
    border-radius: 6px;
    font-size: 11px;
    color: var(--color-text-muted);
  }
  .depth-limit-name {
    font-family: var(--font-mono);
    color: var(--color-text-secondary);
  }
</style>

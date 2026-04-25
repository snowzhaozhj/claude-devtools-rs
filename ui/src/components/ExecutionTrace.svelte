<script lang="ts">
  import type { DisplayItem } from "../lib/displayItemBuilder";
  import { getToolOutput, type ToolExecution, type ToolOutput } from "../lib/api";
  import { renderMarkdown } from "../lib/render";
  import { getToolSummary, getToolStatus, cleanDisplayText, getToolInputTokens, getToolOutputTokens } from "../lib/toolHelpers";
  import { WRENCH, BRAIN, SLASH, MESSAGE_SQUARE } from "../lib/icons";
  import BaseItem from "./BaseItem.svelte";
  import SubagentCard from "./SubagentCard.svelte";
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
  }

  let { items, rootSessionId, sessionId, depth = 0 }: Props = $props();
  const traceSessionId = $derived(sessionId ?? rootSessionId);

  const MAX_DEPTH = 8;

  // per-trace 独立的展开状态
  let expandedKeys = $state(new Set<string>());

  // tool output 懒拉缓存：toolUseId → ToolOutput。仅当 exec.outputOmitted=true
  // 且用户首次展开该 tool 时通过 getToolOutput IPC 拉取。
  let outputCache: Map<string, ToolOutput> = $state(new Map());

  function effectiveExec(exec: ToolExecution): ToolExecution {
    const cached = outputCache.get(exec.toolUseId);
    if (!cached) return exec;
    return { ...exec, output: cached };
  }

  async function ensureToolOutput(exec: ToolExecution): Promise<void> {
    if (!exec.outputOmitted) return;
    if (outputCache.has(exec.toolUseId)) return;
    try {
      const out = await getToolOutput(rootSessionId, traceSessionId, exec.toolUseId);
      const next = new Map(outputCache);
      next.set(exec.toolUseId, out);
      outputCache = next;
    } catch (e) {
      console.warn("[perf] getToolOutput failed", exec.toolUseId, e);
    }
  }

  function toggle(key: string, exec?: ToolExecution) {
    const n = new Set(expandedKeys);
    const opening = !n.has(key);
    if (opening) n.add(key);
    else n.delete(key);
    expandedKeys = n;
    if (opening && exec) {
      void ensureToolOutput(exec);
    }
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
      />
    {:else if item.type === "tool"}
      {@const exec = item.execution}
      {@const key = `tool-${exec.toolUseId}`}
      {@const eff = effectiveExec(exec)}
      <BaseItem
        svgIcon={WRENCH}
        label={exec.toolName}
        summary={getToolSummary(exec.toolName, exec.input)}
        tokenCount={getToolInputTokens(eff)}
        outputTokens={getToolOutputTokens(eff)}
        outputOmitted={!!exec.outputOmitted && eff === exec}
        status={getToolStatus(exec)}
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
    {:else if item.type === "thinking"}
      {@const key = `thinking-${i}`}
      <BaseItem
        svgIcon={BRAIN}
        label="Thinking"
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
    gap: 2px;
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
  .prose-thinking {
    color: var(--thinking-content-text);
    font-size: 12px;
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

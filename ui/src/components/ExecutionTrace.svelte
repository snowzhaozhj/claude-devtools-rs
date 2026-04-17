<script lang="ts">
  import type { DisplayItem } from "../lib/displayItemBuilder";
  import type { ToolExecution } from "../lib/api";
  import { renderMarkdown } from "../lib/render";
  import { getToolSummary, getToolStatus, cleanDisplayText } from "../lib/toolHelpers";
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
    depth?: number;
  }

  let { items, depth = 0 }: Props = $props();

  const MAX_DEPTH = 8;

  // per-trace 独立的展开状态
  let expandedKeys = $state(new Set<string>());

  function toggle(key: string) {
    const n = new Set(expandedKeys);
    if (n.has(key)) n.delete(key);
    else n.add(key);
    expandedKeys = n;
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
      <BaseItem
        svgIcon={WRENCH}
        label={exec.toolName}
        summary={getToolSummary(exec.toolName, exec.input)}
        status={getToolStatus(exec)}
        isExpanded={expandedKeys.has(key)}
        onclick={() => toggle(key)}
      >
        {#snippet children()}
          {#if isReadTool(exec)}
            <ReadToolViewer {exec} />
          {:else if isEditTool(exec)}
            <EditToolViewer {exec} />
          {:else if isWriteTool(exec)}
            <WriteToolViewer {exec} />
          {:else if isBashTool(exec)}
            <BashToolViewer {exec} />
          {:else}
            <DefaultToolViewer {exec} />
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
        <SubagentCard process={item.process} depth={depth + 1} />
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

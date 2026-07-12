<script lang="ts">
  import { untrack } from "svelte";
  import type { DisplayItem } from "../lib/displayItemBuilder";
  import { getToolOutput, type ToolExecution, type ToolOutput, type WorkflowItem } from "../lib/api";
  import { renderMarkdown } from "../lib/render";
  import { getToolSummary, getToolStatus, getToolDurationMs, isToolPending, cleanDisplayText, getToolContextTokens, estimateTokens, viewerUsesOutput } from "../lib/toolHelpers";
  import { WRENCH, BRAIN, SLASH, MESSAGE_SQUARE, USER_ICON } from "../lib/icons";
  import { contextMenu } from "../lib/contextMenu.svelte";
  import { buildMarkdownBlockItems, type MenuItemContext } from "../lib/contextMenu/menu-items";
  import { getMenuSettings } from "../lib/contextMenu/settings.svelte";
  import { getMenuItemDispatch } from "../lib/contextMenu/dispatch";
  import BaseItem from "./BaseItem.svelte";
  import AdaptiveProse from "./AdaptiveProse.svelte";
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

  // 右键复制菜单上下文构造（对齐 SessionDetail::buildMenuCtx）。
  // buildMarkdownBlockItems 对纯 markdown 块复制仅消费 selectionText 与
  // dispatch.copyToClipboard；sessionId / projectId / settings 不参与 markdown
  // 块复制计算（它们服务于文件类 item），故嵌套场景 projectId 为空不影响正确性。
  function buildBlockMenuCtx(): MenuItemContext {
    return {
      sessionId: traceSessionId,
      projectId,
      settings: getMenuSettings(),
      selectionText: window.getSelection()?.toString() ?? "",
      dispatch: getMenuItemDispatch(),
    };
  }

  const MAX_DEPTH = 8;

  // per-trace 独立的展开状态
  let expandedKeys = $state(new Set<string>());

  // tool output 懒拉缓存：toolUseId → ToolOutput。仅当 exec.outputOmitted=true
  // 且用户首次展开该 tool 时通过 getToolOutput IPC 拉取。
  let outputCache: Map<string, ToolOutput> = $state(new Map());
  const outputLoads = new Map<string, Promise<void>>();
  // 懒拉失败哨兵：失败 ≠ 加载中（同 SessionDetail，防占位永挂谎称加载）。
  let failedOutputs: Set<string> = $state(new Set());

  function cachedOutput(exec: ToolExecution): ToolOutput | undefined {
    const cached = outputCache.get(exec.toolUseId);
    return cached?.kind === "missing" ? undefined : cached;
  }

  function isOutputReady(exec: ToolExecution): boolean {
    return !exec.outputOmitted || !!cachedOutput(exec);
  }

  function isOutputLoading(exec: ToolExecution): boolean {
    // missing 标记也算"已就绪"——内容确实不存在，不再显示加载占位。
    return (
      !!exec.outputOmitted &&
      !outputCache.has(exec.toolUseId) &&
      !failedOutputs.has(exec.toolUseId)
    );
  }

  function isOutputLoadFailed(exec: ToolExecution): boolean {
    return (
      !!exec.outputOmitted && !outputCache.has(exec.toolUseId) && failedOutputs.has(exec.toolUseId)
    );
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
    // 重试入口：新一轮加载开始即清失败哨兵。
    if (failedOutputs.has(exec.toolUseId)) {
      const nextFailed = new Set(failedOutputs);
      nextFailed.delete(exec.toolUseId);
      failedOutputs = nextFailed;
    }
    const load = (async () => {
      try {
        const out = await getToolOutput(rootSessionId, traceSessionId, exec.toolUseId);
        // missing 也缓存：终结加载占位态（渲染层 cachedOutput 过滤 missing）；
        // items 替换补拉 effect 会在工具完成推送后再拉。
        const next = new Map(outputCache);
        next.set(exec.toolUseId, out);
        outputCache = next;
      } catch (e) {
        console.warn("getToolOutput failed", exec.toolUseId, e);
        const nextFailed = new Set(failedOutputs);
        nextFailed.add(exec.toolUseId);
        failedOutputs = nextFailed;
      } finally {
        outputLoads.delete(exec.toolUseId);
      }
    })();
    outputLoads.set(exec.toolUseId, load);
    return load;
  }

  // items 替换后（file-change 推送新 trace / 工具完成）补拉所有已展开 +
  // outputOmitted 的工具输出——与 SessionDetail 的 detail 补拉 effect 同构：
  // 没有这层时，展开中拉到 missing（工具还在跑）的项在工具完成后永远显示
  // 空输出（toggle 不会再触发，缓存的 missing 挡住渲染）。
  $effect(() => {
    void items;
    untrack(() => {
      for (const it of items) {
        if (it.type !== "tool") continue;
        const exec = it.execution;
        if (!exec.outputOmitted) continue;
        if (expandedKeys.has(`tool-${exec.toolUseId}`)) {
          // 缓存的 missing 在 cachedOutput 过滤下会触发重拉；真内容命中则 no-op
          const cached = outputCache.get(exec.toolUseId);
          if (!cached || cached.kind === "missing") void ensureToolOutput(exec);
        }
      }
    });
  });

  async function toggle(key: string, exec?: ToolExecution) {
    if (expandedKeys.has(key)) {
      const next = new Set(expandedKeys);
      next.delete(key);
      expandedKeys = next;
      return;
    }
    // 展开即触发懒加载并立即展开；加载期由 viewer 以稳定的限高档占位渲染
    // （spec tool-viewer-routing::工具输出懒加载态的稳定分档，design D6）。
    if (exec && viewerUsesOutput(exec) && !isOutputReady(exec)) {
      void ensureToolOutput(exec);
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
            {@const outputLoading = viewerUsesOutput(exec) && isOutputLoading(exec)}
            {@const outputLoadFailed = viewerUsesOutput(exec) && isOutputLoadFailed(exec)}
            {#if isReadTool(exec)}
              <ReadToolViewer exec={eff} {outputLoading} {outputLoadFailed} />
            {:else if isEditTool(exec)}
              <EditToolViewer exec={eff} {outputLoading} {outputLoadFailed} />
            {:else if isWriteTool(exec)}
              <WriteToolViewer exec={eff} />
            {:else if isBashTool(exec)}
              <BashToolViewer exec={eff} {outputLoading} {outputLoadFailed} />
            {:else}
              <DefaultToolViewer exec={eff} {outputLoading} {outputLoadFailed} />
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
          <div class="prose prose-thinking" use:contextMenu={() => buildMarkdownBlockItems(item.text, buildBlockMenuCtx())}>{@html renderMarkdown(item.text)}</div>
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
          <AdaptiveProse text={item.text} viewportLabel="Output">
            {#snippet body()}
              <div class="prose" use:contextMenu={() => buildMarkdownBlockItems(item.text, buildBlockMenuCtx())}>{@html renderMarkdown(item.text)}</div>
            {/snippet}
          </AdaptiveProse>
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
          <AdaptiveProse text={item.text} viewportLabel="User message">
            {#snippet body()}
              <div class="prose" use:contextMenu={() => buildMarkdownBlockItems(item.text, buildBlockMenuCtx())}>{@html renderMarkdown(item.text)}</div>
            {/snippet}
          </AdaptiveProse>
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

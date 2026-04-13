<script lang="ts">
  import { onMount } from "svelte";
  import { getSessionDetail, type SessionDetail, type Chunk, type AIChunk, type ChunkMetrics, type ToolExecution } from "../lib/api";
  import { renderMarkdown } from "../lib/render";
  import { getToolSummary, getToolStatus, cleanDisplayText } from "../lib/toolHelpers";
  import BaseItem from "../components/BaseItem.svelte";
  import DefaultToolViewer from "../components/tool-viewers/DefaultToolViewer.svelte";
  import ReadToolViewer from "../components/tool-viewers/ReadToolViewer.svelte";
  import EditToolViewer from "../components/tool-viewers/EditToolViewer.svelte";
  import WriteToolViewer from "../components/tool-viewers/WriteToolViewer.svelte";
  import BashToolViewer from "../components/tool-viewers/BashToolViewer.svelte";

  interface Props { projectId: string; sessionId: string; }
  let { projectId, sessionId }: Props = $props();

  let detail: SessionDetail | null = $state(null);
  let loading = $state(true);
  let error: string | null = $state(null);
  let expandedItems: Set<string> = $state(new Set());
  /** 存储被用户手动展开的 AI chunk index。默认全部收起。 */
  let expandedChunks: Set<number> = $state(new Set());

  function toggleChunk(idx: number) {
    const n = new Set(expandedChunks);
    if (n.has(idx)) n.delete(idx); else n.add(idx);
    expandedChunks = n;
  }

  function isChunkToolsVisible(idx: number): boolean {
    return expandedChunks.has(idx);
  }

  onMount(async () => {
    try { detail = await getSessionDetail(projectId, sessionId); }
    catch (e) { error = String(e); }
    finally { loading = false; }
  });

  $effect(() => {
    if (sessionId) {
      loading = true;
      error = null;
      expandedItems = new Set();
      expandedChunks = new Set();
      getSessionDetail(projectId, sessionId)
        .then(d => detail = d)
        .catch(e => error = String(e))
        .finally(() => loading = false);
    }
  });

  function toggle(key: string) {
    const n = new Set(expandedItems);
    if (n.has(key)) n.delete(key); else n.add(key);
    expandedItems = n;
  }

  function sumMetrics(chunks: Chunk[]): ChunkMetrics {
    const r: ChunkMetrics = { inputTokens: 0, outputTokens: 0, cacheCreationTokens: 0, cacheReadTokens: 0, toolCount: 0, costUsd: null };
    for (const c of chunks) {
      r.inputTokens += c.metrics.inputTokens;
      r.outputTokens += c.metrics.outputTokens;
      r.toolCount += c.metrics.toolCount;
    }
    return r;
  }

  function fk(n: number): string {
    if (n >= 1e6) return (n / 1e6).toFixed(1) + "M";
    if (n >= 1e3) return (n / 1e3).toFixed(1) + "k";
    return String(n);
  }

  function ftime(ts: string): string {
    try {
      return new Date(ts).toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit", second: "2-digit", hour12: true });
    } catch { return ""; }
  }

  function utext(content: string | unknown[]): string {
    let raw = "";
    if (typeof content === "string") {
      raw = content;
    } else if (Array.isArray(content)) {
      for (const b of content) {
        if (b && typeof b === "object" && "type" in b) {
          const x = b as Record<string, unknown>;
          if (x.type === "text" && typeof x.text === "string") { raw = x.text; break; }
        }
      }
    }
    return cleanDisplayText(raw);
  }

  function aiModel(chunk: AIChunk): string {
    if (chunk.responses.length > 0) {
      const m = chunk.responses[chunk.responses.length - 1].model;
      if (m) return m.replace("claude-", "").replace(/-\d{8}$/, "");
    }
    return "Claude";
  }

  function aiToolCount(chunk: AIChunk): number {
    return chunk.toolExecutions.length;
  }

  function isReadTool(exec: ToolExecution): boolean { return exec.toolName === "Read" && !exec.isError; }
  function isEditTool(exec: ToolExecution): boolean { return exec.toolName === "Edit"; }
  function isWriteTool(exec: ToolExecution): boolean { return exec.toolName === "Write" && !exec.isError; }
  function isBashTool(exec: ToolExecution): boolean { return ["Bash", "bash"].includes(exec.toolName); }

  function firstUserTitle(chunks: Chunk[]): string {
    for (const c of chunks) {
      if (c.kind === "user") {
        const t = utext(c.content);
        if (t && !t.startsWith("/")) return t.length > 60 ? t.slice(0, 60) + "..." : t;
        // 跳过纯命令消息（如 /model），继续找真正的用户输入
        if (t && t.startsWith("/") && t.length > 1) {
          // 命令消息也可以作为标题，但优先找非命令消息
          continue;
        }
      }
    }
    // fallback: 取第一条任何 user 消息
    for (const c of chunks) {
      if (c.kind === "user") {
        const t = utext(c.content);
        if (t) return t.length > 60 ? t.slice(0, 60) + "..." : t;
      }
    }
    return sessionId.slice(0, 12);
  }
</script>

{#if loading}
  <div class="state-msg">加载中...</div>
{:else if error}
  <div class="state-msg state-err">{error}</div>
{:else if detail}
  {@const m = sumMetrics(detail.chunks)}

  <!-- Top bar -->
  <div class="top-bar">
    <span class="top-title">{firstUserTitle(detail.chunks)}</span>
    <div class="top-meta">
      <span class="top-badge">Context ({detail.chunks.length})</span>
    </div>
  </div>

  <!-- Conversation -->
  <div class="conversation">
    {#each detail.chunks as chunk, i}

      <!-- User -->
      {#if chunk.kind === "user"}
        {@const text = utext(chunk.content)}
        {#if text}
          <div class="msg-row msg-row-user">
            <div class="msg-spacer"></div>
            <div class="msg-bubble msg-bubble-user">
              <div class="msg-bubble-header">
                <span class="msg-time">{ftime(chunk.timestamp)}</span>
                <span class="msg-who-user">You</span>
              </div>
              <div class="prose">{@html renderMarkdown(text)}</div>
            </div>
          </div>
        {/if}

      <!-- AI -->
      {:else if chunk.kind === "ai"}
        {@const toolCount = aiToolCount(chunk)}
        {@const toolsVisible = isChunkToolsVisible(i)}
        <div class="msg-row msg-row-ai">
          <div class="msg-ai-container">
            <!-- AI header -->
            <div class="ai-header-row">
              <span class="ai-avatar">C</span>
              <span class="ai-label">Claude</span>
              <span class="ai-model">{aiModel(chunk)}</span>
              {#if toolCount > 0}
                <!-- svelte-ignore a11y_click_events_have_key_events -->
                <!-- svelte-ignore a11y_no_static_element_interactions -->
                <span class="ai-tool-toggle" onclick={() => toggleChunk(i)}>
                  <span class="ai-tool-chevron" class:ai-tool-chevron-open={toolsVisible}>▸</span>
                  {toolCount} tool{toolCount > 1 ? "s" : ""}
                </span>
              {/if}
              <span class="ai-header-spacer"></span>
              <span class="ai-tokens">{fk(chunk.metrics.inputTokens)} / {fk(chunk.metrics.outputTokens)}</span>
              <span class="ai-time">{ftime(chunk.timestamp)}</span>
            </div>

            <!-- Tool rows (toggle visibility) -->
            {#if toolsVisible}
              <div class="ai-tools-section">
                {#each chunk.semanticSteps as step, si}
                  {#if step.kind === "tool_execution"}
                    {@const exec = chunk.toolExecutions.find(e => e.toolUseId === step.toolUseId)}
                    {#if exec}
                      {@const key = `${i}-tool-${exec.toolUseId}`}
                      <BaseItem
                        icon="T"
                        label={exec.toolName}
                        summary={getToolSummary(exec.toolName, exec.input)}
                        status={getToolStatus(exec)}
                        isExpanded={expandedItems.has(key)}
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
                    {/if}
                  {:else if step.kind === "subagent_spawn"}
                    <BaseItem icon=">" label="Subagent" isExpanded={false} onclick={() => {}} />
                  {/if}
                {/each}
              </div>
            {/if}

            <!-- Text + thinking content (always visible) -->
            <div class="ai-body">
              {#each chunk.semanticSteps as step, si}
                {#if step.kind === "thinking"}
                  <BaseItem
                    icon="*"
                    label="Thinking"
                    isExpanded={expandedItems.has(`${i}-think-${si}`)}
                    onclick={() => toggle(`${i}-think-${si}`)}
                  >
                    {#snippet children()}
                      <div class="prose prose-thinking">{@html renderMarkdown(step.text)}</div>
                    {/snippet}
                  </BaseItem>
                {:else if step.kind === "text"}
                  <div class="prose">{@html renderMarkdown(step.text)}</div>
                {/if}
              {/each}
            </div>
          </div>
        </div>

      <!-- System -->
      {:else if chunk.kind === "system"}
        {@const sysText = cleanDisplayText(chunk.contentText)}
        {#if sysText}
          <div class="msg-row msg-row-system">
            <div class="msg-system">
              <span class="system-label">System</span>
              <span class="system-sep">·</span>
              <span class="system-time">{ftime(chunk.timestamp)}</span>
            </div>
            <div class="system-content">{sysText}</div>
          </div>
        {/if}

      <!-- Compact -->
      {:else if chunk.kind === "compact"}
        {@const compactText = cleanDisplayText(chunk.summaryText)}
        {#if compactText}
          <div class="msg-row msg-row-system">
            <div class="msg-system">
              <span class="system-label">Compact</span>
            </div>
            <div class="system-content">{compactText}</div>
          </div>
        {/if}
      {/if}
    {/each}
  </div>
{/if}

<style>
  /* ── States ── */
  .state-msg {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 200px;
    color: var(--color-text-muted);
    font-size: 14px;
  }
  .state-err { color: var(--tool-result-error-text); }

  /* ── Top bar ── */
  .top-bar {
    display: flex;
    align-items: center;
    padding: 10px 24px;
    border-bottom: 1px solid var(--color-border);
    gap: 12px;
  }

  .top-title {
    flex: 1;
    font-size: 14px;
    font-weight: 500;
    color: var(--color-text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .top-meta {
    display: flex;
    gap: 8px;
    flex-shrink: 0;
  }

  .top-badge {
    font-size: 12px;
    color: var(--color-text-muted);
    background: var(--badge-neutral-bg);
    padding: 2px 10px;
    border-radius: 4px;
  }

  /* ── Conversation ── */
  .conversation {
    padding: 16px 24px 48px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .msg-row {
    display: flex;
  }

  .msg-spacer { flex: 1; min-width: 80px; }

  /* ── User bubble ── */
  .msg-row-user {
    justify-content: flex-end;
  }

  .msg-bubble {
    max-width: 75%;
    border-radius: 12px;
    padding: 10px 14px;
  }

  .msg-bubble-user {
    background: var(--chat-user-bg);
    color: var(--chat-user-text);
    border: 1px solid var(--chat-user-border);
  }

  .msg-bubble-header {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 4px;
  }

  .msg-time {
    font-size: 11px;
    color: var(--color-text-muted);
  }

  .msg-who-user {
    font-size: 12px;
    font-weight: 600;
    color: var(--color-text);
  }

  /* ── AI message ── */
  .msg-row-ai {
    justify-content: flex-start;
  }

  .msg-ai-container {
    width: 100%;
    max-width: 95%;
  }

  .ai-header-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 0;
  }

  .ai-avatar {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    border-radius: 6px;
    background: var(--badge-neutral-bg);
    color: var(--color-text);
    font-size: 11px;
    font-weight: 700;
    flex-shrink: 0;
  }

  .ai-label {
    font-size: 13px;
    font-weight: 600;
    color: var(--color-text);
    flex-shrink: 0;
  }

  .ai-model {
    font-size: 11px;
    color: var(--color-text-muted);
    background: var(--badge-neutral-bg);
    padding: 1px 8px;
    border-radius: 4px;
    font-family: var(--font-mono);
    flex-shrink: 0;
  }

  .ai-tool-toggle {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 11px;
    color: var(--color-text-muted);
    cursor: pointer;
    padding: 2px 8px;
    border-radius: 4px;
    transition: background 0.1s, color 0.1s;
    flex-shrink: 0;
  }

  .ai-tool-toggle:hover {
    background: var(--tool-item-hover-bg);
    color: var(--color-text-secondary);
  }

  .ai-tool-chevron {
    font-size: 9px;
    transition: transform 0.15s ease;
  }

  .ai-tool-chevron-open {
    transform: rotate(90deg);
  }

  .ai-tools-section {
    padding: 4px 0 4px 30px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    border-left: 2px solid var(--color-border);
    margin-left: 10px;
    margin-bottom: 4px;
  }

  .ai-header-spacer { flex: 1; }

  .ai-tokens {
    font-size: 11px;
    color: var(--color-text-muted);
    font-family: var(--font-mono);
    flex-shrink: 0;
  }

  .ai-time {
    font-size: 11px;
    color: var(--color-text-muted);
    flex-shrink: 0;
  }

  .ai-body {
    padding: 0 0 8px 30px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  /* ── System ── */
  .msg-row-system {
    flex-direction: column;
    align-items: center;
    padding: 8px 0;
  }

  .msg-system {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
  }

  .system-label {
    color: var(--color-text-muted);
    font-weight: 500;
  }

  .system-sep { color: var(--color-text-muted); }
  .system-time { color: var(--color-text-muted); font-size: 11px; }

  .system-content {
    font-size: 13px;
    color: var(--color-text-muted);
    text-align: center;
    max-width: 600px;
    word-break: break-word;
    padding: 4px 0;
  }

  /* ── Prose (markdown) ── */
  .prose {
    font-size: 14px;
    color: var(--prose-body);
    line-height: 1.65;
    word-break: break-word;
  }
  .prose :global(p) { margin: 0.35em 0; }
  .prose :global(p:first-child) { margin-top: 0; }
  .prose :global(p:last-child) { margin-bottom: 0; }
  .prose :global(h1), .prose :global(h2), .prose :global(h3), .prose :global(h4) {
    color: var(--prose-heading);
    font-weight: 600;
    margin: 0.9em 0 0.35em;
  }
  .prose :global(h1) { font-size: 1.25em; }
  .prose :global(h2) { font-size: 1.12em; }
  .prose :global(h3) { font-size: 1.05em; }
  .prose :global(ul), .prose :global(ol) { margin: 0.35em 0; padding-left: 1.4em; }
  .prose :global(li) { margin: 0.15em 0; }
  .prose :global(code) {
    background: var(--prose-code-bg);
    color: var(--prose-code-text);
    padding: 1px 5px;
    border-radius: 4px;
    font-family: var(--font-mono);
    font-size: 0.87em;
  }
  .prose :global(pre) {
    background: var(--prose-pre-bg);
    border: 1px solid var(--prose-pre-border);
    border-radius: 6px;
    padding: 12px 14px;
    overflow-x: auto;
    margin: 0.6em 0;
    line-height: 1.5;
  }
  .prose :global(pre code) {
    background: none;
    color: var(--color-text-secondary);
    padding: 0;
    border-radius: 0;
  }
  .prose :global(a) { color: var(--prose-link); text-decoration: none; }
  .prose :global(a:hover) { text-decoration: underline; }
  .prose :global(blockquote) {
    border-left: 3px solid var(--prose-blockquote-border);
    margin: 0.5em 0;
    padding: 2px 0 2px 14px;
    color: var(--prose-muted);
  }
  .prose :global(strong) { color: var(--prose-heading); }
  .prose :global(table) {
    border-collapse: collapse;
    margin: 0.6em 0;
    font-size: 0.92em;
    width: 100%;
  }
  .prose :global(th), .prose :global(td) {
    border: 1px solid var(--prose-table-border);
    padding: 5px 10px;
    text-align: left;
  }
  .prose :global(th) {
    background: var(--prose-table-header-bg);
    font-weight: 600;
  }

  /* Prose syntax tokens */
  .prose :global(.hljs-string) { color: var(--syntax-string); }
  .prose :global(.hljs-number) { color: var(--syntax-number); }
  .prose :global(.hljs-keyword), .prose :global(.hljs-literal) { color: var(--syntax-keyword); }
  .prose :global(.hljs-attr) { color: var(--code-filename); }
  .prose :global(.hljs-comment) { color: var(--syntax-comment); }
  .prose :global(.hljs-function), .prose :global(.hljs-title) { color: var(--syntax-function); }
  .prose :global(.hljs-built_in) { color: var(--syntax-type); }
  .prose :global(.hljs-type) { color: var(--syntax-type); }

  /* Thinking prose */
  .prose-thinking {
    color: var(--thinking-content-text);
    font-size: 13px;
  }
</style>

<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { getSessionDetail, type SessionDetail, type Chunk, type AIChunk, type ChunkMetrics, type ToolExecution } from "../lib/api";
  import { renderMarkdown } from "../lib/render";
  import { getToolSummary, getToolStatus, cleanDisplayText } from "../lib/toolHelpers";
  import { buildDisplayItems, buildSummary } from "../lib/displayItemBuilder";
  import { WRENCH, BRAIN, TERMINAL, SLASH, MESSAGE_SQUARE } from "../lib/icons";
  import { tick } from "svelte";
  import { clearHighlights } from "../lib/searchHighlight";
  import { processMermaidBlocks } from "../lib/mermaid";
  import { getTabUIState, saveTabUIState, getCachedSession, setCachedSession } from "../lib/tabStore.svelte";
  import { registerHandler, unregisterHandler, dedupeRefresh } from "../lib/fileChangeStore.svelte";
  import BaseItem from "../components/BaseItem.svelte";
  import SubagentCard from "../components/SubagentCard.svelte";
  import SearchBar from "../components/SearchBar.svelte";
  import ContextPanel from "../components/ContextPanel.svelte";
  import OngoingBanner from "../components/OngoingBanner.svelte";
  import DefaultToolViewer from "../components/tool-viewers/DefaultToolViewer.svelte";
  import ReadToolViewer from "../components/tool-viewers/ReadToolViewer.svelte";
  import EditToolViewer from "../components/tool-viewers/EditToolViewer.svelte";
  import WriteToolViewer from "../components/tool-viewers/WriteToolViewer.svelte";
  import BashToolViewer from "../components/tool-viewers/BashToolViewer.svelte";

  interface Props { tabId: string; projectId: string; sessionId: string; }
  let { tabId, projectId, sessionId }: Props = $props();

  let detail: SessionDetail | null = $state(null);
  let loading = $state(true);
  let error: string | null = $state(null);
  let conversationEl: HTMLElement | undefined = $state();

  // per-tab UI 状态（从 tabStore 恢复）
  let uiState = getTabUIState(tabId);
  let expandedItems: Set<string> = $state(new Set(uiState.expandedItems));
  let expandedChunks: Set<number> = $state(new Set(uiState.expandedChunks));
  let searchVisible = $state(uiState.searchVisible);
  let contextPanelVisible = $state(uiState.contextPanelVisible);

  function toggleChunk(idx: number) {
    const n = new Set(expandedChunks);
    if (n.has(idx)) n.delete(idx); else n.add(idx);
    expandedChunks = n;
  }

  function isChunkToolsVisible(idx: number): boolean {
    return expandedChunks.has(idx);
  }

  function handleKeydown(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key === "f") {
      e.preventDefault();
      searchVisible = true;
    }
  }

  const fileChangeKey = `session-detail-${tabId}`;

  async function refreshDetail() {
    const wasAtBottom = !!conversationEl
      && conversationEl.scrollTop + conversationEl.clientHeight
        >= conversationEl.scrollHeight - 16;
    try {
      const d = await getSessionDetail(projectId, sessionId);
      detail = d;
      setCachedSession(tabId, d);
      if (wasAtBottom) {
        await tick();
        if (conversationEl) {
          conversationEl.scrollTop = conversationEl.scrollHeight;
        }
      }
    } catch (e) {
      console.warn("auto refresh getSessionDetail failed:", e);
    }
  }

  onMount(async () => {
    document.addEventListener("keydown", handleKeydown);

    // 优先从 tabStore 缓存加载 session 数据
    const cached = getCachedSession(tabId);
    if (cached) {
      detail = cached;
      loading = false;
    } else {
      try {
        const d = await getSessionDetail(projectId, sessionId);
        detail = d;
        setCachedSession(tabId, d);
      } catch (e) { error = String(e); }
      finally { loading = false; }
    }

    // 恢复滚动位置
    if (conversationEl && uiState.scrollTop > 0) {
      conversationEl.scrollTop = uiState.scrollTop;
    }

    // 注册 file-change handler：命中当前 (projectId, sessionId) 时合并刷新
    registerHandler(fileChangeKey, (payload) => {
      if (payload.projectId !== projectId || payload.sessionId !== sessionId) return;
      void dedupeRefresh(`detail:${projectId}|${sessionId}`, refreshDetail);
    });
  });

  // Mermaid 图表后处理：detail 加载后扫描并渲染 mermaid 代码块
  $effect(() => {
    if (detail && conversationEl) {
      tick().then(() => processMermaidBlocks(conversationEl!));
    }
  });

  onDestroy(() => {
    document.removeEventListener("keydown", handleKeydown);
    unregisterHandler(fileChangeKey);
    // 保存 per-tab UI 状态
    saveTabUIState(tabId, {
      expandedChunks: new Set(expandedChunks),
      expandedItems: new Set(expandedItems),
      searchVisible,
      contextPanelVisible,
      scrollTop: conversationEl?.scrollTop ?? 0,
    });
  });

  function toggle(key: string) {
    const n = new Set(expandedItems);
    if (n.has(key)) n.delete(key); else n.add(key);
    expandedItems = n;
  }

  // 为 `{#each detail.chunks}` 提供稳定 key。刷新时 chunks 数组整体被替换，
  // 缺 key 会让 Svelte 按索引 diff — 导致所有 chunk 的 DOM 被视为新节点重建，
  // 出现可见闪烁 + mermaid/highlight.js 重跑。用 UserChunk/System/Compact 的
  // `uuid`，AIChunk 取第一条 response 的 `uuid`（AIChunk 结构无顶层 uuid，
  // 但至少有一条 response）；都缺时回落到 timestamp。
  function chunkKey(c: Chunk): string {
    if (c.kind === "ai") return c.responses[0]?.uuid ?? c.timestamp;
    return c.uuid;
  }

  // 最后一个 AIChunk 的索引。ongoing=true 时它的 lastOutput 位置被
  // `<OngoingBanner />` 替代；结束后换回真正的内容。对齐原版
  // `LastOutputDisplay.tsx` 的 `isLastGroup && isSessionOngoing` 语义——
  // banner 占 lastOutput 坑位，不作为独立节点追加到对话流尾部，从而避免
  // ongoing 切换时 scrollHeight 跳变引起的闪烁。
  const lastAiIndex = $derived.by(() => {
    if (!detail) return -1;
    for (let i = detail.chunks.length - 1; i >= 0; i--) {
      if (detail.chunks[i].kind === "ai") return i;
    }
    return -1;
  });

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

<div class="session-detail">
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
      <button
        class="top-badge"
        class:top-badge-active={contextPanelVisible}
        onclick={() => contextPanelVisible = !contextPanelVisible}
      >Context ({detail.chunks.length})</button>
    </div>
  </div>

  <!-- Search bar -->
  <SearchBar
    visible={searchVisible}
    containerEl={conversationEl ?? null}
    onClose={() => searchVisible = false}
  />

  <!-- Content area (conversation + optional context panel) -->
  <div class="content-area">
  <!-- Conversation -->
  <div class="conversation" bind:this={conversationEl}>
    {#each detail.chunks as chunk, i (chunkKey(chunk))}

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
        {@const di = buildDisplayItems(chunk)}
        {@const summaryText = buildSummary(di.items)}
        {@const toolsVisible = isChunkToolsVisible(i)}
        {@const interruptions = chunk.semanticSteps.filter((s) => s.kind === "interruption")}
        <div class="msg-row msg-row-ai">
          <div class="msg-ai-container">
            <!-- AI header -->
            <div class="ai-header-row">
              <span class="ai-avatar">C</span>
              <span class="ai-label">Claude</span>
              <span class="ai-model">{aiModel(chunk)}</span>
              {#if summaryText}
                <!-- svelte-ignore a11y_click_events_have_key_events -->
                <!-- svelte-ignore a11y_no_static_element_interactions -->
                <span class="ai-tool-toggle" onclick={() => toggleChunk(i)}>
                  <span class="ai-tool-chevron" class:ai-tool-chevron-open={toolsVisible}>▸</span>
                  {summaryText}
                </span>
              {/if}
              <span class="ai-header-spacer"></span>
              <span class="ai-tokens">{fk(chunk.metrics.inputTokens)} / {fk(chunk.metrics.outputTokens)}</span>
              <span class="ai-time">{ftime(chunk.timestamp)}</span>
            </div>

            <!-- Display items (toggle visibility) -->
            {#if toolsVisible}
              <div class="ai-tools-section">
                {#each di.items as item, di_idx}
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
                    {@const key = `${i}-tool-${exec.toolUseId}`}
                    <BaseItem
                      svgIcon={WRENCH}
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
                  {:else if item.type === "thinking"}
                    {@const key = `${i}-think-${di_idx}`}
                    <BaseItem
                      svgIcon={BRAIN}
                      label="Thinking"
                      isExpanded={expandedItems.has(key)}
                      onclick={() => toggle(key)}
                    >
                      {#snippet children()}
                        <div class="prose prose-thinking">{@html renderMarkdown(item.text)}</div>
                      {/snippet}
                    </BaseItem>
                  {:else if item.type === "output"}
                    {@const key = `${i}-output-${di_idx}`}
                    <BaseItem
                      svgIcon={MESSAGE_SQUARE}
                      label="Output"
                      summary={item.text.length > 60 ? item.text.slice(0, 60) + "…" : item.text}
                      isExpanded={expandedItems.has(key)}
                      onclick={() => toggle(key)}
                    >
                      {#snippet children()}
                        <div class="prose">{@html renderMarkdown(item.text)}</div>
                      {/snippet}
                    </BaseItem>
                  {:else if item.type === "subagent"}
                    <SubagentCard process={item.process} />
                  {/if}
                {/each}
              </div>
            {/if}

            <!-- Last output (always visible) -->
            <div class="ai-body">
              {#if i === lastAiIndex && detail.isOngoing}
                <!-- 对齐原版 LastOutputDisplay：最后 AI 组在 ongoing 时
                     banner 占 lastOutput 位置，结束后换回真正的内容 -->
                <OngoingBanner />
              {:else if di.lastOutput}
                <div class="prose">{@html renderMarkdown(di.lastOutput.text)}</div>
              {/if}
              {#each interruptions as _interrupt}
                <div class="interruption-block">Session interrupted by user</div>
              {/each}
            </div>
          </div>
        </div>

      <!-- System -->
      {:else if chunk.kind === "system"}
        {@const sysText = cleanDisplayText(chunk.contentText)}
        {#if sysText}
          <div class="msg-row msg-row-system-left">
            <div class="system-header">
              <svg class="system-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d={TERMINAL}/></svg>
              <span class="system-label">System</span>
              <span class="system-time">{ftime(chunk.timestamp)}</span>
            </div>
            <pre class="system-pre">{sysText}</pre>
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

  {#if contextPanelVisible}
    <ContextPanel {detail} onClose={() => contextPanelVisible = false} />
  {/if}
  </div>
{/if}
</div>

<style>
  /* ── Root ── */
  .session-detail {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }

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
    flex-shrink: 0;
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
    border: none;
    cursor: pointer;
    font-family: inherit;
    transition: background 0.1s, color 0.1s;
  }

  .top-badge:hover {
    background: var(--color-border);
    color: var(--color-text-secondary);
  }

  .top-badge-active {
    background: var(--color-border-emphasis);
    color: var(--color-text);
  }

  /* ── Content area ── */
  .content-area {
    flex: 1;
    display: flex;
    overflow: hidden;
    min-height: 0;
  }

  /* ── Conversation ── */
  .conversation {
    flex: 1;
    overflow-y: auto;
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

  .msg-row-system-left {
    padding: 8px 0;
    max-width: 85%;
  }

  .system-header {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 4px;
  }

  .system-icon {
    width: 14px;
    height: 14px;
    color: var(--color-text-muted);
    flex-shrink: 0;
  }

  .system-label {
    color: var(--color-text-muted);
    font-weight: 500;
    font-size: 12px;
  }

  .system-time { color: var(--color-text-muted); font-size: 11px; }

  .system-pre {
    font-size: 12px;
    font-family: var(--font-mono);
    color: var(--chat-system-text);
    background: var(--chat-system-bg);
    border-radius: 12px;
    padding: 10px 14px;
    margin: 0;
    white-space: pre-wrap;
    overflow-x: auto;
    max-height: 384px;
    overflow-y: auto;
    line-height: 1.5;
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

  /* Interruption 块：红色提示 "Session interrupted by user" */
  .interruption-block {
    margin-top: 8px;
    padding: 6px 10px;
    border-radius: 6px;
    background: var(--color-danger-bg, rgba(239, 68, 68, 0.08));
    border: 1px solid var(--color-danger-border, rgba(239, 68, 68, 0.3));
    color: var(--color-danger-text, #ef4444);
    font-size: 12px;
    line-height: 1.4;
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .interruption-block::before {
    content: "⛔";
    font-size: 12px;
  }
</style>

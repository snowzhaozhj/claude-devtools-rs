<script lang="ts">
  import { onMount } from "svelte";
  import { getSessionDetail, type SessionDetail, type Chunk, type AIChunk, type ChunkMetrics, type ToolExecution, type ToolOutput } from "../lib/api";
  import { renderMarkdown, highlightCode } from "../lib/render";

  interface Props { projectId: string; sessionId: string; }
  let { projectId, sessionId }: Props = $props();

  let detail: SessionDetail | null = $state(null);
  let loading = $state(true);
  let error: string | null = $state(null);
  let expandedChunks: Set<number> = $state(new Set());
  let expandedTools: Set<string> = $state(new Set());

  onMount(async () => {
    try { detail = await getSessionDetail(projectId, sessionId); }
    catch (e) { error = String(e); }
    finally { loading = false; }
  });

  function tog<T>(set: Set<T>, key: T): Set<T> {
    const n = new Set(set);
    if (n.has(key)) n.delete(key); else n.add(key);
    return n;
  }

  function sumMetrics(chunks: Chunk[]): ChunkMetrics {
    const r: ChunkMetrics = { inputTokens: 0, outputTokens: 0, cacheCreationTokens: 0, cacheReadTokens: 0, toolCount: 0, costUsd: null };
    for (const c of chunks) { r.inputTokens += c.metrics.inputTokens; r.outputTokens += c.metrics.outputTokens; r.toolCount += c.metrics.toolCount; }
    return r;
  }

  function fk(n: number): string {
    if (n >= 1e6) return (n/1e6).toFixed(1) + "M";
    if (n >= 1e3) return (n/1e3).toFixed(1) + "k";
    return String(n);
  }

  function ftime(ts: string): string {
    try { return new Date(ts).toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit", second: "2-digit" }); } catch { return ""; }
  }

  function aiPreview(chunk: AIChunk): string {
    for (const s of chunk.semanticSteps) if (s.kind === "text") return s.text.length > 100 ? s.text.slice(0, 100) + "…" : s.text;
    return "";
  }

  function utext(content: string | unknown[]): string {
    if (typeof content === "string") return content;
    if (Array.isArray(content)) for (const b of content) {
      if (b && typeof b === "object" && "type" in b) {
        const x = b as Record<string, unknown>;
        if (x.type === "text" && typeof x.text === "string") return x.text;
      }
    }
    return "";
  }

  function tout(o: ToolOutput): string {
    if (o.kind === "text") return o.text;
    if (o.kind === "structured") return JSON.stringify(o.value, null, 2);
    return "";
  }

  function tlabel(e: ToolExecution): string {
    const i = e.input as Record<string, unknown> | null;
    if (!i) return "";
    const n = e.toolName;
    if (["Read","Edit","Write","read_file","edit_file","write_file"].includes(n)) return sp(String(i.file_path ?? i.filePath ?? ""));
    if (["Bash","bash"].includes(n)) { const c = String(i.command ?? ""); return c.length > 60 ? c.slice(0,60)+"…" : c; }
    if (["Grep","grep"].includes(n)) return String(i.pattern ?? "");
    if (["Glob","glob"].includes(n)) return String(i.pattern ?? "");
    if (n === "Agent") return String(i.description ?? "").slice(0,50);
    return "";
  }

  function sp(p: string): string { return p.replace(/^\/Users\/[^/]+/, "~"); }

  function trunc(t: string, m: number) { return t.length <= m ? { t, f: false } : { t: t.slice(0,m), f: true }; }
</script>

{#if loading}
  <p class="center muted">加载中...</p>
{:else if error}
  <p class="center err-text">{error}</p>
{:else if detail}
  {@const m = sumMetrics(detail.chunks)}

  <!-- stats -->
  <div class="stats-row">
    <span class="stats-item">{detail.chunks.length} chunks</span>
    <span class="stats-sep">·</span>
    <span class="stats-item">↓{fk(m.inputTokens)}</span>
    <span class="stats-sep">·</span>
    <span class="stats-item">↑{fk(m.outputTokens)}</span>
    <span class="stats-sep">·</span>
    <span class="stats-item">{m.toolCount} tools</span>
  </div>

  <!-- conversation -->
  <div class="conv">
    {#each detail.chunks as chunk, i}

      <!-- ──── User ──── -->
      {#if chunk.kind === "user"}
        {@const text = utext(chunk.content)}
        {#if text}
          <div class="turn turn-user">
            <div class="turn-meta">
              <span class="turn-who turn-who-user">You</span>
              <span class="turn-time">{ftime(chunk.timestamp)}</span>
            </div>
            <div class="turn-content prose">{@html renderMarkdown(text)}</div>
          </div>
        {/if}

      <!-- ──── AI ──── -->
      {:else if chunk.kind === "ai"}
        <div class="turn turn-ai">
          <button class="ai-bar" onclick={() => expandedChunks = tog(expandedChunks, i)}>
            <span class="ai-dot"></span>
            <span class="ai-name">Claude</span>
            {#if chunk.toolExecutions.length > 0}
              <span class="ai-tools">{chunk.toolExecutions.length} tool{chunk.toolExecutions.length > 1 ? "s" : ""}</span>
            {/if}
            <span class="ai-preview">{aiPreview(chunk)}</span>
            <span class="ai-tokens">{fk(chunk.metrics.inputTokens)} / {fk(chunk.metrics.outputTokens)}</span>
            <span class="ai-time">{ftime(chunk.timestamp)}</span>
            <span class="chevron">{expandedChunks.has(i) ? "▾" : "▸"}</span>
          </button>

          {#if expandedChunks.has(i)}
            <div class="ai-body">
              {#each chunk.semanticSteps as step}
                {#if step.kind === "thinking"}
                  <details class="think-block">
                    <summary class="think-summary">Thinking</summary>
                    <div class="prose think-prose">{@html renderMarkdown(step.text)}</div>
                  </details>

                {:else if step.kind === "text"}
                  <div class="prose">{@html renderMarkdown(step.text)}</div>

                {:else if step.kind === "tool_execution"}
                  {@const exec = chunk.toolExecutions.find(e => e.toolUseId === step.toolUseId)}
                  {#if exec}
                    {@const lbl = tlabel(exec)}
                    {@const isOpen = expandedTools.has(exec.toolUseId)}
                    <button class="tool-row" class:tool-row-err={exec.isError} onclick={() => expandedTools = tog(expandedTools, exec.toolUseId)}>
                      <span class="tool-row-icon">{exec.isError ? "✕" : "▸"}</span>
                      <span class="tool-row-name">{exec.toolName}</span>
                      {#if lbl}<span class="tool-row-sep">–</span><span class="tool-row-label">{lbl}</span>{/if}
                      <span class="tool-row-chevron">{isOpen ? "▾" : "▸"}</span>
                    </button>
                    {#if isOpen}
                      {@const inStr = JSON.stringify(exec.input, null, 2)}
                      {@const outStr = tout(exec.output)}
                      {@const oT = trunc(outStr, 2000)}
                      <div class="tool-expand">
                        <div class="tool-expand-sec">
                          <span class="tool-expand-lbl">INPUT</span>
                          <pre class="codeblk"><code>{@html highlightCode(inStr, "json")}</code></pre>
                        </div>
                        {#if outStr}
                          <div class="tool-expand-sec">
                            <span class="tool-expand-lbl" class:tool-expand-lbl-err={exec.isError}>{exec.isError ? "ERROR" : "OUTPUT"}</span>
                            <pre class="codeblk" class:codeblk-err={exec.isError}><code>{@html highlightCode(oT.t)}</code></pre>
                            {#if oT.f}
                              <button class="more" onclick={(e) => { e.stopPropagation(); expandedTools = tog(expandedTools, `f-${exec.toolUseId}`); }}>
                                {expandedTools.has(`f-${exec.toolUseId}`) ? "收起" : `展开全部 (${outStr.length} chars)`}
                              </button>
                              {#if expandedTools.has(`f-${exec.toolUseId}`)}
                                <pre class="codeblk"><code>{@html highlightCode(outStr)}</code></pre>
                              {/if}
                            {/if}
                          </div>
                        {/if}
                      </div>
                    {/if}
                  {/if}

                {:else if step.kind === "subagent_spawn"}
                  <div class="tool-row">
                    <span class="tool-row-icon">⚡</span>
                    <span class="tool-row-name">Subagent</span>
                  </div>
                {/if}
              {/each}
            </div>
          {/if}
        </div>

      <!-- ──── System / Compact ──── -->
      {:else if chunk.kind === "system"}
        <div class="turn turn-sys">
          <span class="sys-label">System</span>
          <span class="sys-text">{chunk.contentText}</span>
        </div>
      {:else if chunk.kind === "compact"}
        <div class="turn turn-sys">
          <span class="sys-label">Compact</span>
          <span class="sys-text">{chunk.summaryText}</span>
        </div>
      {/if}
    {/each}
  </div>
{/if}

<style>
  .center { text-align: center; padding: 48px; }
  .muted { color: #565f89; }
  .err-text { color: #f7768e; }

  /* ─── Stats ─── */
  .stats-row {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 0;
    margin-bottom: 8px;
    font-size: 12px;
    font-family: "SF Mono", "Fira Code", monospace;
    color: #565f89;
  }
  .stats-sep { color: #3b4261; }

  /* ─── Conversation ─── */
  .conv { display: flex; flex-direction: column; gap: 0; }

  .turn { padding: 14px 0; }
  .turn + .turn { border-top: 1px solid #222436; }

  /* ─── User turn ─── */
  .turn-user { padding-left: 0; }
  .turn-meta { display: flex; align-items: center; gap: 8px; margin-bottom: 6px; }
  .turn-who {
    font-size: 13px;
    font-weight: 600;
  }
  .turn-who-user { color: #9ece6a; }
  .turn-time { font-size: 11px; color: #444b6a; }
  .turn-content { padding-left: 0; }

  /* ─── AI turn ─── */
  .turn-ai {
    border-left: 2px solid #3d59a1;
    padding-left: 14px;
    margin-left: 0;
  }

  .ai-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0;
    margin-bottom: 4px;
    background: none;
    border: none;
    cursor: pointer;
    width: 100%;
    text-align: left;
    font: inherit;
    color: inherit;
  }
  .ai-bar:hover { opacity: 0.85; }

  .ai-dot {
    width: 8px; height: 8px;
    border-radius: 50%;
    background: #7aa2f7;
    flex-shrink: 0;
  }

  .ai-name {
    font-size: 13px;
    font-weight: 600;
    color: #7aa2f7;
    flex-shrink: 0;
  }

  .ai-tools {
    font-size: 11px;
    color: #e0af68;
    background: rgba(224, 175, 104, 0.08);
    padding: 1px 6px;
    border-radius: 8px;
    flex-shrink: 0;
  }

  .ai-preview {
    flex: 1;
    font-size: 12px;
    color: #565f89;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }

  .ai-tokens {
    font-size: 11px;
    color: #444b6a;
    font-family: "SF Mono", "Fira Code", monospace;
    flex-shrink: 0;
  }

  .ai-time {
    font-size: 11px;
    color: #444b6a;
    flex-shrink: 0;
  }

  .chevron {
    font-size: 12px;
    color: #444b6a;
    width: 14px;
    flex-shrink: 0;
  }

  .ai-body {
    padding: 8px 0 0;
  }

  /* ─── System ─── */
  .turn-sys {
    display: flex;
    gap: 8px;
    align-items: baseline;
    padding: 6px 0;
    font-size: 12px;
  }
  .sys-label {
    color: #565f89;
    font-weight: 500;
    flex-shrink: 0;
    font-style: italic;
  }
  .sys-text {
    color: #444b6a;
    white-space: pre-wrap;
    word-break: break-word;
    font-style: italic;
  }

  /* ─── Prose ─── */
  .prose {
    font-size: 14px;
    color: #c0caf5;
    line-height: 1.65;
    word-break: break-word;
    text-align: left;
  }
  .prose :global(p) { margin: 0.35em 0; }
  .prose :global(p:first-child) { margin-top: 0; }
  .prose :global(p:last-child) { margin-bottom: 0; }
  .prose :global(h1), .prose :global(h2), .prose :global(h3), .prose :global(h4) {
    color: #c0caf5; font-weight: 600; margin: 0.9em 0 0.35em;
  }
  .prose :global(h1) { font-size: 1.25em; }
  .prose :global(h2) { font-size: 1.12em; }
  .prose :global(h3) { font-size: 1.05em; }
  .prose :global(ul), .prose :global(ol) { margin: 0.35em 0; padding-left: 1.4em; }
  .prose :global(li) { margin: 0.15em 0; }
  .prose :global(code) {
    background: rgba(122, 162, 247, 0.08);
    color: #7dcfff;
    padding: 1px 5px;
    border-radius: 4px;
    font-family: "SF Mono", "Fira Code", monospace;
    font-size: 0.87em;
  }
  .prose :global(pre) {
    background: #16161e;
    border-radius: 6px;
    padding: 12px 14px;
    overflow-x: auto;
    margin: 0.6em 0;
    line-height: 1.5;
    border: 1px solid #1e2030;
  }
  .prose :global(pre code) { background: none; color: #a9b1d6; padding: 0; border-radius: 0; }
  .prose :global(a) { color: #7aa2f7; text-decoration: none; }
  .prose :global(a:hover) { text-decoration: underline; }
  .prose :global(blockquote) { border-left: 3px solid #3b4261; margin: 0.5em 0; padding: 2px 0 2px 14px; color: #787c99; }
  .prose :global(strong) { color: #c0caf5; }
  .prose :global(table) { border-collapse: collapse; margin: 0.6em 0; font-size: 0.92em; width: 100%; }
  .prose :global(th), .prose :global(td) { border: 1px solid #292e42; padding: 5px 10px; text-align: left; }
  .prose :global(th) { background: #1e2030; font-weight: 600; }

  /* prose highlight tokens */
  .prose :global(.hljs-string) { color: #9ece6a; }
  .prose :global(.hljs-number) { color: #ff9e64; }
  .prose :global(.hljs-keyword), .prose :global(.hljs-literal) { color: #bb9af7; }
  .prose :global(.hljs-attr) { color: #7dcfff; }
  .prose :global(.hljs-comment) { color: #565f89; }
  .prose :global(.hljs-function), .prose :global(.hljs-title) { color: #7aa2f7; }
  .prose :global(.hljs-built_in) { color: #e0af68; }
  .prose :global(.hljs-type) { color: #2ac3de; }

  /* ─── Thinking ─── */
  .think-block {
    margin: 8px 0;
    border-radius: 6px;
    background: rgba(187, 154, 247, 0.03);
  }
  .think-summary {
    padding: 6px 10px;
    font-size: 12px;
    color: #bb9af7;
    cursor: pointer;
    font-weight: 500;
    list-style: none;
    opacity: 0.7;
  }
  .think-summary:hover { opacity: 1; }
  .think-summary::before { content: "▸ "; font-size: 10px; }
  .think-block[open] .think-summary::before { content: "▾ "; }
  .think-prose { padding: 0 10px 10px; font-size: 13px; color: #787c99; }

  /* ─── Tool rows (like original) ─── */
  .tool-row {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 8px;
    margin: 2px 0;
    border-radius: 4px;
    background: none;
    border: none;
    cursor: pointer;
    width: 100%;
    text-align: left;
    font: inherit;
    color: inherit;
  }
  .tool-row:hover { background: rgba(122, 162, 247, 0.04); }
  .tool-row-err .tool-row-icon { color: #f7768e; }

  .tool-row-icon {
    font-size: 11px;
    color: #565f89;
    width: 14px;
    text-align: center;
    flex-shrink: 0;
  }

  .tool-row-name {
    font-size: 13px;
    font-weight: 500;
    color: #a9b1d6;
    flex-shrink: 0;
  }

  .tool-row-sep {
    color: #3b4261;
    flex-shrink: 0;
    font-size: 12px;
  }

  .tool-row-label {
    flex: 1;
    font-size: 12px;
    color: #565f89;
    font-family: "SF Mono", "Fira Code", monospace;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }

  .tool-row-chevron {
    font-size: 11px;
    color: #444b6a;
    flex-shrink: 0;
    width: 12px;
  }

  /* ─── Tool expanded content ─── */
  .tool-expand {
    margin: 2px 0 8px 20px;
    padding-left: 14px;
    border-left: 1px solid #292e42;
  }

  .tool-expand-sec { margin-top: 6px; }
  .tool-expand-sec:first-child { margin-top: 0; }

  .tool-expand-lbl {
    font-size: 9px; font-weight: 600; color: #444b6a;
    letter-spacing: 1px; margin-bottom: 3px; display: block;
  }
  .tool-expand-lbl-err { color: #f7768e; }

  .codeblk {
    font-size: 12px;
    font-family: "SF Mono", "Fira Code", monospace;
    color: #a9b1d6;
    background: #16161e;
    border-radius: 4px;
    padding: 8px 10px;
    margin: 0;
    white-space: pre-wrap;
    word-break: break-all;
    max-height: 280px;
    overflow-y: auto;
    line-height: 1.45;
    text-align: left;
  }
  .codeblk :global(code) { background: none; padding: 0; color: inherit; font: inherit; border-radius: 0; }
  .codeblk-err { color: #f7768e; }

  .codeblk :global(.hljs-string) { color: #9ece6a; }
  .codeblk :global(.hljs-number) { color: #ff9e64; }
  .codeblk :global(.hljs-keyword), .codeblk :global(.hljs-literal) { color: #bb9af7; }
  .codeblk :global(.hljs-attr) { color: #7dcfff; }
  .codeblk :global(.hljs-comment) { color: #565f89; }
  .codeblk :global(.hljs-punctuation) { color: #545c7e; }

  .more {
    background: none; border: none; color: #7aa2f7;
    font-size: 12px; cursor: pointer; padding: 3px 0;
  }
  .more:hover { text-decoration: underline; }
</style>

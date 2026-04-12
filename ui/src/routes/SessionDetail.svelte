<script lang="ts">
  import { onMount } from "svelte";
  import { getSessionDetail, type SessionDetail, type Chunk, type AIChunk, type ChunkMetrics, type SemanticStep, type ToolExecution, type ToolOutput } from "../lib/api";

  interface Props {
    projectId: string;
    sessionId: string;
  }

  let { projectId, sessionId }: Props = $props();

  let detail: SessionDetail | null = $state(null);
  let loading = $state(true);
  let error: string | null = $state(null);
  let expandedChunks: Set<number> = $state(new Set());
  let expandedOutputs: Set<string> = $state(new Set());

  onMount(async () => {
    try {
      detail = await getSessionDetail(projectId, sessionId);
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  });

  function toggleChunk(index: number) {
    const next = new Set(expandedChunks);
    if (next.has(index)) {
      next.delete(index);
    } else {
      next.add(index);
    }
    expandedChunks = next;
  }

  function toggleOutput(id: string) {
    const next = new Set(expandedOutputs);
    if (next.has(id)) {
      next.delete(id);
    } else {
      next.add(id);
    }
    expandedOutputs = next;
  }

  function aggregateMetrics(chunks: Chunk[]): ChunkMetrics {
    const result: ChunkMetrics = {
      inputTokens: 0,
      outputTokens: 0,
      cacheCreationTokens: 0,
      cacheReadTokens: 0,
      toolCount: 0,
      costUsd: null,
    };
    for (const chunk of chunks) {
      const m = chunk.metrics;
      result.inputTokens += m.inputTokens;
      result.outputTokens += m.outputTokens;
      result.cacheCreationTokens += m.cacheCreationTokens;
      result.cacheReadTokens += m.cacheReadTokens;
      result.toolCount += m.toolCount;
    }
    return result;
  }

  function formatTokens(n: number): string {
    if (n >= 1_000_000) return (n / 1_000_000).toFixed(1) + "M";
    if (n >= 1_000) return (n / 1_000).toFixed(1) + "K";
    return String(n);
  }

  function getAiSummary(chunk: AIChunk): string {
    for (const step of chunk.semanticSteps) {
      if (step.kind === "text") {
        return step.text.length > 120 ? step.text.slice(0, 120) + "…" : step.text;
      }
    }
    if (chunk.responses.length > 0) {
      const content = chunk.responses[0].content;
      const text = typeof content === "string" ? content : "";
      if (text) return text.length > 120 ? text.slice(0, 120) + "…" : text;
    }
    return `${chunk.toolExecutions.length} 个工具调用`;
  }

  function getUserText(content: string | unknown[]): string {
    if (typeof content === "string") return content;
    if (Array.isArray(content)) {
      for (const block of content) {
        if (block && typeof block === "object" && "type" in block) {
          const b = block as Record<string, unknown>;
          if (b.type === "text" && typeof b.text === "string") return b.text;
        }
      }
    }
    return "";
  }

  function formatTime(ts: string): string {
    try {
      const d = new Date(ts);
      return d.toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit", second: "2-digit" });
    } catch {
      return "";
    }
  }

  function toolOutputText(output: ToolOutput): string {
    if (output.kind === "text") return output.text;
    if (output.kind === "structured") return JSON.stringify(output.value, null, 2);
    return "(无输出)";
  }

  function truncate(text: string, max: number): { text: string; truncated: boolean } {
    if (text.length <= max) return { text, truncated: false };
    return { text: text.slice(0, max), truncated: true };
  }
</script>

{#if loading}
  <div class="loading">加载中...</div>
{:else if error}
  <div class="error">{error}</div>
{:else if detail}
  {@const totalMetrics = aggregateMetrics(detail.chunks)}
  <div class="metrics-bar">
    <span class="metric">{detail.chunks.length} chunks</span>
    <span class="metric">⬇ {formatTokens(totalMetrics.inputTokens)} in</span>
    <span class="metric">⬆ {formatTokens(totalMetrics.outputTokens)} out</span>
    <span class="metric">🔧 {totalMetrics.toolCount} tools</span>
  </div>

  <div class="chunk-list">
    {#each detail.chunks as chunk, i}
      {#if chunk.kind === "user"}
        {@const userText = getUserText(chunk.content)}
        {#if userText}
          <div class="chunk chunk-user">
            <div class="chunk-header">
              <span class="chunk-badge badge-user">User</span>
              <span class="chunk-time">{formatTime(chunk.timestamp)}</span>
            </div>
            <div class="chunk-body user-text">{userText}</div>
          </div>
        {/if}
      {:else if chunk.kind === "ai"}
        <div class="chunk chunk-ai">
          <button class="chunk-header clickable" onclick={() => toggleChunk(i)}>
            <span class="chunk-badge badge-ai">Claude</span>
            <span class="chunk-summary">{getAiSummary(chunk)}</span>
            <span class="chunk-meta">
              {#if chunk.toolExecutions.length > 0}
                <span class="tool-count">🔧 {chunk.toolExecutions.length}</span>
              {/if}
              <span class="chunk-time">{formatTime(chunk.timestamp)}</span>
              <span class="expand-icon">{expandedChunks.has(i) ? "▼" : "▶"}</span>
            </span>
          </button>

          {#if expandedChunks.has(i)}
            <div class="chunk-body">
              {#each chunk.semanticSteps as step}
                {#if step.kind === "thinking"}
                  <div class="step step-thinking">
                    <span class="step-label">💭 Thinking</span>
                    <pre class="step-content">{step.text}</pre>
                  </div>
                {:else if step.kind === "text"}
                  <div class="step step-text">
                    <span class="step-label">💬 Text</span>
                    <div class="step-content">{step.text}</div>
                  </div>
                {:else if step.kind === "tool_execution"}
                  {@const exec = chunk.toolExecutions.find(e => e.toolUseId === step.toolUseId)}
                  <div class="step step-tool">
                    <span class="step-label">🔧 {step.toolName}</span>
                    {#if exec}
                      {@const inputStr = JSON.stringify(exec.input, null, 2)}
                      {@const inputTrunc = truncate(inputStr, 500)}
                      {@const outStr = toolOutputText(exec.output)}
                      {@const outTrunc = truncate(outStr, 500)}
                      <div class="tool-detail">
                        <div class="tool-section">
                          <span class="tool-section-label">Input</span>
                          <pre class="tool-content">{inputTrunc.text}{#if inputTrunc.truncated}…{/if}</pre>
                          {#if inputTrunc.truncated}
                            <button class="expand-btn" onclick={() => toggleOutput(`in-${exec.toolUseId}`)}>
                              {expandedOutputs.has(`in-${exec.toolUseId}`) ? "收起" : "展开全部"}
                            </button>
                            {#if expandedOutputs.has(`in-${exec.toolUseId}`)}
                              <pre class="tool-content">{inputStr}</pre>
                            {/if}
                          {/if}
                        </div>
                        <div class="tool-section">
                          <span class="tool-section-label" class:tool-error={exec.isError}>
                            {exec.isError ? "❌ Error" : "Output"}
                          </span>
                          <pre class="tool-content" class:tool-error-content={exec.isError}>{outTrunc.text}{#if outTrunc.truncated}…{/if}</pre>
                          {#if outTrunc.truncated}
                            <button class="expand-btn" onclick={() => toggleOutput(`out-${exec.toolUseId}`)}>
                              {expandedOutputs.has(`out-${exec.toolUseId}`) ? "收起" : "展开全部"}
                            </button>
                            {#if expandedOutputs.has(`out-${exec.toolUseId}`)}
                              <pre class="tool-content">{outStr}</pre>
                            {/if}
                          {/if}
                        </div>
                      </div>
                    {/if}
                  </div>
                {:else if step.kind === "subagent_spawn"}
                  <div class="step step-subagent">
                    <span class="step-label">🤖 Subagent</span>
                  </div>
                {/if}
              {/each}
            </div>
          {/if}
        </div>
      {:else if chunk.kind === "system"}
        <div class="chunk chunk-system">
          <div class="chunk-header">
            <span class="chunk-badge badge-system">System</span>
            <span class="chunk-time">{formatTime(chunk.timestamp)}</span>
          </div>
          <div class="chunk-body system-text">{chunk.contentText}</div>
        </div>
      {:else if chunk.kind === "compact"}
        <div class="chunk chunk-compact">
          <div class="chunk-header">
            <span class="chunk-badge badge-compact">Compact</span>
            <span class="chunk-time">{formatTime(chunk.timestamp)}</span>
          </div>
          <div class="chunk-body compact-text">{chunk.summaryText}</div>
        </div>
      {/if}
    {/each}
  </div>
{/if}

<style>
  .loading, .error {
    text-align: center;
    padding: 40px;
    color: #565f89;
  }
  .error { color: #f7768e; }

  .metrics-bar {
    display: flex;
    gap: 16px;
    padding: 10px 14px;
    background: #24283b;
    border: 1px solid #3b4261;
    border-radius: 6px;
    margin-bottom: 12px;
  }
  .metric {
    font-size: 13px;
    color: #7aa2f7;
    font-family: "SF Mono", "Fira Code", monospace;
  }

  .chunk-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .chunk {
    border: 1px solid #3b4261;
    border-radius: 6px;
    overflow: hidden;
  }

  .chunk-header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 12px;
    background: #24283b;
    width: 100%;
    text-align: left;
    border: none;
    color: inherit;
    font: inherit;
  }
  .chunk-header.clickable {
    cursor: pointer;
  }
  .chunk-header.clickable:hover {
    background: #292e42;
  }

  .chunk-badge {
    font-size: 11px;
    font-weight: 600;
    padding: 2px 8px;
    border-radius: 3px;
    flex-shrink: 0;
  }
  .badge-user { background: rgba(158, 206, 106, 0.15); color: #9ece6a; }
  .badge-ai { background: rgba(122, 162, 247, 0.15); color: #7aa2f7; }
  .badge-system { background: rgba(224, 175, 104, 0.15); color: #e0af68; }
  .badge-compact { background: rgba(187, 154, 247, 0.15); color: #bb9af7; }

  .chunk-summary {
    flex: 1;
    font-size: 13px;
    color: #a9b1d6;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .chunk-meta {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }

  .chunk-time {
    font-size: 11px;
    color: #565f89;
    flex-shrink: 0;
  }

  .tool-count {
    font-size: 11px;
    color: #7aa2f7;
  }

  .expand-icon {
    font-size: 10px;
    color: #565f89;
  }

  .chunk-body {
    padding: 10px 12px;
  }

  .user-text {
    font-size: 14px;
    color: #c0caf5;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .system-text, .compact-text {
    font-size: 13px;
    color: #a9b1d6;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .step {
    margin-bottom: 10px;
    border-left: 2px solid #3b4261;
    padding-left: 10px;
  }
  .step:last-child { margin-bottom: 0; }

  .step-label {
    display: block;
    font-size: 12px;
    font-weight: 600;
    color: #7aa2f7;
    margin-bottom: 4px;
  }
  .step-thinking .step-label { color: #bb9af7; }
  .step-text .step-label { color: #9ece6a; }
  .step-tool .step-label { color: #e0af68; }
  .step-subagent .step-label { color: #7dcfff; }

  .step-content {
    font-size: 13px;
    color: #a9b1d6;
    white-space: pre-wrap;
    word-break: break-word;
    margin: 0;
  }

  .tool-detail {
    margin-top: 4px;
  }

  .tool-section {
    margin-bottom: 6px;
  }
  .tool-section:last-child { margin-bottom: 0; }

  .tool-section-label {
    font-size: 11px;
    color: #565f89;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }
  .tool-section-label.tool-error { color: #f7768e; }

  .tool-content {
    font-size: 12px;
    font-family: "SF Mono", "Fira Code", monospace;
    color: #a9b1d6;
    background: #1a1b26;
    border: 1px solid #3b4261;
    border-radius: 4px;
    padding: 6px 8px;
    margin: 4px 0 0;
    white-space: pre-wrap;
    word-break: break-all;
    max-height: 300px;
    overflow-y: auto;
  }
  .tool-error-content {
    border-color: rgba(247, 118, 142, 0.3);
    color: #f7768e;
  }

  .expand-btn {
    background: none;
    border: none;
    color: #7aa2f7;
    font-size: 12px;
    cursor: pointer;
    padding: 2px 0;
  }
  .expand-btn:hover { text-decoration: underline; }
</style>

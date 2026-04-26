<script lang="ts">
  import { onMount, onDestroy, untrack } from "svelte";
  import { getSessionDetail, getToolOutput, type SessionDetail, type Chunk, type AIChunk, type ChunkMetrics, type ToolExecution, type ToolOutput } from "../lib/api";
  import { getToolSummary, getToolStatus, cleanDisplayText, parseTaskNotifications, getToolContextTokens, estimateTokens } from "../lib/toolHelpers";
  import { buildDisplayItemsCached, buildSummary } from "../lib/displayItemBuilder";
  import { WRENCH, BRAIN, TERMINAL, SLASH, MESSAGE_SQUARE, CHEVRON_RIGHT, CLOCK_SVG, USER_SVG } from "../lib/icons";
  import { tick } from "svelte";
  import { clearHighlights } from "../lib/searchHighlight";
  import { processMermaidBlocks } from "../lib/mermaid";
  import { createLazyMarkdownObserver, estimatePlaceholderHeight } from "../lib/lazyMarkdown.svelte";
  import { getTabUIState, saveTabUIState, getCachedSession, setCachedSession } from "../lib/tabStore.svelte";
  import { registerHandler, unregisterHandler, scheduleRefresh, cancelScheduledRefresh } from "../lib/fileChangeStore.svelte";
  import BaseItem from "../components/BaseItem.svelte";
  import SubagentCard from "../components/SubagentCard.svelte";
  import TeammateMessageItem from "../components/TeammateMessageItem.svelte";
  import { getTeamColorSet } from "../lib/teamColors";
  import SearchBar from "../components/SearchBar.svelte";
  import ContextPanel from "../components/ContextPanel.svelte";
  import OngoingBanner from "../components/OngoingBanner.svelte";
  import SessionDetailSkeleton from "../components/SessionDetailSkeleton.svelte";
  import ImageBlock from "../components/ImageBlock.svelte";
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

  // Lazy markdown observer：root 必须是 conversation 容器；mount 时创建，
  // unmount 时 disconnect。observer 创建于 conversationEl 首次绑定后
  // （processMermaid 后处理需要它，所以 attach 时 lazy 检查）。
  let lazyObserver: ReturnType<typeof createLazyMarkdownObserver> | null = null;
  function ensureObserver(): ReturnType<typeof createLazyMarkdownObserver> | null {
    if (!lazyObserver && conversationEl) {
      lazyObserver = createLazyMarkdownObserver(conversationEl);
    }
    return lazyObserver;
  }
  function attachMarkdown(text: string, kind: "user" | "ai" | "system" | "thinking" | "output" | "slash" | "teammate") {
    return (el: HTMLElement) => {
      const obs = ensureObserver();
      if (!obs) return;
      // 占位高度估算：进入视口前 min-height 控制 layout 稳定
      el.style.minHeight = `${estimatePlaceholderHeight(text, kind)}px`;
      obs.observe(el, text, async (rendered) => {
        // 渲染完成后清理 min-height（让真实高度接管），并扫该子树的 mermaid block
        rendered.style.minHeight = "";
        await processMermaidBlocks(rendered);
      });
    };
  }

  // per-tab UI 状态（从 tabStore 恢复）—— tabId 在组件生命周期内不会变（切 tab 走 destroy/recreate），
  // 用 untrack 显式声明只读初始值，消 Svelte 5 state_referenced_locally warning
  let uiState = getTabUIState(untrack(() => tabId));
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

  const fileChangeKey = `session-detail-${untrack(() => tabId)}`;

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

    // 性能探针：拆 IPC / DOM-mount / mermaid 三段。仅首次（无缓存）首屏采样。
    // 走 console，便于在 Tauri devtools 里直接看；不接入正式 telemetry。
    const t_mount = performance.now();

    // 优先从 tabStore 缓存加载 session 数据
    const cached = getCachedSession(tabId);
    if (cached) {
      detail = cached;
      loading = false;
      console.info(`[perf] SessionDetail ${sessionId.slice(0, 8)} cached hit`);
    } else {
      try {
        const t_ipc = performance.now();
        const d = await getSessionDetail(projectId, sessionId);
        const ipc_ms = performance.now() - t_ipc;
        const chunks_len = d.chunks.length;
        const payload_kb = JSON.stringify(d).length / 1024;
        detail = d;
        setCachedSession(tabId, d);
        console.info(
          `[perf] SessionDetail ${sessionId.slice(0, 8)} IPC ${ipc_ms.toFixed(0)}ms (chunks=${chunks_len}, payload=${payload_kb.toFixed(0)}KB)`
        );
      } catch (e) { error = String(e); }
      finally { loading = false; }

      // 等 DOM 真正 mount 完
      await tick();
      const total_ms = performance.now() - t_mount;
      console.info(`[perf] SessionDetail ${sessionId.slice(0, 8)} first-paint ${total_ms.toFixed(0)}ms`);
    }

    // 恢复滚动位置
    if (conversationEl && uiState.scrollTop > 0) {
      conversationEl.scrollTop = uiState.scrollTop;
    }

    // 注册 file-change handler：命中当前 (projectId, sessionId) 时合并刷新
    registerHandler(fileChangeKey, (payload) => {
      if (payload.projectId !== projectId || payload.sessionId !== sessionId) return;
      scheduleRefresh(`detail:${projectId}|${sessionId}`, refreshDetail);
    });
  });

  // Mermaid 图表后处理：旧版本在首屏 effect 全树扫描；现在迁移到
  // lazy markdown observer 的 onRendered 回调内（按 chunk 子树扫描），
  // 见 attachMarkdown 与 design.md decision 3。

  onDestroy(() => {
    document.removeEventListener("keydown", handleKeydown);
    unregisterHandler(fileChangeKey);
    cancelScheduledRefresh(`detail:${projectId}|${sessionId}`);
    lazyObserver?.disconnect();
    lazyObserver = null;
    // 保存 per-tab UI 状态
    saveTabUIState(tabId, {
      expandedChunks: new Set(expandedChunks),
      expandedItems: new Set(expandedItems),
      searchVisible,
      contextPanelVisible,
      scrollTop: conversationEl?.scrollTop ?? 0,
    });
  });

  // tool output 懒拉缓存：toolUseId → ToolOutput。仅当 exec.outputOmitted=true
  // 且用户首次展开该 tool 时通过 getToolOutput IPC 拉取。展开后渲染走
  // `effectiveOutput()` —— cache 命中优先，否则用 exec.output（兼容老后端 / 回滚）。
  //
  // LRU 上限：长会话连续展开多 tool 时上限 200，超出按插入顺序（Map 迭代顺序）
  // 淘汰最旧项。命中时把 key 重新 set 到尾部，保持最近使用排序。
  const OUTPUT_CACHE_LIMIT = 200;
  let outputCache: Map<string, ToolOutput> = $state(new Map());

  function effectiveExec(exec: ToolExecution): ToolExecution {
    const cached = outputCache.get(exec.toolUseId);
    if (!cached) return exec;
    return { ...exec, output: cached };
  }

  async function ensureToolOutput(exec: ToolExecution): Promise<void> {
    if (!exec.outputOmitted) return;
    const cached = outputCache.get(exec.toolUseId);
    if (cached) {
      // 命中即升级到 LRU 末尾：delete + set 让 Map 迭代顺序反映"最近使用"。
      // 这里在 user-triggered toggle 路径上调（不在 render 路径），mutate
      // outputCache state 触发的重渲染就是"展开 tool"本身想要的，无副作用。
      const next = new Map(outputCache);
      next.delete(exec.toolUseId);
      next.set(exec.toolUseId, cached);
      outputCache = next;
      return;
    }
    try {
      const out = await getToolOutput(sessionId, sessionId, exec.toolUseId);
      const next = new Map(outputCache);
      next.set(exec.toolUseId, out);
      while (next.size > OUTPUT_CACHE_LIMIT) {
        const firstKey = next.keys().next().value;
        if (firstKey === undefined) break;
        next.delete(firstKey);
      }
      outputCache = next;
    } catch (e) {
      console.warn("[perf] getToolOutput failed", exec.toolUseId, e);
      // 失败保持 exec.output 原样（空），ToolViewer 显示 broken/missing 状态
    }
  }

  function toggle(key: string, exec?: ToolExecution) {
    const n = new Set(expandedItems);
    const opening = !n.has(key);
    if (opening) n.add(key);
    else n.delete(key);
    expandedItems = n;
    if (opening && exec) {
      // fire-and-forget：UI 立即展开占位，IPC 完成后 outputCache 触发重渲染
      void ensureToolOutput(exec);
    }
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

  function fduration(ms: number): string {
    if (ms < 1000) return `${ms}ms`;
    const s = ms / 1000;
    if (s < 60) return `${s.toFixed(1)}s`;
    const m = Math.floor(s / 60);
    const rs = Math.floor(s % 60);
    return `${m}m ${rs}s`;
  }

  function toolDurationMs(exec: ToolExecution): number | undefined {
    if (!exec.endTs) return undefined;
    const start = Date.parse(exec.startTs);
    const end = Date.parse(exec.endTs);
    if (Number.isNaN(start) || Number.isNaN(end)) return undefined;
    return Math.max(0, end - start);
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

  /** 抽出 user content blocks 内的所有 image，附带稳定 blockId（chunkUuid:blockIndex）。*/
  function uimages(content: string | unknown[], chunkUuid: string): { source: import("../lib/api").ImageSource; blockId: string }[] {
    if (!Array.isArray(content)) return [];
    const out: { source: import("../lib/api").ImageSource; blockId: string }[] = [];
    content.forEach((b, idx) => {
      if (b && typeof b === "object" && "type" in b) {
        const x = b as Record<string, unknown>;
        if (x.type === "image" && x.source && typeof x.source === "object") {
          out.push({
            source: x.source as import("../lib/api").ImageSource,
            blockId: `${chunkUuid}:${idx}`,
          });
        }
      }
    });
    return out;
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
  <SessionDetailSkeleton />
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
        {@const images = uimages(chunk.content, chunk.uuid)}
        {@const taskNotifications = parseTaskNotifications(chunk.content)}
        {#if text || images.length > 0 || taskNotifications.length > 0}
          <div class="msg-row msg-row-user">
            <div class="msg-spacer"></div>
            <div class="msg-bubble msg-bubble-user">
              <div class="msg-bubble-header">
                <span class="msg-time">{ftime(chunk.timestamp)}</span>
                <span class="msg-who-user">You</span>
                <span class="msg-avatar-user">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{@html USER_SVG}</svg>
                </span>
              </div>
              {#if text}
                <div class="prose lazy-md" {@attach attachMarkdown(text, "user")}></div>
              {/if}
              {#each images as img (img.blockId)}
                <ImageBlock
                  source={img.source}
                  rootSessionId={sessionId}
                  sessionId={sessionId}
                  blockId={img.blockId}
                />
              {/each}
              {#each taskNotifications as notif (notif.taskId)}
                {@const isFailed = notif.status === "failed" || notif.status === "error"}
                {@const isCompleted = notif.status === "completed"}
                {@const cmdMatch = /"([^"]+)"/.exec(notif.summary)}
                {@const cmdName = cmdMatch?.[1] ?? notif.summary}
                {@const exitMatch = /\(exit code (\d+)\)/.exec(notif.summary)}
                {@const exitCode = exitMatch?.[1]}
                {@const fileBase = notif.outputFile.split("/").pop() ?? ""}
                <div
                  class="task-notif"
                  class:task-notif-done={isCompleted}
                  class:task-notif-fail={isFailed}
                >
                  <span class="task-notif-icon" aria-hidden="true">
                    {#if isFailed}✕{:else if isCompleted}✓{:else}○{/if}
                  </span>
                  <div class="task-notif-body">
                    <div class="task-notif-name">{cmdName || notif.taskId}</div>
                    <div class="task-notif-meta">
                      <span class="task-notif-status">{notif.status}</span>
                      {#if exitCode != null}
                        <span>exit {exitCode}</span>
                      {/if}
                      {#if fileBase}
                        <span class="task-notif-file">{fileBase}</span>
                      {/if}
                    </div>
                  </div>
                </div>
              {/each}
            </div>
          </div>
        {/if}

      <!-- AI -->
      {:else if chunk.kind === "ai"}
        {@const di = buildDisplayItemsCached(chunk)}
        {@const summaryText = buildSummary(di.items)}
        {@const toolsVisible = isChunkToolsVisible(i)}
        {@const interruptions = chunk.semanticSteps.filter((s) => s.kind === "interruption")}
        <!--
          对齐原版 AIChatGroup.tsx:234-248 "Get the LAST assistant message's
          usage (represents current context window snapshot)"——Anthropic API
          的 usage 每次 call 都返回整段历史的 cache 大小，累加会重复计数；
          取最后一条 response.usage 即为该 AI turn 结束时的 context snapshot。
        -->
        {@const lastUsage = [...chunk.responses].reverse().find((r) => r.usage)?.usage ?? null}
        {@const headerInputTokens = lastUsage?.input_tokens ?? 0}
        {@const headerOutputTokens = lastUsage?.output_tokens ?? 0}
        {@const headerCacheRead = lastUsage?.cache_read_input_tokens ?? 0}
        {@const headerCacheCreation = lastUsage?.cache_creation_input_tokens ?? 0}
        {@const totalTokens = headerInputTokens + headerOutputTokens + headerCacheRead + headerCacheCreation}
        <div class="msg-row msg-row-ai">
          <div class="msg-ai-container">
            <!-- AI header -->
            <div class="ai-header-row">
              <span class="ai-avatar" aria-hidden="true">
                <!-- lucide Bot：与原版 AIChatGroup.tsx 行 408 的 <Bot/> 对齐（多 path / rect） -->
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <path d="M12 8V4H8" />
                  <rect width="16" height="12" x="4" y="8" rx="2" />
                  <path d="M2 14h2" />
                  <path d="M20 14h2" />
                  <path d="M15 13v2" />
                  <path d="M9 13v2" />
                </svg>
              </span>
              <span class="ai-label">Claude</span>
              <span class="ai-model">{aiModel(chunk)}</span>
              {#if summaryText}
                <!-- svelte-ignore a11y_click_events_have_key_events -->
                <!-- svelte-ignore a11y_no_static_element_interactions -->
                <span class="ai-tool-toggle" onclick={() => toggleChunk(i)}>
                  <span class="ai-tool-chevron" class:ai-tool-chevron-open={toolsVisible}>
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d={CHEVRON_RIGHT} /></svg>
                  </span>
                  {summaryText}
                </span>
              {/if}
              <span class="ai-header-spacer"></span>
              {#if totalTokens > 0}
                <span class="ai-tokens">
                  <!-- lucide Info：对齐原版 TokenUsageDisplay.tsx 的 Info icon 前缀 -->
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="ai-tokens-info" aria-hidden="true">
                    <circle cx="12" cy="12" r="10" />
                    <path d="M12 16v-4" />
                    <path d="M12 8h.01" />
                  </svg>
                  <span>{fk(totalTokens)}</span>
                  <span class="ai-tokens-popover" role="tooltip">
                    <span class="tok-row tok-row-total"><span>Total</span><span>{totalTokens.toLocaleString()}</span></span>
                    <span class="tok-row"><span>Input</span><span>{headerInputTokens.toLocaleString()}</span></span>
                    <span class="tok-row"><span>Output</span><span>{headerOutputTokens.toLocaleString()}</span></span>
                    <span class="tok-row"><span>Cache create</span><span>{headerCacheCreation.toLocaleString()}</span></span>
                    <span class="tok-row"><span>Cache read</span><span>{headerCacheRead.toLocaleString()}</span></span>
                  </span>
                </span>
              {/if}
              {#if chunk.durationMs != null && chunk.durationMs > 0}
                <span class="ai-duration" title="耗时">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{@html CLOCK_SVG}</svg>
                  {fduration(chunk.durationMs)}
                </span>
              {/if}
              <span class="ai-time">{ftime(chunk.timestamp)}</span>
            </div>

            <!-- Display items (toggle visibility) -->
            {#if toolsVisible}
              <div class="ai-tools-section">
                {#each di.items as item, di_idx}
                  {#if item.type === "slash"}
                    {@const slashKey = `${i}-slash-${di_idx}`}
                    {@const hasInstructions = !!item.slash.instructions}
                    <BaseItem
                      svgIcon={SLASH}
                      label={"/" + item.slash.name}
                      summary={item.slash.args ?? item.slash.message ?? ""}
                      tokenCount={hasInstructions ? Math.ceil((item.slash.instructions ?? "").length / 4) : undefined}
                      status={hasInstructions ? "ok" : undefined}
                      isExpanded={hasInstructions && expandedItems.has(slashKey)}
                      onclick={hasInstructions ? () => toggle(slashKey) : () => {}}
                    >
                      {#snippet children()}
                        {#if item.slash.instructions}
                          <div class="prose slash-instructions lazy-md" {@attach attachMarkdown(item.slash.instructions, "slash")}></div>
                        {/if}
                      {/snippet}
                    </BaseItem>
                  {:else if item.type === "tool"}
                    {@const exec = item.execution}
                    {@const key = `${i}-tool-${exec.toolUseId}`}
                    {@const eff = effectiveExec(exec)}
                    <BaseItem
                      svgIcon={WRENCH}
                      label={exec.toolName}
                      summary={getToolSummary(exec.toolName, exec.input)}
                      tokenCount={getToolContextTokens(exec)}
                      status={getToolStatus(exec)}
                      durationMs={toolDurationMs(exec)}
                      isExpanded={expandedItems.has(key)}
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
                    {@const key = `${i}-think-${di_idx}`}
                    <BaseItem
                      svgIcon={BRAIN}
                      label="Thinking"
                      tokenCount={estimateTokens(item.text)}
                      isExpanded={expandedItems.has(key)}
                      onclick={() => toggle(key)}
                    >
                      {#snippet children()}
                        <div class="prose prose-thinking lazy-md" {@attach attachMarkdown(item.text, "thinking")}></div>
                      {/snippet}
                    </BaseItem>
                  {:else if item.type === "output"}
                    {@const key = `${i}-output-${di_idx}`}
                    <BaseItem
                      svgIcon={MESSAGE_SQUARE}
                      label="Output"
                      summary={item.text.length > 60 ? item.text.slice(0, 60) + "…" : item.text}
                      tokenCount={estimateTokens(item.text)}
                      isExpanded={expandedItems.has(key)}
                      onclick={() => toggle(key)}
                    >
                      {#snippet children()}
                        <div class="prose lazy-md" {@attach attachMarkdown(item.text, "output")}></div>
                      {/snippet}
                    </BaseItem>
                  {:else if item.type === "subagent"}
                    <SubagentCard process={item.process} rootSessionId={sessionId} />
                  {:else if item.type === "teammate_message"}
                    <TeammateMessageItem
                      teammateMessage={item.teammateMessage}
                      attachBody={attachMarkdown(item.teammateMessage.body, "teammate")}
                      rootSessionId={sessionId}
                    />
                  {:else if item.type === "teammate_spawn"}
                    {@const colors = getTeamColorSet(item.color)}
                    <div class="teammate-spawn-row">
                      <span class="teammate-spawn-dot" style:background-color={colors.border}></span>
                      <span
                        class="teammate-spawn-badge"
                        style:background-color={colors.badge}
                        style:color={colors.text}
                        style:border-color="{colors.border}40"
                      >
                        {item.name}
                      </span>
                      <span class="teammate-spawn-label">Teammate spawned</span>
                    </div>
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
                <div class="prose lazy-md" {@attach attachMarkdown(di.lastOutput.text, "ai")}></div>
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

  /* ── Teammate spawn 极简单行（对齐原版 LinkedToolItem.tsx::isTeammateSpawned）── */
  .teammate-spawn-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 12px;
  }
  .teammate-spawn-dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    flex-shrink: 0;
  }
  .teammate-spawn-badge {
    font-size: 10px;
    font-weight: 500;
    letter-spacing: 0.03em;
    padding: 1px 6px;
    border-radius: 4px;
    border: 1px solid transparent;
    flex-shrink: 0;
  }
  .teammate-spawn-label {
    font-size: 12px;
    color: var(--card-icon-muted);
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
    box-shadow: var(--chat-user-shadow);
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
    color: var(--color-text-secondary);
  }

  .msg-avatar-user {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: var(--color-text-secondary);
  }

  .msg-avatar-user svg {
    width: 13px;
    height: 13px;
  }

  /* task-notification 卡片：移植自原版 UserChatGroup.tsx 末尾的 task notif 渲染 */
  .task-notif {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 6px 12px;
    margin-top: 6px;
    border-radius: 8px;
    background: var(--card-bg);
    border: 1px solid var(--card-border);
  }

  .task-notif-icon {
    flex-shrink: 0;
    margin-top: 1px;
    font-size: 14px;
    line-height: 1;
    color: var(--color-text-muted);
  }

  .task-notif-done .task-notif-icon { color: var(--badge-success-text, #22c55e); }
  .task-notif-fail .task-notif-icon { color: var(--error-highlight-text, #ef4444); }

  .task-notif-body {
    min-width: 0;
    flex: 1;
  }

  .task-notif-name {
    font-size: 12px;
    font-weight: 500;
    color: var(--color-text-secondary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .task-notif-meta {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 10px;
    color: var(--color-text-muted);
  }

  .task-notif-status { text-transform: capitalize; }
  .task-notif-file {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 240px;
  }

  /* ── AI message ── */
  .msg-row-ai {
    justify-content: flex-start;
  }

  .msg-ai-container {
    width: 100%;
    max-width: 95%;
    border-left: 2px solid var(--chat-ai-border);
    padding-left: 12px;
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
    color: var(--color-text-secondary);
    flex-shrink: 0;
  }

  .ai-avatar svg {
    width: 16px;
    height: 16px;
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
    display: inline-flex;
    align-items: center;
    justify-content: center;
    transition: transform 0.15s ease;
  }

  .ai-tool-chevron svg {
    width: 11px;
    height: 11px;
  }

  .ai-tool-chevron-open {
    transform: rotate(90deg);
  }

  .ai-tools-section {
    padding: 4px 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin-bottom: 4px;
  }

  .ai-header-spacer { flex: 1; }

  .ai-tokens {
    position: relative;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 11px;
    color: var(--color-text-muted);
    font-family: var(--font-mono);
    flex-shrink: 0;
    cursor: default;
  }

  .ai-tokens-info {
    width: 12px;
    height: 12px;
    flex-shrink: 0;
    opacity: 0.7;
  }

  .ai-tokens-popover {
    position: absolute;
    top: calc(100% + 6px);
    right: 0;
    z-index: 20;
    min-width: 160px;
    padding: 8px 10px;
    border-radius: 6px;
    background: var(--card-bg);
    border: 1px solid var(--card-border);
    box-shadow: 0 4px 14px rgba(0, 0, 0, 0.15);
    display: none;
    flex-direction: column;
    gap: 4px;
    font-size: 11px;
  }

  .ai-tokens:hover .ai-tokens-popover {
    display: flex;
  }

  .tok-row {
    display: flex;
    justify-content: space-between;
    gap: 12px;
  }

  .tok-row > :first-child {
    color: var(--color-text-muted);
  }

  .tok-row > :last-child {
    color: var(--color-text-secondary);
    font-variant-numeric: tabular-nums;
  }

  .tok-row-total {
    padding-bottom: 4px;
    margin-bottom: 2px;
    border-bottom: 1px solid var(--card-separator, var(--card-border));
    font-weight: 600;
  }

  .tok-row-total > :last-child {
    color: var(--color-text);
  }

  .ai-duration {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 11px;
    color: var(--color-text-muted);
    font-family: var(--font-mono);
    flex-shrink: 0;
    white-space: nowrap;
  }

  .ai-duration svg {
    width: 11px;
    height: 11px;
  }

  .ai-time {
    font-size: 11px;
    color: var(--color-text-muted);
    flex-shrink: 0;
  }

  .ai-body {
    padding: 0 0 8px 0;
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

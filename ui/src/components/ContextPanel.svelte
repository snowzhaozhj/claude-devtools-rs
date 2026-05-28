<script lang="ts">
  import {
    parseInjections,
    selectActivePhaseInjections,
    sumTokens,
    formatTokens,
    CATEGORY_COLORS,
    type ContextInjection,
  } from "../lib/contextExtractor";
  import type { SessionDetail } from "../lib/api";
  import UserMessagesSection from "./contextPanel/UserMessagesSection.svelte";
  import ClaudeMdFilesSection from "./contextPanel/ClaudeMdFilesSection.svelte";
  import MentionedFilesSection from "./contextPanel/MentionedFilesSection.svelte";
  import ToolOutputsSection from "./contextPanel/ToolOutputsSection.svelte";
  import TaskCoordinationSection from "./contextPanel/TaskCoordinationSection.svelte";
  import ThinkingTextSection from "./contextPanel/ThinkingTextSection.svelte";
  import PhaseSelector from "./contextPanel/PhaseSelector.svelte";

  interface Props {
    detail: SessionDetail;
    onClose: () => void;
    onNavigateToChunk: (chunkId: string) => void;
    onNavigateToTool: (chunkId: string, toolUseId: string) => void;
    onNavigateToUserGroup: (aiGroupId: string) => void;
  }

  let { detail, onClose, onNavigateToChunk, onNavigateToTool, onNavigateToUserGroup }: Props =
    $props();

  type ViewMode = "category" | "ranked";
  type RankedMode = "grouped" | "flat";
  let viewMode: ViewMode = $state("category");
  let rankedMode: RankedMode = $state("grouped");

  // Phase 切换：null = Latest
  let selectedPhase: number | null = $state(null);

  // 默认展开所有 Section（对齐 TS 原版）
  type SectionKey = "user" | "claudemd" | "mentioned" | "tool" | "task" | "thinking";
  let expandedSections: Set<SectionKey> = $state(
    new Set<SectionKey>(["user", "claudemd", "mentioned", "tool", "task", "thinking"]),
  );

  function toggleSection(key: SectionKey) {
    const n = new Set(expandedSections);
    if (n.has(key)) n.delete(key);
    else n.add(key);
    expandedSections = n;
  }

  // 按 phase 过滤的当前 injections（D5b：直接从 injectionsByPhase 取）
  const injections = $derived(selectActivePhaseInjections(detail, selectedPhase));

  const totalTokens = $derived(sumTokens(injections));

  // 按 category 分组（Category 视图各 Section 用）
  const claudeMd = $derived(
    injections.filter((i): i is Extract<ContextInjection, { category: "claude-md" }> =>
      i.category === "claude-md",
    ),
  );
  const mentionedFiles = $derived(
    injections.filter((i): i is Extract<ContextInjection, { category: "mentioned-file" }> =>
      i.category === "mentioned-file",
    ),
  );
  const toolOutputs = $derived(
    injections.filter((i): i is Extract<ContextInjection, { category: "tool-output" }> =>
      i.category === "tool-output",
    ),
  );
  const thinkingTexts = $derived(
    injections.filter((i): i is Extract<ContextInjection, { category: "thinking-text" }> =>
      i.category === "thinking-text",
    ),
  );
  const taskCoordinations = $derived(
    injections.filter((i): i is Extract<ContextInjection, { category: "task-coordination" }> =>
      i.category === "task-coordination",
    ),
  );
  const userMessages = $derived(
    injections.filter((i): i is Extract<ContextInjection, { category: "user-message" }> =>
      i.category === "user-message",
    ),
  );

  // Ranked 视图
  const ranked = $derived([...injections].sort((a, b) => b.estimatedTokens - a.estimatedTokens));
  const rankedGrouped = $derived.by(() => {
    const buckets = new Map<ContextInjection["category"], ContextInjection[]>();
    for (const inj of ranked) {
      const list = buckets.get(inj.category) ?? [];
      list.push(inj);
      buckets.set(inj.category, list);
    }
    return Array.from(buckets.entries());
  });

  // Phase Selector 相关
  const phases = $derived(detail.phaseInfo?.phases ?? []);
  const showPhaseSelector = $derived(phases.length > 1);

  // injection 跳转到 user-message 时走 user group helper
  function navigateUserMessage(aiGroupId: string) {
    onNavigateToUserGroup(aiGroupId);
  }

  function rowLabel(inj: ContextInjection): string {
    switch (inj.category) {
      case "claude-md":
        return inj.displayName || inj.path;
      case "mentioned-file":
        return inj.displayName || inj.path;
      case "tool-output":
        return inj.toolBreakdown.map((b) => b.toolName).join(", ") || `${inj.toolCount} tools`;
      case "thinking-text":
        return `Turn ${inj.turnIndex + 1} thinking/text`;
      case "task-coordination":
        return `Turn ${inj.turnIndex + 1} coordination`;
      case "user-message":
        return inj.textPreview || `Turn ${inj.turnIndex + 1}`;
    }
  }

  function rowNavigate(inj: ContextInjection) {
    if (inj.category === "user-message") {
      onNavigateToUserGroup(inj.aiGroupId);
      return;
    }
    if (inj.category === "claude-md") {
      // claude-md 没有 aiGroupId，定位不到 chunk——保持 no-op
      return;
    }
    if (inj.category === "mentioned-file") {
      onNavigateToChunk(inj.firstSeenInGroup);
      return;
    }
    onNavigateToChunk(inj.aiGroupId);
  }
</script>

<aside class="context-panel">
  <!-- Header -->
  <div class="cp-header">
    <div class="cp-title-row">
      <div class="cp-title-wrap">
        <svg class="cp-title-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z" />
          <path d="M14 2v4a2 2 0 0 0 2 2h4" />
          <path d="M10 9H8" />
          <path d="M16 13H8" />
          <path d="M16 17H8" />
        </svg>
        <span class="cp-title">Visible Context</span>
        <span class="cp-count-badge">{injections.length}</span>
      </div>
      <button class="cp-close" onclick={onClose} title="关闭" aria-label="关闭 Context 面板">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <path d="M18 6 6 18" />
          <path d="m6 6 12 12" />
        </svg>
      </button>
    </div>
    <div class="cp-token-row">
      <span class="cp-token-muted">Visible:</span>
      <span class="cp-token-value">~{formatTokens(totalTokens)}</span>
    </div>
    <div class="cp-mode-row">
      <span class="cp-mode-label">View:</span>
      <button
        class="cp-mode-btn"
        class:cp-mode-active={viewMode === "category"}
        onclick={() => (viewMode = "category")}
      >
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M3 6h18" /><path d="M3 12h18" /><path d="M3 18h18" /></svg>
        Category
      </button>
      <button
        class="cp-mode-btn"
        class:cp-mode-active={viewMode === "ranked"}
        onclick={() => (viewMode = "ranked")}
      >
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="m3 16 4 4 4-4" /><path d="M7 20V4" /><path d="M11 4h10" /><path d="M11 8h7" /><path d="M11 12h4" /></svg>
        By Size
      </button>
    </div>
    {#if showPhaseSelector}
      <PhaseSelector {phases} selected={selectedPhase} onChange={(v) => (selectedPhase = v)} />
    {/if}
  </div>

  <div class="cp-body">
    {#if injections.length === 0}
      <div class="cp-empty">
        {selectedPhase !== null ? "本 phase 无 injection" : "本会话暂无 context injection"}
      </div>
    {:else if viewMode === "category"}
      <UserMessagesSection
        injections={userMessages}
        expanded={expandedSections.has("user")}
        onToggle={() => toggleSection("user")}
        onNavigate={navigateUserMessage}
      />
      <ClaudeMdFilesSection
        injections={claudeMd}
        expanded={expandedSections.has("claudemd")}
        onToggle={() => toggleSection("claudemd")}
      />
      <MentionedFilesSection
        injections={mentionedFiles}
        expanded={expandedSections.has("mentioned")}
        onToggle={() => toggleSection("mentioned")}
        onNavigate={onNavigateToChunk}
      />
      <ToolOutputsSection
        injections={toolOutputs}
        expanded={expandedSections.has("tool")}
        onToggle={() => toggleSection("tool")}
        onNavigateTool={onNavigateToTool}
      />
      <TaskCoordinationSection
        injections={taskCoordinations}
        expanded={expandedSections.has("task")}
        onToggle={() => toggleSection("task")}
        onNavigate={onNavigateToChunk}
      />
      <ThinkingTextSection
        injections={thinkingTexts}
        expanded={expandedSections.has("thinking")}
        onToggle={() => toggleSection("thinking")}
        onNavigate={onNavigateToChunk}
      />
    {:else}
      <!-- Ranked 视图 -->
      <div class="cp-ranked-toggle">
        <button
          class="cp-ranked-btn"
          class:cp-ranked-active={rankedMode === "grouped"}
          onclick={() => (rankedMode = "grouped")}
        >
          Grouped
        </button>
        <button
          class="cp-ranked-btn"
          class:cp-ranked-active={rankedMode === "flat"}
          onclick={() => (rankedMode = "flat")}
        >
          Flat
        </button>
      </div>

      {#if rankedMode === "flat"}
        <div class="cp-ranked-list">
          {#each ranked as inj (inj.id)}
            {@const color = CATEGORY_COLORS[inj.category]}
            <button type="button" class="cp-ranked-item" onclick={() => rowNavigate(inj)}>
              <span class="cp-cat-tag" style:background={color.bg} style:color={color.text}>
                {color.label}
              </span>
              <span class="cp-ranked-label">{rowLabel(inj)}</span>
              <span class="cp-ranked-tokens">~{formatTokens(inj.estimatedTokens)}</span>
            </button>
          {/each}
        </div>
      {:else}
        <!-- Grouped：按 category 分块，块内按 token 降序 -->
        {#each rankedGrouped as [cat, items] (cat)}
          {@const color = CATEGORY_COLORS[cat]}
          <div class="cp-ranked-bucket">
            <div class="cp-ranked-bucket-header" style:color={color.text}>
              <span class="cp-cat-tag" style:background={color.bg} style:color={color.text}>
                {color.label}
              </span>
              <span class="cp-bucket-count">{items.length}</span>
            </div>
            {#each items as inj (inj.id)}
              <button type="button" class="cp-ranked-item cp-ranked-item-grouped" onclick={() => rowNavigate(inj)}>
                <span class="cp-ranked-label">{rowLabel(inj)}</span>
                <span class="cp-ranked-tokens">~{formatTokens(inj.estimatedTokens)}</span>
              </button>
            {/each}
          </div>
        {/each}
      {/if}
    {/if}
  </div>
</aside>

<style>
  .context-panel {
    position: absolute;
    top: 0;
    right: 0;
    bottom: 0;
    z-index: 30;
    width: min(320px, 100%);
    min-width: 0;
    max-width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    border-left: 1px solid var(--color-border);
    background: var(--color-surface);
    box-shadow: -8px 0 24px rgba(0, 0, 0, 0.08);
    overflow: hidden;
  }

  .cp-header {
    flex-shrink: 0;
    border-bottom: 1px solid var(--color-border);
    padding: 12px 14px 10px;
  }

  .cp-title-row,
  .cp-title-wrap,
  .cp-token-row,
  .cp-mode-row,
  .cp-mode-btn {
    display: flex;
    align-items: center;
  }

  .cp-title-row {
    justify-content: space-between;
    gap: 12px;
  }

  .cp-title-wrap {
    gap: 8px;
    min-width: 0;
  }

  .cp-title-icon {
    width: 16px;
    height: 16px;
    color: var(--color-text-secondary);
    flex-shrink: 0;
  }

  .cp-title {
    font-size: 14px;
    font-weight: 650;
    color: var(--color-text);
    letter-spacing: 0.01em;
  }

  .cp-count-badge {
    border-radius: 5px;
    background: var(--color-surface-overlay, var(--badge-neutral-bg));
    color: var(--color-text-secondary);
    font-size: 12px;
    line-height: 1;
    padding: 3px 6px;
  }

  .cp-close {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    background: transparent;
    border: none;
    color: var(--color-text-secondary);
    cursor: pointer;
    padding: 0;
    border-radius: 6px;
    transition: background 0.1s, color 0.1s;
  }

  .cp-close svg {
    width: 16px;
    height: 16px;
  }

  .cp-close:hover {
    background: var(--tool-item-hover-bg);
    color: var(--color-text);
  }

  .cp-token-row {
    gap: 4px;
    justify-content: flex-start;
    margin-top: 9px;
    padding-top: 9px;
    border-top: 1px solid var(--color-border-subtle, var(--color-border));
    font-size: 12px;
  }

  .cp-token-muted {
    color: var(--color-text-muted);
  }

  .cp-token-value {
    color: var(--color-text-secondary);
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }

  .cp-mode-row {
    gap: 6px;
    margin-top: 9px;
    padding-top: 9px;
    border-top: 1px solid var(--color-border-subtle, var(--color-border));
  }

  .cp-mode-label {
    margin-right: 2px;
    font-size: 10px;
    color: var(--color-text-muted);
  }

  .cp-mode-btn {
    gap: 4px;
    font-size: 10px;
    font-family: inherit;
    color: var(--color-text-muted);
    background: var(--color-surface-overlay, var(--badge-neutral-bg));
    border: 1px solid transparent;
    border-radius: 5px;
    padding: 3px 8px;
    cursor: pointer;
    transition: background 0.1s, color 0.1s, border-color 0.1s;
  }

  .cp-mode-btn svg {
    width: 10px;
    height: 10px;
  }

  .cp-mode-btn:hover {
    color: var(--color-text-secondary);
    border-color: var(--color-border-subtle, var(--color-border));
  }

  .cp-mode-active {
    background: rgba(99, 102, 241, 0.18);
    background: color-mix(in oklch, var(--color-accent-indigo) 18%, transparent);
    color: var(--color-accent-indigo);
    border-color: rgba(99, 102, 241, 0.24);
    border-color: color-mix(in oklch, var(--color-accent-indigo) 24%, transparent);
  }

  .cp-body {
    flex: 1;
    overflow-y: auto;
    overflow-x: hidden;
    scrollbar-gutter: stable;
    padding: 12px 10px 18px 14px;
  }

  .cp-empty {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 40px 16px;
    font-size: 12px;
    color: var(--color-text-muted);
    text-align: center;
  }

  /* ── Ranked 视图 ── */

  .cp-ranked-toggle {
    display: flex;
    gap: 4px;
    margin-bottom: 8px;
    padding-bottom: 8px;
    border-bottom: 1px solid var(--color-border-subtle, var(--color-border));
  }

  .cp-ranked-btn {
    font-size: 10px;
    font-family: inherit;
    color: var(--color-text-muted);
    background: var(--color-surface-overlay, var(--badge-neutral-bg));
    border: 1px solid transparent;
    border-radius: 4px;
    padding: 3px 8px;
    cursor: pointer;
    transition: background 0.1s, color 0.1s;
  }

  .cp-ranked-btn:hover {
    color: var(--color-text-secondary);
  }

  .cp-ranked-active {
    background: color-mix(in oklch, var(--color-accent-indigo) 18%, transparent);
    color: var(--color-accent-indigo);
  }

  .cp-ranked-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .cp-ranked-bucket {
    margin-bottom: 10px;
  }

  .cp-ranked-bucket-header {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 4px;
    font-size: 11px;
    font-weight: 600;
  }

  .cp-bucket-count {
    color: var(--color-text-muted);
    font-size: 10px;
    font-variant-numeric: tabular-nums;
  }

  .cp-ranked-item {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 7px 9px;
    border: 1px solid var(--color-border-subtle, var(--color-border));
    border-radius: 7px;
    background: var(--color-surface-raised);
    color: inherit;
    font-family: inherit;
    text-align: left;
    cursor: pointer;
    transition: background 0.1s, border-color 0.1s;
  }

  .cp-ranked-item-grouped {
    grid-template-columns: minmax(0, 1fr) auto;
    border: none;
    border-radius: 5px;
    padding: 5px 8px;
    background: transparent;
  }

  .cp-ranked-item:hover {
    background: var(--tool-item-hover-bg);
    border-color: var(--color-border-emphasis);
  }

  .cp-ranked-item-grouped:hover {
    background: var(--tool-item-hover-bg);
    border-color: transparent;
  }

  .cp-cat-tag {
    font-size: 9px;
    font-weight: 600;
    padding: 2px 6px;
    border-radius: 4px;
    flex-shrink: 0;
    letter-spacing: 0.2px;
  }

  .cp-ranked-label {
    font-size: 12px;
    color: var(--color-text);
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    line-height: 1.3;
  }

  .cp-ranked-tokens {
    font-size: 11px;
    color: var(--color-text-muted);
    font-family: var(--font-mono);
    flex-shrink: 0;
  }
</style>

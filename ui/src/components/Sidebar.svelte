<script lang="ts">
  import { onMount } from "svelte";
  import { listProjects, listSessions, type ProjectInfo, type SessionSummary, type PaginatedResponse } from "../lib/api";
  import SidebarHeader from "./SidebarHeader.svelte";
  import SessionContextMenu from "./SessionContextMenu.svelte";
  import { openTab } from "../lib/tabStore.svelte";
  import {
    getSidebarWidth, setSidebarWidth,
    isPinned, togglePin,
    isHidden, toggleHide,
    getShowHidden, toggleShowHidden,
    getHiddenCount,
    loadProjectPrefs,
  } from "../lib/sidebarStore.svelte";

  interface Props {
    selectedProjectId: string;
    activeSessionId: string;
    onSelectProject: (id: string, name: string) => void;
    onSelectSession: (sessionId: string, label: string) => void;
  }

  let { selectedProjectId, activeSessionId, onSelectProject, onSelectSession }: Props = $props();

  let projects: ProjectInfo[] = $state([]);
  let sessions: SessionSummary[] = $state([]);
  let projectsLoading = $state(true);
  let sessionsLoading = $state(false);
  let filterQuery = $state("");

  // ---------------------------------------------------------------------------
  // Resize
  // ---------------------------------------------------------------------------

  let isResizing = $state(false);

  function startResize(e: MouseEvent) {
    e.preventDefault();
    isResizing = true;
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";

    function onMove(ev: MouseEvent) {
      setSidebarWidth(ev.clientX);
    }
    function onUp() {
      isResizing = false;
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
      document.removeEventListener("mousemove", onMove);
      document.removeEventListener("mouseup", onUp);
    }
    document.addEventListener("mousemove", onMove);
    document.addEventListener("mouseup", onUp);
  }

  // ---------------------------------------------------------------------------
  // Context menu
  // ---------------------------------------------------------------------------

  let ctxMenu: { x: number; y: number; session: SessionSummary } | null = $state(null);

  function onContextMenu(e: MouseEvent, session: SessionSummary) {
    e.preventDefault();
    ctxMenu = { x: e.clientX, y: e.clientY, session };
  }

  // ---------------------------------------------------------------------------
  // Data loading
  // ---------------------------------------------------------------------------

  onMount(async () => {
    try {
      projects = await listProjects();
      if (projects.length > 0 && !selectedProjectId) {
        onSelectProject(projects[0].id, projects[0].displayName);
      }
    } catch (e) {
      console.error("Failed to load projects:", e);
    } finally {
      projectsLoading = false;
    }
  });

  async function loadSessions(projectId: string) {
    if (!projectId) { sessions = []; return; }
    sessionsLoading = true;
    try {
      const result: PaginatedResponse<SessionSummary> = await listSessions(projectId);
      sessions = result.items;
    } catch (e) {
      console.error("Failed to load sessions:", e);
      sessions = [];
    } finally {
      sessionsLoading = false;
    }
  }

  $effect(() => {
    if (selectedProjectId) {
      loadSessions(selectedProjectId);
      // 首次访问此 project 时从后端拉取 pin/hide 持久化状态（幂等）
      void loadProjectPrefs(selectedProjectId);
    }
  });

  // ---------------------------------------------------------------------------
  // Date grouping
  // ---------------------------------------------------------------------------

  interface DateGroup {
    label: string;
    sessions: SessionSummary[];
  }

  function groupByDate(items: SessionSummary[]): DateGroup[] {
    const now = new Date();
    const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
    const yesterday = new Date(today.getTime() - 86400000);
    const weekAgo = new Date(today.getTime() - 7 * 86400000);

    const groups: DateGroup[] = [
      { label: "TODAY", sessions: [] },
      { label: "YESTERDAY", sessions: [] },
      { label: "PREVIOUS 7 DAYS", sessions: [] },
      { label: "OLDER", sessions: [] },
    ];

    for (const s of items) {
      const d = new Date(s.timestamp);
      if (d >= today) groups[0].sessions.push(s);
      else if (d >= yesterday) groups[1].sessions.push(s);
      else if (d >= weekAgo) groups[2].sessions.push(s);
      else groups[3].sessions.push(s);
    }

    return groups.filter(g => g.sessions.length > 0);
  }

  function formatTime(timestamp: number): string {
    if (timestamp === 0) return "";
    const date = new Date(timestamp);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffMins = Math.floor(diffMs / 60000);
    if (diffMins < 1) return "刚刚";
    if (diffMins < 60) return `${diffMins}m`;
    const diffHours = Math.floor(diffMins / 60);
    if (diffHours < 24) return `${diffHours}h`;
    const diffDays = Math.floor(diffHours / 24);
    if (diffDays < 7) return `${diffDays}d`;
    return date.toLocaleDateString("zh-CN", { month: "short", day: "numeric" });
  }

  function sessionLabel(s: SessionSummary): string {
    return s.title || s.sessionId.slice(0, 8);
  }

  // ---------------------------------------------------------------------------
  // Derived: filter → hide → pin split → group
  // ---------------------------------------------------------------------------

  const filteredSessions = $derived(
    filterQuery
      ? sessions.filter(s => (s.title || s.sessionId).toLowerCase().includes(filterQuery.toLowerCase()))
      : sessions
  );

  const visibleSessions = $derived.by(() => {
    if (getShowHidden()) return filteredSessions;
    return filteredSessions.filter(s => !isHidden(selectedProjectId, s.sessionId));
  });

  const pinnedSessions = $derived(
    visibleSessions.filter(s => isPinned(selectedProjectId, s.sessionId))
  );

  const unpinnedSessions = $derived(
    visibleSessions.filter(s => !isPinned(selectedProjectId, s.sessionId))
  );

  const dateGroups = $derived(groupByDate(unpinnedSessions));
  const totalSessions = $derived(sessions.length);
  const hiddenCount = $derived(getHiddenCount(selectedProjectId));
  const sidebarWidth = $derived(getSidebarWidth());
</script>

<aside class="sidebar" style:width="{sidebarWidth}px" style:min-width="{sidebarWidth}px">
  <SidebarHeader
    {projects}
    {selectedProjectId}
    {onSelectProject}
  />

  <!-- Session filter + count -->
  {#if !sessionsLoading && selectedProjectId}
    <div class="session-filter-bar">
      <input
        class="session-filter-input"
        type="text"
        placeholder="搜索会话…"
        bind:value={filterQuery}
      />
      <span class="session-count-num">{visibleSessions.length}/{totalSessions}</span>
      {#if hiddenCount > 0}
        <button
          class="show-hidden-btn"
          class:show-hidden-active={getShowHidden()}
          title={getShowHidden() ? "隐藏已隐藏会话" : `显示 ${hiddenCount} 个隐藏会话`}
          onclick={toggleShowHidden}
        >
          {#if getShowHidden()}
            <!-- eye open -->
            <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8z"/>
              <circle cx="12" cy="12" r="3"/>
            </svg>
          {:else}
            <!-- eye off -->
            <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94"/>
              <path d="M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19"/>
              <path d="M1 1l22 22"/>
            </svg>
          {/if}
        </button>
      {/if}
    </div>
  {/if}

  <div class="session-list">
    {#if projectsLoading || sessionsLoading}
      <div class="sidebar-status">加载中...</div>
    {:else if sessions.length === 0}
      <div class="sidebar-status">暂无会话</div>
    {:else if visibleSessions.length === 0}
      <div class="sidebar-status">无匹配会话</div>
    {:else}
      <!-- Pinned section -->
      {#if pinnedSessions.length > 0}
        <div class="date-group">
          <div class="date-group-label">PINNED</div>
          {#each pinnedSessions as session (session.sessionId)}
            <button
              class="session-item"
              class:session-item-active={session.sessionId === activeSessionId}
              class:session-item-hidden={isHidden(selectedProjectId, session.sessionId)}
              onclick={() => onSelectSession(session.sessionId, sessionLabel(session))}
              oncontextmenu={(e) => onContextMenu(e, session)}
            >
              <div class="session-title">
                <svg class="pin-icon" viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <path d="M12 17v5"/>
                  <path d="M9 10.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24V16a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2v-.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V7a1 1 0 0 1 1-1 2 2 0 0 0 0-4H8a2 2 0 0 0 0 4 1 1 0 0 1 1 1z"/>
                </svg>
                <span class="session-title-text">
                  {session.title || session.sessionId.slice(0, 8) + "…"}
                </span>
              </div>
              <div class="session-meta">
                <span class="session-msg-count">C{session.messageCount || ""}</span>
                <span class="session-time">{formatTime(session.timestamp)}</span>
              </div>
            </button>
          {/each}
        </div>
      {/if}

      <!-- Date-grouped unpinned sections -->
      {#each dateGroups as group}
        <div class="date-group">
          <div class="date-group-label">{group.label}</div>
          {#each group.sessions as session (session.sessionId)}
            <button
              class="session-item"
              class:session-item-active={session.sessionId === activeSessionId}
              class:session-item-hidden={isHidden(selectedProjectId, session.sessionId)}
              onclick={() => onSelectSession(session.sessionId, sessionLabel(session))}
              oncontextmenu={(e) => onContextMenu(e, session)}
            >
              <div class="session-title">
                <span class="session-title-text">
                  {session.title || session.sessionId.slice(0, 8) + "…"}
                </span>
              </div>
              <div class="session-meta">
                <span class="session-msg-count">C{session.messageCount || ""}</span>
                <span class="session-time">{formatTime(session.timestamp)}</span>
              </div>
            </button>
          {/each}
        </div>
      {/each}
    {/if}
  </div>

  <!-- Resize handle -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="resize-handle"
    class:resize-handle-active={isResizing}
    onmousedown={startResize}
  ></div>
</aside>

<!-- Context menu (rendered outside sidebar to avoid overflow clipping) -->
{#if ctxMenu}
  {@const ctx = ctxMenu}
  <SessionContextMenu
    x={ctx.x}
    y={ctx.y}
    sessionId={ctx.session.sessionId}
    isPinned={isPinned(selectedProjectId, ctx.session.sessionId)}
    isHidden={isHidden(selectedProjectId, ctx.session.sessionId)}
    onOpenInNewTab={() => openTab(ctx.session.sessionId, selectedProjectId, sessionLabel(ctx.session))}
    onTogglePin={() => togglePin(selectedProjectId, ctx.session.sessionId)}
    onToggleHide={() => toggleHide(selectedProjectId, ctx.session.sessionId)}
    onClose={() => { ctxMenu = null; }}
  />
{/if}

<style>
  .sidebar {
    position: relative;
    height: 100vh;
    display: flex;
    flex-direction: column;
    background: var(--color-surface-sidebar);
    border-right: 1px solid var(--color-border);
    overflow: hidden;
  }

  .session-filter-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--color-border);
  }

  .session-filter-input {
    flex: 1;
    min-width: 0;
    font-size: 12px;
    font-family: inherit;
    color: var(--color-text);
    background: var(--color-surface);
    border: 1px solid var(--color-border);
    border-radius: 6px;
    padding: 4px 8px;
    outline: none;
    transition: border-color 0.15s;
  }

  .session-filter-input:focus {
    border-color: var(--color-border-emphasis);
  }

  .session-filter-input::placeholder {
    color: var(--color-text-muted);
  }

  .session-count-num {
    font-size: 11px;
    color: var(--color-text-muted);
    flex-shrink: 0;
    font-family: var(--font-mono);
  }

  .show-hidden-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 2px;
    background: none;
    border: none;
    border-radius: 4px;
    color: var(--color-text-muted);
    cursor: pointer;
    flex-shrink: 0;
    transition: background 0.1s, color 0.1s;
  }

  .show-hidden-btn:hover {
    background: var(--tool-item-hover-bg);
  }

  .show-hidden-active {
    color: #3b82f6;
  }

  .session-list {
    flex: 1;
    overflow-y: auto;
    padding: 4px 8px;
  }

  .sidebar-status {
    padding: 24px 12px;
    text-align: center;
    color: var(--color-text-muted);
    font-size: 13px;
  }

  .date-group {
    margin-bottom: 4px;
  }

  .date-group-label {
    font-size: 11px;
    font-weight: 600;
    color: var(--color-text-muted);
    padding: 10px 8px 4px;
    letter-spacing: 0.3px;
  }

  .session-item {
    display: flex;
    flex-direction: column;
    gap: 2px;
    width: 100%;
    padding: 8px 10px;
    background: none;
    border: none;
    border-radius: 6px;
    cursor: pointer;
    font: inherit;
    color: var(--color-text);
    text-align: left;
    transition: background 0.1s, opacity 0.15s;
  }

  .session-item:hover {
    background: var(--tool-item-hover-bg);
  }

  .session-item-active {
    background: var(--color-surface-raised);
  }

  .session-item-hidden {
    opacity: 0.5;
  }

  .session-title {
    display: flex;
    align-items: center;
    gap: 4px;
    overflow: hidden;
  }

  .session-title-text {
    font-size: 13px;
    font-weight: 400;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--color-text);
    line-height: 1.4;
  }

  .pin-icon {
    flex-shrink: 0;
    color: #3b82f6;
  }

  .session-meta {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .session-msg-count {
    font-size: 11px;
    color: var(--color-text-muted);
    font-family: var(--font-mono);
  }

  .session-time {
    font-size: 11px;
    color: var(--color-text-muted);
  }

  /* Resize handle */
  .resize-handle {
    position: absolute;
    right: -2px;
    top: 0;
    width: 5px;
    height: 100%;
    cursor: col-resize;
    background: transparent;
    transition: background 0.15s;
    z-index: 5;
  }

  .resize-handle:hover,
  .resize-handle-active {
    background: rgba(59, 130, 246, 0.5);
  }
</style>

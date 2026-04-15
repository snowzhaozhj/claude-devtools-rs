<script lang="ts">
  import { onMount } from "svelte";
  import { listProjects, listSessions, type ProjectInfo, type SessionSummary, type PaginatedResponse } from "../lib/api";
  import SidebarHeader from "./SidebarHeader.svelte";

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
    }
  });

  interface DateGroup {
    label: string;
    sessions: SessionSummary[];
  }

  function groupByDate(items: SessionSummary[]): DateGroup[] {
    const now = new Date();
    const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
    const yesterday = new Date(today.getTime() - 86400000);
    const weekAgo = new Date(today.getTime() - 7 * 86400000);

    const groups: { label: string; sessions: SessionSummary[] }[] = [
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

  const filteredSessions = $derived(
    filterQuery
      ? sessions.filter(s => (s.title || s.sessionId).toLowerCase().includes(filterQuery.toLowerCase()))
      : sessions
  );
  const dateGroups = $derived(groupByDate(filteredSessions));
  const totalSessions = $derived(sessions.length);
</script>

<aside class="sidebar">
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
      <span class="session-count-num">{filteredSessions.length}/{totalSessions}</span>
    </div>
  {/if}

  <div class="session-list">
    {#if projectsLoading || sessionsLoading}
      <div class="sidebar-status">加载中...</div>
    {:else if sessions.length === 0}
      <div class="sidebar-status">暂无会话</div>
    {:else if filteredSessions.length === 0}
      <div class="sidebar-status">无匹配会话</div>
    {:else}
      {#each dateGroups as group}
        <div class="date-group">
          <div class="date-group-label">{group.label}</div>
          {#each group.sessions as session}
            <button
              class="session-item"
              class:session-item-active={session.sessionId === activeSessionId}
              onclick={() => onSelectSession(session.sessionId, session.title || session.sessionId.slice(0, 8))}
            >
              <div class="session-title">
                {session.title || session.sessionId.slice(0, 8) + "…"}
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
</aside>

<style>
  .sidebar {
    width: 280px;
    min-width: 280px;
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
    transition: background 0.1s;
  }

  .session-item:hover {
    background: var(--tool-item-hover-bg);
  }

  .session-item-active {
    background: var(--color-surface-raised);
  }

  .session-title {
    font-size: 13px;
    font-weight: 400;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--color-text);
    line-height: 1.4;
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
</style>

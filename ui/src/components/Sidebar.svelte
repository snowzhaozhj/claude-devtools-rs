<script lang="ts">
  import { onMount } from "svelte";
  import { listProjects, listSessions, getSessionDetail, type ProjectInfo, type SessionSummary, type PaginatedResponse, type Chunk } from "../lib/api";
  import { cleanDisplayText } from "../lib/toolHelpers";
  import SidebarHeader from "./SidebarHeader.svelte";

  interface Props {
    selectedProjectId: string;
    selectedSessionId: string;
    onSelectProject: (id: string, name: string) => void;
    onSelectSession: (sessionId: string) => void;
  }

  let { selectedProjectId, selectedSessionId, onSelectProject, onSelectSession }: Props = $props();

  let projects: ProjectInfo[] = $state([]);
  let sessions: SessionSummary[] = $state([]);
  let projectsLoading = $state(true);
  let sessionsLoading = $state(false);

  /** 缓存 session 的第一条 user message 作为标题 */
  let sessionTitles: Map<string, string> = $state(new Map());

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
      // 异步加载每个 session 的第一条消息作为标题
      for (const s of result.items) {
        if (!sessionTitles.has(s.sessionId)) {
          loadSessionTitle(projectId, s.sessionId);
        }
      }
    } catch (e) {
      console.error("Failed to load sessions:", e);
      sessions = [];
    } finally {
      sessionsLoading = false;
    }
  }

  async function loadSessionTitle(projectId: string, sessionId: string) {
    try {
      const detail = await getSessionDetail(projectId, sessionId);
      const title = extractFirstUserMessage(detail.chunks);
      if (title) {
        const updated = new Map(sessionTitles);
        updated.set(sessionId, title);
        sessionTitles = updated;
      }
    } catch { /* ignore */ }
  }

  function extractFirstUserMessage(chunks: Chunk[]): string {
    // 优先找非命令的用户消息
    for (const c of chunks) {
      if (c.kind === "user") {
        const raw = typeof c.content === "string" ? c.content : extractTextContent(c.content);
        const text = cleanDisplayText(raw);
        if (text && !text.startsWith("/")) return text.slice(0, 80);
      }
    }
    // fallback: 任何 user 消息
    for (const c of chunks) {
      if (c.kind === "user") {
        const raw = typeof c.content === "string" ? c.content : extractTextContent(c.content);
        const text = cleanDisplayText(raw);
        if (text) return text.slice(0, 80);
      }
    }
    return "";
  }

  function extractTextContent(content: unknown[]): string {
    for (const b of content) {
      if (b && typeof b === "object" && "type" in b) {
        const x = b as Record<string, unknown>;
        if (x.type === "text" && typeof x.text === "string") return x.text;
      }
    }
    return "";
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

  const dateGroups = $derived(groupByDate(sessions));
  const totalSessions = $derived(sessions.length);
</script>

<aside class="sidebar">
  <SidebarHeader
    {projects}
    {selectedProjectId}
    {onSelectProject}
  />

  <!-- Session count -->
  {#if !sessionsLoading && selectedProjectId}
    <div class="session-count-bar">
      <span class="session-count-label">SESSIONS</span>
      <span class="session-count-num">{totalSessions}</span>
    </div>
  {/if}

  <div class="session-list">
    {#if projectsLoading || sessionsLoading}
      <div class="sidebar-status">加载中...</div>
    {:else if sessions.length === 0}
      <div class="sidebar-status">暂无会话</div>
    {:else}
      {#each dateGroups as group}
        <div class="date-group">
          <div class="date-group-label">{group.label}</div>
          {#each group.sessions as session}
            <button
              class="session-item"
              class:session-item-active={session.sessionId === selectedSessionId}
              onclick={() => onSelectSession(session.sessionId)}
            >
              <div class="session-title">
                {sessionTitles.get(session.sessionId) || session.sessionId.slice(0, 8) + "…"}
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

  .session-count-bar {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 16px;
    border-bottom: 1px solid var(--color-border);
  }

  .session-count-label {
    font-size: 11px;
    font-weight: 600;
    color: var(--color-text-muted);
    letter-spacing: 0.5px;
  }

  .session-count-num {
    font-size: 11px;
    color: var(--color-text-muted);
    background: var(--badge-neutral-bg);
    padding: 0 6px;
    border-radius: 10px;
    font-weight: 500;
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

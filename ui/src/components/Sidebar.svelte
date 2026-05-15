<script lang="ts">
  import { onMount, onDestroy, untrack } from "svelte";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import {
    listProjects,
    listSessions,
    getSessionSummariesByIds,
    listRepositoryGroups,
    getProjectMemory,
    type ProjectInfo,
    type RepositoryGroup,
    type ProjectMemory,
    type SessionSummary,
    type SessionMetadataUpdate,
    type PaginatedResponse,
  } from "../lib/api";
  import SidebarHeader from "./SidebarHeader.svelte";
  import SessionContextMenu from "./SessionContextMenu.svelte";
  import OngoingIndicator from "./OngoingIndicator.svelte";
  import { openTab, openOrReplaceTab, openTabInNewPane, getPaneLayout, openMemoryTab } from "../lib/tabStore.svelte";
  import { MAX_PANES } from "../lib/paneTypes";
  import {
    getSidebarWidth, setSidebarWidth,
    isPinned, togglePin,
    isHidden, toggleHide,
    getShowHidden, toggleShowHidden,
    getPinnedIds,
    getHiddenIds,
    getHiddenCount,
    loadProjectPrefs,
    toggleSidebarCollapsed,
  } from "../lib/sidebarStore.svelte";
  import { registerHandler, unregisterHandler, scheduleRefresh, cancelScheduledRefresh } from "../lib/fileChangeStore.svelte";
  import { createVirtualWindow } from "../lib/virtualList.svelte";
  import { applySilentRefresh, mergeSessions } from "../lib/sessionMerge";
  import { MESSAGE_SQUARE, GIT_BRANCH_SVG, BOOK_OPEN_TEXT_SVG } from "../lib/icons";

  // 虚拟滚动行高（实测 .session-item ≈ 44px：padding 8+8 + title 13×1.4 +
  // meta 11×1.4）；header 行高强制对齐 44 让单一 windowing 单元生效。
  const ITEM_HEIGHT = 44;
  const SESSION_PAGE_SIZE = 20;
  const HISTORY_SCROLL_THRESHOLD = ITEM_HEIGHT * 2;

  interface Props {
    selectedProjectId: string;
    activeSessionId: string | null;
    collapsed?: boolean;
    onSelectProject: (id: string, name: string) => void;
    onSelectSession: (sessionId: string, label: string, event: MouseEvent) => void;
  }

  let {
    selectedProjectId,
    activeSessionId,
    collapsed = false,
    onSelectProject,
    onSelectSession,
  }: Props = $props();

  let projects: ProjectInfo[] = $state([]);
  let repositoryGroups: RepositoryGroup[] = $state([]);
  let sessions: SessionSummary[] = $state([]);
  let projectMemory: ProjectMemory | null = $state(null);
  let projectsLoading = $state(true);
  let sessionsLoading = $state(false);
  let sessionsLoadingMore = $state(false);
  let sessionsNextCursor: string | null = $state(null);
  let browsingHistory = $state(false);
  let hasDeferredSessionRefresh = $state(false);
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

  let metadataUnlisten: UnlistenFn | null = null;
  let sessionListEl: HTMLElement | null = null;

  async function loadProjects(silent = false) {
    if (!silent) projectsLoading = true;
    try {
      // 优先 listRepositoryGroups（grouped 视图）；任何失败 fallback 到
      // listProjects 平铺，保持 sidebar 在 worktree-grouper 出错或老后端
      // 上仍能工作。
      let nextProjects: ProjectInfo[];
      try {
        const groups = await listRepositoryGroups();
        repositoryGroups = groups;
        // 派生扁平 projects 列表，每个 worktree 都暴露为一个 ProjectInfo，
        // 兼容下游 selectedProjectId / loadSessions 既有路径（D7b）。
        nextProjects = groups.flatMap((g) =>
          g.worktrees.map((w) => ({
            id: w.id,
            path: w.path,
            displayName: w.name,
            sessionCount: w.sessions.length,
          })),
        );
      } catch (gErr) {
        console.warn("listRepositoryGroups failed, fallback to listProjects:", gErr);
        repositoryGroups = [];
        nextProjects = await listProjects();
      }
      projects = nextProjects;
      const selectedExists = nextProjects.some((p) => p.id === selectedProjectId);
      if (nextProjects.length > 0 && (!selectedProjectId || !selectedExists)) {
        // 默认选中"最近活动 group 的 main worktree"（spec sidebar-navigation
        // §"活跃 worktree 选中状态"）：repositoryGroups 已按 mostRecentSession
        // 倒序，worktrees 已 main 优先排序——直接取第一个 group 的 [0]。
        const first = repositoryGroups[0]?.worktrees?.[0];
        if (first) {
          onSelectProject(first.id, first.name);
        } else {
          onSelectProject(nextProjects[0].id, nextProjects[0].displayName);
        }
      }
    } catch (e) {
      console.error("Failed to load projects:", e);
    } finally {
      if (!silent) projectsLoading = false;
    }
  }

  onMount(async () => {
    // 先注册 listener，再触发可能 emit 的 loadProjects 链路。否则
    // `loadProjects → onSelectProject → 父组件 set selectedProjectId →
    // $effect loadSessions → 后端 list_sessions spawn 扫描 → emit
    // session-metadata-update` 会跑在 listener 注册之前，tauri emit
    // 在无订阅者时 fire-and-forget 直接丢失，列表项卡在 title=null
    // 永久 fallback 到 sessionId 前 8 字符（不稳定复现根因）。
    //
    // 订阅后端元数据增量 patch；按 sessionId 定位 in-place 替换三个元数据字段，
    // 不改变 sessions 数组顺序与稳定 key，复用 DOM 节点不触发动画重启
    // （spec sidebar-navigation §"会话元数据增量 patch"）
    metadataUnlisten = await listen<SessionMetadataUpdate>(
      "session-metadata-update",
      (event) => {
        const payload = event.payload;
        // 切 project 期间残留的旧 project 事件忽略
        if (payload.projectId !== selectedProjectId) return;
        sessions = sessions.map((s) =>
          s.sessionId === payload.sessionId
            ? {
                ...s,
                title: payload.title,
                messageCount: payload.messageCount,
                isOngoing: payload.isOngoing,
                gitBranch: payload.gitBranch,
              }
            : s,
        );
      },
    );

    try {
      await loadProjects();
    } finally {
      projectsLoading = false;
    }
  });

  async function reconcilePinnedAndHidden(projectId: string, current: SessionSummary[]) {
    const neededIds = [...new Set([...getPinnedIds(projectId), ...getHiddenIds(projectId)])]
      .filter((id) => !current.some((s) => s.sessionId === id));
    if (neededIds.length === 0) return current;
    const summaries = await getSessionSummariesByIds(projectId, neededIds);
    return mergeSessions(current, summaries);
  }

  async function loadProjectMemory(projectId: string) {
    if (!projectId) {
      projectMemory = null;
      return;
    }
    try {
      const memory = await getProjectMemory(projectId);
      if (projectId === selectedProjectId) projectMemory = memory;
    } catch (e) {
      console.warn("Failed to load project memory:", e);
      if (projectId === selectedProjectId) projectMemory = null;
    }
  }

  async function loadSessions(projectId: string, silent = false) {
    if (!projectId) {
      sessions = [];
      sessionsNextCursor = null;
      return;
    }
    if (!silent) sessionsLoading = true;
    try {
      await loadProjectPrefs(projectId);
      const result: PaginatedResponse<SessionSummary> = await listSessions(projectId, SESSION_PAGE_SIZE);
      if (projectId !== selectedProjectId) return;
      // silent 路径：合并到现有列表保留尾部 + 保留分页 cursor（避免 sessions 缩水
      // 与计数跳变，spec sidebar-navigation §"会话元数据增量 patch"）。非 silent：
      // 替换式加载第一页 + 取本次 cursor。
      let fresh: SessionSummary[];
      let nextCursor: string | null;
      if (silent) {
        const merged = applySilentRefresh(sessions, sessionsNextCursor, result.items);
        fresh = merged.sessions;
        nextCursor = merged.nextCursor;
      } else {
        fresh = result.items;
        nextCursor = result.nextCursor;
      }
      fresh = await reconcilePinnedAndHidden(projectId, fresh);
      if (projectId !== selectedProjectId) return;
      sessions = fresh;
      sessionsNextCursor = nextCursor;
      hasDeferredSessionRefresh = false;
      queueMicrotask(() => maybeLoadMoreSessions(true));
    } catch (e) {
      console.error("Failed to load sessions:", e);
      if (!silent && projectId === selectedProjectId) {
        sessions = [];
        sessionsNextCursor = null;
      }
    } finally {
      if (!silent && projectId === selectedProjectId) sessionsLoading = false;
    }
  }

  async function loadMoreSessions() {
    const projectId = selectedProjectId;
    const cursor = sessionsNextCursor;
    if (!projectId || !cursor || sessionsLoading || sessionsLoadingMore) return;
    sessionsLoadingMore = true;
    try {
      const result = await listSessions(projectId, SESSION_PAGE_SIZE, cursor);
      if (projectId !== selectedProjectId || cursor !== sessionsNextCursor) return;
      sessions = mergeSessions(sessions, result.items, false);
      sessionsNextCursor = result.nextCursor;
    } catch (e) {
      console.error("Failed to load more sessions:", e);
    } finally {
      if (projectId === selectedProjectId) sessionsLoadingMore = false;
    }
  }

  function maybeLoadMoreSessions(allowAutoFill = false) {
    const el = sessionListEl;
    if (!el || !sessionsNextCursor || sessionsLoading || sessionsLoadingMore) return;
    const remaining = el.scrollHeight - el.scrollTop - el.clientHeight;
    const threshold = ITEM_HEIGHT * 8;
    if (remaining < threshold && (allowAutoFill || browsingHistory)) void loadMoreSessions();
  }

  function refreshDeferredSessions() {
    if (!selectedProjectId || !hasDeferredSessionRefresh) return;
    hasDeferredSessionRefresh = false;
    void loadSessions(selectedProjectId, true);
  }

  function onSessionListScroll(e: Event) {
    vlist.onScroll(e);
    const el = e.currentTarget as HTMLElement | null;
    browsingHistory = !!el && el.scrollTop > HISTORY_SCROLL_THRESHOLD;
    if (!browsingHistory) refreshDeferredSessions();
    maybeLoadMoreSessions();
  }

  $effect(() => {
    if (selectedProjectId) {
      loadSessions(selectedProjectId);
      void loadProjectMemory(selectedProjectId);
      // 首次访问此 project 时从后端拉取 pin/hide 持久化状态（幂等）
      void loadProjectPrefs(selectedProjectId);
    }
  });

  // 注册 file-change handler；依赖 selectedProjectId，切 project 时
  // 重新注册让闭包捕获最新值
  $effect(() => {
    const currentProjectId = selectedProjectId;
    registerHandler("sidebar", (payload) => {
      if (payload.projectListChanged) {
        scheduleRefresh("sidebar:projects", () =>
          untrack(() => loadProjects(true)),
        );
      }
      if (!currentProjectId || payload.projectId !== currentProjectId || !payload.sessionId) return;
      if (browsingHistory) {
        hasDeferredSessionRefresh = true;
        return;
      }
      scheduleRefresh(`sidebar:${currentProjectId}`, () =>
        untrack(() => loadSessions(currentProjectId, true)),
      );
    });
    return () => {
      unregisterHandler("sidebar");
      if (currentProjectId) cancelScheduledRefresh(`sidebar:${currentProjectId}`);
      cancelScheduledRefresh("sidebar:projects");
    };
  });

  onDestroy(() => {
    unregisterHandler("sidebar");
    metadataUnlisten?.();
    metadataUnlisten = null;
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
  const memoryCount = $derived.by(() => projectMemory ? projectMemory.count : 0);
  const sidebarWidth = $derived(getSidebarWidth());

  // ---------------------------------------------------------------------------
  // Flat virtual list：把 PINNED 与日期分组摊平为单一 windowing 容器
  // ---------------------------------------------------------------------------

  type FlatItem =
    | { kind: "header"; key: string; label: string }
    | { kind: "session"; key: string; session: SessionSummary; pinned: boolean };

  const flatItems = $derived.by<FlatItem[]>(() => {
    const items: FlatItem[] = [];
    if (pinnedSessions.length > 0) {
      items.push({ kind: "header", key: "h:PINNED", label: "PINNED" });
      for (const s of pinnedSessions) {
        items.push({ kind: "session", key: s.sessionId, session: s, pinned: true });
      }
    }
    for (const group of dateGroups) {
      items.push({ kind: "header", key: `h:${group.label}`, label: group.label });
      for (const s of group.sessions) {
        items.push({ kind: "session", key: s.sessionId, session: s, pinned: false });
      }
    }
    return items;
  });

  const vlist = createVirtualWindow({
    total: () => flatItems.length,
    itemHeight: ITEM_HEIGHT,
    overscan: 5,
  });

  const visibleSlice = $derived(flatItems.slice(vlist.startIndex(), vlist.endIndex()));
</script>

<aside
  class="sidebar"
  class:sidebar-collapsed={collapsed}
  style:width="{collapsed ? 0 : sidebarWidth}px"
  style:min-width="{collapsed ? 0 : sidebarWidth}px"
  aria-hidden={collapsed}
>
  <SidebarHeader
    {projects}
    {repositoryGroups}
    {selectedProjectId}
    {onSelectProject}
    onToggleCollapsed={toggleSidebarCollapsed}
  />

  {#if selectedProjectId && memoryCount > 0}
    <button
      class="memory-entry"
      onclick={() => openMemoryTab(selectedProjectId, "Memory")}
    >
      <svg class="memory-entry-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        {@html BOOK_OPEN_TEXT_SVG}
      </svg>
      <span>Memory ({memoryCount})</span>
    </button>
  {/if}

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
      {#if hasDeferredSessionRefresh}
        <button class="refresh-pending-btn" onclick={refreshDeferredSessions}>有更新</button>
      {/if}
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

  <div
    class="session-list"
    onscroll={onSessionListScroll}
    {@attach (el) => {
      sessionListEl = el;
      vlist.bindScrollEl(el);
      // height>0 guard：sidebar collapsed 时 width:0 + overflow:hidden 不会
      // 改变 session-list 的 height（仍由 flex column 撑满），但兜底防御
      // 任何 flex 计算 race 把 0 写入 vlist 导致 visibleSlice 清空——再展开
      // 时空→填充会出现一帧白屏闪烁。
      const ro = new ResizeObserver(() => {
        const h = el.clientHeight;
        if (h > 0) {
          vlist.setContainerHeight(h);
          maybeLoadMoreSessions(true);
        }
      });
      ro.observe(el);
      return () => {
        ro.disconnect();
        sessionListEl = null;
        vlist.bindScrollEl(null);
      };
    }}
  >
    {#if projectsLoading || sessionsLoading}
      <div class="sidebar-status">加载中...</div>
    {:else if sessions.length === 0}
      <div class="sidebar-status">暂无会话</div>
    {:else if visibleSessions.length === 0}
      <div class="sidebar-status">无匹配会话</div>
    {:else}
      <div class="vlist-spacer" style:height="{vlist.topSpacer()}px"></div>
      {#each visibleSlice as item (item.key)}
        {#if item.kind === "header"}
          <div class="date-group-label" style:height="{ITEM_HEIGHT}px">{item.label}</div>
        {:else}
          {@const session = item.session}
          <button
            class="session-item"
            class:session-item-active={session.sessionId === activeSessionId}
            class:session-item-hidden={isHidden(selectedProjectId, session.sessionId)}
            style:height="{ITEM_HEIGHT}px"
            onclick={(e) => onSelectSession(session.sessionId, sessionLabel(session), e)}
            oncontextmenu={(e) => onContextMenu(e, session)}
          >
            <div class="session-title">
              {#if session.isOngoing}
                <OngoingIndicator />
              {/if}
              {#if item.pinned}
                <svg class="pin-icon" viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <path d="M12 17v5"/>
                  <path d="M9 10.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24V16a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2v-.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V7a1 1 0 0 1 1-1 2 2 0 0 0 0-4H8a2 2 0 0 0 0 4 1 1 0 0 1 1 1z"/>
                </svg>
              {/if}
              <span class="session-title-text">
                {session.title || session.sessionId.slice(0, 8) + "…"}
              </span>
            </div>
            <div class="session-meta">
              <span class="session-msg-count">
                <svg class="meta-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d={MESSAGE_SQUARE} /></svg>
                {session.messageCount || 0}
              </span>
              <span class="session-meta-sep">·</span>
              <span class="session-time">{formatTime(session.timestamp)}</span>
              {#if session.gitBranch}
                <span class="session-meta-sep">·</span>
                <span class="session-branch" title={session.gitBranch}>
                  <svg class="meta-icon session-branch-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{@html GIT_BRANCH_SVG}</svg>
                  <span class="session-branch-name">{session.gitBranch}</span>
                </span>
              {/if}
            </div>
          </button>
        {/if}
      {/each}
      <div class="vlist-spacer" style:height="{vlist.bottomSpacer()}px"></div>
      {#if sessionsLoadingMore}
        <div class="sidebar-status sidebar-status-inline">加载更多...</div>
      {/if}
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
  {@const canSplit = getPaneLayout().panes.length < MAX_PANES}
  <SessionContextMenu
    x={ctx.x}
    y={ctx.y}
    sessionId={ctx.session.sessionId}
    isPinned={isPinned(selectedProjectId, ctx.session.sessionId)}
    isHidden={isHidden(selectedProjectId, ctx.session.sessionId)}
    {canSplit}
    onOpenInCurrentTab={() => openOrReplaceTab(ctx.session.sessionId, selectedProjectId, sessionLabel(ctx.session))}
    onOpenInNewTab={() => openTab(ctx.session.sessionId, selectedProjectId, sessionLabel(ctx.session))}
    onOpenInNewPane={() => openTabInNewPane(ctx.session.sessionId, selectedProjectId, sessionLabel(ctx.session))}
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

  /* collapsed 时通过宽度归零隐藏（不用 display:none）——保留组件挂载，避免
     销毁/重建造成的 ResizeObserver 重测量 + vlist 空→填充闪烁。border-right
     在 width:0 时按 box-sizing 仍占 1px 视觉宽度，需要主动抑制。 */
  .sidebar-collapsed {
    border-right: none;
    pointer-events: none;
  }

  .memory-entry {
    display: flex;
    align-items: center;
    gap: 8px;
    width: calc(100% - 16px);
    margin: 8px;
    padding: 9px 10px;
    border: none;
    border-radius: 8px;
    background: transparent;
    color: var(--color-text);
    font: inherit;
    font-size: 13px;
    font-weight: 600;
    text-align: left;
    cursor: pointer;
  }

  .memory-entry:hover {
    background: var(--tool-item-hover-bg);
  }

  .memory-entry-icon {
    width: 16px;
    height: 16px;
    flex-shrink: 0;
    color: var(--color-text-muted);
  }

  .session-filter-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    /* 不加 border-bottom：原版 sidebar 内只有 SidebarHeader 一条顶部
       border，search filter bar 跟 list 之间靠 padding 视觉分隔即可。
       加 border 会让 sidebar 内出现第二条横线，跟右侧 TabBar 唯一一
       条横线对不齐（用户视觉上的「分隔线没齐平」）。 */
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

  .sidebar-status-inline {
    padding: 8px 0;
    font-size: 11px;
  }

  .session-count-num {
    font-size: 11px;
    color: var(--color-text-muted);
    flex-shrink: 0;
    font-family: var(--font-mono);
  }

  .refresh-pending-btn {
    flex-shrink: 0;
    padding: 2px 6px;
    border: 1px solid var(--color-border);
    border-radius: 999px;
    background: var(--color-surface);
    color: var(--color-text-muted);
    font-size: 11px;
    cursor: pointer;
  }

  .refresh-pending-btn:hover {
    color: var(--color-text);
    border-color: var(--color-border-emphasis);
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

  .vlist-spacer {
    width: 100%;
    pointer-events: none;
  }

  .date-group-label {
    display: flex;
    align-items: flex-end;
    font-size: 11px;
    font-weight: 600;
    color: var(--color-text-muted);
    padding: 10px 8px 4px;
    letter-spacing: 0.3px;
    box-sizing: border-box;
  }

  .session-item {
    position: relative;
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 2px;
    width: 100%;
    padding: 0 10px;
    background: none;
    border: none;
    border-radius: 6px;
    cursor: pointer;
    font: inherit;
    color: var(--color-text);
    text-align: left;
    box-sizing: border-box;
    transition: background 0.1s, opacity 0.15s;
  }

  .session-item:hover {
    background: var(--tool-item-hover-bg);
  }

  /* 选中态：左侧 3px accent stripe + 略加重背景，让用户一眼看到当前会话 */
  .session-item-active {
    background: var(--color-surface-raised);
  }
  .session-item-active::before {
    content: "";
    position: absolute;
    left: 0;
    top: 4px;
    bottom: 4px;
    width: 3px;
    border-radius: 2px;
    background: var(--color-border-emphasis);
  }
  .session-item-active .session-title-text {
    color: var(--color-text);
    font-weight: 600;
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
    line-height: 1.2;
  }

  .session-msg-count {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    font-size: 10px;
    color: var(--color-text-muted);
    font-variant-numeric: tabular-nums;
    flex-shrink: 0;
    white-space: nowrap;
  }

  .meta-icon {
    width: 10px;
    height: 10px;
    flex-shrink: 0;
  }

  .session-meta-sep {
    font-size: 10px;
    color: var(--color-text-muted);
    opacity: 0.5;
  }

  .session-time {
    font-size: 10px;
    color: var(--color-text-muted);
    font-variant-numeric: tabular-nums;
    flex-shrink: 0;
    white-space: nowrap;
  }

  .session-branch {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    font-size: 10px;
    color: var(--color-text-muted);
    font-family: var(--font-mono);
    min-width: 0;
    flex-shrink: 1;
    overflow: hidden;
  }

  .session-branch-icon {
    color: rgba(52, 211, 153, 0.7);
  }

  .session-branch-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
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

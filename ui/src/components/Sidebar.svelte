<script lang="ts">
  import { onMount, onDestroy, untrack } from "svelte";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import {
    listSessions,
    getSessionSummariesByIds,
    getProjectMemory,
    type ProjectInfo,
    type RepositoryGroup,
    type ProjectMemory,
    type SessionSummary,
    type SessionMetadataUpdate,
    type PaginatedResponse,
  } from "../lib/api";
  import { loadProjectData } from "../lib/projectDataStore.svelte";
  import SessionContextMenu from "./SessionContextMenu.svelte";
  import OngoingIndicator from "./OngoingIndicator.svelte";
  import SkeletonList from "./SkeletonList.svelte";
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
  } from "../lib/sidebarStore.svelte";
  import { registerHandler, unregisterHandler, scheduleRefresh, cancelScheduledRefresh } from "../lib/fileChangeStore.svelte";
  import { createVirtualWindow } from "../lib/virtualList.svelte";
  import { applySilentRefresh, mergeSessions, applyPendingMetadata } from "../lib/sessionMerge";
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
  // 后端 `list_sessions` 响应的 `result.total`：项目维度 read_dir 后的全部 session
  // 数。spec `sidebar-navigation/spec.md::会话总数显示口径` 要求 `totalSessions`
  // 取本字段，**而非** `sessions.length`，避免翻页累加 20 → 40 → 60 跳变。
  // 切 project 时 reset 为 0；非 silent + silent 路径都覆盖；loadMore 翻页**不**改。
  let sessionsTotal = $state(0);

  // listener 收到 `session-metadata-update` 时若 `sessions` 数组还没扩展到对应
  // sessionId（典型 race：多 page 并存扫描 + 高速 broadcast emit + IPC return
  // 还没落到 svelte state），patch 用的 `sessions.map` 找不到目标，update 静默
  // 丢失——broadcast 不重发，session 永远卡在 sessionId 占位。
  //
  // 兜底：listener 始终把 update 写入此 buffer（per project，按 sessionId 覆盖
  // 最新值），每次 `sessions = ...` 更新后调 `applyPendingMetadata` 把 buffer 中
  // 已存在于新 sessions 的 sessionId 一次性 patch 上去。切 project 时清空 buffer。
  //
  // 详见 spec `sidebar-navigation/spec.md::会话元数据增量 patch` Scenario
  // "更新到达时 sessions 数组还未包含 sessionId 时缓冲到 pending buffer"。
  let pendingMetadataUpdates = new Map<string, SessionMetadataUpdate>();
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
  let refreshProjectsListener: (() => void) | null = null;
  let sessionListEl: HTMLElement | null = null;

  async function loadProjects(silent = false) {
    if (!silent) projectsLoading = true;
    try {
      const result = await loadProjectData({ refresh: silent });
      repositoryGroups = result.repositoryGroups;
      projects = result.worktreeProjects;
      const selectedExists = result.worktreeProjects.some((p) => p.id === selectedProjectId);
      if (result.worktreeProjects.length > 0 && (!selectedProjectId || !selectedExists)) {
        // 默认选中"最近活动 group 的 main worktree"（spec sidebar-navigation
        // §"活跃 worktree 选中状态"）：repositoryGroups 已按 mostRecentSession
        // 倒序，worktrees 已 main 优先排序——直接取第一个 group 的 [0]。
        const first = result.repositoryGroups[0]?.worktrees?.[0];
        if (first) {
          onSelectProject(first.id, first.name);
        } else {
          onSelectProject(result.worktreeProjects[0].id, result.worktreeProjects[0].displayName);
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
        // 始终先写 pending buffer（即使当前 sessions 已含此 sessionId 也覆盖，
        // 让 update 是最终 source of truth）；buffer 在切 project / sessions 重置
        // 时清空，避免 stale。详见上方 `pendingMetadataUpdates` doc-comment。
        pendingMetadataUpdates.set(payload.sessionId, payload);
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

    refreshProjectsListener = () => {
      scheduleRefresh("sidebar:projects", () => untrack(() => loadProjects(true)));
    };
    window.addEventListener("cdt-refresh-projects", refreshProjectsListener);

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
      sessionsTotal = 0;
      pendingMetadataUpdates.clear();
      return;
    }
    // 非 silent 路径（切 project / 首次加载）SHALL 在 await 之**前**清空 buffer：
    // 后端 list_sessions 在 IPC return 之前已 spawn 扫描任务并可能 broadcast emit，
    // listener 在 `await listSessions(...)` 阻塞期间收到的新 project update 必须
    // 保留到 apply 时。clear 放 await 之后会把这些"早到的" update 一起清掉，
    // 正是 race buffer 想修的核心 bug（codex 二审第三轮找到，详见 commit 6833ba8
    // 之后的修订）。
    if (!silent) {
      pendingMetadataUpdates.clear();
      sessionsLoading = true;
    }
    try {
      await loadProjectPrefs(projectId);
      const result: PaginatedResponse<SessionSummary> = await listSessions(projectId, SESSION_PAGE_SIZE);
      if (projectId !== selectedProjectId) return;
      // silent 路径：合并到现有列表保留尾部 + 保留分页 cursor（避免 sessions 缩水
      // 与计数跳变，spec sidebar-navigation §"会话元数据增量 patch"）。非 silent：
      // 替换式加载第一页 + 取本次 cursor（buffer 在 await 前已清空，仅含 await
      // 期间到达的新 project update）。
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
      // sessions 写入后立即把 pending buffer 中已存在的 sessionId 应用上去——
      // 兜底 broadcast 在 IPC return 之前到达时找不到目标的 race。
      sessions = applyPendingMetadata(fresh, pendingMetadataUpdates);
      sessionsNextCursor = nextCursor;
      // spec sidebar-navigation §"会话总数显示口径"：silent / 非 silent 路径都用
      // 后端 `result.total`（项目维度全量 session 计数）覆盖 `sessionsTotal`。
      // loadMoreSessions 翻页路径**不**改 sessionsTotal。
      sessionsTotal = result.total;
      hasDeferredSessionRefresh = false;
      queueMicrotask(() => maybeLoadMoreSessions(true));
    } catch (e) {
      console.error("Failed to load sessions:", e);
      if (!silent && projectId === selectedProjectId) {
        sessions = [];
        sessionsNextCursor = null;
        sessionsTotal = 0;
        pendingMetadataUpdates.clear();
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
      // 翻页扩展 sessions 后立即把 pending buffer 应用上去——broadcast 可能在
      // 这次 IPC return 之前已 emit 了新增 page 的 update，那些 update 此前
      // sessions.map 找不到目标被 buffer 截胡。
      sessions = applyPendingMetadata(mergeSessions(sessions, result.items, false), pendingMetadataUpdates);
      sessionsNextCursor = result.nextCursor;
      // spec sidebar-navigation §"会话总数显示口径"：loadMore **不**改
      // sessionsTotal——首次加载时已由 loadSessions 写入正确值；翻页累加期间
      // total 不应变化。后续 silent 刷新会再用最新 result.total 覆盖。
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
    // 滚到顶部展示新加载内容——deferred refresh 默认在用户向下浏览
    // 历史时被推迟（browsingHistory=true），按钮触发的意图就是"看新内容"，
    // 默认就把视图带回顶部，避免点完按钮看似无反应。
    sessionListEl?.scrollTo({ top: 0, behavior: "smooth" });
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

  // 切 project 自动清空 filter：filterQuery 是 project 维度的过滤，
  // 在 A 项目输入 "fix" 后切到 B 项目时若不 reset，B 项目会卡在
  // "无匹配会话" 的假空状态，需要用户额外手动清空 input 才能看到列表。
  $effect(() => {
    selectedProjectId;
    untrack(() => { filterQuery = ""; });
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
    if (refreshProjectsListener) {
      window.removeEventListener("cdt-refresh-projects", refreshProjectsListener);
      refreshProjectsListener = null;
    }
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
  // 项目维度全量 session 计数。取后端 `list_sessions` 响应的 `result.total`
  // （由 `sessionsTotal` 维护），非 `sessions.length`——后者会随 loadMore 累加
  // 跳变。详见 spec `sidebar-navigation/spec.md::会话总数显示口径`。
  const totalSessions = $derived(sessionsTotal);
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
        type="search"
        placeholder="搜索会话…"
        bind:value={filterQuery}
        autocomplete="off"
        autocorrect="off"
        autocapitalize="off"
        spellcheck="false"
        enterkeyhint="search"
        aria-label="搜索会话"
      />
      <span
        class="session-count-num"
        title="可见 {visibleSessions.length} / 总 {totalSessions}"
      >{visibleSessions.length}</span>
      {#if hasDeferredSessionRefresh}
        <button
          class="refresh-pending-btn"
          onclick={refreshDeferredSessions}
          title="加载列表更新"
          aria-label="加载列表更新"
        >
          <svg viewBox="0 0 24 24" width="11" height="11" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path d="M21 12a9 9 0 1 1-3-6.7L21 8"/>
            <path d="M21 3v5h-5"/>
          </svg>
          <span>刷新</span>
        </button>
      {/if}
      {#if hiddenCount > 0}
        <button
          class="show-hidden-btn"
          class:show-hidden-active={getShowHidden()}
          title={getShowHidden() ? `隐藏 ${hiddenCount} 个会话` : `展开 ${hiddenCount} 个隐藏会话`}
          aria-label={getShowHidden() ? `隐藏 ${hiddenCount} 个已隐藏会话` : `显示 ${hiddenCount} 个隐藏会话`}
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
          <span class="hidden-count-badge" aria-hidden="true">{hiddenCount}</span>
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
    {#if (projectsLoading || sessionsLoading) && sessions.length === 0}
      <SkeletonList count={8} rowHeight={48} gap={6} padding="4px 8px" label="正在加载会话列表" />
    {:else if sessions.length === 0}
      <div class="sidebar-status">暂无会话</div>
    {:else if visibleSessions.length === 0}
      <div class="sidebar-status">
        <div class="sidebar-status-text">无匹配会话</div>
        {#if filterQuery}
          <button class="sidebar-status-link" onclick={() => { filterQuery = ""; }}>清除搜索</button>
        {/if}
      </div>
    {:else}
      <div class="vlist-spacer" style:height="{vlist.topSpacer()}px"></div>
      {#each visibleSlice as item (item.key)}
        {#if item.kind === "header"}
          {@const isPinned = item.label === "PINNED"}
          <div
            class="date-group-label"
            class:date-group-label-pinned={isPinned}
            style:height="{ITEM_HEIGHT}px"
          >
            {#if isPinned}
              <svg class="date-group-pin-icon" viewBox="0 0 24 24" width="11" height="11" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <path d="M12 17v5"/>
                <path d="M9 10.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24V16a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2v-.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V7a1 1 0 0 1 1-1 2 2 0 0 0 0-4H8a2 2 0 0 0 0 4 1 1 0 0 1 1 1z"/>
              </svg>
            {/if}
            <span>{item.label}</span>
          </div>
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
              <span class="session-title-text" title={session.title || session.sessionId}>
                {session.title || session.sessionId}
              </span>
            </div>
            <div class="session-meta">
              <span class="session-msg-count">
                <svg class="meta-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d={MESSAGE_SQUARE} /></svg>
                {session.messageCount || 0}
              </span>
              <span class="session-time">{formatTime(session.timestamp)}</span>
              {#if session.gitBranch}
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

  <!-- Resize handle —— 用 role="separator" + aria 暴露给键盘流；
       左右方向键调整宽度（10px 步长，与 sidebar 视觉密度匹配）。
       WAI-ARIA 1.2 「Window Splitter」明确 focusable separator 是合法的
       可交互控件（携带 aria-valuemin/max/now），但 svelte-check 仍把 separator
       归类为 non-interactive，需要明确忽略两个 a11y 警告。 -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    class="resize-handle"
    class:resize-handle-active={isResizing}
    role="separator"
    tabindex="0"
    aria-orientation="vertical"
    aria-label="拖动调整侧栏宽度"
    aria-valuemin={200}
    aria-valuemax={500}
    aria-valuenow={sidebarWidth}
    onmousedown={startResize}
    onkeydown={(e) => {
      const step = e.shiftKey ? 40 : 10;
      if (e.key === "ArrowLeft") {
        e.preventDefault();
        setSidebarWidth(sidebarWidth - step);
      } else if (e.key === "ArrowRight") {
        e.preventDefault();
        setSidebarWidth(sidebarWidth + step);
      } else if (e.key === "Home") {
        e.preventDefault();
        setSidebarWidth(200);
      } else if (e.key === "End") {
        e.preventDefault();
        setSidebarWidth(500);
      }
    }}
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
  /* sidebar 高度撑满父容器（app-layout）而非 100vh——chrome 拍平后顶
     部 unified toolbar 占 ~44px，sidebar 不再是 viewport 顶级容器；
     100vh 会让 sidebar 向下溢出 toolbar 高度，session-list 底部内容
     被裁切（用户实测"翻不到最底部"的根因）。改 100% 让 flex 父级
     app-layout 控制可用高度。

     sidebar 局部 AA-safe color tokens（codex 二审第二轮发现项目级
     `--color-accent-blue` / `--color-accent-indigo` 在浅色 sidebar
     bg 上对 11–12px 小字仅 3.x:1，未达 WCAG AA 4.5:1）：本组件需要
     蓝/靛文字与 outline 的所有点都走 sidebar 局部更深变体，避免污染
     全局 token 体系；其他组件复用项目 token 不变。 */
  .sidebar {
    --sidebar-accent: #1d4ed8;
    --sidebar-active-outline: var(--color-accent-blue-hover);
    --sidebar-pinned: #4338ca;
    position: relative;
    height: 100%;
    min-height: 0;
    display: flex;
    flex-direction: column;
    background: var(--color-surface-sidebar);
    border-right: 1px solid var(--color-border);
    overflow: hidden;
  }

  :global([data-theme="dark"]) .sidebar {
    --sidebar-accent: #93c5fd;
    --sidebar-active-outline: var(--color-accent-blue);
    --sidebar-pinned: #a5b4fc;
  }

  @media (prefers-color-scheme: dark) {
    :global([data-theme="system"]) .sidebar {
      --sidebar-accent: #93c5fd;
      --sidebar-active-outline: var(--color-accent-blue);
      --sidebar-pinned: #a5b4fc;
    }
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
    /* 不加 border-bottom：chrome 已在 sidebar 上方持有 1 px 分隔线，sidebar 内只在
       border，search filter bar 跟 list 之间靠 padding 视觉分隔即可。
       加 border 会让 sidebar 内出现第二条横线，跟右侧 TabBar 唯一一
       条横线对不齐（用户视觉上的「分隔线没齐平」）。 */
  }

  .session-filter-input {
    flex: 1;
    min-width: 0;
    font-size: 13px;
    font-family: inherit;
    color: var(--color-text);
    background: var(--color-surface);
    border: 1px solid var(--color-border);
    border-radius: 6px;
    padding: 6px 10px;
    outline: none;
    transition: border-color 0.15s, box-shadow 0.15s;
  }

  /* focus 用 accent-blue 边色 + 22% 同色 ring 让搜索框成为视觉焦点。
     ring 仅 2px 不影响 sidebar 已有的紧凑度。 */
  .session-filter-input:focus {
    border-color: var(--color-accent-blue);
    box-shadow: 0 0 0 2px color-mix(in oklch, var(--color-accent-blue) 22%, transparent);
  }

  .session-filter-input::placeholder {
    color: var(--color-text-muted);
  }

  .session-filter-input::-webkit-search-cancel-button,
  .session-filter-input::-webkit-search-decoration {
    appearance: none;
    -webkit-appearance: none;
  }

  .sidebar-status-inline {
    padding: 8px 0;
    font-size: 11px;
  }

  /* "5" 单数字 + hover tooltip 显 "可见 5 / 总 127"。原本 "5/127"
     永久占位无语义提示，新用户读不出含义，且和右侧按钮挤在一起。 */
  .session-count-num {
    font-size: 11px;
    color: var(--color-text-muted);
    flex-shrink: 0;
    font-variant-numeric: tabular-nums;
    font-family: var(--font-mono);
    cursor: default;
  }

  /* "刷新"按钮：原蓝胶囊版本太抢眼（用户实测反馈），重新设计为内联
     文字 link 形态——透明 bg、无 border，仅 icon + 单字"刷新"。色彩
     用 sidebar 局部 --sidebar-accent（浅 #1d4ed8 / 深 #93c5fd）保证
     11px 小字 WCAG AA ≥4.5:1，权重视觉降到与"返回 Dashboard"链接同
     级。hover 时显微底反馈用户击中目标。
     语义保持：focus-blue 表达"实时 / 有新数据待加载"（DESIGN.md
     `The Ongoing Owns Blue Rule` 同类），不引入新色。 */
  .refresh-pending-btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    flex-shrink: 0;
    padding: 3px 6px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--sidebar-accent);
    font: inherit;
    font-size: 11px;
    font-weight: 500;
    line-height: 1.2;
    cursor: pointer;
    transition: background 0.12s;
  }

  .refresh-pending-btn:hover {
    background: color-mix(in oklch, var(--color-accent-blue) 10%, transparent);
  }

  .refresh-pending-btn:focus-visible {
    outline: 2px solid var(--color-accent-blue);
    outline-offset: 1px;
  }

  .show-hidden-btn {
    position: relative;
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
    color: var(--color-accent-blue);
  }

  /* hiddenCount 暴露在按钮右上角的小数字 badge：把"有 N 条隐藏"这一
     关键信息从 hover title 提到视觉一级。默认 muted 不和 focus-blue
     抢戏；按钮 active（展开隐藏）时和按钮一起转蓝形成颜色一致性。 */
  .hidden-count-badge {
    position: absolute;
    top: -3px;
    right: -4px;
    min-width: 13px;
    height: 13px;
    padding: 0 3px;
    border-radius: 7px;
    background: var(--color-surface-overlay);
    color: var(--color-text-secondary);
    font-size: 9px;
    font-weight: 600;
    line-height: 13px;
    text-align: center;
    font-variant-numeric: tabular-nums;
    box-sizing: border-box;
    pointer-events: none;
  }

  .show-hidden-active .hidden-count-badge {
    background: color-mix(in oklch, var(--color-accent-blue) 22%, var(--color-surface));
    color: var(--sidebar-accent);
  }

  .session-list {
    flex: 1;
    overflow-y: auto;
    padding: 4px 8px;
  }

  .sidebar-status {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    padding: 24px 12px;
    text-align: center;
    color: var(--color-text-muted);
    font-size: 13px;
  }

  .sidebar-status-text {
    color: var(--color-text-muted);
  }

  /* 空状态指路链接：filter 输入了 typo 时让用户一键 reset，避免被
     困在死路里。仅在 filterQuery 非空 → 出现"无匹配会话"时渲染。
     色彩走 sidebar 局部 --sidebar-accent 保证 WCAG AA 4.5:1。 */
  .sidebar-status-link {
    background: none;
    border: none;
    color: var(--sidebar-accent);
    font: inherit;
    font-size: 12px;
    cursor: pointer;
    padding: 2px 8px;
    border-radius: 4px;
    transition: background 0.12s;
  }

  .sidebar-status-link:hover {
    background: color-mix(in oklch, var(--color-accent-blue) 10%, transparent);
  }

  .sidebar-status-link:focus-visible {
    outline: 2px solid var(--color-accent-blue);
    outline-offset: 1px;
  }

  .vlist-spacer {
    width: 100%;
    pointer-events: none;
  }

  .date-group-label {
    display: flex;
    align-items: flex-end;
    gap: 5px;
    font-size: 11px;
    font-weight: 600;
    color: var(--color-text-muted);
    padding: 10px 8px 4px;
    letter-spacing: 0.3px;
    box-sizing: border-box;
  }

  /* PINNED 是用户主动标记的"我在意这条"行为，视觉上应区别于 TODAY /
     YESTERDAY 这种被动时间分组。色彩走 sidebar 局部 --sidebar-pinned
     深一档 indigo（codex 二审第二轮：项目级 --color-accent-indigo
     #6366f1 在浅 sidebar bg 上 11px 仅 3.96:1 < AA 4.5:1）。 */
  .date-group-label-pinned {
    color: var(--sidebar-pinned);
  }

  .date-group-pin-icon {
    color: var(--sidebar-pinned);
    flex-shrink: 0;
    margin-bottom: 1px;
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

  /* 选中态：左 2px 蓝色结构性 indicator + surface-overlay 加深背景
     + 标题字重 600。原 1px 蓝 outline 在浅色暖灰 sidebar 上对比度
     ≈7.8:1 形成"相框效应"，视觉权重反而盖过 SessionDetail 头部，
     不符合 sidebar 应作为"位置标记"而非"call to action"的定位
     （DESIGN.md `Status Owns the Color` + `Navigation rows::Active
     item 可以用细窄 2-3px indicator`）。
     - indicator 用 box-shadow inset，不占 box-model 空间 / 不触发
       reflow，等价于 outline 的性能特征。沿用 --sidebar-active-outline
       token 保证浅 #2563eb / 深 #60a5fa 跨主题对比度：indicator on
       sidebar bg ≥6:1，远超 WCAG 1.4.11 非文本 3:1。
     - bg surface-overlay 提供第二条层级信号；title 字重 600 提供
       第三条字重信号（与 hover 的默认 400 拉开）。
     - 字重从 700 降到 600：sidebar 选中态不应是该 surface 上最重的
       视觉元素——详情页主标题才是当前焦点，sidebar 选中只指示"在
       看哪条"。 */
  .session-item-active {
    background: var(--color-surface-overlay);
    box-shadow: inset 2px 0 0 var(--sidebar-active-outline);
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
    color: var(--color-accent-blue);
  }

  /* meta 行（msgCount · time · branch）：原本用显式 "·" 分隔符（10px
     muted opacity 0.5），视觉接近噪点；改用 14px gap 留白做分隔，
     扫读三段权重更清晰，密度不变。msgCount 与 time 保持 flex-shrink:0
     不让出空间，branch 用默认 flex-shrink:1 + overflow ellipsis 自然
     吸收剩余宽度——容器变窄时 branch 优先被截断而非 msgCount/time
     被压扁（msgCount/time 本来只有 ~22px 不值得参与收缩竞争）。 */
  .session-meta {
    display: flex;
    gap: 14px;
    align-items: center;
    line-height: 1.2;
    min-width: 0;
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

  .session-time {
    font-size: 10px;
    color: var(--color-text-muted);
    font-variant-numeric: tabular-nums;
    flex-shrink: 0;
    white-space: nowrap;
  }

  /* branch 是 "我在哪个 worktree" 的核心线索；让它吸收 meta row 剩余
     宽度，msgCount/time 不与之竞争（见 .session-meta 注释）。删除显式
     "·" 分隔符后，每个分隔节省 ~14px 横向空间（原 8+6+8 = 22px → 现
     14px gap），branch 比原本多 ~16px 可见区间。 */
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
    color: var(--color-text-muted);
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
  .resize-handle-active,
  .resize-handle:focus-visible {
    background: rgba(59, 130, 246, 0.5);
    background: color-mix(in oklch, var(--color-accent-blue) 50%, transparent);
    outline: none;
  }
</style>

<script lang="ts">
  import { onMount, onDestroy, untrack } from "svelte";
  import {
    listGroupSessions,
    getSessionSummariesByIds,
    getProjectMemory,
    type ProjectInfo,
    type RepositoryGroup,
    type ProjectMemory,
    type Worktree,
    type SessionSummary,
    type SessionMetadataUpdate,
    type GroupSessionPage,
  } from "../lib/api";
  import { loadProjectData } from "../lib/projectDataStore.svelte";
  import OngoingIndicator from "./OngoingIndicator.svelte";
  import SkeletonList from "./SkeletonList.svelte";
  import WorktreeChipCluster from "../lib/components/WorktreeChipCluster.svelte";
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
  import { subscribeEvent, type Unsubscribe } from "../lib/transport";
  import { accumulate as telemetryAccumulateCorrectness } from "../lib/correctnessTelemetryStore.svelte";
  import { isTauriRuntime } from "../lib/runtime";
  import { createVirtualWindow } from "../lib/virtualList.svelte";
  import {
    applySilentRefresh,
    mergeSessions,
    mergeRecoveryResponse,
    applyPendingMetadata,
  } from "../lib/sessionMerge";
  import {
    read as readSessionListCache,
    setSessions as cacheSessions,
    applyMetadata as cacheApplyMetadata,
  } from "../lib/sessionListStore.svelte";
  import { buildFilterCursor, sessionListCacheKey } from "../lib/groupCursor";
  import { MESSAGE_SQUARE, BOOK_OPEN_TEXT_SVG } from "../lib/icons";
  import {
    contextMenu,
    type ContextMenuItem,
  } from "../lib/contextMenu.svelte";

  // 虚拟滚动行高（实测 .session-item ≈ 44px：padding 8+8 + title 13×1.4 +
  // meta 11×1.4）；header 行高强制对齐 44 让单一 windowing 单元生效。
  const ITEM_HEIGHT = 44;
  // change `simplify-repository-as-project::D3`：k-way merge 路径每页 50 条，
  // 比单 worktree 的 20 略大——多 worktree group 合并后期望首屏 ~50 条覆盖
  // 最常用的 "TODAY + YESTERDAY" 时间窗口。
  const SESSION_PAGE_SIZE = 50;
  const HISTORY_SCROLL_THRESHOLD = ITEM_HEIGHT * 2;
  const ALL_WORKTREES = "__all__";

  interface Props {
    /** App 顶层选中的项目入口 id —— `RepositoryGroup.id`（D7 rename）。 */
    selectedGroupId: string;
    activeSessionId: string | null;
    collapsed?: boolean;
    onSelectProject: (id: string, name: string) => void;
    onSelectSession: (sessionId: string, projectId: string, groupId: string, label: string, event: MouseEvent) => void;
  }

  let {
    selectedGroupId,
    activeSessionId,
    collapsed = false,
    onSelectProject,
    onSelectSession,
  }: Props = $props();

  let projects: ProjectInfo[] = $state([]);
  let repositoryGroups: RepositoryGroup[] = $state([]);
  let sessions: SessionSummary[] = $state([]);
  let projectMemory: ProjectMemory | null = $state(null);
  // by-worktree-id memory cache：切 group 时同步 hydrate 让 memory-entry
  // 显隐与 selectedGroupId 切换瞬时同步，避免等 async getProjectMemory
  // return 期间 entry 闪现/消失引发 sidebar 顶部 layout shift。命中走 SWR：
  // 先 set 当前值同时后台 refresh；miss 走正常 fetch。
  const memoryCache = new Map<string, ProjectMemory | null>();
  let projectsLoading = $state(true);
  let sessionsLoading = $state(false);
  let sessionsLoadingMore = $state(false);
  let sessionsNextCursor: string | null = $state(null);
  // 当前 scope 总量由 `scopeTotal` derived 派生，按 worktreeFilter 走（spec
  // sidebar-navigation §"会话总数显示口径"）：filter=ALL → group 全集；
  // filter=具体 wt → 该 wt 集合。切 group / 切 chip 时 derived 自动更新。
  /** 当前 worktree filter（`ALL_WORKTREES` = "全部"；否则为 worktree.id）。
   * session-scoped，切 group 时 reset 为 "全部"（不跨会话持久化）。 */
  let worktreeFilter: string = $state(ALL_WORKTREES);

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
  // 通过 use:contextMenu action（lib/contextMenu.svelte.ts）接管右键，菜单
  // portal 到 document.body 由 AppContextMenu 渲染——避免被 sidebar 虚拟滚动
  // 容器 overflow clip，且统一全应用菜单视觉与 a11y。

  function buildSessionContextItems(
    session: SessionSummary,
  ): ContextMenuItem[] {
    const sessionProjectId = session.worktreeId ?? selectedGroupId;
    const canSplit = getPaneLayout().panes.length < MAX_PANES;
    const pinned = isPinned(sessionProjectId, session.sessionId);
    const hidden = isHidden(sessionProjectId, session.sessionId);
    const label = sessionLabel(session);
    return [
      {
        label: "在当前标签页打开",
        action: () =>
          openOrReplaceTab(session.sessionId, sessionProjectId, label),
      },
      {
        label: "在新标签页打开",
        action: () => openTab(session.sessionId, sessionProjectId, label),
      },
      {
        label: "在新 Pane 打开",
        disabled: !canSplit,
        action: () =>
          openTabInNewPane(session.sessionId, sessionProjectId, label),
      },
      { separator: true },
      {
        label: pinned ? "取消置顶" : "置顶会话",
        action: () => togglePin(sessionProjectId, session.sessionId),
      },
      {
        label: hidden ? "取消隐藏" : "隐藏会话",
        action: () => toggleHide(sessionProjectId, session.sessionId),
      },
      { separator: true },
      {
        label: "复制 Session ID",
        action: () => navigator.clipboard.writeText(session.sessionId),
        feedback: { label: "已复制!" },
      },
      {
        label: "复制恢复命令",
        action: () =>
          navigator.clipboard.writeText(`claude --resume ${session.sessionId}`),
        feedback: { label: "已复制!" },
      },
    ];
  }

  // ---------------------------------------------------------------------------
  // Data loading
  // ---------------------------------------------------------------------------

  let metadataUnlisten: Unsubscribe | null = null;
  let sseRecoveredUnlisten: Unsubscribe | null = null;
  let sseLaggedUnlisten: Unsubscribe | null = null;
  let refreshProjectsListener: (() => void) | null = null;
  let sessionListEl: HTMLElement | null = null;

  async function loadProjects(silent = false) {
    if (!silent) projectsLoading = true;
    try {
      const result = await loadProjectData({ refresh: silent });
      repositoryGroups = result.repositoryGroups;
      projects = result.worktreeProjects;
      // change `simplify-repository-as-project::D7`：顶层导航持 group id。
      // 默认选中"最近活动 group"——`repositoryGroups` 已按 mostRecentSession 倒序。
      const selectedExists = result.repositoryGroups.some((g) => g.id === selectedGroupId);
      if (!selectedExists) {
        const firstGroup = result.repositoryGroups[0];
        if (firstGroup) {
          onSelectProject(firstGroup.id, firstGroup.name);
        } else if (result.worktreeProjects.length > 0) {
          // fallback：listRepositoryGroups 返空时走老 listProjects 平铺路径
          onSelectProject(result.worktreeProjects[0].id, result.worktreeProjects[0].displayName);
        }
      }
    } catch (e) {
      console.error("Failed to load projects:", e);
    } finally {
      if (!silent) projectsLoading = false;
    }
  }

  /** 当前选中 group 的 worktrees（按 D1 排序：repo 根 → 主 worktree → mtime 倒序）。
   * grouper 输出已按此排序，无需前端二次排序。 */
  const selectedGroup = $derived(repositoryGroups.find((g) => g.id === selectedGroupId) ?? null);
  const groupWorktrees: Worktree[] = $derived(selectedGroup?.worktrees ?? []);
  const showWorktreeFilter = $derived(groupWorktrees.length > 1);

  /** "锚点" worktree —— 用于 Pin/Hide 等 per-project state 的
   * projectId。spec sidebar-navigation D7："per-project memory / prefs 维持
   * per-worktree"——worktree filter 选了具体 worktree 时锚点 SHALL 跟随；
   * "全部" 模式下 fallback 到 repo 根 → 主 worktree → 第一个。 */
  const anchorWorktreeId = $derived.by(() => {
    if (groupWorktrees.length === 0) return selectedGroupId;
    if (worktreeFilter !== ALL_WORKTREES) {
      const filtered = groupWorktrees.find((w) => w.id === worktreeFilter);
      if (filtered) return filtered.id;
    }
    return (
      groupWorktrees.find((w) => w.isRepoRoot)?.id
      ?? groupWorktrees.find((w) => w.isMainWorktree)?.id
      ?? groupWorktrees[0].id
    );
  });

  /** Memory 入口专用锚点 —— **不**跟随 worktree filter，恒定指向 group
   * 的 repo 根 worktree。
   *
   * Memory 文件物理上写在 Claude Code 父进程 cwd 编码出的 project_dir 下
   * （`~/.claude/projects/<encoded-cwd>/memory/`）。绝大多数用户只在 repo
   * 根目录跑 Claude Code，每个 worktree 各自的 encoded project_dir 下根本
   * 不存在 memory 目录——若 anchor 跟随 worktree filter，切到具体 worktree
   * 后端按 worktree id 查 → `count=0` → sidebar 顶部 memory 入口消失。
   *
   * pin/hide 仍跟随 anchorWorktreeId（per-worktree 有意义：worktree 间 session
   * 列表不同，置顶/隐藏 应该 per-worktree 隔离）。 */
  const memoryAnchorWorktreeId = $derived.by(() => {
    if (groupWorktrees.length === 0) return selectedGroupId;
    return (
      groupWorktrees.find((w) => w.isRepoRoot)?.id
      ?? groupWorktrees.find((w) => w.isMainWorktree)?.id
      ?? groupWorktrees[0].id
    );
  });

  /** 当前 sessions / context-menu 项查 Pin/Hide / Memory 状态的 projectId。
   * 优先用 session 自带 `worktreeId`（IPC join 填），否则 fallback 到锚点 worktree。 */
  function projectIdForSession(s: SessionSummary | null | undefined): string {
    return s?.worktreeId ?? anchorWorktreeId;
  }

  function buildSessionListCacheKey(): string {
    return sessionListCacheKey(
      selectedGroupId,
      worktreeFilter === ALL_WORKTREES ? null : worktreeFilter,
    );
  }

  /** "全部" → null；具体 worktree → 构造 server-side filter cursor（D6）。 */
  function initialFilterCursor(): string | null {
    if (worktreeFilter === ALL_WORKTREES) return null;
    return buildFilterCursor(groupWorktrees, worktreeFilter);
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
    metadataUnlisten = await subscribeEvent<SessionMetadataUpdate>(
      "session-metadata-update",
      (event) => {
        const payload = event.payload;
        // 切 group 期间残留的旧 group 事件忽略。D7：后端 `SessionMetadataUpdate`
        // 新增 `groupId` 字段——优先按 groupId 匹配 selectedGroupId；缺省
        // （未跑过 list_repository_groups）时 fallback 按 projectId 匹配
        // selectedGroupId，单 worktree group 字符串相同仍能命中。
        const eventGroupId = payload.groupId ?? payload.projectId;
        if (eventGroupId !== selectedGroupId) return;
        // 始终先写 pending buffer（即使当前 sessions 已含此 sessionId 也覆盖，
        // 让 update 是最终 source of truth）；buffer 在切 group / sessions 重置
        // 时清空，避免 stale。详见上方 `pendingMetadataUpdates` doc-comment。
        pendingMetadataUpdates.set(payload.sessionId, payload);
        // Telemetry: 检测 stale-update —— 旧 title 已是 not-null 真值且与新值不
        // 一致时累计 `stale_update.triggered` counter。store 内部 5s/50 阈值节流
        // 批量 flush 给 IPC `record_correctness_events`，避免 file-change 风暴
        // 把这条低频信号变成 IPC 热点（详见 add-telemetry-signal-bus design D10）。
        const prev = sessions.find((s) => s.sessionId === payload.sessionId);
        if (
          prev &&
          prev.title != null &&
          payload.title != null &&
          prev.title !== payload.title
        ) {
          telemetryAccumulateCorrectness("stale_update.triggered");
        }
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
        // 同步写 store 缓存：下次切回该 group 时立即看到已 patch 的真值。
        // cache key 走 `buildSessionListCacheKey()` 复合 (groupId + filter)
        // 与 loadSessions / loadMore 写入路径对齐。
        cacheApplyMetadata(buildSessionListCacheKey(), payload);
      },
    );

    // SSE 恢复兜底（codex 二审 issue 1 / issue 2 修法的 UI 层）：
    // - `sse-recovered`：transport 层 ensureSseReady 1000 ms 超时放行 fetch 后，
    //   后端发出的 metadata patch 在 SSE OPEN 前已丢失，OPEN 时通知 UI 重拉
    // - `sse-lagged`：HTTP server 端 broadcast 容量打满 + slow client 跟不上时
    //   sse handler 推送的 sentinel，告知 UI "中间已丢若干 PushEvent"
    // 两条都触发当前 project 一次 silent refresh，让后端重启扫描 + emit 新
    // 一轮 metadata patch；silent merge 不会用骨架 null 覆盖已 patch 真值。
    //
    // 仅 BrowserTransport 会 synthesize 这两个事件——Tauri runtime 直接走
    // `@tauri-apps/api` 的 listen()，不存在 SSE OPEN/lag 概念。Tauri runtime
    // 跳过订阅避免在测试 mock 环境下额外触发 listen() 调用让 transformCallback
    // 路径报错，同时也省掉真桌面运行时无谓的 listen() 注册。
    if (!isTauriRuntime()) {
      const recoverHandler = () => {
        const groupId = selectedGroupId;
        if (!groupId) return;
        // 触发后端按**已加载范围**重新扫描，pageSize=sessions.length 让
        // 后端 `take(pagination.page_size)` 覆盖 page 1 + page 2+。两类
        // 真值都需要消费 response 写回：
        //
        // 1. **cache hit 项**：后端 `try_lookup_cached_metadata` fast-path
        //    inline 返真值（`crates/cdt-api/src/ipc/local.rs:809-820`），
        //    **不**入 `page_jobs` 后台扫描，**不**会 emit SSE patch；前端
        //    必须从 response 拿真值合并（codex 二审 round 4）
        // 2. **cache miss 项**：后端 spawn 扫描后通过 SSE 广播
        //    `SessionMetadataUpdate`，UI 走既有 listener patch 路径写回；
        //    response 里仍是骨架——`mergeSilentMetadata` 让新骨架不会
        //    覆盖 prev 已 patch 真值，安全
        //
        // 决策记录（codex 4 轮验证后定型）：
        // - round 1 silent loadSessions 重扫 page 1 → page 2+ pending 永久卡空
        // - round 2 + getSessionSummariesByIds 补齐 → 该 IPC 是 light
        //   skeleton（contract 固化），无效
        // - round 3 listSessions(已加载范围) fire-and-forget → cache hit
        //   不 emit SSE，response 真值丢失
        // - round 4（当前）listSessions(已加载范围) **消费 response** 走
        //   mergeSessions/applyPendingMetadata 写回——cache hit 真值从
        //   response 拿、cache miss 真值从 SSE patch 拿
        const pageSize = Math.max(sessions.length, SESSION_PAGE_SIZE);
        const cacheKey = buildSessionListCacheKey();
        void (async () => {
          try {
            // D6/D7：走 listGroupSessions + 当前 worktree filter 的初始 cursor
            const result = await listGroupSessions(groupId, pageSize, initialFilterCursor());
            // race guard：异步完成时 user 可能已切到别的 group / filter
            if (groupId !== selectedGroupId) return;
            if (cacheKey !== buildSessionListCacheKey()) return;
            // recovery 专用合并语义：response 真值（cache hit fast-path）
            // 优先覆盖 prev stale 真值，response 骨架则保留 prev 已 patched
            // 真值。常规 mergeSessions 用 mergeSilentMetadata 始终保留 prev
            // 真值——recovery 场景下 prev 真值可能 stale，会卡 stale 状态
            // （codex 二审 round 5）。
            //
            // 这里**不**调 applyPendingMetadata：buffer 中可能保留了 lag 之前
            // 的旧 SSE patch（buffer 跨 SSE 异常生命周期持久），applyPendingMetadata
            // 会用 buffer 旧值覆盖刚刚 mergeRecoveryResponse 写入的 response
            // 新真值，让 stale 自愈失败（codex 二审 round 6）。recovery 路径
            // sessions 已含全部 sessionId（因为 pageSize=sessions.length），
            // listener 同时直接走 sessions.map in-place patch，buffer 兜底
            // 路径在 recovery 场景下无必要。
            sessions = mergeRecoveryResponse(sessions, result.sessions);
            cacheSessions(cacheKey, sessions, sessionsNextCursor, scopeTotal);
          } catch (e) {
            console.warn("[sidebar] sse-recovery rescan failed:", e);
          }
        })();
      };
      sseRecoveredUnlisten = await subscribeEvent<unknown>("sse-recovered", recoverHandler);
      sseLaggedUnlisten = await subscribeEvent<unknown>("sse-lagged", recoverHandler);
    }

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
    // 同步 hydrate：cache 命中立即 set，避免等 IPC return 期间 memory-entry
    // 显隐引发 sidebar 顶部 layout shift（用户来回切已访问过的项目时）。
    const cached = memoryCache.get(projectId);
    if (cached !== undefined) {
      projectMemory = cached;
      // SWR 后台 refresh 拉新值并写 cache；只有 memoryAnchorWorktreeId 仍是
      // 当前 projectId 时才回写 projectMemory，避免覆盖用户期间已切换到
      // 其它 group 的显示。
      void (async () => {
        try {
          const fresh = await getProjectMemory(projectId);
          memoryCache.set(projectId, fresh);
          if (projectId === memoryAnchorWorktreeId) projectMemory = fresh;
        } catch (e) {
          console.warn("Failed to refresh project memory:", e);
        }
      })();
      return;
    }
    try {
      const memory = await getProjectMemory(projectId);
      memoryCache.set(projectId, memory);
      if (projectId === memoryAnchorWorktreeId) projectMemory = memory;
    } catch (e) {
      console.warn("Failed to load project memory:", e);
      if (projectId === memoryAnchorWorktreeId) projectMemory = null;
    }
  }

  async function loadSessions(groupId: string, silent = false) {
    if (!groupId) {
      sessions = [];
      sessionsNextCursor = null;
      pendingMetadataUpdates.clear();
      return;
    }
    const cacheKey = buildSessionListCacheKey();
    const anchor = anchorWorktreeId;
    // 非 silent 路径（切 group / 首次加载）：先查 sessionListStore 缓存；
    // 命中则立即 sync hydrate 三态（避免"加载中..."中间态），同时触发 silent
    // SWR refresh。详见 spec sidebar-navigation §"Sessions store
    // stale-while-revalidate 缓存"。
    if (!silent) {
      const cached = readSessionListCache(cacheKey);
      if (cached) {
        // 切 group 的瞬间：buffer 也清掉（与下面 non-cached 分支同步行为），
        // 旧 group 残留的 update 不应继承到新 group 显示。
        pendingMetadataUpdates.clear();
        sessions = cached.sessions;
        sessionsNextCursor = cached.nextCursor;
        sessionsLoading = false;
        // 后台 silent refresh 兜底拉最新；走原 silent merge 路径保留尾部
        void loadSessions(groupId, true);
        return;
      }
    }
    // 非 silent 路径（切 group / 首次加载）SHALL 在 await 之**前**清空 buffer：
    // 后端 list_group_sessions 在 IPC return 之前已 spawn 扫描任务并可能 broadcast emit，
    // listener 在 `await listGroupSessions(...)` 阻塞期间收到的新 group update 必须
    // 保留到 apply 时。clear 放 await 之后会把这些"早到的" update 一起清掉，
    // 正是 race buffer 想修的核心 bug（codex 二审第三轮找到，详见 commit 6833ba8
    // 之后的修订）。
    if (!silent) {
      pendingMetadataUpdates.clear();
      sessionsLoading = true;
    }
    try {
      // pin/hide prefs 仍按 worktree 维度持久化——用 anchor worktree id 拉取
      await loadProjectPrefs(anchor);
      // D6/D7：走 listGroupSessions + 当前 worktree filter 的初始 cursor。
      // silent 路径**也**走 `initialFilterCursor()`——之前 `silent ? null :`
      // 让 file-change 触发的 silent refresh 用 null cursor 拉全 group 数据，
      // 与 cacheKey (groupId + filter) 口径不一致，applySilentRefresh merge
      // 时会把其他 worktree 的 session 混进当前 filter view（codex 二审
      // round 3 Blocker，2026-05-21）。`initialFilterCursor()` 在 filter=ALL
      // 时本就返 null 与原行为兼容，filter=worktreeX 时返 base64 cursor
      // 让 silent 只拉到 worktreeX 数据，与 cacheKey 一致。
      const result: GroupSessionPage = await listGroupSessions(
        groupId,
        SESSION_PAGE_SIZE,
        initialFilterCursor(),
      );
      // 同时校验 groupId + (groupId, filter) 复合键——切 group 时 reset
      // worktreeFilter effect 与本 loadSessions effect 在同 microtask 触发，
      // 旧 filter 构造的请求可能晚于新 filter 请求返回，late response 会用空
      // 列表覆盖正确列表（codex 二审 Major 1，2026-05-21）。
      if (cacheKey !== buildSessionListCacheKey()) return;
      // silent 路径：合并到现有列表保留尾部 + 保留分页 cursor（避免 sessions 缩水
      // 与计数跳变，spec sidebar-navigation §"会话元数据增量 patch"）。非 silent：
      // 替换式加载第一页 + 取本次 cursor（buffer 在 await 前已清空，仅含 await
      // 期间到达的新 group update）。
      let fresh: SessionSummary[];
      let nextCursor: string | null;
      if (silent) {
        const merged = applySilentRefresh(sessions, sessionsNextCursor, result.sessions);
        fresh = merged.sessions;
        nextCursor = merged.nextCursor;
      } else {
        fresh = result.sessions;
        nextCursor = result.nextCursor;
      }
      fresh = await reconcilePinnedAndHidden(anchor, fresh);
      if (cacheKey !== buildSessionListCacheKey()) return;
      // sessions 写入后立即把 pending buffer 中已存在的 sessionId 应用上去——
      // 兜底 broadcast 在 IPC return 之前到达时找不到目标的 race。
      sessions = applyPendingMetadata(fresh, pendingMetadataUpdates);
      sessionsNextCursor = nextCursor;
      // 同步写 by-(groupId + filter) 缓存供下次切回立即 hydrate；total 取
      // 当前 scope 的 `scopeTotal` derived，让 cache 与 UI 顶部 count 显示
      // 口径一致。GroupSessionPage 无 `.total`，total 由 derived 维护。
      cacheSessions(cacheKey, sessions, sessionsNextCursor, scopeTotal);
      hasDeferredSessionRefresh = false;
      queueMicrotask(() => maybeLoadMoreSessions(true));
    } catch (e) {
      console.error("Failed to load sessions:", e);
      if (!silent && cacheKey === buildSessionListCacheKey()) {
        sessions = [];
        sessionsNextCursor = null;
        pendingMetadataUpdates.clear();
      }
    } finally {
      if (!silent && cacheKey === buildSessionListCacheKey()) sessionsLoading = false;
    }
  }

  async function loadMoreSessions() {
    const groupId = selectedGroupId;
    const cursor = sessionsNextCursor;
    if (!groupId || !cursor || sessionsLoading || sessionsLoadingMore) return;
    const cacheKey = buildSessionListCacheKey();
    sessionsLoadingMore = true;
    try {
      const result = await listGroupSessions(groupId, SESSION_PAGE_SIZE, cursor);
      if (groupId !== selectedGroupId || cursor !== sessionsNextCursor) return;
      if (cacheKey !== buildSessionListCacheKey()) return;
      // 翻页扩展 sessions 后立即把 pending buffer 应用上去——broadcast 可能在
      // 这次 IPC return 之前已 emit 了新增 page 的 update，那些 update 此前
      // sessions.map 找不到目标被 buffer 截胡。
      sessions = applyPendingMetadata(mergeSessions(sessions, result.sessions, false), pendingMetadataUpdates);
      sessionsNextCursor = result.nextCursor;
      // spec sidebar-navigation §"会话总数显示口径"：loadMore **不**改
      // scopeTotal——total 由 `selectedGroup.totalSessions` /
      // `Worktree.sessions.length` derived 维护，翻页累加 sessions 不影响 scope 总量。
      // 同步写 store 缓存（保留 total 不变，nextCursor 推进）
      cacheSessions(cacheKey, sessions, sessionsNextCursor, scopeTotal);
    } catch (e) {
      console.error("Failed to load more sessions:", e);
    } finally {
      // 无条件复位：防 stale 数据写回的责任已被上方 `groupId !== selectedGroupId
      // || cursor !== sessionsNextCursor` 早返担住；`sessionsLoadingMore` 是
      // 纯 UI 闸门，SHALL 在 IPC 返回后无条件清零。
      //
      // 回归：PR #202 为 sub-window race 加 `if (groupId === selectedGroupId)`
      // 守卫，但忽略了"SSH 断开 → loadProjects 自动 onSelectProject(local 第一个
      // group) → selectedGroupId 已变"路径——旧 SSH IPC 完成时守卫不放，
      // sessionsLoadingMore 永卡 true，sidebar 翻页死锁 + "加载更多..."常驻。
      sessionsLoadingMore = false;
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
    if (!selectedGroupId || !hasDeferredSessionRefresh) return;
    hasDeferredSessionRefresh = false;
    void loadSessions(selectedGroupId, true);
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
    if (selectedGroupId) {
      // memory 走 memoryAnchorWorktreeId（恒定 group repo 根，不随 filter 漂）；
      // pin/hide 仍 per-worktree 持久化——用 anchorWorktreeId 跟随 filter。
      void loadProjectMemory(memoryAnchorWorktreeId);
      // 首次访问此 group 的 anchor worktree 时从后端拉取 pin/hide 持久化状态（幂等）
      void loadProjectPrefs(anchorWorktreeId);
    }
  });

  // 合并 (selectedGroupId, worktreeFilter) 驱动 loadSessions —— Svelte 5 在同
  // microtask 内合并依赖变更，单次 effect 跑一次。切 group 时 reset filter
  // 与 group 变更同时入栈，避免双触发（ui-reviewer 反馈 #3：原本 selectedGroupId
  // effect + worktreeFilter effect 各调一次 IPC，浪费 1 个 list_group_sessions 调用）。
  $effect(() => {
    const gid = selectedGroupId;
    worktreeFilter;
    untrack(() => {
      if (gid) void loadSessions(gid);
      // spec §"切 chip 构造 server-side filter cursor" / §"切 group 时
      // session-list 滚动位置重置"：任何使 sessions 集合整体替换的操作
      // SHALL 滚回顶部，避免旧 scrollTop 残留导致新列表停在中段或空白。
      // try/catch 容错单测里 Object.defineProperty mock 出 read-only scrollTop
      // 的场景；浏览器 / WKWebView 真实环境 scrollTop 永远可写。
      try {
        if (sessionListEl) sessionListEl.scrollTop = 0;
      } catch { /* noop */ }
    });
  });

  // 切 group 自动清空 filter：filterQuery 是 group 维度的过滤，
  // 在 A group 输入 "fix" 后切到 B group 时若不 reset，B group 会卡在
  // "无匹配会话" 的假空状态，需要用户额外手动清空 input 才能看到列表。
  // 切 group 时同时把 worktreeFilter 复位到"全部"，避免上一个 group 的
  // worktree id 在新 group 不存在时 derived cursor 把所有 worktree 都标
  // Exhausted 让列表空白。
  $effect(() => {
    selectedGroupId;
    untrack(() => {
      filterQuery = "";
      worktreeFilter = ALL_WORKTREES;
    });
  });

  // 注册 file-change handler；依赖 selectedGroupId / anchorWorktreeId，
  // 切 group 时重新注册让闭包捕获最新值。file-change 事件按 worktree
  // 触发（payload.projectId 是 worktree id）。
  //
  // 全部模式（filter=ALL）下列表合并 group 内所有 worktree，应接受 group
  // 内**任一** worktree 命中即刷新——否则其他非 anchor worktree 新增
  // session 不会触发列表更新（codex 二审 round 3 Major，2026-05-21）。
  // 具体 filter 模式只接 anchor，保持精准刷新。
  $effect(() => {
    const currentGroupId = selectedGroupId;
    const currentAnchor = anchorWorktreeId;
    const currentFilter = worktreeFilter;
    const currentGroupWorktreeIds = new Set(groupWorktrees.map((w) => w.id));
    registerHandler("sidebar", (payload) => {
      if (payload.projectListChanged) {
        scheduleRefresh("sidebar:projects", () =>
          untrack(() => loadProjects(true)),
        );
      }
      if (!currentGroupId || !payload.sessionId) return;
      const inGroup = currentFilter === ALL_WORKTREES
        ? currentGroupWorktreeIds.has(payload.projectId)
        : payload.projectId === currentAnchor;
      if (!inGroup) return;
      if (browsingHistory) {
        hasDeferredSessionRefresh = true;
        return;
      }
      scheduleRefresh(`sidebar:${currentGroupId}`, () =>
        untrack(() => loadSessions(currentGroupId, true)),
      );
      // session 增 / 删事件 SHALL 同步触发 list_repository_groups SWR revalidate，
      // 让 `selectedGroup.totalSessions` / `wt.sessions.length`（scopeTotal 唯一权威源）
      // 与列表 silent refresh 一起跟新，否则顶部 count 会停在旧值——spec
      // sidebar-navigation §"会话总数显示口径" 要求 silent 刷新时 scopeTotal
      // 同步下降 / 上升。仅在 projectListChanged 未走过该 schedule 时触发，
      // 避免同 payload 重复入队（codex round 2 Minor）。
      if (!payload.projectListChanged) {
        scheduleRefresh("sidebar:projects", () =>
          untrack(() => loadProjects(true)),
        );
      }
    });
    return () => {
      unregisterHandler("sidebar");
      if (currentGroupId) cancelScheduledRefresh(`sidebar:${currentGroupId}`);
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
    sseRecoveredUnlisten?.();
    sseRecoveredUnlisten = null;
    sseLaggedUnlisten?.();
    sseLaggedUnlisten = null;
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
    return filteredSessions.filter(s => !isHidden(anchorWorktreeId, s.sessionId));
  });

  const pinnedSessions = $derived(
    visibleSessions.filter(s => isPinned(anchorWorktreeId, s.sessionId))
  );

  const unpinnedSessions = $derived(
    visibleSessions.filter(s => !isPinned(anchorWorktreeId, s.sessionId))
  );

  const dateGroups = $derived(groupByDate(unpinnedSessions));
  // 当前 scope 内的 session 总量——按 worktreeFilter 派生（spec
  // sidebar-navigation §"会话总数显示口径"）：filter=ALL → group 全集；
  // filter=具体 wt → 该 wt 集合。fallback 仅在 selectedGroup 暂未就绪
  // 的早期渲染窗口（loadProjects 未完成）兜底。loadMore 累加 sessions
  // 不影响 scopeTotal——derived 直接消费 list_repository_groups 已返回的
  // RepositoryGroup.totalSessions / Worktree.sessions.length。
  const scopeTotal = $derived.by(() => {
    if (worktreeFilter === ALL_WORKTREES) {
      return selectedGroup?.totalSessions ?? sessions.length;
    }
    const wt = groupWorktrees.find((w) => w.id === worktreeFilter);
    return wt?.sessions.length ?? sessions.length;
  });
  // 搜索激活时的命中数——visibleSessions 已经过 filter + hide 过滤。
  const matchCount = $derived(visibleSessions.length);
  // chip cluster 选项：「全部」chip 永远在最前（无 ⌗ 前缀）；其余按
  // 「isRepoRoot 优先 → isMainWorktree 次之 → mostRecentSession 倒序」
  // 排序（spec sidebar-navigation §"chip 数据顺序"），label 加 `⌗` 前缀
  // 与 PR-A 行内 `.session-wt-label` 的 mono 信号语言对齐。排序责任归
  // Sidebar 调用方，WorktreeChipCluster 子组件按传入顺序渲染。
  const chipOptions = $derived.by(() => {
    const sorted = groupWorktrees.slice().sort((a, b) => {
      if ((a.isRepoRoot ?? false) !== (b.isRepoRoot ?? false)) {
        return a.isRepoRoot ? -1 : 1;
      }
      if (a.isMainWorktree !== b.isMainWorktree) {
        return a.isMainWorktree ? -1 : 1;
      }
      return (b.mostRecentSession ?? 0) - (a.mostRecentSession ?? 0);
    });
    return [
      { value: ALL_WORKTREES, label: "全部" },
      // path / name 透传给 ChipOption 让 WorktreeChipCluster 能挂右键菜单
      // （spec sidebar-navigation Phase 2 / Task 9.5）
      ...sorted.map((wt) => ({
        value: wt.id,
        label: `⌗${wt.name}`,
        path: wt.path,
        name: wt.name,
      })),
    ];
  });
  const hiddenCount = $derived(getHiddenCount(anchorWorktreeId));
  const memoryCount = $derived.by(() => projectMemory ? projectMemory.count : 0);
  const sidebarWidth = $derived(getSidebarWidth());

  // ---------------------------------------------------------------------------
  // Flat virtual list：把 PINNED 与日期分组摊平为单一 windowing 容器
  // ---------------------------------------------------------------------------

  type FlatItem =
    | { kind: "header"; key: string; label: string; count: number }
    | { kind: "session"; key: string; session: SessionSummary; pinned: boolean };

  const flatItems = $derived.by<FlatItem[]>(() => {
    const items: FlatItem[] = [];
    if (pinnedSessions.length > 0) {
      items.push({ kind: "header", key: "h:PINNED", label: "PINNED", count: pinnedSessions.length });
      for (const s of pinnedSessions) {
        items.push({ kind: "session", key: s.sessionId, session: s, pinned: true });
      }
    }
    for (const group of dateGroups) {
      items.push({ kind: "header", key: `h:${group.label}`, label: group.label, count: group.sessions.length });
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
  {#if selectedGroupId && memoryCount > 0}
    <button
      class="memory-entry"
      onclick={() => openMemoryTab(memoryAnchorWorktreeId, "Memory")}
    >
      <svg class="memory-entry-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        {@html BOOK_OPEN_TEXT_SVG}
      </svg>
      <span>Memory ({memoryCount})</span>
    </button>
  {/if}

  <!-- Session filter + count
       始终在 selectedGroupId 存在时渲染（不再因 sessionsLoading 隐藏）：
       loading 期间整条 bar 隐藏会让下方 session-list 顶部上移约 40 px，
       IPC return 后重新出现 → 整个列表跳一下。SkeletonList 在 .session-list
       内承载 loading 视觉，filter-bar 保持 DOM 稳定不参与显隐。
       count span 在 sessionsLoading 时仍隐藏避免显 "0" 误导用户。 -->
  {#if selectedGroupId}
    {#if showWorktreeFilter}
      <!-- 多 worktree group 顶部 worktree filter chip cluster（spec
           sidebar-navigation §"Worktree filter chip cluster for multi-worktree
           group"）：所有 wt 一眼可见 + 一次点击切换；切 chip 通过 worktreeFilter
           state → $effect → loadSessions 链路重拉，cursor 让非选 wt Exhausted。
           chip 顺序（「全部」→ isRepoRoot → 其余按 mostRecentSession 倒序）由
           Sidebar 端 chipOptions derived 维护，子组件按传入顺序渲染。 -->
      <div class="worktree-filter-bar">
        <WorktreeChipCluster
          ariaLabel="按 worktree 过滤会话"
          value={worktreeFilter}
          options={chipOptions}
          onChange={(v) => (worktreeFilter = v)}
        />
      </div>
    {/if}
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
        aria-describedby="session-search-hint"
        title="在已加载范围内搜索"
      />
      <span id="session-search-hint" class="visually-hidden">在已加载范围内搜索</span>
      {#if !sessionsLoading}
        <!-- 双态显示（spec §"会话总数显示口径"）：默认显单数字 scopeTotal；
             搜索激活显 `{matchCount} 匹配`。tooltip 基础一层「总 N」，
             hiddenCount > 0 时追加「· N 已隐藏」。 -->
        <span
          class="session-count-num"
          title={hiddenCount > 0 ? `总 ${scopeTotal} · ${hiddenCount} 已隐藏` : `总 ${scopeTotal}`}
        >{filterQuery ? `${matchCount} 匹配` : `${scopeTotal}`}</span>
      {/if}
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
            <span class="date-group-count" aria-label="{item.count} 条">· {item.count}</span>
          </div>
        {:else}
          {@const session = item.session}
          {@const sessionProjectId = session.worktreeId ?? selectedGroupId}
          <button
            class="session-item"
            class:session-item-active={session.sessionId === activeSessionId}
            class:session-item-hidden={isHidden(sessionProjectId, session.sessionId)}
            class:metadata-pending={!session.title && session.messageCount === 0 && !session.isOngoing}
            style:height="{ITEM_HEIGHT}px"
            data-session-id={session.sessionId}
            data-project-id={sessionProjectId}
            onclick={(e) => onSelectSession(session.sessionId, sessionProjectId, selectedGroupId, sessionLabel(session), e)}
            use:contextMenu={() => buildSessionContextItems(session)}
          >
            <div class="session-title">
              {#if session.isOngoing}
                <OngoingIndicator />
              {/if}
              {#if item.pinned}
                <svg class="pin-icon" viewBox="0 0 24 24" width="10" height="10" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                  <path d="M12 17v5"/>
                  <path d="M9 10.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24V16a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2v-.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V7a1 1 0 0 1 1-1 2 2 0 0 0 0-4H8a2 2 0 0 0 0 4 1 1 0 0 1 1 1z"/>
                </svg>
              {/if}
              <span class="session-title-text" title={session.title || session.sessionId}>
                {session.title || session.sessionId}
              </span>
            </div>
            <div class="session-meta" class:session-meta-multi-wt={showWorktreeFilter && session.worktreeName}>
              <span class="session-msg-count">
                <svg class="meta-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d={MESSAGE_SQUARE} /></svg>
                {session.messageCount || 0}
              </span>
              <span class="session-time">{formatTime(session.timestamp)}</span>
              {#if showWorktreeFilter && session.worktreeName}
                <span class="session-wt-label" title={session.worktreeName}>⌗{session.worktreeName}</span>
              {/if}
            </div>
          </button>
        {/if}
      {/each}
      <div class="vlist-spacer" style:height="{vlist.bottomSpacer()}px"></div>
      {#if sessionsLoadingMore}
        <div class="load-more-row load-more-loading">加载中…</div>
      {:else if sessionsNextCursor}
        <button
          class="load-more-row load-more-btn"
          onclick={() => void loadMoreSessions()}
          aria-label="加载更多会话，剩余 {Math.max(scopeTotal - sessions.length, 1)} 条"
        >
          <svg viewBox="0 0 24 24" width="11" height="11" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <polyline points="6 9 12 15 18 9"/>
          </svg>
          <span>加载更多 · 剩 {Math.max(scopeTotal - sessions.length, 1)} 条</span>
        </button>
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
    /* 持久选中 indicator 走通用强调边框灰 —— 比 text-secondary 再浅一档，更
       克制（DESIGN.md `The Persistent Selection Is Quiet Rule`）。selection
       由「bg surface-overlay 加深 + title 字重 600 + indicator」三层共同表达，
       indicator 本身不必抢眼；用 border-emphasis 对齐项目通用「分隔/边框」灰
       色族（同 --card-separator），与 sidebar bg ~2.x:1 仍清晰可辨。 */
    --sidebar-active-indicator: var(--color-border-emphasis);
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
    --sidebar-pinned: #a5b4fc;
  }

  @media (prefers-color-scheme: dark) {
    :global([data-theme="system"]) .sidebar {
      --sidebar-accent: #93c5fd;
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

  /* Memory entry：group 级入口（PR #210 spec：anchor 锁 group repo 根 worktree，
     不随 worktree filter 漂）。视觉降级 46px → 32px：从"主行动按钮"改为"轻量
     入口 chip"，与下方 worktree filter / search bar 统一为顶部 region 三行
     紧凑布局；group 维度的语义通过仍保持独立一行表达，不和 worktree filter
     合并（合并会让用户误解 Memory 是 per-worktree 的，违反 spec）。 */
  .memory-entry {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 32px;
    width: calc(100% - 16px);
    margin: 8px 8px 0;
    padding: 0 10px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: var(--color-text);
    font: inherit;
    font-size: 12px;
    font-weight: 500;
    text-align: left;
    cursor: pointer;
    box-sizing: border-box;
  }

  .memory-entry:hover {
    background: var(--tool-item-hover-bg);
  }

  .memory-entry-icon {
    width: 14px;
    height: 14px;
    flex-shrink: 0;
    color: var(--color-text-muted);
  }

  /* 多 worktree group filter chip cluster 容器：与 Memory entry / Session
     search bar 同行高族（32 px），padding 与 .session-filter-bar 对齐；底部
     不留分隔线避免与下方 session-filter-bar 之间出现双线。chip cluster 子
     组件自带横向滚动 + 右侧 fade mask（spec §"chip overflow 处理"）。 */
  .worktree-filter-bar {
    display: flex;
    align-items: center;
    height: 32px;
    padding: 4px 12px 0;
    /* min-width 0 让内层 chip cluster 可触发 overflow-x:auto 而不是把
       sidebar 撑出去——flex 子项默认 min-width:auto 不收缩。 */
    min-width: 0;
  }
  .worktree-filter-bar :global(.worktree-chip-cluster) {
    flex: 1;
    min-width: 0;
  }

  /* a11y 隐藏文本（aria-describedby 引用目标）：视觉不渲染但屏幕阅读器
     可读。沿用 WAI-ARIA 推荐的 visually-hidden 模式。 */
  .visually-hidden {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border: 0;
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
    /* bottom 8 给末项留呼吸感：原依赖 .load-more-end footer（已删）撑出
       与底部 sidebar-status 之间的间距，移除后由 padding 直接接住。 */
    padding: 4px 8px 8px;
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

  /* 列表底部 load-more / loading 二态行：
     - load-more-btn: 可点击，显 "▼ 加载更多 · 剩 N 条"
     - load-more-loading: 加载中文本（不可点）
     已到底（cursor=null）不渲染——列表自然结束即终态信号，与 IDE / Linear
     等工具习惯一致；group label 已承载 PINNED/TODAY/YESTERDAY 段总数。 */
  .load-more-row {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    margin: 4px 8px 8px;
    padding: 8px;
    border-radius: 6px;
    font-size: 11px;
    line-height: 1.2;
    text-align: center;
  }

  .load-more-btn {
    background: transparent;
    border: none;
    color: var(--sidebar-accent);
    font: inherit;
    font-size: 11px;
    cursor: pointer;
    transition: background 0.12s;
  }

  .load-more-btn:hover {
    background: var(--tool-item-hover-bg);
  }

  .load-more-btn:focus-visible {
    outline: 2px solid var(--color-accent-blue);
    outline-offset: 1px;
  }

  .load-more-loading {
    color: var(--color-text-muted);
    font-variant-numeric: tabular-nums;
    cursor: default;
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

  /* group 计数：标识"该 group 当前可见多少条"，让用户对 PINNED / TODAY /
     YESTERDAY 等分组规模有数感。计数受 worktreeFilter / filterQuery /
     showHidden 联动影响（用 group.sessions.length 实时反映）。视觉降级：
     不抢 label 主色，用 muted 与 label 拉开权重。 */
  .date-group-count {
    color: var(--color-text-muted);
    font-weight: 400;
    font-variant-numeric: tabular-nums;
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
    /* 兜底防 WKWebView smart-select：右键时光标下"词"被自动选中。
       use:contextMenu 的 mousedown 防护是主防线，CSS 是双保险——
       会话标题 / metadata 不是用户用来选词复制的内容（复制 sessionId /
       恢复命令均从右键菜单走）。spec frontend-context-menu 第 8 个
       Requirement。 */
    user-select: none;
    -webkit-user-select: none;
  }

  .session-item:hover {
    background: var(--tool-item-hover-bg);
  }

  /* 选中态：左 2px 暖中性 indicator + surface-overlay 加深背景 +
     标题字重 600。**完全去蓝**——sidebar 列表选中是"持久态"（用户
     切换后一直存在直到下次切换），不是"瞬时焦点"，不应使用 Focus
     Blue。Focus Blue 留给瞬时焦点 + ongoing/live（DESIGN.md
     `The Persistent Selection Is Quiet Rule` + `The Ongoing Owns
     Blue Rule`）。历史迭代：原 1px 蓝 outline → 2px 蓝 indicator →
     2px 暖中性 indicator —— 前两版蓝色都让选中行视觉权重持续盖过
     SessionDetail 头部主标题；本版让 sidebar 完全脱离 Focus Blue
     语义。
     三通道信号（任两条 ≥3:1 即满足"持久选中"合规模式）：
     - 暖中性 indicator (#6b6964 浅 / #a8a5a0 深) on sidebar bg
       ≈4.5:1 / 5.2:1，超 WCAG 1.4.11 非文本 3:1。
     - bg surface-overlay 比 hover 加深一档，提供 tonal layering。
     - title 字重 600，与 hover 的默认 400 拉开。
     box-shadow inset 不占 box-model 空间，不触发 reflow。 */
  .session-item-active {
    background: var(--color-surface-overlay);
    box-shadow: inset 2px 0 0 var(--sidebar-active-indicator);
  }
  .session-item-active .session-title-text {
    color: var(--color-text);
    font-weight: 600;
  }

  .session-item-hidden {
    opacity: 0.5;
  }

  /* 骨架占位"加载中"语义：静态 opacity 0.55 + 静态 linear-gradient 占位
     背景，让用户感知"未加载"在视觉上与真值有层次差，但不通过周期动画提示
     "加载中"——遵循 PRODUCT.md::Design Principle 5「实时但不闪烁，避免
     loading 中间态打断阅读」与 DESIGN.md::The One Live Signal Rule 边界
     条款（DESIGN.md:198）「Skeleton placeholder 必须**静态** opacity 占位，
     **禁用** shimmer」。Metadata patch 到达后移除 `.metadata-pending`，子
     元素 `transition: opacity 150ms ease-out`（见 .session-title-text /
     .session-meta 基础规则）让真值 fade-in。
     Spec `sidebar-navigation/spec.md::Metadata 占位字段视觉渐显`。 */
  .session-item.metadata-pending .session-title-text,
  .session-item.metadata-pending .session-meta {
    opacity: 0.55;
    background: linear-gradient(
      90deg,
      transparent 0%,
      var(--color-surface-overlay) 50%,
      transparent 100%
    );
    background-size: 200% 100%;
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
    /* metadata patch 到达 → `.metadata-pending` class 被移除 → opacity
       从 0.55 → 1 通过此 transition 渐显。spec `sidebar-navigation/
       spec.md::Metadata 占位字段视觉渐显::Metadata patch 到达后字段渐显`
       的 SHALL 句直接落点。 */
    transition: opacity 150ms ease-out;
  }

  /* 行内 pin 标识：与顶部 PINNED group label 的 📌 同色（`--sidebar-pinned`
     pin indigo），保持 DESIGN.md `Pin / Hide 等专属状态指示器使用各自专属
     颜色（pin indigo）` 的颜色契约（codex 二审 PR #215 finding #2）；
     accent-blue 不能用（已被 ongoing / 瞬时焦点 / 链接占用，DESIGN.md
     `The Ongoing Owns Blue Rule`），text-muted 也不行（弱化 pin 专属语义）。
     尺寸 12→10px 单独降权重——pin icon 在 PINNED group label 已带 📌 的
     前提下是冗余 fallback（group label 滚出视口时识别）。 */
  .pin-icon {
    flex-shrink: 0;
    color: var(--sidebar-pinned);
  }

  /* meta 行：grid 三列 [count fixed | time fixed | wt flex]。固定前两列
     宽度让所有行的 💬count 与 time 严格列对齐，wt label 末尾吸收剩余宽度
     可截断——改自旧 flex 布局，旧版 4 个元素 free-flex 在窄宽下 branch +
     path-chip 互相挤压、msgCount / time 水平位置不齐。
     gitBranch / cwdRelativeToRepoRoot 两字段不再在 sidebar 行展示——前者
     与 worktreeName 90%+ 重叠（git worktree 设计本意），后者已被 worktree
     label 表达"哪个 worktree / 子目录"；详细信息留在 SessionDetail。 */
  .session-meta {
    display: grid;
    grid-template-columns: [count] 28px [time] auto;
    column-gap: 10px;
    align-items: center;
    line-height: 1.2;
    min-width: 0;
    /* 与 .session-title-text 同源——`.metadata-pending` 移除时 opacity
       0.55 → 1 通过 transition 渐显。 */
    transition: opacity 150ms ease-out;
  }

  /* 多 worktree group：加第三列 wt label，flex 吸收剩余宽度 + 末尾截断。
     单 wt group 沿用基础 .session-meta 两列布局（用户已知所在 wt，行内
     不必再标）。 */
  .session-meta-multi-wt {
    grid-template-columns: [count] 28px [time] auto [wt] minmax(0, 1fr);
  }

  .session-msg-count {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    font-size: 10px;
    color: var(--color-text-muted);
    font-variant-numeric: tabular-nums;
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
    white-space: nowrap;
  }

  /* worktree label：多 wt group 行末位置，承担"哪个 worktree"识别。从尾
     截断（保留前缀），因为用户对 worktree 名的记忆主体在前段（feat-sidebar /
     hotfix-urgent）；不用 RTL 反向截断（branch 才需要保留末段 task 信息）。 */
  .session-wt-label {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--color-text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
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

<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import Sidebar from "./components/Sidebar.svelte";
  import CommandPalette from "./components/CommandPalette.svelte";
  import PaneContainer from "./components/layout/PaneContainer.svelte";
  import UnifiedTitleBar from "./components/UnifiedTitleBar.svelte";
  import ToastContainer from "./components/ToastContainer.svelte";
  import WorkspaceIndicator from "./lib/components/WorkspaceIndicator.svelte";
  import ContextSwitchOverlay from "./lib/components/ContextSwitchOverlay.svelte";
  import { updateStore, type UpdateAvailablePayload } from "./lib/updateStore.svelte";
  import {
    loadProjectData,
    getProjectData,
    isProjectDataLoading,
  } from "./lib/projectDataStore.svelte";
  import type { ProjectInfo, RepositoryGroup } from "./lib/api";
  import {
    openSessionTab,
    setSessionClickBehavior,
    type SessionClickBehavior,
    getActiveTab,
    getActiveTabId,
    getTabs,
    setActiveTab,
    closeTab,
    getPaneLayout,
    getFocusedPaneId,
    focusPane,
    splitPane,
  } from "./lib/tabStore.svelte";
  import { MAX_PANES } from "./lib/paneTypes";
  import { getConfig, isRunningUnderRosetta } from "./lib/api";
  import { refreshUnreadCount } from "./lib/notificationStore.svelte";
  import { applyTheme } from "./lib/theme";
  import { applyFonts } from "./lib/fonts";
  import { setTimeFormat } from "./lib/displayPrefs.svelte";
  import { loadAgentConfigs } from "./lib/agentConfigsStore.svelte";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { initFileChangeStore, registerHandler, unregisterHandler } from "./lib/fileChangeStore.svelte";
  import { subscribeEvent } from "./lib/transport";
  import { getSidebarCollapsed, toggleSidebarCollapsed } from "./lib/sidebarStore.svelte";
  import { attachExternalLinkInterceptor } from "./lib/externalLinks";
  import { initializeJobs, cleanupJobs } from "./lib/jobsStore.svelte";
  import { bootstrapOverrides } from "./lib/keyboard/customization";
  import { registerAppShortcuts } from "./lib/keyboard/register-app-shortcuts";
  import { setMenuSettings } from "./lib/contextMenu/settings.svelte";

  let selectedGroupId: string = $state("");
  let selectedProjectName: string = $state("");
  let commandPaletteOpen = $state(false);
  let unlistenNotif: UnlistenFn | null = null;
  let unlistenNotifAdded: UnlistenFn | null = null;
  let unlistenUpdater: UnlistenFn | null = null;
  let detachExternalLinks: (() => void) | null = null;
  let unregisterShortcuts: (() => void) | null = null;
  let notificationPollTimer: ReturnType<typeof setInterval> | undefined;
  // macOS 上 Tauri 进程跑 Rosetta 翻译时为 true；其他平台 / 非 Rosetta 时永远 false
  let rosettaWarningVisible = $state(false);

  // UnifiedTitleBar 内 ProjectSwitcher 数据源——与 Sidebar 共享 loadProjectData
  // cache（projectDataStore 内部 in-flight dedupe + memoize），重复调用近瞬时
  let projects: ProjectInfo[] = $state([]);
  let repositoryGroups: RepositoryGroup[] = $state([]);
  // 首屏 projects 还在 fetch 时 ProjectSwitcher 应显示"加载中…"，避免一闪
  // "无项目"再切到真名（codex PR 二审 #7）
  const projectsLoading = $derived(isProjectDataLoading());

  async function refreshChromeProjects() {
    try {
      const result = await loadProjectData({ refresh: false });
      projects = result.worktreeProjects;
      repositoryGroups = result.repositoryGroups;
    } catch { /* 静默：Sidebar 内有更详细的错误处理 + UI 反馈 */ }
  }

  async function onNotificationUpdate() {
    try {
      await refreshUnreadCount();
    } catch { /* 静默 */ }
  }

  // 全局快捷键迁移到 keyboard registry（change `add-keyboard-shortcut-system`）：
  // 17 条 App-owned spec 通过 `registerAppShortcuts` 在 onMount 注册；详见
  // `ui/src/lib/keyboard/register-app-shortcuts.ts` 与 design.md::D6 边界。
  // dispatcher 的单一 document keydown listener 由 registry 自管，这里 **不再**
  // 自挂 keydown listener / 自实 IME 与 input-focus 守卫。
  function switchToTabByIndex(idx: number): boolean | void {
    const list = getTabs();
    if (idx >= list.length) return false; // 超界 → 不消费、不 preventDefault
    setActiveTab(list[idx].id);
  }

  function buildAppShortcutHandlers() {
    const handlers: Record<string, (e: KeyboardEvent) => boolean | void> = {
      "command-palette.toggle": () => {
        commandPaletteOpen = !commandPaletteOpen;
      },
      // Cmd/Ctrl + B → 切换 sidebar 折叠/展开
      // 详见 openspec/specs/sidebar-navigation/spec.md §"侧栏折叠/展开"
      "sidebar.toggle": () => {
        toggleSidebarCollapsed();
      },
      // Cmd/Ctrl + W → 关闭当前 tab；无 active tab 时不消费让浏览器原生
      "tab.close": () => {
        const activeId = getActiveTabId();
        if (!activeId) return false;
        closeTab(activeId);
      },
      // Cmd/Ctrl + ] → 下一个 tab（循环）
      "tab.next": () => {
        const list = getTabs();
        if (list.length === 0) return false;
        const activeId = getActiveTabId();
        const currentIdx = activeId ? list.findIndex((t) => t.id === activeId) : -1;
        if (currentIdx === -1) return false;
        const nextIdx = (currentIdx + 1) % list.length;
        setActiveTab(list[nextIdx].id);
      },
      // Cmd/Ctrl + [ → 上一个 tab（循环）
      "tab.prev": () => {
        const list = getTabs();
        if (list.length === 0) return false;
        const activeId = getActiveTabId();
        const currentIdx = activeId ? list.findIndex((t) => t.id === activeId) : -1;
        if (currentIdx === -1) return false;
        const nextIdx = (currentIdx - 1 + list.length) % list.length;
        setActiveTab(list[nextIdx].id);
      },
      // Cmd/Ctrl + \ → split focused pane 的 activeTab 到右侧（新 pane）
      "pane.split": () => {
        const layout = getPaneLayout();
        if (layout.panes.length >= MAX_PANES) return false;
        const focusedId = getFocusedPaneId();
        const activeId = getActiveTabId();
        if (!activeId) return false;
        splitPane(focusedId, activeId, "right");
      },
      // Cmd/Ctrl + Option/Alt + → → focus 下一个 pane（循环）
      "pane.focus.next": () => {
        const layout = getPaneLayout();
        if (layout.panes.length <= 1) return false;
        const idx = layout.panes.findIndex((p) => p.id === layout.focusedPaneId);
        if (idx === -1) return false;
        const nextIdx = (idx + 1) % layout.panes.length;
        focusPane(layout.panes[nextIdx].id);
      },
      // Cmd/Ctrl + Option/Alt + ← → focus 上一个 pane（循环）
      "pane.focus.prev": () => {
        const layout = getPaneLayout();
        if (layout.panes.length <= 1) return false;
        const idx = layout.panes.findIndex((p) => p.id === layout.focusedPaneId);
        if (idx === -1) return false;
        const nextIdx = (idx - 1 + layout.panes.length) % layout.panes.length;
        focusPane(layout.panes[nextIdx].id);
      },
    };
    // tab.switch.1 ~ tab.switch.9（1-based；超界 → return false）
    for (let n = 1; n <= 9; n += 1) {
      const idx = n - 1;
      handlers[`tab.switch.${n}`] = () => switchToTabByIndex(idx);
    }
    return handlers;
  }

  const openCommandPalette = () => {
    commandPaletteOpen = true;
  };

  onMount(async () => {
    // 先 bootstrap 用户 overrides 进 registry pendingOverrides，再 register；
    // register 时自动用 override 替代 defaultBinding。IPC 失败 → registry 走
    // builtin defaults + setConfigLoadError（详 customization.ts::bootstrapOverrides）。
    await bootstrapOverrides();
    unregisterShortcuts = registerAppShortcuts(buildAppShortcutHandlers());
    window.addEventListener("cdt-open-command-palette", openCommandPalette);
    // 拦截 markdown 内的外链点击，走系统默认浏览器而非 webview 窗口内导航
    detachExternalLinks = attachExternalLinkInterceptor();
    // 监听后端 notification-update 事件（mark-as-read 后刷新 badge）
    unlistenNotif = await subscribeEvent("notification-update", onNotificationUpdate);
    // 监听自动通知管线新产生的通知：立即刷新 badge + 请求前台页面 reload 列表
    unlistenNotifAdded = await subscribeEvent("notification-added", onNotificationUpdate);
    // 监听后端启动检查 emit 的 updater://available 事件，写入 store 弹横幅
    unlistenUpdater = await subscribeEvent<UpdateAvailablePayload>(
      "updater://available",
      (e) => updateStore.showAvailable(e.payload)
    );
    try {
      const config = await getConfig();
      applyTheme(config.general.theme);
      applyFonts(config);
      const behavior = config.general.sessionClickBehavior;
      if (behavior === "replace" || behavior === "new-tab") {
        setSessionClickBehavior(behavior as SessionClickBehavior);
      }
      const tf = config.display?.timeFormat;
      if (tf === "24h" || tf === "12h") {
        setTimeFormat(tf);
      } else {
        setTimeFormat("24h");
      }
      // 同步 context menu 用 settings 快照（spec frontend-context-menu Phase 2
      // / D6）：让 selectionMenu ctxProvider + 各 surface 组件读取的快照一致
      setMenuSettings(config.general);
    } catch {
      // 加载失败保持默认浅色 + 默认字体；显式复位 timeFormat 防 HMR / 重挂载场景
      // 下模块级 $state 残留前一次 setTimeFormat("12h") 的旧值（codex CR #1）
      setTimeFormat("24h");
    }
    // 加载 agent configs 供 subagent 彩色 badge 使用
    await loadAgentConfigs();
    // 单例 listen("file-change") —— 路由组件通过 fileChangeStore 注册 handler
    await initFileChangeStore();
    // 全局兜底：结构性 file-change 事件刷新 projectDataStore，确保无论哪个
    // 页面 mounted（DashboardView / SessionDetail / Settings），ProjectSwitcher
    // 下拉和项目列表都能及时更新。与 Sidebar/DashboardView handler 共用
    // projectDataStore inflight dedupe，不会产生额外 IPC。
    registerHandler("app-global-projects", (payload) => {
      if (payload.projectListChanged || payload.sessionListChanged || payload.deleted) {
        void loadProjectData({ refresh: true }).catch(() => {});
      }
    });
    // 初始化 Background Jobs store（TitleBar 入口依赖 jobsDirExists 判断是否渲染）
    await initializeJobs();
    // 启动时同步一次 Dock badge（显示持久化的未读数）
    await onNotificationUpdate();
    // 主路径走 push event：`notification-update`（mark-as-read）+ `notification-added`
    // （新通知）订阅在 onMount 顶部完成。这里只保留 5 min 兜底轮询防 event 丢失
    // （Tauri runtime listener 失效 / 浏览器 SSE 重连窗口期错过 notification-update
    // 等少数路径）。30s 高频轮询会阻止 WKWebView 进入 idle（power nap）→ idle 1 min
    // 主线程 mach_msg wait 占比受影响，反 perf.md "辅助工具 idle 稳态 < 2%" 约束
    // （详 issue #258）。
    notificationPollTimer = setInterval(onNotificationUpdate, 300000);
    // Rosetta 翻译运行检测：Apple Silicon 上跑 Intel binary 时提示用户换 ARM 包。
    // localStorage 内 dismissed 状态由 RosettaStatusIcon 自身管理。
    try {
      rosettaWarningVisible = await isRunningUnderRosetta();
    } catch { /* 调用失败静默：icon 默认不显示 */ }

    // 首次加载 chrome 内 ProjectSwitcher 数据；后续由 Sidebar 触发的
    // loadProjectData 经 projectDataStore 内 cache 自动同步（同一模块级 data
    // 引用），App 这边 $effect 监听 getProjectData() 同步本地副本
    await refreshChromeProjects();

    // dev/test 信号：所有异步初始化完毕（含 initializeJobs → jobsDirExists 就绪）。
    // Playwright 等 __cdtReady 而非 __cdtTest——后者在 mount 前就注入，此时 TitleBar
    // 的条件渲染（如 jobs icon）可能还没数据。
    if (import.meta.env.DEV) {
      (window as unknown as Record<string, unknown>).__cdtReady = true;
    }
  });

  // Sidebar 也调 loadProjectData，模块级 cache 更新后这边通过 effect 同步
  $effect(() => {
    const cached = getProjectData();
    if (cached) {
      projects = cached.worktreeProjects;
      repositoryGroups = cached.repositoryGroups;
    }
  });

  onDestroy(() => {
    unregisterShortcuts?.();
    unregisterHandler("app-global-projects");
    window.removeEventListener("cdt-open-command-palette", openCommandPalette);
    unlistenNotif?.();
    unlistenNotifAdded?.();
    unlistenUpdater?.();
    detachExternalLinks?.();
    cleanupJobs();
    if (notificationPollTimer) clearInterval(notificationPollTimer);
  });

  const activeTab = $derived(getActiveTab());

  function selectProject(id: string, name: string) {
    selectedGroupId = id;
    selectedProjectName = name;
  }

  function selectSession(
    sessionId: string,
    projectId: string,
    groupId: string,
    label: string,
    event: MouseEvent,
  ) {
    // Cmd/Ctrl + 点击翻转 sessionClickBehavior 默认（对齐 Chrome）
    // 按 change `simplify-repository-as-project::D7` 双 id 分层：
    // detail API 入参用 worktree id (projectId)；sidebar 高亮关联 group id。
    const forceNewTab = event.ctrlKey || event.metaKey;
    openSessionTab(
      sessionId,
      projectId,
      label || sessionId,
      forceNewTab ? { forceNewTab: true, groupId } : { groupId },
    );
  }
</script>

<div class="app-root">
  <UnifiedTitleBar
    {projects}
    {repositoryGroups}
    {selectedGroupId}
    projectsLoading={projectsLoading}
    onSelectProject={selectProject}
    rosettaVisible={rosettaWarningVisible}
  />
  <div class="app-layout">
    <!-- 始终挂载 Sidebar（用 CSS width:0 收起，不用 {#if} 销毁/重建）：
         避免每次 toggle 都 destroy → ResizeObserver 重测量 → vlist 空→填充
         的视觉闪烁。展开/收起入口现在统一在 UnifiedTitleBar 折叠按钮。 -->
    <Sidebar
      {selectedGroupId}
      activeSessionId={activeTab?.sessionId ?? null}
      collapsed={getSidebarCollapsed()}
      onSelectProject={selectProject}
      onSelectSession={selectSession}
    />

    <div class="main-area">
      <!-- PaneContainer / CommandPalette 的 Props prop 名仍是 selectedProjectId
           （per design D7：组件内部 detail API / per-project state 路径用
           tab.projectId 即 worktree id；这里传 group id 作为"当前 group 视角"
           显示标识，字符串值与单 worktree group 的 project id 字符串相同）。 -->
      <PaneContainer
        selectedProjectId={selectedGroupId}
        onSelectProject={selectProject}
      />
    </div>
  </div>
  <ToastContainer />
</div>

<WorkspaceIndicator />
<ContextSwitchOverlay />

{#if commandPaletteOpen}
  <CommandPalette
    selectedProjectId={selectedGroupId}
    onSelectProject={selectProject}
    onClose={() => { commandPaletteOpen = false; }}
  />
{/if}

<style>
  .app-root {
    display: flex;
    flex-direction: column;
    height: 100vh;
    overflow: hidden;
  }

  .app-layout {
    display: flex;
    flex: 1;
    overflow: hidden;
    min-height: 0;
  }

  .main-area {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    min-width: 0;
  }
</style>

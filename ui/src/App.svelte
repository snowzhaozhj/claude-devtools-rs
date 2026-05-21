<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import Sidebar from "./components/Sidebar.svelte";
  import CommandPalette from "./components/CommandPalette.svelte";
  import PaneContainer from "./components/layout/PaneContainer.svelte";
  import UnifiedTitleBar from "./components/UnifiedTitleBar.svelte";
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
  import { initFileChangeStore } from "./lib/fileChangeStore.svelte";
  import { subscribeEvent } from "./lib/transport";
  import { getSidebarCollapsed, toggleSidebarCollapsed } from "./lib/sidebarStore.svelte";
  import { attachExternalLinkInterceptor } from "./lib/externalLinks";

  let selectedGroupId: string = $state("");
  let selectedProjectName: string = $state("");
  let commandPaletteOpen = $state(false);
  let unlistenNotif: UnlistenFn | null = null;
  let unlistenNotifAdded: UnlistenFn | null = null;
  let unlistenUpdater: UnlistenFn | null = null;
  let detachExternalLinks: (() => void) | null = null;
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

  function handleGlobalKeydown(e: KeyboardEvent) {
    if (!(e.metaKey || e.ctrlKey)) return;

    if (e.key === "k") {
      e.preventDefault();
      commandPaletteOpen = !commandPaletteOpen;
      return;
    }

    // Cmd/Ctrl + B → 切换 sidebar 折叠/展开
    // 详见 openspec/specs/sidebar-navigation/spec.md §"侧栏折叠/展开"
    if (e.key === "b") {
      e.preventDefault();
      toggleSidebarCollapsed();
      return;
    }

    // Cmd/Ctrl + 1~9 → 切到对应索引的 tab（1-based；超界忽略）
    if (/^[1-9]$/.test(e.key)) {
      const idx = Number(e.key) - 1;
      const list = getTabs();
      if (idx < list.length) {
        e.preventDefault();
        setActiveTab(list[idx].id);
      }
      return;
    }

    // Cmd/Ctrl + W → 关闭当前 tab
    if (e.key === "w") {
      const activeId = getActiveTabId();
      if (activeId) {
        e.preventDefault();
        closeTab(activeId);
      }
      return;
    }

    // Cmd/Ctrl + [ / ] → 上一个 / 下一个 tab（循环）
    if (e.key === "[" || e.key === "]") {
      const list = getTabs();
      if (list.length === 0) return;
      const activeId = getActiveTabId();
      const currentIdx = activeId ? list.findIndex((t) => t.id === activeId) : -1;
      if (currentIdx === -1) return;
      e.preventDefault();
      const nextIdx = e.key === "["
        ? (currentIdx - 1 + list.length) % list.length
        : (currentIdx + 1) % list.length;
      setActiveTab(list[nextIdx].id);
      return;
    }

    // Cmd/Ctrl + \ → split focused pane 的 activeTab 到右侧（新 pane）
    if (e.key === "\\") {
      const layout = getPaneLayout();
      if (layout.panes.length >= MAX_PANES) return;
      const focusedId = getFocusedPaneId();
      const activeId = getActiveTabId();
      if (!activeId) return;
      e.preventDefault();
      splitPane(focusedId, activeId, "right");
      return;
    }

    // Cmd/Ctrl + Option/Alt + ← / → → focus 上/下一个 pane（循环）
    if (e.altKey && (e.key === "ArrowLeft" || e.key === "ArrowRight")) {
      const layout = getPaneLayout();
      if (layout.panes.length <= 1) return;
      const idx = layout.panes.findIndex((p) => p.id === layout.focusedPaneId);
      if (idx === -1) return;
      e.preventDefault();
      const nextIdx = e.key === "ArrowLeft"
        ? (idx - 1 + layout.panes.length) % layout.panes.length
        : (idx + 1) % layout.panes.length;
      focusPane(layout.panes[nextIdx].id);
      return;
    }
  }

  const openCommandPalette = () => {
    commandPaletteOpen = true;
  };

  onMount(async () => {
    document.addEventListener("keydown", handleGlobalKeydown);
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
    } catch {
      // 加载失败保持默认浅色 + 默认字体；显式复位 timeFormat 防 HMR / 重挂载场景
      // 下模块级 $state 残留前一次 setTimeFormat("12h") 的旧值（codex CR #1）
      setTimeFormat("24h");
    }
    // 加载 agent configs 供 subagent 彩色 badge 使用
    await loadAgentConfigs();
    // 单例 listen("file-change") —— 路由组件通过 fileChangeStore 注册 handler
    await initFileChangeStore();
    // 启动时同步一次 Dock badge（显示持久化的未读数）
    await onNotificationUpdate();
    notificationPollTimer = setInterval(onNotificationUpdate, 30000);
    // Rosetta 翻译运行检测：Apple Silicon 上跑 Intel binary 时提示用户换 ARM 包。
    // localStorage 内 dismissed 状态由 RosettaStatusIcon 自身管理。
    try {
      rosettaWarningVisible = await isRunningUnderRosetta();
    } catch { /* 调用失败静默：icon 默认不显示 */ }

    // 首次加载 chrome 内 ProjectSwitcher 数据；后续由 Sidebar 触发的
    // loadProjectData 经 projectDataStore 内 cache 自动同步（同一模块级 data
    // 引用），App 这边 $effect 监听 getProjectData() 同步本地副本
    await refreshChromeProjects();
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
    document.removeEventListener("keydown", handleGlobalKeydown);
    window.removeEventListener("cdt-open-command-palette", openCommandPalette);
    unlistenNotif?.();
    unlistenNotifAdded?.();
    unlistenUpdater?.();
    detachExternalLinks?.();
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
      label || sessionId.slice(0, 12),
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

<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import Sidebar from "./components/Sidebar.svelte";
  import CommandPalette from "./components/CommandPalette.svelte";
  import PaneContainer from "./components/layout/PaneContainer.svelte";
  import UpdateBanner from "./components/UpdateBanner.svelte";
  import RosettaBanner from "./components/RosettaBanner.svelte";
  import { updateStore, type UpdateAvailablePayload } from "./lib/updateStore.svelte";
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
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { initFileChangeStore } from "./lib/fileChangeStore.svelte";
  import { getSidebarCollapsed, toggleSidebarCollapsed } from "./lib/sidebarStore.svelte";
  import { attachExternalLinkInterceptor } from "./lib/externalLinks";

  let selectedProjectId: string = $state("");
  let selectedProjectName: string = $state("");
  let commandPaletteOpen = $state(false);
  let unlistenNotif: UnlistenFn | null = null;
  let unlistenNotifAdded: UnlistenFn | null = null;
  let unlistenUpdater: UnlistenFn | null = null;
  let detachExternalLinks: (() => void) | null = null;
  let notificationPollTimer: ReturnType<typeof setInterval> | undefined;
  // macOS 上 Tauri 进程跑 Rosetta 翻译时为 true；其他平台 / 非 Rosetta 时永远 false
  let rosettaWarningVisible = $state(false);

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

  onMount(async () => {
    document.addEventListener("keydown", handleGlobalKeydown);
    // 拦截 markdown 内的外链点击，走系统默认浏览器而非 webview 窗口内导航
    detachExternalLinks = attachExternalLinkInterceptor();
    // 监听后端 notification-update 事件（mark-as-read 后刷新 badge）
    unlistenNotif = await listen("notification-update", onNotificationUpdate);
    // 监听自动通知管线新产生的通知：立即刷新 badge + 请求前台页面 reload 列表
    unlistenNotifAdded = await listen("notification-added", onNotificationUpdate);
    // 监听后端启动检查 emit 的 updater://available 事件，写入 store 弹横幅
    unlistenUpdater = await listen<UpdateAvailablePayload>(
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
    // localStorage 内 banner dismissed 状态由 RosettaBanner 自身管理。
    try {
      rosettaWarningVisible = await isRunningUnderRosetta();
    } catch { /* 调用失败静默：banner 默认不显示 */ }
  });

  onDestroy(() => {
    document.removeEventListener("keydown", handleGlobalKeydown);
    unlistenNotif?.();
    unlistenNotifAdded?.();
    unlistenUpdater?.();
    detachExternalLinks?.();
    if (notificationPollTimer) clearInterval(notificationPollTimer);
  });

  const activeTab = $derived(getActiveTab());

  function selectProject(id: string, name: string) {
    selectedProjectId = id;
    selectedProjectName = name;
  }

  function selectSession(sessionId: string, label: string, event: MouseEvent) {
    // Cmd/Ctrl + 点击翻转 sessionClickBehavior 默认（对齐 Chrome）
    const forceNewTab = event.ctrlKey || event.metaKey;
    openSessionTab(
      sessionId,
      selectedProjectId,
      label || sessionId.slice(0, 12),
      forceNewTab ? { forceNewTab: true } : undefined,
    );
  }
</script>

<div class="app-root">
  <RosettaBanner visible={rosettaWarningVisible} />
  <UpdateBanner />
  <div class="app-layout">
    <!-- 始终挂载 Sidebar（用 CSS width:0 收起，不用 {#if} 销毁/重建）：
         避免每次 toggle 都 destroy → ResizeObserver 重测量 → vlist 空→填充
         的视觉闪烁。展开按钮在 TabBar，关入口在 SidebarHeader collapse-btn。 -->
    <Sidebar
      {selectedProjectId}
      activeSessionId={activeTab?.sessionId ?? null}
      collapsed={getSidebarCollapsed()}
      onSelectProject={selectProject}
      onSelectSession={selectSession}
    />

    <div class="main-area">
      <PaneContainer
        {selectedProjectId}
        onSelectProject={selectProject}
      />
    </div>
  </div>
</div>

{#if commandPaletteOpen}
  <CommandPalette
    {selectedProjectId}
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

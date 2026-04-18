<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import Sidebar from "./components/Sidebar.svelte";
  import TabBar from "./components/TabBar.svelte";
  import CommandPalette from "./components/CommandPalette.svelte";
  import SessionDetail from "./routes/SessionDetail.svelte";
  import SettingsView from "./routes/SettingsView.svelte";
  import NotificationsView from "./routes/NotificationsView.svelte";
  import DashboardView from "./routes/DashboardView.svelte";
  import { openTab, getActiveTab, setUnreadCount } from "./lib/tabStore.svelte";
  import { getConfig, getNotifications } from "./lib/api";
  import { applyTheme } from "./lib/theme";
  import { loadAgentConfigs } from "./lib/agentConfigsStore.svelte";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { initFileChangeStore } from "./lib/fileChangeStore.svelte";

  let selectedProjectId: string = $state("");
  let selectedProjectName: string = $state("");
  let commandPaletteOpen = $state(false);
  let unlistenNotif: UnlistenFn | null = null;
  let unlistenNotifAdded: UnlistenFn | null = null;

  async function onNotificationUpdate() {
    try {
      const r = await getNotifications(1, 0);
      setUnreadCount(r.unreadCount);
      // 同步 macOS Dock badge（Windows 不支持，会静默失败）
      try {
        await getCurrentWindow().setBadgeCount(r.unreadCount > 0 ? r.unreadCount : undefined);
      } catch { /* 非 macOS 平台静默 */ }
    } catch { /* 静默 */ }
  }

  function handleGlobalKeydown(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key === "k") {
      e.preventDefault();
      commandPaletteOpen = !commandPaletteOpen;
    }
  }

  onMount(async () => {
    document.addEventListener("keydown", handleGlobalKeydown);
    // 监听后端 notification-update 事件（mark-as-read 后刷新 badge）
    unlistenNotif = await listen("notification-update", onNotificationUpdate);
    // 监听自动通知管线新产生的通知：立即刷新 badge + 请求前台页面 reload 列表
    unlistenNotifAdded = await listen("notification-added", onNotificationUpdate);
    try {
      const config = await getConfig();
      applyTheme(config.general.theme);
    } catch { /* 加载失败保持默认浅色 */ }
    // 加载 agent configs 供 subagent 彩色 badge 使用
    await loadAgentConfigs();
    // 单例 listen("file-change") —— 路由组件通过 fileChangeStore 注册 handler
    await initFileChangeStore();
    // 启动时同步一次 Dock badge（显示持久化的未读数）
    await onNotificationUpdate();
  });

  onDestroy(() => {
    document.removeEventListener("keydown", handleGlobalKeydown);
    unlistenNotif?.();
    unlistenNotifAdded?.();
  });

  const activeTab = $derived(getActiveTab());

  function selectProject(id: string, name: string) {
    selectedProjectId = id;
    selectedProjectName = name;
  }

  function selectSession(sessionId: string, label: string) {
    openTab(sessionId, selectedProjectId, label || sessionId.slice(0, 12));
  }
</script>

<div class="app-layout">
  <Sidebar
    {selectedProjectId}
    activeSessionId={activeTab?.sessionId ?? ""}
    onSelectProject={selectProject}
    onSelectSession={selectSession}
  />

  <div class="main-area">
    <TabBar />

    <main class="main-content">
      {#if activeTab?.type === "settings"}
        <SettingsView />
      {:else if activeTab?.type === "notifications"}
        <NotificationsView />
      {:else if activeTab?.type === "session"}
        {#key activeTab.id}
          <SessionDetail
            tabId={activeTab.id}
            projectId={activeTab.projectId}
            sessionId={activeTab.sessionId}
          />
        {/key}
      {:else}
        <DashboardView onSelectProject={selectProject} />
      {/if}
    </main>
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
  .app-layout {
    display: flex;
    height: 100vh;
    overflow: hidden;
  }

  .main-area {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    min-width: 0;
  }

  .main-content {
    flex: 1;
    overflow: hidden;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

</style>

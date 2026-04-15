<script lang="ts">
  import Sidebar from "./components/Sidebar.svelte";
  import TabBar from "./components/TabBar.svelte";
  import SessionDetail from "./routes/SessionDetail.svelte";
  import { openTab, getActiveTab } from "./lib/tabStore.svelte";

  let selectedProjectId: string = $state("");
  let selectedProjectName: string = $state("");

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
      {#if activeTab}
        {#key activeTab.id}
          <SessionDetail
            tabId={activeTab.id}
            projectId={activeTab.projectId}
            sessionId={activeTab.sessionId}
          />
        {/key}
      {:else}
        <div class="empty-state">
          <div class="empty-icon">◈</div>
          <div class="empty-title">
            {#if selectedProjectId}
              选择一个会话查看详情
            {:else}
              选择一个项目开始
            {/if}
          </div>
        </div>
      {/if}
    </main>
  </div>
</div>

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

  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    gap: 12px;
    color: var(--color-text-muted);
  }

  .empty-icon {
    font-size: 48px;
    opacity: 0.3;
  }

  .empty-title {
    font-size: 14px;
  }
</style>

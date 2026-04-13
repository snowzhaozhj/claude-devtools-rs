<script lang="ts">
  import Sidebar from "./components/Sidebar.svelte";
  import SessionDetail from "./routes/SessionDetail.svelte";

  let selectedProjectId: string = $state("");
  let selectedProjectName: string = $state("");
  let selectedSessionId: string = $state("");

  function selectProject(id: string, name: string) {
    selectedProjectId = id;
    selectedProjectName = name;
    selectedSessionId = "";
  }

  function selectSession(sessionId: string) {
    selectedSessionId = sessionId;
  }
</script>

<div class="app-layout">
  <Sidebar
    {selectedProjectId}
    {selectedSessionId}
    onSelectProject={selectProject}
    onSelectSession={selectSession}
  />

  <main class="main-content">
    {#if selectedSessionId}
      <SessionDetail projectId={selectedProjectId} sessionId={selectedSessionId} />
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

<style>
  .app-layout {
    display: flex;
    height: 100vh;
    overflow: hidden;
  }

  .main-content {
    flex: 1;
    overflow-y: auto;
    min-width: 0;
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

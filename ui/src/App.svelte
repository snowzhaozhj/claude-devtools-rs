<script lang="ts">
  import ProjectList from "./routes/ProjectList.svelte";
  import SessionList from "./routes/SessionList.svelte";
  import SessionDetail from "./routes/SessionDetail.svelte";

  let currentView: "projects" | "sessions" | "detail" = $state("projects");
  let selectedProjectId: string = $state("");
  let selectedProjectName: string = $state("");
  let selectedSessionId: string = $state("");

  function selectProject(id: string, name: string) {
    selectedProjectId = id;
    selectedProjectName = name;
    currentView = "sessions";
  }

  function selectSession(sessionId: string) {
    selectedSessionId = sessionId;
    currentView = "detail";
  }

  function goBack() {
    if (currentView === "detail") {
      currentView = "sessions";
    } else {
      currentView = "projects";
    }
  }
</script>

<main>
  <header>
    <div class="header-content">
      {#if currentView !== "projects"}
        <button class="back-btn" onclick={goBack}>← 返回</button>
      {/if}
      <h1>
        {#if currentView === "projects"}
          Claude DevTools
        {:else if currentView === "sessions"}
          {selectedProjectName}
        {:else}
          会话详情
        {/if}
      </h1>
    </div>
  </header>

  <div class="content">
    {#if currentView === "projects"}
      <ProjectList onSelect={selectProject} />
    {:else if currentView === "sessions"}
      <SessionList projectId={selectedProjectId} onSelect={selectSession} />
    {:else}
      <SessionDetail projectId={selectedProjectId} sessionId={selectedSessionId} />
    {/if}
  </div>
</main>

<style>
  :global(body) {
    margin: 0;
    padding: 0;
    background: #1a1b26;
    color: #c0caf5;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
  }

  main {
    height: 100vh;
    display: flex;
    flex-direction: column;
  }

  header {
    background: #24283b;
    border-bottom: 1px solid #3b4261;
    padding: 12px 20px;
  }

  .header-content {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  h1 {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
    color: #7aa2f7;
  }

  .back-btn {
    background: none;
    border: 1px solid #3b4261;
    color: #7aa2f7;
    padding: 4px 10px;
    border-radius: 4px;
    cursor: pointer;
    font-size: 13px;
  }

  .back-btn:hover {
    background: #3b4261;
  }

  .content {
    flex: 1;
    overflow-y: auto;
    padding: 16px 20px;
  }
</style>

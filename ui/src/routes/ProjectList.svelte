<script lang="ts">
  import { onMount } from "svelte";
  import { listProjects, type ProjectInfo } from "../lib/api";

  interface Props {
    onSelect: (id: string, name: string) => void;
  }

  let { onSelect }: Props = $props();

  let projects: ProjectInfo[] = $state([]);
  let loading = $state(true);
  let error: string | null = $state(null);

  onMount(async () => {
    try {
      projects = await listProjects();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  });

  function formatPath(path: string): string {
    return path.replace(/^\/Users\/[^/]+/, "~");
  }
</script>

<div class="project-list">
  {#if loading}
    <div class="loading">加载中...</div>
  {:else if error}
    <div class="error">{error}</div>
  {:else if projects.length === 0}
    <div class="empty">
      <p>未发现 Claude Code 项目</p>
      <p class="hint">确认 ~/.claude/projects/ 目录存在</p>
    </div>
  {:else}
    {#each projects as project}
      <button
        class="project-card"
        onclick={() => onSelect(project.id, project.displayName)}
      >
        <div class="project-name">{project.displayName}</div>
        <div class="project-path">{formatPath(project.path)}</div>
        <div class="project-meta">
          <span class="session-count">{project.sessionCount} 个会话</span>
        </div>
      </button>
    {/each}
  {/if}
</div>

<style>
  .project-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .loading, .error, .empty {
    text-align: center;
    padding: 40px;
    color: #565f89;
  }

  .error {
    color: #f7768e;
  }

  .hint {
    font-size: 13px;
    color: #565f89;
  }

  .project-card {
    display: block;
    width: 100%;
    text-align: left;
    background: #24283b;
    border: 1px solid #3b4261;
    border-radius: 8px;
    padding: 14px 16px;
    cursor: pointer;
    transition: border-color 0.15s, background 0.15s;
  }

  .project-card:hover {
    border-color: #7aa2f7;
    background: #292e42;
  }

  .project-name {
    font-size: 15px;
    font-weight: 600;
    color: #c0caf5;
    margin-bottom: 4px;
  }

  .project-path {
    font-size: 12px;
    color: #565f89;
    font-family: "SF Mono", "Fira Code", monospace;
    margin-bottom: 8px;
  }

  .project-meta {
    display: flex;
    gap: 12px;
  }

  .session-count {
    font-size: 12px;
    color: #7aa2f7;
    background: rgba(122, 162, 247, 0.1);
    padding: 2px 8px;
    border-radius: 4px;
  }
</style>

<script lang="ts">
  import { onMount } from "svelte";
  import { listProjects, type ProjectInfo } from "../lib/api";
  import { shortenPath } from "../lib/toolHelpers";
  import { FOLDER_GIT2_SVG } from "../lib/icons";

  interface Props {
    onSelectProject: (id: string, name: string) => void;
  }

  let { onSelectProject }: Props = $props();

  let projects: ProjectInfo[] = $state([]);
  let loading = $state(true);
  let filterQuery = $state("");

  onMount(async () => {
    try {
      projects = await listProjects();
    } catch (e) {
      console.error("Failed to load projects:", e);
    } finally {
      loading = false;
    }
  });

  const filtered = $derived(
    filterQuery
      ? projects.filter(p =>
          p.displayName.toLowerCase().includes(filterQuery.toLowerCase()) ||
          p.path.toLowerCase().includes(filterQuery.toLowerCase()))
      : projects
  );
</script>

<div class="dashboard">
  <div class="dashboard-inner">
    <!-- 搜索框 -->
    <div class="dash-search-wrap">
      <input
        class="dash-search"
        type="text"
        placeholder="搜索项目..."
        bind:value={filterQuery}
      />
      <kbd class="dash-kbd">⌘K</kbd>
    </div>

    <!-- 标题 -->
    <div class="dash-section-header">
      <span class="dash-section-title">
        {filterQuery ? "搜索结果" : "最近项目"}
      </span>
      <span class="dash-section-count">{filtered.length} 个项目</span>
    </div>

    <!-- 卡片网格 -->
    {#if loading}
      <div class="dash-status">加载中...</div>
    {:else if filtered.length === 0}
      <div class="dash-status">
        {filterQuery ? "无匹配项目" : "未发现项目"}
      </div>
    {:else}
      <div class="dash-grid">
        {#each filtered as project}
          <button
            class="dash-card"
            onclick={() => onSelectProject(project.id, project.displayName)}
          >
            <div class="dash-card-icon">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                {@html FOLDER_GIT2_SVG}
              </svg>
            </div>
            <div class="dash-card-name">{project.displayName}</div>
            <div class="dash-card-path">{shortenPath(project.path)}</div>
            <div class="dash-card-meta">
              {project.sessionCount} 个会话
            </div>
          </button>
        {/each}
      </div>
    {/if}
  </div>
</div>

<style>
  .dashboard {
    display: flex;
    justify-content: center;
    height: 100%;
    overflow-y: auto;
    overflow-x: hidden;
    padding: 48px 24px;
  }

  .dashboard-inner {
    width: 100%;
    max-width: 1100px;
    min-width: 0;
  }

  .dash-search-wrap {
    position: relative;
    margin-bottom: 32px;
  }

  .dash-search {
    width: 100%;
    font-size: 15px;
    font-family: inherit;
    color: var(--color-text);
    background: var(--color-surface);
    border: 1px solid var(--color-border-emphasis);
    border-radius: 10px;
    padding: 12px 16px;
    padding-right: 60px;
    outline: none;
    transition: border-color 0.15s, box-shadow 0.15s;
  }

  .dash-search:focus {
    border-color: #3b82f6;
    box-shadow: 0 0 0 3px rgba(59, 130, 246, 0.15);
  }

  .dash-search::placeholder {
    color: var(--color-text-muted);
  }

  .dash-kbd {
    position: absolute;
    right: 12px;
    top: 50%;
    transform: translateY(-50%);
    font-size: 11px;
    font-family: var(--font-mono);
    color: var(--color-text-muted);
    background: var(--badge-neutral-bg);
    padding: 2px 8px;
    border-radius: 4px;
    pointer-events: none;
  }

  .dash-section-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 12px;
  }

  .dash-section-title {
    font-size: 13px;
    font-weight: 600;
    color: var(--color-text-secondary);
  }

  .dash-section-count {
    font-size: 12px;
    color: var(--color-text-muted);
  }

  .dash-status {
    text-align: center;
    padding: 48px 0;
    color: var(--color-text-muted);
    font-size: 14px;
  }

  .dash-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
    gap: 12px;
    width: 100%;
  }

  .dash-card {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 16px;
    background: var(--color-surface);
    border: 1px solid var(--color-border);
    border-radius: 8px;
    cursor: pointer;
    text-align: left;
    font: inherit;
    color: var(--color-text);
    transition: border-color 0.15s, box-shadow 0.15s, background 0.15s;
    min-width: 0;
  }

  .dash-card:hover {
    border-color: var(--color-border-emphasis);
    background: var(--color-surface-raised);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.06);
  }

  .dash-card-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    border-radius: 6px;
    background: var(--color-surface-overlay);
    border: 1px solid var(--color-border);
    color: var(--color-text-secondary);
    margin-bottom: 8px;
  }

  .dash-card-icon svg {
    width: 16px;
    height: 16px;
  }

  .dash-card-name {
    font-size: 14px;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .dash-card-path {
    font-size: 11px;
    font-family: var(--font-mono);
    color: var(--color-text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .dash-card-meta {
    font-size: 12px;
    color: var(--color-text-secondary);
    margin-top: 4px;
  }
</style>

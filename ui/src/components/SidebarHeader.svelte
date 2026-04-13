<script lang="ts">
  import { type ProjectInfo } from "../lib/api";

  interface Props {
    projects: ProjectInfo[];
    selectedProjectId: string;
    onSelectProject: (id: string, name: string) => void;
  }

  let { projects, selectedProjectId, onSelectProject }: Props = $props();
  let dropdownOpen = $state(false);

  function toggleDropdown() {
    dropdownOpen = !dropdownOpen;
  }

  function select(p: ProjectInfo) {
    onSelectProject(p.id, p.displayName);
    dropdownOpen = false;
  }

  function formatPath(path: string): string {
    return path.replace(/^\/Users\/[^/]+/, "~");
  }

  const selectedName = $derived(
    projects.find(p => p.id === selectedProjectId)?.displayName ?? "选择项目"
  );
</script>

<div class="sidebar-header">
  <button class="project-selector" onclick={toggleDropdown}>
    <span class="project-icon">◆</span>
    <span class="project-name">{selectedName}</span>
    <span class="dropdown-arrow">{dropdownOpen ? "▴" : "▾"}</span>
  </button>

  {#if dropdownOpen}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="dropdown-backdrop" onclick={() => dropdownOpen = false}></div>
    <div class="dropdown">
      {#each projects as project}
        <button
          class="dropdown-item"
          class:dropdown-item-active={project.id === selectedProjectId}
          onclick={() => select(project)}
        >
          <span class="dropdown-item-name">{project.displayName}</span>
          <span class="dropdown-item-path">{formatPath(project.path)}</span>
        </button>
      {/each}
      {#if projects.length === 0}
        <div class="dropdown-empty">未发现项目</div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .sidebar-header {
    position: relative;
    padding: 10px 12px;
    border-bottom: 1px solid var(--color-border);
  }

  .project-selector {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px 10px;
    background: none;
    border: none;
    border-radius: 6px;
    color: var(--color-text);
    font: inherit;
    font-size: 14px;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.1s;
  }

  .project-selector:hover {
    background: var(--tool-item-hover-bg);
  }

  .project-icon {
    font-size: 10px;
    color: var(--color-text-muted);
  }

  .project-name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    text-align: left;
  }

  .dropdown-arrow {
    font-size: 10px;
    color: var(--color-text-muted);
    flex-shrink: 0;
  }

  .dropdown-backdrop {
    position: fixed;
    inset: 0;
    z-index: 9;
  }

  .dropdown {
    position: absolute;
    left: 8px;
    right: 8px;
    top: calc(100% + 4px);
    background: var(--color-surface);
    border: 1px solid var(--color-border-emphasis);
    border-radius: 8px;
    padding: 4px;
    z-index: 10;
    max-height: 300px;
    overflow-y: auto;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.12);
  }

  .dropdown-item {
    display: flex;
    flex-direction: column;
    gap: 2px;
    width: 100%;
    padding: 8px 10px;
    background: none;
    border: none;
    border-radius: 6px;
    cursor: pointer;
    text-align: left;
    font: inherit;
    color: var(--color-text);
    transition: background 0.1s;
  }

  .dropdown-item:hover {
    background: var(--tool-item-hover-bg);
  }

  .dropdown-item-active {
    background: var(--color-surface-raised);
  }

  .dropdown-item-name {
    font-size: 13px;
    font-weight: 500;
  }

  .dropdown-item-path {
    font-size: 11px;
    color: var(--color-text-muted);
    font-family: var(--font-mono);
  }

  .dropdown-empty {
    padding: 12px;
    text-align: center;
    color: var(--color-text-muted);
    font-size: 13px;
  }
</style>

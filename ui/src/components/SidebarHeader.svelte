<script lang="ts">
  import type { ProjectInfo, SessionSummary } from "../lib/api";
  import {
    CHEVRON_DOWN,
    CHECK_SVG,
    FOLDER_GIT2_SVG,
    GIT_BRANCH_SVG,
    PANEL_LEFT_SVG,
  } from "../lib/icons";

  interface Props {
    projects: ProjectInfo[];
    selectedProjectId: string;
    sessions: SessionSummary[];
    activeSessionId: string | null;
    onSelectProject: (id: string, name: string) => void;
    onToggleCollapsed: () => void;
  }

  let {
    projects,
    selectedProjectId,
    sessions,
    activeSessionId,
    onSelectProject,
    onToggleCollapsed,
  }: Props = $props();
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

  // 优先 active session 的 gitBranch；只有 active 不存在或不在列表时才回退到
  // sessions[0]（按 timestamp desc）。active 存在但其 gitBranch=null（骨架态/
  // 非 git 项目）时 SHALL 显示 active 自身的 null（即不渲染该栏），不要回退到
  // 别的 session 的 branch（codex 二审找到的 bug）。
  // 详见 openspec/specs/sidebar-navigation/spec.md §"项目 git 分支只读栏"。
  const branch = $derived.by<string | null>(() => {
    if (activeSessionId) {
      const active = sessions.find(s => s.sessionId === activeSessionId);
      if (active) return active.gitBranch;
    }
    return sessions[0]?.gitBranch ?? null;
  });
</script>

<div class="sidebar-header">
  <div class="header-row">
    <button class="project-selector" onclick={toggleDropdown}>
      <span class="project-icon">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          {@html FOLDER_GIT2_SVG}
        </svg>
      </span>
      <span class="project-name">{selectedName}</span>
      <span class="dropdown-arrow" class:dropdown-arrow-open={dropdownOpen}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d={CHEVRON_DOWN} />
        </svg>
      </span>
    </button>
    <button
      class="collapse-btn"
      title="收起侧栏 (⌘B)"
      aria-label="收起侧栏"
      onclick={onToggleCollapsed}
    >
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        {@html PANEL_LEFT_SVG}
      </svg>
    </button>
  </div>

  {#if branch}
    <div class="branch-row">
      <svg class="branch-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        {@html GIT_BRANCH_SVG}
      </svg>
      <span class="branch-name">{branch}</span>
    </div>
  {/if}

  {#if dropdownOpen}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="dropdown-backdrop" onclick={() => dropdownOpen = false}></div>
    <div class="dropdown">
      <div class="dropdown-title">切换项目</div>
      {#each projects as project}
        {@const isActive = project.id === selectedProjectId}
        <button
          class="dropdown-item"
          class:dropdown-item-active={isActive}
          onclick={() => select(project)}
        >
          <div class="dropdown-item-info">
            <span class="dropdown-item-name" class:dropdown-item-name-active={isActive}>{project.displayName}</span>
            <span class="dropdown-item-path">{formatPath(project.path)}</span>
          </div>
          <span class="dropdown-item-count">{project.sessionCount}</span>
          {#if isActive}
            <svg class="dropdown-item-check" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              {@html CHECK_SVG}
            </svg>
          {/if}
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
    padding: 8px 8px 0;
    border-bottom: 1px solid var(--color-border);
  }

  .header-row {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .project-selector {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 8px;
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
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: var(--color-text-secondary);
    flex-shrink: 0;
  }

  .project-icon svg {
    width: 16px;
    height: 16px;
  }

  .project-name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    text-align: left;
  }

  .dropdown-arrow {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: var(--color-text-secondary);
    flex-shrink: 0;
    transition: transform 0.15s ease;
  }

  .dropdown-arrow svg {
    width: 14px;
    height: 14px;
  }

  .dropdown-arrow-open {
    transform: rotate(180deg);
  }

  .collapse-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    padding: 0;
    flex-shrink: 0;
    background: none;
    border: none;
    border-radius: 6px;
    color: var(--color-text-muted);
    cursor: pointer;
    transition: background 0.1s, color 0.1s;
  }

  .collapse-btn:hover {
    background: var(--tool-item-hover-bg);
    color: var(--color-text-secondary);
  }

  .collapse-btn svg {
    width: 16px;
    height: 16px;
  }

  /* git 分支只读栏 */
  .branch-row {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 12px 8px;
    color: var(--color-text-muted);
    overflow: hidden;
  }

  .branch-icon {
    width: 14px;
    height: 14px;
    flex-shrink: 0;
    color: rgba(52, 211, 153, 0.85);
  }

  .branch-name {
    font-family: var(--font-mono);
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
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
    max-height: 350px;
    overflow-y: auto;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.12);
  }

  .dropdown-title {
    padding: 6px 10px 4px;
    font-size: 10px;
    font-weight: 600;
    color: var(--color-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.6px;
  }

  .dropdown-item {
    display: flex;
    flex-direction: row;
    align-items: center;
    gap: 8px;
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

  .dropdown-item-info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .dropdown-item-name {
    font-size: 13px;
    color: var(--color-text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .dropdown-item-name-active {
    color: var(--color-text);
    font-weight: 500;
  }

  .dropdown-item-path {
    font-size: 10px;
    color: var(--color-text-muted);
    font-family: var(--font-mono);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .dropdown-item-count {
    flex-shrink: 0;
    font-size: 10px;
    color: var(--color-text-muted);
    font-variant-numeric: tabular-nums;
  }

  .dropdown-item-check {
    flex-shrink: 0;
    width: 14px;
    height: 14px;
    color: #6366f1;
  }

  .dropdown-empty {
    padding: 12px;
    text-align: center;
    color: var(--color-text-muted);
    font-size: 13px;
  }
</style>

<script lang="ts">
  import type { ProjectInfo, RepositoryGroup, Worktree } from "../lib/api";
  import {
    CHEVRON_DOWN,
    CHECK_SVG,
    GIT_BRANCH_SVG,
  } from "../lib/icons";
  import {
    isGroupExpanded,
    toggleGroupExpanded,
    setGroupExpanded,
  } from "../lib/sidebarStore.svelte";

  interface Props {
    projects: ProjectInfo[];
    repositoryGroups?: RepositoryGroup[];
    selectedProjectId: string;
    onSelectProject: (id: string, name: string) => void;
    /** projectDataStore 首次 fetch 时为 true；为 true 且无 projects 时显示
     *  "加载中…" 而非 "无项目"，避免视觉闪烁（codex PR #140 二审 #7） */
    loading?: boolean;
  }

  let {
    projects,
    repositoryGroups = [],
    selectedProjectId,
    onSelectProject,
    loading = false,
  }: Props = $props();

  let dropdownOpen = $state(false);
  let lastDropdownOpen = $state(false);
  $effect(() => {
    if (dropdownOpen && !lastDropdownOpen) {
      for (const g of repositoryGroups) {
        if (
          g.worktrees.length > 1 &&
          g.worktrees.some((w) => w.id === selectedProjectId)
        ) {
          setGroupExpanded(g.id, true);
        }
      }
    }
    lastDropdownOpen = dropdownOpen;
  });

  function toggleDropdown() {
    dropdownOpen = !dropdownOpen;
  }

  function select(p: ProjectInfo) {
    onSelectProject(p.id, p.displayName);
    dropdownOpen = false;
  }

  function selectWorktree(wt: Worktree) {
    onSelectProject(wt.id, wt.name);
    dropdownOpen = false;
  }

  function formatPath(path: string): string {
    return path.replace(/^\/Users\/[^/]+/, "~");
  }

  const selectedName = $derived.by(() => {
    for (const g of repositoryGroups) {
      const wt = g.worktrees.find((w) => w.id === selectedProjectId);
      if (wt) {
        if (g.worktrees.length <= 1 || wt.isMainWorktree) return g.name;
        return `${g.name} · ${wt.name}`;
      }
    }
    return projects.find((p) => p.id === selectedProjectId)?.displayName ?? "选择项目";
  });

  const useGroupedView = $derived(repositoryGroups.length > 0);
  const hasAny = $derived(useGroupedView ? repositoryGroups.length > 0 : projects.length > 0);

  // 首屏 loading 且无数据时显示 placeholder 而非 "无项目"；按钮保持 disabled
  // 防止误开空 dropdown
  const placeholderText = $derived.by(() => {
    if (hasAny) return selectedName;
    if (loading) return "加载中…";
    return "无项目";
  });
</script>

<div class="project-switcher">
  <button
    class="project-selector"
    class:project-selector-loading={!hasAny && loading}
    data-tauri-drag-region="false"
    onclick={toggleDropdown}
    disabled={!hasAny}
    aria-haspopup="listbox"
    aria-expanded={dropdownOpen}
    aria-busy={loading && !hasAny}
  >
    <span class="project-name">{placeholderText}</span>
    <span class="dropdown-arrow" class:dropdown-arrow-open={dropdownOpen}>
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d={CHEVRON_DOWN} />
      </svg>
    </span>
  </button>

  {#if dropdownOpen}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="dropdown-backdrop" onclick={() => dropdownOpen = false}></div>
    <div class="dropdown" role="listbox" data-tauri-drag-region="false">
      <div class="dropdown-title">切换项目</div>

      {#if useGroupedView}
        {#each repositoryGroups as group (group.id)}
          {#if group.worktrees.length === 1}
            {@const wt = group.worktrees[0]}
            {@const isActive = wt.id === selectedProjectId}
            <button
              class="dropdown-item"
              class:dropdown-item-active={isActive}
              onclick={() => selectWorktree(wt)}
            >
              <div class="dropdown-item-info">
                <span class="dropdown-item-name" class:dropdown-item-name-active={isActive}>{group.name}</span>
                <span class="dropdown-item-path">{formatPath(wt.path)}</span>
              </div>
              <span class="dropdown-item-count">{group.totalSessions}</span>
              {#if isActive}
                <svg class="dropdown-item-check" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  {@html CHECK_SVG}
                </svg>
              {/if}
            </button>
          {:else}
            {@const expanded = isGroupExpanded(group.id)}
            {@const hasActive = group.worktrees.some((w) => w.id === selectedProjectId)}
            <button
              class="dropdown-group-row"
              class:dropdown-group-row-active={hasActive}
              onclick={() => toggleGroupExpanded(group.id)}
              aria-expanded={expanded}
            >
              <span class="dropdown-group-chevron" class:dropdown-group-chevron-open={expanded}>
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <path d={CHEVRON_DOWN} />
                </svg>
              </span>
              <div class="dropdown-item-info">
                <span class="dropdown-item-name" class:dropdown-item-name-active={hasActive}>{group.name}</span>
              </div>
              <span class="dropdown-group-badge" title="worktree 数量">{group.worktrees.length}</span>
              <span class="dropdown-item-count">{group.totalSessions}</span>
            </button>

            {#if expanded}
              {#each group.worktrees as wt (wt.id)}
                {@const isActive = wt.id === selectedProjectId}
                <button
                  class="dropdown-item dropdown-item-worktree"
                  class:dropdown-item-active={isActive}
                  onclick={() => selectWorktree(wt)}
                >
                  <div class="dropdown-item-info">
                    <span class="dropdown-item-name" class:dropdown-item-name-active={isActive}>
                      {wt.isMainWorktree ? "main" : wt.name}
                    </span>
                    <span class="dropdown-item-path">{formatPath(wt.path)}</span>
                  </div>
                  {#if wt.gitBranch}
                    <span class="dropdown-item-branch" title={wt.gitBranch}>
                      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        {@html GIT_BRANCH_SVG}
                      </svg>
                      <span class="dropdown-item-branch-text">{wt.gitBranch}</span>
                    </span>
                  {/if}
                  <span class="dropdown-item-count">{wt.sessions.length}</span>
                  {#if isActive}
                    <svg class="dropdown-item-check" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                      {@html CHECK_SVG}
                    </svg>
                  {/if}
                </button>
              {/each}
            {/if}
          {/if}
        {/each}
      {:else}
        {#each projects as project (project.id)}
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
      {/if}
    </div>
  {/if}
</div>

<style>
  .project-switcher {
    position: relative;
    display: inline-flex;
    align-items: center;
    min-width: 0;
  }

  .project-selector {
    min-width: 0;
    max-width: 240px;
    flex-shrink: 1;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 4px 8px;
    background: none;
    border: none;
    border-radius: 6px;
    color: var(--color-text);
    font: inherit;
    font-size: 14px;
    font-weight: 600;
    line-height: 1;
    cursor: pointer;
    transition: background 0.12s ease;
  }

  .project-selector:hover:not(:disabled) {
    background: rgba(0, 0, 0, 0.06);
  }

  .project-selector:disabled {
    color: var(--color-text-muted);
    cursor: default;
  }

  .project-selector-loading {
    /* 微弱呼吸提示数据正在加载，避免静默"无项目"误导 */
    animation: pulse-text 1.4s ease-in-out infinite;
  }

  @keyframes pulse-text {
    0%, 100% { opacity: 0.55; }
    50% { opacity: 0.85; }
  }

  .project-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    text-align: left;
  }

  .dropdown-arrow {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: var(--color-text-muted);
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

  .dropdown-backdrop {
    position: fixed;
    inset: 0;
    z-index: 19;
  }

  .dropdown {
    position: absolute;
    left: 0;
    top: calc(100% + 6px);
    min-width: 260px;
    max-width: 360px;
    background: var(--color-surface);
    border: 1px solid var(--color-border-emphasis);
    border-radius: 8px;
    padding: 4px;
    z-index: 20;
    max-height: 420px;
    overflow-y: auto;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.18);
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
    color: var(--color-accent-indigo);
  }

  .dropdown-empty {
    padding: 12px;
    text-align: center;
    color: var(--color-text-muted);
    font-size: 13px;
  }

  .dropdown-group-row {
    display: flex;
    flex-direction: row;
    align-items: center;
    gap: 6px;
    width: 100%;
    padding: 6px 8px;
    background: none;
    border: none;
    border-radius: 6px;
    cursor: pointer;
    text-align: left;
    font: inherit;
    color: var(--color-text);
    transition: background 0.1s;
  }

  .dropdown-group-row:hover {
    background: var(--tool-item-hover-bg);
  }

  .dropdown-group-row-active {
    background: var(--color-surface-raised);
  }

  .dropdown-group-chevron {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 14px;
    height: 14px;
    color: var(--color-text-muted);
    transition: transform 0.15s ease;
  }

  .dropdown-group-chevron svg {
    width: 14px;
    height: 14px;
    transform: rotate(-90deg);
    transition: transform 0.15s ease;
  }

  .dropdown-group-chevron-open svg {
    transform: rotate(0deg);
  }

  .dropdown-group-badge {
    flex-shrink: 0;
    padding: 1px 6px;
    border-radius: 8px;
    font-size: 10px;
    font-variant-numeric: tabular-nums;
    color: var(--color-text-muted);
    background: var(--color-surface-raised);
  }

  .dropdown-item-worktree {
    padding-left: 28px;
  }

  .dropdown-item-branch {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    flex-shrink: 0;
    max-width: 120px;
    color: var(--color-text-muted);
    font-size: 10px;
  }

  .dropdown-item-branch svg {
    width: 11px;
    height: 11px;
    flex-shrink: 0;
  }

  .dropdown-item-branch-text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>

<script lang="ts">
  import type { ProjectInfo, RepositoryGroup } from "../lib/api";
  import { BELL, SETTINGS, PANEL_LEFT_SVG } from "../lib/icons";
  import {
    getSidebarCollapsed,
    toggleSidebarCollapsed,
  } from "../lib/sidebarStore.svelte";
  import { openNotificationsTab, openSettingsTab, getUnreadCount } from "../lib/tabStore.svelte";
  import ProjectSwitcher from "./ProjectSwitcher.svelte";
  import RosettaStatusIcon from "./RosettaStatusIcon.svelte";
  import UpdateStatusPill from "./UpdateStatusPill.svelte";

  interface Props {
    projects: ProjectInfo[];
    repositoryGroups?: RepositoryGroup[];
    selectedGroupId: string;
    onSelectProject: (id: string, name: string) => void;
    rosettaVisible: boolean;
    projectsLoading?: boolean;
  }

  let {
    projects,
    repositoryGroups = [],
    selectedGroupId,
    onSelectProject,
    rosettaVisible,
    projectsLoading = false,
  }: Props = $props();

  // macOS 隐藏原生 title bar，traffic light 浮绘在 chrome 左上 (12, 20)；
  // 80px = 12 (window margin) + 3 × 14 (traffic lights) + 2 × 8 (gaps) + 14 (留白)
  const isMac =
    typeof navigator !== "undefined" && navigator.userAgent.includes("Macintosh");

  const collapsed = $derived(getSidebarCollapsed());
  const unreadCount = $derived(getUnreadCount());
</script>

<header
  class="chrome"
  class:chrome-mac={isMac}
  data-tauri-drag-region
  aria-label="应用工具栏"
>
  {#if isMac}
    <div class="zone-platform-padding" aria-hidden="true"></div>
  {/if}

  <div class="zone-left-center">
    <ProjectSwitcher
      {projects}
      {repositoryGroups}
      {selectedGroupId}
      {onSelectProject}
      loading={projectsLoading}
    />

    <button
      class="icon-btn"
      data-tauri-drag-region="false"
      title={collapsed ? "展开侧栏 (⌘B)" : "收起侧栏 (⌘B)"}
      aria-label={collapsed ? "展开侧栏" : "收起侧栏"}
      onclick={toggleSidebarCollapsed}
    >
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        {@html PANEL_LEFT_SVG}
      </svg>
    </button>
  </div>

  <div class="zone-drag-flex" data-tauri-drag-region></div>

  <div class="zone-status">
    <RosettaStatusIcon visible={rosettaVisible} />
    <UpdateStatusPill />

    <button
      class="icon-btn"
      data-tauri-drag-region="false"
      onclick={() => openNotificationsTab()}
      title="通知"
      aria-label="通知"
    >
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d={BELL} />
      </svg>
      {#if unreadCount > 0}
        <span class="badge" aria-label="{unreadCount} 条未读通知">{unreadCount > 99 ? "99+" : unreadCount}</span>
      {/if}
    </button>

    <button
      class="icon-btn"
      data-tauri-drag-region="false"
      onclick={() => openSettingsTab()}
      title="设置"
      aria-label="设置"
    >
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d={SETTINGS} />
      </svg>
    </button>
  </div>
</header>

<style>
  .chrome {
    height: var(--chrome-height, 44px);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--chrome-control-gap, 8px);
    padding: 0 12px 0 0;
    background: var(--color-surface-sidebar, var(--color-surface));
    border-bottom: 1px solid var(--color-border-emphasis);
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.4);
    box-sizing: border-box;
    user-select: none;
    -webkit-user-select: none;
  }

  .zone-platform-padding {
    width: var(--chrome-mac-padding-left, 80px);
    flex-shrink: 0;
  }

  .chrome:not(.chrome-mac) {
    padding-left: 8px;
  }

  .zone-left-center {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    min-width: 0;
    flex-shrink: 1;
  }

  .zone-drag-flex {
    flex: 1;
    min-width: 8px;
    align-self: stretch;
  }

  .zone-status {
    display: inline-flex;
    align-items: center;
    gap: var(--chrome-control-gap, 8px);
    flex-shrink: 0;
  }

  .icon-btn {
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    padding: 0;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
    transition: background 0.12s ease, color 0.12s ease;
  }

  .icon-btn:hover {
    background: var(--tool-item-hover-bg);
    color: var(--color-text);
  }

  .icon-btn:focus-visible {
    outline: 2px solid var(--color-accent-blue);
    outline-offset: 2px;
  }

  .icon-btn svg {
    width: 16px;
    height: 16px;
  }

  .badge {
    position: absolute;
    top: 2px;
    right: 0;
    min-width: 16px;
    height: 16px;
    border-radius: 8px;
    background: var(--color-danger);
    color: var(--color-text-on-accent);
    font-size: 10px;
    font-weight: 600;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0 4px;
    line-height: 1;
    pointer-events: none;
  }
</style>

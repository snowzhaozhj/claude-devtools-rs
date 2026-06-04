<script lang="ts">
  import type { ProjectInfo, RepositoryGroup } from "../lib/api";
  import { BELL, SETTINGS, PANEL_LEFT_SVG, JOBS_SVG } from "../lib/icons";
  import {
    getSidebarCollapsed,
    toggleSidebarCollapsed,
  } from "../lib/sidebarStore.svelte";
  import { openNotificationsTab, openSettingsTab, openJobsTab, getUnreadCount } from "../lib/tabStore.svelte";
  import {
    getJobsDirExists,
    getBadgeColor,
  } from "../lib/jobsStore.svelte";
  import ProjectSwitcher from "./ProjectSwitcher.svelte";
  import RosettaStatusIcon from "./RosettaStatusIcon.svelte";
  import UpdateStatusPill from "./UpdateStatusPill.svelte";
  import { isTauriRuntime } from "../lib/runtime";

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

  // tauri.conf.json 的 titleBarStyle:"Overlay" + hiddenTitle 让 macOS 桌面端
  // 隐藏原生 title bar，traffic light 浮绘在 chrome 左上 (12, 20)，需要 80px
  // 让位（12 window margin + 3×14 traffic lights + 2×8 gaps + 14 留白）。这两
  // 个字段是 macOS 专属——HTTP server mode 浏览器（?http=1）下既无 Tauri
  // runtime 也无 traffic light，光靠 UA 判 OS 会让 ProjectSwitcher 前出现 80px
  // 假留白，故 SHALL 同时验证 isTauriRuntime()。
  // mock 模式（?mock=1）通过 mockIPC 注入 __TAURI_INTERNALS__，isTauriRuntime()
  // 仍为 true，与真桌面同分支——mock 是 dev 调试入口，理应模拟桌面视觉。
  const isMacOS =
    typeof navigator !== "undefined" && navigator.userAgent.includes("Macintosh");
  const needsTrafficLightPadding = isMacOS && isTauriRuntime();

  const collapsed = $derived(getSidebarCollapsed());
  const unreadCount = $derived(getUnreadCount());
  const jobsDirExists = $derived(getJobsDirExists());
  const jobsBadge = $derived(getBadgeColor());
</script>

<header
  class="chrome"
  class:chrome-mac={needsTrafficLightPadding}
  data-tauri-drag-region
  aria-label="应用工具栏"
>
  {#if needsTrafficLightPadding}
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

    {#if jobsDirExists}
      <button
        class="icon-btn"
        data-tauri-drag-region="false"
        onclick={() => openJobsTab()}
        title="后台任务"
        aria-label="后台任务"
      >
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          {@html JOBS_SVG}
        </svg>
        {#if jobsBadge === "red"}
          <span class="badge badge-red" aria-label="有失败的后台任务"></span>
        {:else if jobsBadge === "amber"}
          <span class="badge badge-amber" aria-label="有需要输入的后台任务"></span>
        {/if}
      </button>
    {/if}

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

  /* Jobs badge — 无数字的色点 */
  .badge-red,
  .badge-amber {
    min-width: 8px;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    padding: 0;
    top: 4px;
    right: 2px;
  }

  .badge-red {
    background: var(--color-danger);
  }

  .badge-amber {
    background: var(--color-warning);
  }

</style>

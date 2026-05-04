<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import type { ProjectInfo } from "../lib/api";
  import {
    CHEVRON_DOWN,
    CHECK_SVG,
    FOLDER_GIT2_SVG,
    PANEL_LEFT_SVG,
  } from "../lib/icons";

  interface Props {
    projects: ProjectInfo[];
    selectedProjectId: string;
    onSelectProject: (id: string, name: string) => void;
    onToggleCollapsed: () => void;
  }

  let {
    projects,
    selectedProjectId,
    onSelectProject,
    onToggleCollapsed,
  }: Props = $props();
  let dropdownOpen = $state(false);

  // macOS 隐藏原生 title bar 后保留 traffic lights 浮在内容上层；header
  // Row 1 兼任 drag region，左侧需为 traffic lights 让位 72px。其余平台
  // 无 traffic lights，padding 与 drag region 不生效。
  const isMac = typeof navigator !== "undefined" && navigator.userAgent.includes("Macintosh");

  /**
   * 主动接管 drag region 的 mousedown，preventDefault 阻止浏览器 native
   * text selection（避免光标拖到下方时把会话标题一起选中），并按单击 /
   * 双击分别调 startDragging / toggleMaximize。
   *
   * **不**用 `data-tauri-drag-region` —— Tauri 2 默认注入的 drag.js 在
   * capture 阶段会 `stopImmediatePropagation`，导致本组件的 bubbling
   * 阶段 onmousedown 拿不到事件、preventDefault 没机会跑，文本选择穿透
   * 到下方列表 + 双击 maximize 在 NSWindow overlay 模式下静默失败。
   * 直接自己监听 mousedown 完整接管 drag / maximize / 阻止选择。
   */
  async function handleDragRegionMouseDown(e: MouseEvent) {
    if (e.button !== 0) return;
    // 点击 button（含其内部 svg/span）时跳过让 onclick 正常工作；
    // 其余 row 内空白（含 traffic light padding 区）都触发 drag/maximize
    const target = e.target as HTMLElement | null;
    if (target?.closest("button")) return;
    e.preventDefault();
    const win = getCurrentWindow();
    if (e.detail === 2) {
      await win.toggleMaximize();
    } else {
      await win.startDragging();
    }
  }

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
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="header-row"
    class:header-row-mac={isMac}
    onmousedown={handleDragRegionMouseDown}
  >
    <button
      class="project-selector"
      onclick={toggleDropdown}
    >
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
  }

  .header-row {
    display: flex;
    align-items: center;
    gap: 4px;
    height: 40px;
    padding: 0 8px;
    box-sizing: border-box;
    /* border 直接由 row 持有 + box-sizing border-box 让 row 总高 = 40px；
       否则 sidebar-header 自带 border-bottom + row height 40 = 41px，与
       右侧 TabBar 40px 错位 1px（左右分隔线对不齐）。 */
    border-bottom: 1px solid var(--color-border);
    /* drag region 上 mousedown 会启动 native text selection；除 JS 层
       preventDefault 兜底外这里也禁掉，避免光标拖到下方时选中会话标题。 */
    user-select: none;
    -webkit-user-select: none;
  }

  /* macOS 隐藏原生 title bar 后 traffic lights 浮在 webview 左上角，预留
     72px 让位（对齐原版 HEADER_ROW1 + getTrafficLightPaddingForZoom(1)） */
  .header-row-mac {
    padding-left: 72px;
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

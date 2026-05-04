<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import {
    closeTab,
    getPaneById,
    getPaneLayout,
    getUnreadCount,
    openNotificationsTab,
    openSettingsTab,
    setActiveTab,
    setUnreadCount,
    splitPane,
  } from "../lib/tabStore.svelte";
  import { MAX_PANES } from "../lib/paneTypes";
  import { beginDrag, getDragSource, getHit, isDragging } from "../lib/dragSession.svelte";
  import { getNotifications } from "../lib/api";
  import TabContextMenu from "./TabContextMenu.svelte";
  import { BELL, SETTINGS, FILE_TEXT_SVG, PANEL_LEFT_SVG } from "../lib/icons";
  import { getSidebarCollapsed, toggleSidebarCollapsed } from "../lib/sidebarStore.svelte";

  interface Props {
    paneId: string;
    /** 本 pane 是否是最左 pane；只在最左 pane + 折叠态显示展开按钮 */
    isFirstPane?: boolean;
  }

  let { paneId, isFirstPane = false }: Props = $props();

  const showExpandBtn = $derived(isFirstPane && getSidebarCollapsed());

  // sidebar 折叠后 traffic lights 暴露在最左 pane 顶部（macOS 隐藏 title bar
  // 时由 OS 浮绘在 webview 上）；对齐原版 TabBar.tsx 的 sidebarCollapsed
  // && isLeftmostPane 时加 traffic light padding 让 expand 按钮和 tab 让位。
  const isMac = typeof navigator !== "undefined" && navigator.userAgent.includes("Macintosh");
  const reserveTrafficLightSpace = $derived(isMac && isFirstPane && getSidebarCollapsed());

  const pane = $derived(getPaneById(paneId));
  const tabs = $derived(pane?.tabs ?? []);
  const activeTabId = $derived(pane?.activeTabId ?? null);
  const unreadCount = $derived(getUnreadCount());
  const dragSource = $derived(getDragSource());
  const dragActive = $derived(isDragging());
  const hit = $derived(getHit());

  // 拖拽状态由 dragSession（模块级）统一管理；TabBar 只触发 beginDrag
  // + 根据 hit 派生 drop indicator 视觉

  // 30 秒轮询 unreadCount
  let pollTimer: ReturnType<typeof setInterval>;

  async function refreshUnreadCount() {
    try {
      const result = await getNotifications(1, 0);
      setUnreadCount(result.unreadCount);
    } catch {
      /* 静默失败 */
    }
  }

  onMount(() => {
    refreshUnreadCount();
    pollTimer = setInterval(refreshUnreadCount, 30000);
  });

  onDestroy(() => {
    clearInterval(pollTimer);
  });

  function handleClose(e: Event, tabId: string) {
    e.stopPropagation();
    closeTab(tabId);
  }

  function handlePointerDown(e: PointerEvent, index: number, tabId: string) {
    if (e.button !== 0) return;
    beginDrag(tabId, paneId, index, e.clientX);
  }

  // ---------- 右键菜单 ----------
  let ctxMenu: { x: number; y: number; tabId: string } | null = $state(null);

  function handleContextMenu(e: MouseEvent, tabId: string) {
    e.preventDefault();
    ctxMenu = { x: e.clientX, y: e.clientY, tabId };
  }

  function closeOthers(keepTabId: string) {
    const p = getPaneById(paneId);
    if (!p) return;
    // 复制快照避免迭代时修改
    const toClose = p.tabs.filter((t) => t.id !== keepTabId).map((t) => t.id);
    for (const id of toClose) closeTab(id);
  }

  function isSourceTab(index: number): boolean {
    return (
      dragActive &&
      !!dragSource &&
      dragSource.paneId === paneId &&
      dragSource.sourceIndex === index
    );
  }

  function isDropTargetTab(index: number): boolean {
    if (!dragActive || !hit || hit.kind !== "tab") return false;
    if (hit.paneId !== paneId) return false;
    if (hit.index !== index) return false;
    // 同 pane 且与 source 同 index 时不算 drop target
    if (dragSource && dragSource.paneId === paneId && dragSource.sourceIndex === index) return false;
    return true;
  }

  // TabBar 顶部条整体作为 drag region：单击 + 拖移动窗口、双击 maximize；
  // 点击 button / tab-item 等可交互子元素时跳过。对齐原版 TabBar.tsx 的
  // `WebkitAppRegion: 'drag'` on isLeftmostPane 行为，但本仓所有 pane 顶
  // 部条都启用 drag 让多 pane 拆分时也能拖动窗口。
  async function handleTabBarMouseDown(e: MouseEvent) {
    if (e.button !== 0) return;
    const target = e.target as HTMLElement | null;
    if (target?.closest("button, .tab-item")) return;
    e.preventDefault();
    const win = getCurrentWindow();
    if (e.detail === 2) {
      await win.toggleMaximize();
    } else {
      await win.startDragging();
    }
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="tab-bar"
  class:tab-bar-mac-collapsed={reserveTrafficLightSpace}
  onmousedown={handleTabBarMouseDown}
>
  {#if showExpandBtn}
    <button
      class="expand-sidebar-btn"
      title="展开侧栏 (⌘B)"
      aria-label="展开侧栏"
      onclick={toggleSidebarCollapsed}
    >
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        {@html PANEL_LEFT_SVG}
      </svg>
    </button>
  {/if}
  <div class="tab-list" role="tablist">
    {#each tabs as tab, index (tab.id)}
      <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
      <div
        class="tab-item"
        class:tab-item-active={tab.id === activeTabId}
        class:tab-item-dragging={isSourceTab(index)}
        class:tab-item-drop-target={isDropTargetTab(index)}
        role="tab"
        tabindex="0"
        aria-selected={tab.id === activeTabId}
        data-tab-index={index}
        data-pane-id={paneId}
        title={tab.label}
        onpointerdown={(e) => handlePointerDown(e, index, tab.id)}
        oncontextmenu={(e) => handleContextMenu(e, tab.id)}
        onkeydown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            setActiveTab(tab.id);
          }
        }}
      >
        <span class="tab-icon">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            {#if tab.type === "settings"}
              <path d={SETTINGS} />
            {:else if tab.type === "notifications"}
              <path d={BELL} />
            {:else}
              {@html FILE_TEXT_SVG}
            {/if}
          </svg>
        </span>
        <span class="tab-label">{tab.label}</span>
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <span
          class="tab-close"
          role="button"
          tabindex="-1"
          aria-label="关闭标签"
          onpointerdown={(e) => e.stopPropagation()}
          onclick={(e) => handleClose(e, tab.id)}
          onkeydown={(e) => {
            if (e.key === "Enter") handleClose(e, tab.id);
          }}
        >×</span>
      </div>
    {/each}
  </div>

  <div class="tab-actions">
    <button class="tab-action-btn" onclick={() => openNotificationsTab()} title="通知">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d={BELL} />
      </svg>
      {#if unreadCount > 0}
        <span class="badge">{unreadCount > 99 ? "99+" : unreadCount}</span>
      {/if}
    </button>
    <button class="tab-action-btn" onclick={() => openSettingsTab()} title="设置">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d={SETTINGS} />
      </svg>
    </button>
  </div>
</div>

{#if ctxMenu}
  {@const ctx = ctxMenu}
  {@const pane = getPaneById(paneId)}
  {@const canSplit = getPaneLayout().panes.length < MAX_PANES}
  {@const canCloseOthers = (pane?.tabs.length ?? 0) > 1}
  <TabContextMenu
    x={ctx.x}
    y={ctx.y}
    {canSplit}
    {canCloseOthers}
    onClose={() => { ctxMenu = null; }}
    onCloseTab={() => closeTab(ctx.tabId)}
    onCloseOthers={() => closeOthers(ctx.tabId)}
    onSplitLeft={() => splitPane(paneId, ctx.tabId, "left")}
    onSplitRight={() => splitPane(paneId, ctx.tabId, "right")}
  />
{/if}

<style>
  .tab-bar {
    height: 40px;
    min-height: 40px;
    display: flex;
    align-items: stretch;
    background: var(--color-bg-tertiary, var(--color-surface-sidebar));
    border-bottom: 1px solid var(--color-border);
    overflow: hidden;
    /* border-box 让 border 含在 40px 总高内，对齐左侧 .header-row（也是
       border-box + height 40），否则两者底部 border 错位 1px。 */
    box-sizing: border-box;
    user-select: none;
    -webkit-user-select: none;
  }

  /* sidebar 折叠时 macOS traffic lights 占用最左 pane 顶部 72px，让 expand
     按钮和 tab 列表往右挪避开（对齐原版 TabBar.tsx 同等逻辑） */
  .tab-bar-mac-collapsed {
    padding-left: 72px;
  }

  .expand-sidebar-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 36px;
    flex-shrink: 0;
    background: none;
    border: none;
    border-right: 1px solid var(--color-border);
    color: var(--color-text-muted);
    cursor: pointer;
    transition: background 0.1s, color 0.1s;
  }

  .expand-sidebar-btn:hover {
    background: var(--tool-item-hover-bg);
    color: var(--color-text-secondary);
  }

  .expand-sidebar-btn svg {
    width: 16px;
    height: 16px;
  }

  .tab-list {
    display: flex;
    align-items: stretch;
    overflow-x: auto;
    overflow-y: hidden;
    flex: 1;
    scrollbar-width: none;
  }

  .tab-list::-webkit-scrollbar {
    display: none;
  }

  .tab-item {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 12px;
    min-width: 0;
    max-width: 200px;
    border: none;
    border-right: 1px solid var(--color-border);
    background: transparent;
    color: var(--color-text-muted);
    font: inherit;
    font-size: 12px;
    cursor: grab;
    user-select: none;
    -webkit-user-select: none;
    /* 禁用 WKWebView 原生 drag：pointer 方案不需要它，开启反而会
       让系统以为用户在往应用外拖，派发跨应用 copy 导致 drop 丢失。 */
    -webkit-user-drag: none;
    transition: background 0.1s, color 0.1s, opacity 0.1s;
    flex-shrink: 0;
    /* 为 drop-target 左边缘 indicator 预留定位上下文 */
    position: relative;
    /* 拖拽过程中屏蔽触控手势的横向滑动默认行为 */
    touch-action: none;
  }

  .tab-item:active {
    cursor: grabbing;
  }

  .tab-item:hover {
    background: var(--tool-item-hover-bg);
    color: var(--color-text-secondary);
  }

  .tab-item-active {
    background: var(--color-bg-primary, var(--color-surface));
    color: var(--color-text);
    border-bottom: 2px solid var(--color-border-emphasis);
  }

  /* 正在被拖动的 tab：半透明，对齐原版 opacity 0.3 */
  .tab-item-dragging {
    opacity: 0.3;
  }

  /* drop 目标：左边缘 2px 竖线 indicator */
  .tab-item-drop-target::before {
    content: "";
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: 2px;
    background: var(--color-border-emphasis);
    pointer-events: none;
  }

  .tab-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: var(--color-text-muted);
    flex-shrink: 0;
  }

  .tab-icon svg {
    width: 13px;
    height: 13px;
  }

  .tab-item-active .tab-icon {
    color: var(--color-text-secondary);
  }

  .tab-label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
    text-align: left;
    pointer-events: none;
  }

  .tab-close {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--color-text-muted);
    font-size: 14px;
    line-height: 1;
    cursor: pointer;
    flex-shrink: 0;
    padding: 0;
    transition: background 0.1s, color 0.1s;
  }

  .tab-close:hover {
    background: var(--color-border);
    color: var(--color-text);
  }

  .tab-actions {
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 0 8px;
    flex-shrink: 0;
    border-left: 1px solid var(--color-border);
  }

  .tab-action-btn {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
    transition: background 0.1s, color 0.1s;
    padding: 0;
  }

  .tab-action-btn:hover {
    background: var(--color-surface-raised);
    color: var(--color-text);
  }

  .tab-action-btn svg {
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
    background: #e53e3e;
    color: #fff;
    font-size: 10px;
    font-weight: 600;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0 4px;
    line-height: 1;
  }
</style>

<script lang="ts">
  import { onMount, onDestroy } from "svelte";
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
  import { BELL, SETTINGS } from "../lib/icons";

  interface Props {
    paneId: string;
  }

  let { paneId }: Props = $props();

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

  function tabIcon(type: string): string {
    if (type === "settings") return "⚙";
    if (type === "notifications") return "🔔";
    return "";
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
</script>

<div class="tab-bar">
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
        {#if tab.type !== "session"}
          <span class="tab-icon">{tabIcon(tab.type)}</span>
        {/if}
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
    height: 36px;
    min-height: 36px;
    display: flex;
    align-items: stretch;
    background: var(--color-bg-tertiary, var(--color-surface-sidebar));
    border-bottom: 1px solid var(--color-border);
    overflow: hidden;
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
    font-size: 13px;
    flex-shrink: 0;
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

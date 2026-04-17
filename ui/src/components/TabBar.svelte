<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import {
    getTabs,
    getActiveTabId,
    setActiveTab,
    closeTab,
    openSettingsTab,
    openNotificationsTab,
    getUnreadCount,
    setUnreadCount,
    reorderTab,
  } from "../lib/tabStore.svelte";
  import { getNotifications } from "../lib/api";

  const tabs = $derived(getTabs());
  const activeTabId = $derived(getActiveTabId());
  const unreadCount = $derived(getUnreadCount());

  // ---------- 拖拽状态（pointer events 方案） ----------
  // 选择 pointer 而非 HTML5 drag：macOS WKWebView 会把 HTML5 drag
  // 当成跨应用 copy 操作（dropEffect=copy），drop 事件在 document 内不触发。
  // 这里自己做 pointerdown/move/up 状态机，完全绕开 WKWebView 的行为差异。
  let dragSourceIndex = $state<number | null>(null);
  let dragOverIndex = $state<number | null>(null);
  let isDragging = $state(false);
  let dragStartX = 0;
  const DRAG_THRESHOLD = 5; // px，超过该距离才进入拖拽态，低于则视为点击

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

  // ---------- 拖拽处理（pointer 状态机） ----------
  function handlePointerDown(e: PointerEvent, index: number) {
    // 只处理鼠标左键 / 主指针
    if (e.button !== 0) return;
    dragSourceIndex = index;
    dragStartX = e.clientX;
    isDragging = false;
    dragOverIndex = null;
    // 全局监听 move/up/cancel，保证即使鼠标滑出 TabBar 也能收到事件
    document.addEventListener("pointermove", handlePointerMove);
    document.addEventListener("pointerup", handlePointerUp);
    document.addEventListener("pointercancel", handlePointerCancel);
  }

  function handlePointerMove(e: PointerEvent) {
    if (dragSourceIndex === null) return;
    if (!isDragging) {
      if (Math.abs(e.clientX - dragStartX) <= DRAG_THRESHOLD) return;
      isDragging = true;
    }
    // 命中测试：找鼠标下方的 .tab-item 及其 data-tab-index
    const el = document.elementFromPoint(e.clientX, e.clientY);
    const tabEl = el?.closest<HTMLElement>(".tab-item");
    if (!tabEl) {
      dragOverIndex = null;
      return;
    }
    const raw = tabEl.dataset.tabIndex;
    const idx = raw === undefined ? Number.NaN : Number(raw);
    dragOverIndex = Number.isNaN(idx) ? null : idx;
  }

  function handlePointerUp() {
    const wasDragging = isDragging;
    const src = dragSourceIndex;
    const tgt = dragOverIndex;
    cleanupDrag();
    if (wasDragging) {
      if (src !== null && tgt !== null && tgt !== src) {
        reorderTab(src, tgt);
      }
    } else if (src !== null) {
      // 未越过 threshold → 视为普通点击，激活 tab
      const tab = tabs[src];
      if (tab) setActiveTab(tab.id);
    }
  }

  function handlePointerCancel() {
    cleanupDrag();
  }

  function cleanupDrag() {
    dragSourceIndex = null;
    dragOverIndex = null;
    isDragging = false;
    document.removeEventListener("pointermove", handlePointerMove);
    document.removeEventListener("pointerup", handlePointerUp);
    document.removeEventListener("pointercancel", handlePointerCancel);
  }
</script>

<div class="tab-bar">
  <div class="tab-list" role="tablist">
    {#each tabs as tab, index (tab.id)}
      <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
      <div
        class="tab-item"
        class:tab-item-active={tab.id === activeTabId}
        class:tab-item-dragging={isDragging && dragSourceIndex === index}
        class:tab-item-drop-target={isDragging && dragOverIndex === index && dragSourceIndex !== index}
        role="tab"
        tabindex="0"
        aria-selected={tab.id === activeTabId}
        data-tab-index={index}
        title={tab.label}
        onpointerdown={(e) => handlePointerDown(e, index)}
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
      <span class="bell-icon">🔔</span>
      {#if unreadCount > 0}
        <span class="badge">{unreadCount > 99 ? "99+" : unreadCount}</span>
      {/if}
    </button>
    <button class="tab-action-btn" onclick={() => openSettingsTab()} title="设置">
      ⚙
    </button>
  </div>
</div>

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
    border-radius: 4px;
    background: transparent;
    color: var(--color-text-muted);
    font-size: 14px;
    cursor: pointer;
    transition: background 0.1s, color 0.1s;
  }

  .tab-action-btn:hover {
    background: var(--tool-item-hover-bg);
    color: var(--color-text);
  }

  .bell-icon {
    font-size: 14px;
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

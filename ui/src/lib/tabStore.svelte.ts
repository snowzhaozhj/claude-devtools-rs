import type { SessionDetail } from "./api";

// ---------------------------------------------------------------------------
// Tab 数据模型
// ---------------------------------------------------------------------------

export interface Tab {
  id: string;
  sessionId: string;
  projectId: string;
  label: string;
  createdAt: number;
}

export interface TabUIState {
  expandedChunks: Set<number>;
  expandedItems: Set<string>;
  searchVisible: boolean;
  contextPanelVisible: boolean;
  scrollTop: number;
}

function createDefaultUIState(): TabUIState {
  return {
    expandedChunks: new Set(),
    expandedItems: new Set(),
    searchVisible: false,
    contextPanelVisible: false,
    scrollTop: 0,
  };
}

// ---------------------------------------------------------------------------
// 响应式状态（模块级 $state）
// ---------------------------------------------------------------------------

let tabs: Tab[] = $state([]);
let activeTabId: string | null = $state(null);

const tabUIStates = new Map<string, TabUIState>();
const tabSessionCache = new Map<string, SessionDetail>();

// ---------------------------------------------------------------------------
// 只读访问
// ---------------------------------------------------------------------------

export function getTabs(): Tab[] {
  return tabs;
}

export function getActiveTabId(): string | null {
  return activeTabId;
}

export function getActiveTab(): Tab | null {
  if (!activeTabId) return null;
  return tabs.find((t) => t.id === activeTabId) ?? null;
}

// ---------------------------------------------------------------------------
// Tab 操作
// ---------------------------------------------------------------------------

export function openTab(
  sessionId: string,
  projectId: string,
  label: string,
): void {
  // 已有同 session 的 tab → 切换焦点
  const existing = tabs.find((t) => t.sessionId === sessionId);
  if (existing) {
    activeTabId = existing.id;
    return;
  }

  const tab: Tab = {
    id: crypto.randomUUID(),
    sessionId,
    projectId,
    label: label.length > 50 ? label.slice(0, 50) + "…" : label,
    createdAt: Date.now(),
  };

  tabs = [...tabs, tab];
  activeTabId = tab.id;
}

export function closeTab(tabId: string): void {
  const idx = tabs.findIndex((t) => t.id === tabId);
  if (idx === -1) return;

  // 清理 per-tab 状态和缓存
  tabUIStates.delete(tabId);
  tabSessionCache.delete(tabId);

  const newTabs = tabs.filter((t) => t.id !== tabId);
  tabs = newTabs;

  // 若关闭的是活跃 tab，切到相邻 tab
  if (activeTabId === tabId) {
    if (newTabs.length === 0) {
      activeTabId = null;
    } else {
      // 优先同位置，否则前一个
      const nextIdx = Math.min(idx, newTabs.length - 1);
      activeTabId = newTabs[nextIdx].id;
    }
  }
}

export function setActiveTab(tabId: string): void {
  if (tabs.some((t) => t.id === tabId)) {
    activeTabId = tabId;
  }
}

// ---------------------------------------------------------------------------
// Per-tab UI 状态
// ---------------------------------------------------------------------------

export function getTabUIState(tabId: string): TabUIState {
  let st = tabUIStates.get(tabId);
  if (!st) {
    st = createDefaultUIState();
    tabUIStates.set(tabId, st);
  }
  return st;
}

export function saveTabUIState(tabId: string, state: TabUIState): void {
  tabUIStates.set(tabId, state);
}

// ---------------------------------------------------------------------------
// Per-tab session 数据缓存
// ---------------------------------------------------------------------------

export function getCachedSession(tabId: string): SessionDetail | null {
  return tabSessionCache.get(tabId) ?? null;
}

export function setCachedSession(
  tabId: string,
  detail: SessionDetail,
): void {
  tabSessionCache.set(tabId, detail);
}

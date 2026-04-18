import {
  moveTabToNewPane,
  moveTabToPane,
  reorderTabInPane,
  setActiveTab,
} from "./tabStore.svelte";

// ---------------------------------------------------------------------------
// Tab 拖拽全局状态 — macOS WKWebView HTML5 drag 不可靠，统一用 pointer events
// ---------------------------------------------------------------------------

interface DragSource {
  tabId: string;
  paneId: string;
  sourceIndex: number;
  startX: number;
}

type HitTarget =
  | { kind: "tab"; paneId: string; index: number }
  | { kind: "drop-zone"; paneId: string; side: "left" | "right" }
  | null;

const DRAG_THRESHOLD = 5; // px

let source: DragSource | null = $state(null);
let active: boolean = $state(false);
let hit: HitTarget = $state(null);

export function getDragSource(): DragSource | null {
  return source;
}

export function isDragging(): boolean {
  return active;
}

export function getHit(): HitTarget {
  return hit;
}

export function beginDrag(
  tabId: string,
  paneId: string,
  sourceIndex: number,
  startX: number,
): void {
  source = { tabId, paneId, sourceIndex, startX };
  active = false;
  hit = null;
  document.addEventListener("pointermove", onPointerMove);
  document.addEventListener("pointerup", onPointerUp);
  document.addEventListener("pointercancel", onPointerCancel);
}

function applyDragCursor() {
  // 拖拽活跃期间全局禁选文本 + 统一光标；cleanup 时还原
  document.body.style.userSelect = "none";
  (document.body.style as CSSStyleDeclaration & { webkitUserSelect?: string }).webkitUserSelect = "none";
  document.body.style.cursor = "grabbing";
}

function resetDragCursor() {
  document.body.style.userSelect = "";
  (document.body.style as CSSStyleDeclaration & { webkitUserSelect?: string }).webkitUserSelect = "";
  document.body.style.cursor = "";
}

function onPointerMove(e: PointerEvent): void {
  if (!source) return;
  if (!active) {
    if (Math.abs(e.clientX - source.startX) <= DRAG_THRESHOLD) return;
    active = true;
    applyDragCursor();
  }
  hit = hitTest(e.clientX, e.clientY);
}

function hitTest(x: number, y: number): HitTarget {
  const el = document.elementFromPoint(x, y);
  if (!el) return null;
  const dz = el.closest<HTMLElement>(".pane-drop-zone");
  if (dz) {
    const paneId = dz.dataset.paneId;
    const side = dz.dataset.side;
    if (paneId && (side === "left" || side === "right")) {
      return { kind: "drop-zone", paneId, side };
    }
  }
  const tabEl = el.closest<HTMLElement>(".tab-item");
  if (tabEl) {
    const paneId = tabEl.dataset.paneId;
    const raw = tabEl.dataset.tabIndex;
    const idx = raw === undefined ? Number.NaN : Number(raw);
    if (paneId && !Number.isNaN(idx)) {
      return { kind: "tab", paneId, index: idx };
    }
  }
  return null;
}

function onPointerUp(): void {
  const wasActive = active;
  const src = source;
  const target = hit;
  cleanup();
  if (!src) return;
  if (!wasActive) {
    // 未越阈值 → 当作单击，激活 tab
    setActiveTab(src.tabId);
    return;
  }
  if (!target) return;

  if (target.kind === "tab") {
    if (target.paneId === src.paneId) {
      if (target.index !== src.sourceIndex) {
        reorderTabInPane(src.paneId, src.sourceIndex, target.index);
      }
    } else {
      moveTabToPane(src.tabId, src.paneId, target.paneId, target.index);
    }
  } else if (target.kind === "drop-zone") {
    moveTabToNewPane(src.tabId, src.paneId, target.paneId, target.side);
  }
}

function onPointerCancel(): void {
  cleanup();
}

function cleanup(): void {
  source = null;
  active = false;
  hit = null;
  resetDragCursor();
  document.removeEventListener("pointermove", onPointerMove);
  document.removeEventListener("pointerup", onPointerUp);
  document.removeEventListener("pointercancel", onPointerCancel);
}

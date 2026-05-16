interface DynamicVirtualizerOpts {
  count: () => number;
  itemKey: (index: number) => string;
  estimateSize: (index: number) => number;
  overscanPx?: number;
}

export interface VirtualItem {
  index: number;
  key: string;
  start: number;
  size: number;
}

export interface DynamicVirtualizer {
  bindScrollEl: (el: HTMLElement | null) => void;
  onScroll: (e: Event) => void;
  setViewportHeight: (height: number) => void;
  measure: (index: number, height: number) => void;
  resetMeasurements: () => void;
  resetScroll: () => void;
  scrollToEnd: () => void;
  virtualItems: () => VirtualItem[];
  topSpacer: () => number;
  bottomSpacer: () => number;
  totalSize: () => number;
  scrollTop: () => number;
}

const DEFAULT_OVERSCAN_PX = 1000;

export function createDynamicVirtualizer(opts: DynamicVirtualizerOpts): DynamicVirtualizer {
  const overscanPx = opts.overscanPx ?? DEFAULT_OVERSCAN_PX;
  let scrollTop = $state(0);
  let viewportHeight = $state(0);
  let measurements = $state(new Map<string, number>());
  let scrollEl: HTMLElement | null = null;

  function sizeAt(index: number): number {
    const key = opts.itemKey(index);
    return measurements.get(key) ?? Math.max(1, opts.estimateSize(index));
  }

  const offsets = $derived.by(() => {
    const count = opts.count();
    const next = new Array<number>(count + 1);
    next[0] = 0;
    for (let i = 0; i < count; i += 1) {
      next[i + 1] = next[i] + sizeAt(i);
    }
    return next;
  });

  const totalSize = $derived(offsets[offsets.length - 1] ?? 0);

  function lowerBound(values: number[], target: number): number {
    let lo = 0;
    let hi = values.length - 1;
    while (lo < hi) {
      const mid = Math.floor((lo + hi) / 2);
      if (values[mid] < target) lo = mid + 1;
      else hi = mid;
    }
    return lo;
  }

  const virtualItems = $derived.by(() => {
    const count = opts.count();
    if (count === 0) return [];
    if (viewportHeight <= 0) {
      const size = sizeAt(0);
      return [{ index: 0, key: opts.itemKey(0), start: 0, size }];
    }

    const startOffset = Math.max(0, scrollTop - overscanPx);
    const endOffset = Math.min(totalSize, scrollTop + viewportHeight + overscanPx);
    const startIndex = Math.min(count - 1, lowerBound(offsets, startOffset));
    let endIndex = Math.min(count - 1, lowerBound(offsets, endOffset));
    if (offsets[endIndex] < endOffset) endIndex = Math.min(count - 1, endIndex + 1);

    const items: VirtualItem[] = [];
    for (let index = startIndex; index <= endIndex; index += 1) {
      items.push({
        index,
        key: opts.itemKey(index),
        start: offsets[index],
        size: sizeAt(index),
      });
    }
    return items;
  });

  const topSpacer = $derived(virtualItems[0]?.start ?? 0);
  const bottomSpacer = $derived.by(() => {
    const last = virtualItems[virtualItems.length - 1];
    if (!last) return 0;
    return Math.max(0, totalSize - last.start - last.size);
  });

  function syncFromElement(el: HTMLElement) {
    scrollTop = el.scrollTop;
    viewportHeight = el.clientHeight;
  }

  return {
    bindScrollEl: (el: HTMLElement | null) => {
      scrollEl = el;
      if (el) syncFromElement(el);
    },
    onScroll: (e: Event) => {
      const target = e.currentTarget as HTMLElement | null;
      if (target) scrollTop = target.scrollTop;
    },
    setViewportHeight: (height: number) => {
      viewportHeight = Math.max(0, height);
    },
    measure: (index: number, height: number) => {
      if (index < 0 || index >= opts.count()) return;
      const rounded = Math.max(1, Math.ceil(height));
      const key = opts.itemKey(index);
      if (measurements.get(key) === rounded) return;
      const next = new Map(measurements);
      next.set(key, rounded);
      measurements = next;
    },
    resetMeasurements: () => {
      measurements = new Map();
    },
    resetScroll: () => {
      scrollTop = 0;
      if (scrollEl) scrollEl.scrollTop = 0;
    },
    scrollToEnd: () => {
      if (!scrollEl) return;
      scrollEl.scrollTop = Math.max(0, totalSize - scrollEl.clientHeight);
      syncFromElement(scrollEl);
    },
    virtualItems: () => virtualItems,
    topSpacer: () => topSpacer,
    bottomSpacer: () => bottomSpacer,
    totalSize: () => totalSize,
    scrollTop: () => scrollTop,
  };
}

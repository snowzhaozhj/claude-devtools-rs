// 轻量自写虚拟滚动（windowing）。Svelte 5 runes 风格。
//
// 适用：固定行高的 flat 列表，scroll 容器外部维护，本模块只负责根据 scrollTop
// + containerHeight 计算可见区间和上下 spacer 高度。
//
// 设计选择：固定 itemHeight 而非 dynamic measurement，避免 ResizeObserver
// 与 Svelte 渲染时序竞争；列表一旦混入变高项需自行折算（如 header）。
// 详见 design.md decision 6。

interface VirtualWindowOpts {
  /** 总条目数（响应式 getter，每次 derived 重算时调用）。 */
  total: () => number;
  /** 单条目固定高度（px）。 */
  itemHeight: number;
  /** 视口外预渲染的 overscan 行数，默认 5。 */
  overscan?: number;
}

interface VirtualWindow {
  /** scroll 容器的 onscroll 回调，回写 scrollTop。 */
  onScroll: (e: Event) => void;
  /** 视口高度变化时回写（如窗口 resize）。 */
  setContainerHeight: (h: number) => void;
  /** 当前 scroll 容器引用（外部 bind:this 后回写）。 */
  bindScrollEl: (el: HTMLElement | null) => void;
  /** 起始可见 index（含 overscan）。 */
  startIndex: () => number;
  /** 结束可见 index（含 overscan，**不含**——半开区间 [start, end)）。 */
  endIndex: () => number;
  /** 上 spacer 高度（startIndex 之前不渲染部分）。 */
  topSpacer: () => number;
  /** 下 spacer 高度（endIndex 之后不渲染部分）。 */
  bottomSpacer: () => number;
}

export function createVirtualWindow(opts: VirtualWindowOpts): VirtualWindow {
  const overscan = opts.overscan ?? 5;
  let scrollTop = $state(0);
  let containerHeight = $state(0);
  let scrollEl: HTMLElement | null = null;

  const startIndex = $derived.by(() => {
    if (containerHeight <= 0) return 0;
    const raw = Math.floor(scrollTop / opts.itemHeight) - overscan;
    return Math.max(0, raw);
  });

  const endIndex = $derived.by(() => {
    const total = opts.total();
    if (total === 0 || containerHeight <= 0) return 0;
    const visibleCount = Math.ceil(containerHeight / opts.itemHeight);
    const raw = Math.ceil(scrollTop / opts.itemHeight) + visibleCount + overscan;
    return Math.min(total, raw);
  });

  const topSpacer = $derived(startIndex * opts.itemHeight);
  const bottomSpacer = $derived(
    Math.max(0, (opts.total() - endIndex) * opts.itemHeight),
  );

  return {
    onScroll: (e: Event) => {
      const target = e.currentTarget as HTMLElement | null;
      if (target) scrollTop = target.scrollTop;
    },
    setContainerHeight: (h: number) => {
      containerHeight = h;
    },
    bindScrollEl: (el: HTMLElement | null) => {
      scrollEl = el;
      if (el) {
        scrollTop = el.scrollTop;
        containerHeight = el.clientHeight;
      }
    },
    startIndex: () => startIndex,
    endIndex: () => endIndex,
    topSpacer: () => topSpacer,
    bottomSpacer: () => bottomSpacer,
  };
}

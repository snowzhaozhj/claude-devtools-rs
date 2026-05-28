<script lang="ts">
  import { tick, untrack } from "svelte";
  import { contextMenu } from "../contextMenu.svelte";
  import { buildWorktreeChipItems, type MenuItemContext } from "../contextMenu/menu-items";
  import { getMenuSettings } from "../contextMenu/settings.svelte";
  import { getMenuItemDispatch } from "../contextMenu/dispatch";

  export interface ChipOption {
    value: string;
    label: string;
    /** 可选：worktree 文件系统路径——有值时该 chip 弹"在终端打开 / 在编辑器打开
     *  / 复制路径"右键菜单（Phase 2 spec sidebar-navigation::worktree chip 右键菜单）。
     *  "全部"等聚合 chip 不传 path → 无右键菜单。 */
    path?: string;
    /** 可选：worktree 名称（仅 chip 自身 label 已含 `⌗` 前缀，独立 name 给菜单
     *  用于"复制项目名"等纯文本场景） */
    name?: string;
  }

  interface Props {
    value: string;
    options: ChipOption[];
    onChange: (v: string) => void;
    ariaLabel?: string;
  }

  let { value, options, onChange, ariaLabel }: Props = $props();

  function buildCtx(): MenuItemContext {
    return {
      sessionId: "",
      projectId: "",
      settings: getMenuSettings(),
      selectionText: window.getSelection()?.toString() ?? "",
      dispatch: getMenuItemDispatch(),
    };
  }

  /** 仅含 path 的 chip 才弹菜单——"全部"等聚合 chip 跳过 */
  function chipMenuProvider(opt: ChipOption) {
    return () => {
      if (!opt.path) return [];
      return buildWorktreeChipItems(
        { path: opt.path, name: opt.name ?? opt.label },
        buildCtx(),
      );
    };
  }

  let chipEls: HTMLButtonElement[] = $state([]);

  function selectAt(i: number) {
    const opt = options[i];
    if (!opt) return;
    if (opt.value !== value) onChange(opt.value);
  }

  async function focusAt(i: number) {
    await tick();
    chipEls[i]?.focus();
  }

  function onKeydown(e: KeyboardEvent, i: number) {
    if (e.key === "ArrowRight") {
      if (i >= options.length - 1) return;
      e.preventDefault();
      selectAt(i + 1);
      void focusAt(i + 1);
    } else if (e.key === "ArrowLeft") {
      if (i <= 0) return;
      e.preventDefault();
      selectAt(i - 1);
      void focusAt(i - 1);
    } else if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      selectAt(i);
    }
  }

  // --- Scroll arrow logic ---
  let canScrollLeft = $state(false);
  let canScrollRight = $state(false);
  let clusterEl: HTMLDivElement | undefined = $state(undefined);

  function updateOverflow() {
    if (!clusterEl) return;
    const { scrollLeft, scrollWidth, clientWidth } = clusterEl;
    canScrollLeft = scrollLeft > 2;
    canScrollRight = scrollWidth - clientWidth - scrollLeft > 2;
  }

  function scrollBy(dir: -1 | 1) {
    if (!clusterEl) return;
    const amount = clusterEl.clientWidth * 0.6;
    clusterEl.scrollBy({ left: dir * amount, behavior: "smooth" });
  }

  function onWheel(e: WheelEvent) {
    if (!clusterEl) return;
    const { scrollLeft, scrollWidth, clientWidth } = clusterEl;
    if (scrollWidth <= clientWidth) return;
    if (Math.abs(e.deltaY) > Math.abs(e.deltaX) && e.deltaX === 0) {
      const maxScroll = scrollWidth - clientWidth;
      const canScroll = (e.deltaY > 0 && scrollLeft < maxScroll) ||
                        (e.deltaY < 0 && scrollLeft > 0);
      if (!canScroll) return;
      e.preventDefault();
      clusterEl.scrollLeft += e.deltaY;
    }
  }

  function attachCluster(el: HTMLDivElement) {
    clusterEl = el;

    let rafId: number | undefined;
    const scheduleUpdate = () => {
      if (rafId != null) return;
      rafId = requestAnimationFrame(() => {
        rafId = undefined;
        updateOverflow();
      });
    };

    const ro = new ResizeObserver(scheduleUpdate);
    ro.observe(el);
    el.addEventListener("scroll", scheduleUpdate, { passive: true });

    scheduleUpdate();

    return () => {
      ro.disconnect();
      el.removeEventListener("scroll", scheduleUpdate);
      if (rafId != null) cancelAnimationFrame(rafId);
    };
  }

  // active chip 变更时 scrollIntoView（仅 value 真正变化时触发）
  let prevValue: string = untrack(() => value);
  $effect(() => {
    if (value === prevValue) return;
    prevValue = value;
    const activeIdx = options.findIndex((o) => o.value === value);
    if (activeIdx >= 0 && chipEls[activeIdx]) {
      chipEls[activeIdx].scrollIntoView({ inline: "nearest", behavior: "smooth", block: "nearest" });
    }
  });

  // options 变化时刷新 overflow 状态（如新增/删除 worktree）
  $effect(() => {
    void options.length;
    void tick().then(updateOverflow);
  });
</script>

<div class="chip-scroll-container">
  {#if canScrollLeft}
    <button
      type="button"
      class="scroll-arrow scroll-arrow-left"
      aria-label="向左滚动筛选项"
      tabindex={-1}
      onclick={() => scrollBy(-1)}
    >
      <svg width="10" height="10" viewBox="0 0 10 10" fill="none" aria-hidden="true">
        <path d="M6.5 2L3.5 5L6.5 8" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/>
      </svg>
    </button>
  {/if}

  <div
    class="worktree-chip-cluster"
    role="radiogroup"
    aria-label={ariaLabel}
    onwheel={onWheel}
    {@attach attachCluster}
  >
    {#each options as opt, i (opt.value)}
      <button
        type="button"
        class="worktree-chip"
        class:worktree-chip-active={opt.value === value}
        role="radio"
        aria-checked={opt.value === value}
        tabindex={opt.value === value ? 0 : -1}
        onclick={() => selectAt(i)}
        onkeydown={(e) => onKeydown(e, i)}
        bind:this={chipEls[i]}
        use:contextMenu={chipMenuProvider(opt)}
      >{opt.label}</button>
    {/each}
  </div>

  {#if canScrollRight}
    <button
      type="button"
      class="scroll-arrow scroll-arrow-right"
      aria-label="向右滚动筛选项"
      tabindex={-1}
      onclick={() => scrollBy(1)}
    >
      <svg width="10" height="10" viewBox="0 0 10 10" fill="none" aria-hidden="true">
        <path d="M3.5 2L6.5 5L3.5 8" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/>
      </svg>
    </button>
  {/if}
</div>

<style>
  .chip-scroll-container {
    position: relative;
    display: flex;
    align-items: center;
    min-width: 0;
  }

  .worktree-chip-cluster {
    display: flex;
    flex: 1;
    flex-wrap: nowrap;
    align-items: center;
    gap: 4px;
    overflow-x: auto;
    overflow-y: hidden;
    scrollbar-width: none;
    min-width: 0;
  }
  .worktree-chip-cluster::-webkit-scrollbar {
    display: none;
  }

  .scroll-arrow {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    margin: 0 -4px;
    border: none;
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
    border-radius: 4px;
    padding: 0;
    transition: color 0.12s, background-color 0.12s;
  }
  .scroll-arrow:hover {
    color: var(--color-text-secondary);
    background: var(--tool-item-hover-bg, var(--color-surface-overlay));
  }

  .worktree-chip {
    flex-shrink: 0;
    height: 24px;
    padding: 3px 10px;
    border-radius: 6px;
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--color-text-secondary);
    border: 1px solid transparent;
    background: transparent;
    cursor: pointer;
    white-space: nowrap;
    user-select: none;
    outline: none;
    transition: background-color 0.12s, color 0.12s, border-color 0.12s;
  }
  .worktree-chip:hover {
    background: var(--tool-item-hover-bg, var(--color-surface-overlay));
  }
  .worktree-chip-active {
    background: var(--color-surface-overlay);
    color: var(--color-text);
    border-color: var(--color-border-emphasis, var(--color-border));
  }
  .worktree-chip:focus-visible {
    outline: 2px solid var(--color-accent-blue);
    outline-offset: 1px;
  }
</style>

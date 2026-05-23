<!--
  AppContextMenu — 全应用右键菜单的通用浮层。

  spec: openspec/specs/frontend-context-menu/spec.md
  design.md D3 视觉 / D4 键盘 a11y / D6 关闭触发 / D7 portal

  使用方式：通过 `use:contextMenu` action（lib/contextMenu.svelte.ts）的 portal mount
  调用，外部不直接 render 这个组件。
-->
<script lang="ts" module>
  export interface ContextMenuItem {
    /** separator=true 时其它字段忽略 */
    separator?: boolean;
    label?: string;
    /** 可选 lucide path 字符串（Phase 1 不用，留 Phase 2）*/
    icon?: string;
    /** disabled 用 aria-disabled，不剔除 a11y 树 */
    disabled?: boolean;
    /** 危险动作（destructive），文字色染红 */
    danger?: boolean;
    /** 触发动作；disabled / separator 不需要 */
    action?: () => void;
    /** action 触发后短暂展示 feedback label，再关菜单（沿用 SessionContextMenu 600ms 复制反馈模式）*/
    feedback?: { label: string; durationMs?: number };
  }
</script>

<script lang="ts">
  interface Props {
    x: number;
    y: number;
    items: ContextMenuItem[];
    onClose: () => void;
  }

  const { x, y, items, onClose }: Props = $props();

  // ---------- 几何与定位 ----------
  // viewport 边界 clamp：菜单距 viewport 边 ≥ 8px。
  // 高度按"item 数 × 行高 + padding + separator"近似估算；不要求像素级精确，
  // overflow 仍由 max-height + scroll 保护（极少触发，菜单通常 ≤ 10 项）。
  const MENU_WIDTH = 220;
  const ITEM_HEIGHT = 30;
  const SEPARATOR_HEIGHT = 9;
  const PADDING = 8; // 4 上 + 4 下
  const EDGE_GAP = 8;

  const estimatedHeight = $derived(
    PADDING +
      items.reduce((sum, it) => sum + (it.separator ? SEPARATOR_HEIGHT : ITEM_HEIGHT), 0)
  );
  const clampedX = $derived(Math.max(EDGE_GAP, Math.min(x, window.innerWidth - MENU_WIDTH - EDGE_GAP)));
  const clampedY = $derived(
    Math.max(EDGE_GAP, Math.min(y, window.innerHeight - estimatedHeight - EDGE_GAP))
  );

  // ---------- 焦点环 ----------
  /** 第一个非 separator 的 item 索引，作为打开后初始 focus 目标 */
  const firstFocusableIndex = $derived(items.findIndex((it) => !it.separator));
  let activeIndex: number = $state(-1);

  /** 在 items 中按方向找下一个可 focus 的 index（跳过 separator，但**经过** disabled）*/
  function nextFocusable(from: number, dir: 1 | -1): number {
    if (items.length === 0) return -1;
    const n = items.length;
    let i = from;
    for (let step = 0; step < n; step += 1) {
      i = (i + dir + n) % n;
      if (!items[i].separator) return i;
    }
    return -1;
  }

  // ---------- 反馈态（"已复制!"600ms 后关闭）----------
  let feedbackIndex: number | null = $state(null);
  let feedbackText: string | null = $state(null);
  let feedbackTimer: ReturnType<typeof setTimeout> | null = null;

  function triggerAction(idx: number): void {
    const item = items[idx];
    if (!item || item.separator) return;
    if (item.disabled) return; // aria-disabled item Enter no-op
    item.action?.();
    if (item.feedback) {
      feedbackIndex = idx;
      feedbackText = item.feedback.label;
      const ms = item.feedback.durationMs ?? 600;
      feedbackTimer = setTimeout(() => {
        feedbackTimer = null;
        onClose();
      }, ms);
      return;
    }
    onClose();
  }

  // ---------- 键盘事件 ----------
  function handleKeyDown(e: KeyboardEvent): void {
    switch (e.key) {
      case "ArrowDown": {
        e.preventDefault();
        const next = nextFocusable(activeIndex < 0 ? -1 : activeIndex, 1);
        if (next >= 0) activeIndex = next;
        break;
      }
      case "ArrowUp": {
        e.preventDefault();
        const next = nextFocusable(activeIndex < 0 ? items.length : activeIndex, -1);
        if (next >= 0) activeIndex = next;
        break;
      }
      case "Home": {
        e.preventDefault();
        const idx = nextFocusable(-1, 1);
        if (idx >= 0) activeIndex = idx;
        break;
      }
      case "End": {
        e.preventDefault();
        const idx = nextFocusable(items.length, -1);
        if (idx >= 0) activeIndex = idx;
        break;
      }
      case "Enter":
      case " ": {
        if (activeIndex < 0) return;
        e.preventDefault();
        triggerAction(activeIndex);
        break;
      }
      case "Escape": {
        e.preventDefault();
        onClose();
        break;
      }
      case "Tab": {
        // Tab 不在菜单内移动；任何 Tab 都关菜单（让用户回到正常 chrome 焦点流）
        e.preventDefault();
        onClose();
        break;
      }
    }
  }

  // ---------- DOM 副作用：mount 后 focus 进第一项；destroy 清理定时器 ----------
  // 用 {@attach} 内聚（ui/CLAUDE.md::Svelte 5 陷阱）。
  function attachMenu(node: HTMLDivElement) {
    node.focus();
    // APG menu pattern：打开后立即 active 第一个非 separator menuitem
    if (firstFocusableIndex >= 0) activeIndex = firstFocusableIndex;
    return () => {
      if (feedbackTimer) {
        clearTimeout(feedbackTimer);
        feedbackTimer = null;
      }
    };
  }

  // 当 activeIndex 变化时把 focus 移到对应 item DOM 节点。
  let itemEls: Array<HTMLElement | null> = $state([]);
  $effect(() => {
    if (activeIndex >= 0) {
      const el = itemEls[activeIndex];
      if (el && document.activeElement !== el) el.focus({ preventScroll: true });
    }
  });
</script>

<!--
  role="menu" + aria-orientation 让屏读宣告"菜单 N 项"；tabindex=-1 让容器可
  接受 focus 但不进 Tab 序列。键盘 keydown 在容器上监听（冒泡 child item 也覆盖）。
-->
<div
  class="cm-root"
  role="menu"
  aria-orientation="vertical"
  tabindex="-1"
  style="left: {clampedX}px; top: {clampedY}px; min-width: {MENU_WIDTH}px;"
  onkeydown={handleKeyDown}
  {@attach attachMenu}
>
  {#each items as item, idx (idx)}
    {#if item.separator}
      <div class="cm-sep" role="separator"></div>
    {:else}
      <!-- 键盘交互（↑↓ Enter Esc）由容器 .cm-root 的 onkeydown 统一处理；item
           本身不需要也不应该重复挂 onkeydown（避免 Enter 双触发）。 -->
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <div
        class="cm-item"
        class:cm-item-disabled={item.disabled}
        class:cm-item-danger={item.danger}
        class:cm-item-active={activeIndex === idx}
        role="menuitem"
        tabindex="-1"
        aria-disabled={item.disabled ? "true" : undefined}
        bind:this={itemEls[idx]}
        onclick={() => triggerAction(idx)}
        onmouseenter={() => {
          if (!item.disabled) activeIndex = idx;
        }}
      >
        {feedbackIndex === idx && feedbackText !== null ? feedbackText : item.label ?? ""}
      </div>
    {/if}
  {/each}
</div>

<style>
  /*
   * 视觉 token 沿用现 SessionContextMenu / TabContextMenu（design.md D3）：
   * - bg: --color-surface
   * - 1px solid --color-border-emphasis
   * - 8px radius / 4px padding / 0 4px 16px rgba(0,0,0,.15) shadow
   *
   * 浮层 portal 到 document.body（design.md D7 / contextMenu.svelte.ts），
   * 所以 z-index 不需要太高（已无父 stacking context 干扰）；保留 100 与
   * 现有浮层（Dropdown / CommandPalette）档位一致。
   */
  .cm-root {
    position: fixed;
    z-index: 100;
    background: var(--color-surface);
    border: 1px solid var(--color-border-emphasis);
    border-radius: 8px;
    padding: 4px;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.15);
    /* outline:none 让容器自身 focus 不画 ring；item 自己负责 active 视觉 */
    outline: none;
  }

  .cm-item {
    display: block;
    width: 100%;
    padding: 7px 12px;
    background: none;
    border: none;
    border-radius: 4px;
    font: inherit;
    font-size: 13px;
    color: var(--color-text);
    text-align: left;
    cursor: pointer;
    transition: background 0.1s;
    user-select: none;
    -webkit-user-select: none;
  }

  /*
   * hover 与 keyboard active 共用同一态视觉（design.md D-V2：菜单 item 的瞬时
   * 焦点用 --tool-item-hover-bg，不沾 Focus Blue；键盘 active 额外加极淡 outline
   * 作为瞬时键盘焦点提示，是 a11y 必需的合规模式）。
   */
  .cm-item:hover:not(.cm-item-disabled),
  .cm-item-active:not(.cm-item-disabled) {
    background: var(--tool-item-hover-bg);
  }

  .cm-item-active {
    /* 仅键盘焦点时显眼一点；hover 没 outline 即可（避免双视觉抢戏）*/
    outline: 2px solid rgba(59, 130, 246, 0.15);
    outline-offset: -2px;
  }

  .cm-item-disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .cm-item-danger:not(.cm-item-disabled) {
    color: var(--color-danger);
  }

  .cm-item-danger:not(.cm-item-disabled):hover,
  .cm-item-danger.cm-item-active:not(.cm-item-disabled) {
    background: rgba(220, 38, 38, 0.08);
  }

  .cm-sep {
    height: 1px;
    margin: 4px 8px;
    background: var(--color-border);
  }
</style>

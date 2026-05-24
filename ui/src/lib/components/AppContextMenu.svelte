<!--
  AppContextMenu — 全应用右键菜单的通用浮层。

  spec: openspec/specs/frontend-context-menu/spec.md
    ::Requirement AppContextMenu submenu 渲染 / 视觉规格扩展 / ContextMenuItem 类型扩展
  design: D3 视觉 / D4 键盘 a11y / D6 关闭触发 / D7 portal / D-V2 shortcut hint
    / D-V4 submenu / D-V6 max-width / D-V7 暗色 submenu

  使用方式：通过 `use:contextMenu` action（lib/contextMenu.svelte.ts）的 portal mount
  调用，外部不直接 render 这个组件。Submenu 通过组件自身 self-import 递归渲染
  （不 mount 独立 instance——同一组件内 conditional render，position: fixed
  让 submenu 浮层脱离父级 layout 占位但保留同一 stacking context）。
-->
<script lang="ts" module>
  // ContextMenuItem 类型迁移到 ../contextMenu/types.ts（Phase 2 重构）
  // 这里 re-export 让外部 `import { ContextMenuItem } from "lib/components/AppContextMenu.svelte"`
  // 保持兼容；新代码 SHALL 直接 import from "lib/contextMenu/types"。
  export type { ContextMenuItem } from "../contextMenu/types";
</script>

<script lang="ts">
  import type { ContextMenuItem } from "../contextMenu/types";
  // self-import：submenu 用同一组件递归渲染。Svelte 5 + Vite 处理同文件 import 安全
  // （懒解析；不构成循环 import 加载死锁）。
  import Self from "./AppContextMenu.svelte";

  interface Props {
    x: number;
    y: number;
    items: ContextMenuItem[];
    /** 关闭本菜单实例。root 由 portal 调用方提供 unmount；submenu 由 parent
     *  传入"清除 submenuOpenIdx" closure。 */
    onClose: () => void;
    /** 当前嵌套深度。root=0；spec 渲染深度上限 2（depth=2 后忽略 submenu 字段）。 */
    depth?: number;
    /** 关闭整棵菜单树（Esc 时调用）。root 等价 onClose；submenu 由 parent 传入
     *  指向 root 的 onCloseTree。无传入时 fallback 到 onClose（仅根菜单情况）。 */
    onCloseTree?: () => void;
  }

  const { x, y, items, onClose, depth = 0, onCloseTree }: Props = $props();

  // ---------- 几何与定位 ----------
  // viewport 边界 clamp：菜单距 viewport 边 ≥ 8px。
  // 高度按"item 数 × 行高 + padding + separator"近似估算；不要求像素级精确，
  // overflow 仍由 max-height + scroll 保护（极少触发，菜单通常 ≤ 10 项）。
  const MENU_MIN_WIDTH = 200;
  const MENU_MAX_WIDTH = 320;
  // 用于 submenu 翻转判定的最大估算宽度
  const MENU_WIDTH_FOR_CLAMP = MENU_MAX_WIDTH;
  const ITEM_HEIGHT = 30;
  const SEPARATOR_HEIGHT = 9;
  const PADDING = 8; // 4 上 + 4 下
  const EDGE_GAP = 8;

  const estimatedHeight = $derived(
    PADDING +
      items.reduce((sum, it) => sum + (it.separator ? SEPARATOR_HEIGHT : ITEM_HEIGHT), 0)
  );
  const clampedX = $derived(Math.max(EDGE_GAP, Math.min(x, window.innerWidth - MENU_WIDTH_FOR_CLAMP - EDGE_GAP)));
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
    // 含 submenu：点击不调 action（chevron item 用于展开 submenu）；
    // ArrowRight / 200ms hover 走 openSubmenu 路径
    if (canSpawnSubmenu(item)) {
      openSubmenuFor(idx);
      return;
    }
    item.action?.();
    if (item.feedback) {
      feedbackIndex = idx;
      feedbackText = item.feedback.label;
      const ms = item.feedback.durationMs ?? 600;
      feedbackTimer = setTimeout(() => {
        feedbackTimer = null;
        // 关整树——feedback 是"动作完成"信号，整个菜单链都该收
        rootCloseTree();
      }, ms);
      return;
    }
    rootCloseTree();
  }

  // ---------- Submenu 状态机 ----------
  // 仅一个 submenu 同时打开（同 parent 的兄弟 item hover → 关旧开新）。
  // hover hysteresis：parent item hover 200ms 后展开；hover 兄弟 item 时立即
  // 取消 timer。不做"安全三角"——简化版（详 design.md::D-V4 风险段）。

  const SUBMENU_HOVER_MS = 200;
  let submenuOpenIdx: number | null = $state(null);
  let submenuOpenTimer: ReturnType<typeof setTimeout> | null = null;
  // 缓存 item DOM 引用——计算 submenu position 用 getBoundingClientRect
  let itemEls: Array<HTMLElement | null> = $state([]);

  function canSpawnSubmenu(item: ContextMenuItem): boolean {
    return depth < 2 && Array.isArray(item.submenu) && item.submenu.length > 0;
  }

  function scheduleOpenSubmenu(idx: number) {
    if (submenuOpenIdx === idx) return; // 已展开
    cancelSubmenuTimer();
    submenuOpenTimer = setTimeout(() => {
      submenuOpenTimer = null;
      openSubmenuFor(idx);
    }, SUBMENU_HOVER_MS);
  }

  function cancelSubmenuTimer() {
    if (submenuOpenTimer) {
      clearTimeout(submenuOpenTimer);
      submenuOpenTimer = null;
    }
  }

  function openSubmenuFor(idx: number) {
    cancelSubmenuTimer();
    const item = items[idx];
    if (!canSpawnSubmenu(item)) return;
    submenuOpenIdx = idx;
  }

  function closeSubmenu() {
    cancelSubmenuTimer();
    submenuOpenIdx = null;
  }

  /** 关闭整棵菜单树（Esc / item action 完成 / feedback 结束）。
   *  root 走 onClose（实际触发 portal unmount）；submenu 走 parent 传入的
   *  onCloseTree 链向上传递。*/
  function rootCloseTree() {
    if (onCloseTree) {
      onCloseTree();
    } else {
      // 兜底：root 没传 onCloseTree（典型路径），onClose 即等价整树关闭
      onClose();
    }
  }

  // submenu 几何：基于 parent item 的 bounding rect 决定位置 + viewport 翻转
  const submenuGeom = $derived.by(() => {
    if (submenuOpenIdx === null) return null;
    const parentEl = itemEls[submenuOpenIdx];
    if (!parentEl) return null;
    const r = parentEl.getBoundingClientRect();
    // 默认右展开：与 parent 右边缘重叠 4px 让视觉连续
    let sx = r.right - 4;
    let sy = r.top - 4; // 顶部对齐（parent item top - 4 让 submenu 视觉对齐 parent menu padding）
    // viewport 翻转：右边距不足 → 左展开
    if (sx + MENU_WIDTH_FOR_CLAMP + EDGE_GAP > window.innerWidth) {
      sx = r.left - MENU_WIDTH_FOR_CLAMP + 4;
    }
    return { x: sx, y: sy };
  });

  const openSubmenuItem = $derived(
    submenuOpenIdx !== null ? items[submenuOpenIdx] ?? null : null
  );

  // ---------- 键盘事件 ----------
  function handleKeyDown(e: KeyboardEvent): void {
    // submenu 已打开时大部分键由 submenu 内部处理——本菜单仅处理 ArrowLeft
    // （submenu 内传给本组件层）已无意义；ArrowLeft 在 submenu 自身 keydown 内
    // 调 onClose 关 submenu。本菜单仅在 submenu 关闭状态下处理键盘。
    if (submenuOpenIdx !== null && openSubmenuItem) {
      // 让事件冒泡到 submenu（submenu 在同一 dom tree 但不是子节点——通过
      // self-import 渲染的 submenu 是兄弟元素）。submenu 自身的 keydown 监听独立。
      // 这里仅处理 ArrowLeft（关 submenu）和 Escape（关整树）以兼容键盘焦点
      // 在父菜单元素时的边缘场景。
    }

    switch (e.key) {
      case "ArrowDown": {
        e.preventDefault();
        const next = nextFocusable(activeIndex < 0 ? -1 : activeIndex, 1);
        if (next >= 0) {
          activeIndex = next;
          // 切换到不同的 item 时取消 submenu 等待计时器；若新 item 也有 submenu，
          // 不主动开（仅 hover/ArrowRight 触发，避免方向键移动时菜单乱弹）
          cancelSubmenuTimer();
          // 若已开 submenu 不在当前 item，关闭它
          if (submenuOpenIdx !== null && submenuOpenIdx !== next) {
            closeSubmenu();
          }
        }
        break;
      }
      case "ArrowUp": {
        e.preventDefault();
        const next = nextFocusable(activeIndex < 0 ? items.length : activeIndex, -1);
        if (next >= 0) {
          activeIndex = next;
          cancelSubmenuTimer();
          if (submenuOpenIdx !== null && submenuOpenIdx !== next) {
            closeSubmenu();
          }
        }
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
      case "ArrowRight": {
        // ArrowRight on item with submenu → 即时打开 + focus 进首项
        if (activeIndex < 0) return;
        const item = items[activeIndex];
        if (canSpawnSubmenu(item)) {
          e.preventDefault();
          openSubmenuFor(activeIndex);
        }
        break;
      }
      case "ArrowLeft": {
        // ArrowLeft：仅在 submenu（depth > 0）时关闭自身回到 parent；root 上 no-op
        if (depth > 0) {
          e.preventDefault();
          onClose();
        }
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
        rootCloseTree();
        break;
      }
      case "Tab": {
        // Tab 不在菜单内移动；任何 Tab 都关菜单（让用户回到正常 chrome 焦点流）
        e.preventDefault();
        rootCloseTree();
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
      cancelSubmenuTimer();
    };
  }

  // 当 activeIndex 变化时把 focus 移到对应 item DOM 节点（仅当本菜单未被
  // submenu 接管 focus 时）。
  $effect(() => {
    if (submenuOpenIdx !== null) return; // submenu 拥有 focus，本菜单不抢
    if (activeIndex >= 0) {
      const el = itemEls[activeIndex];
      if (el && document.activeElement !== el) el.focus({ preventScroll: true });
    }
  });

  // 渲染时 label 优先级：feedback > pathLabel.short > label
  function renderLabel(item: ContextMenuItem, idx: number): string {
    if (feedbackIndex === idx && feedbackText !== null) return feedbackText;
    if (item.pathLabel) return item.pathLabel.short;
    return item.label ?? "";
  }
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
  data-cm-depth={depth}
  style="left: {clampedX}px; top: {clampedY}px;"
  onkeydown={handleKeyDown}
  {@attach attachMenu}
>
  {#each items as item, idx (idx)}
    {#if item.separator}
      <div class="cm-sep" role="separator"></div>
    {:else}
      {@const hasSubmenu = canSpawnSubmenu(item)}
      <!-- 键盘交互（↑↓ Enter Esc ArrowRight）由容器 .cm-root 的 onkeydown 统一
           处理；item 本身不需要也不应该重复挂 onkeydown（避免 Enter 双触发）。-->
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <div
        class="cm-item"
        class:cm-item-disabled={item.disabled}
        class:cm-item-danger={item.danger}
        class:cm-item-active={activeIndex === idx || submenuOpenIdx === idx}
        class:cm-item-has-submenu={hasSubmenu}
        role="menuitem"
        tabindex="-1"
        aria-disabled={item.disabled ? "true" : undefined}
        aria-haspopup={hasSubmenu ? "menu" : undefined}
        aria-expanded={hasSubmenu ? (submenuOpenIdx === idx ? "true" : "false") : undefined}
        title={item.pathLabel?.full ?? undefined}
        bind:this={itemEls[idx]}
        onclick={() => triggerAction(idx)}
        onmouseenter={() => {
          if (item.disabled) return;
          activeIndex = idx;
          if (hasSubmenu) {
            scheduleOpenSubmenu(idx);
          } else if (submenuOpenIdx !== null) {
            // hover 到无 submenu 的兄弟 item → 关已开 submenu
            closeSubmenu();
          }
        }}
        onmouseleave={() => {
          // 离开 item 时 cancel pending open；不主动关已开 submenu，让用户能
          // 用鼠标横穿 gap 进入 submenu 区域（简化 hysteresis）
          if (hasSubmenu && submenuOpenIdx !== idx) {
            cancelSubmenuTimer();
          }
        }}
      >
        <span class="cm-item-label">{renderLabel(item, idx)}</span>
        {#if hasSubmenu}
          <span class="cm-item-chevron" aria-hidden="true">›</span>
        {:else if item.shortcut}
          <span class="cm-item-shortcut" aria-hidden="true">{item.shortcut}</span>
        {/if}
      </div>
    {/if}
  {/each}
</div>

<!--
  Submenu：自身组件递归渲染（self-import）。position: fixed 让 submenu 浮层脱离
  父 menu 的 box，按 viewport 坐标定位；视觉规格与父菜单完全相同（同 bg/border/
  radius/shadow——D-V7"submenu 不加深 bg"）。
  spec: AppContextMenu submenu 渲染::Scenario "submenu 视觉与父菜单完全一致"
-->
{#if openSubmenuItem && submenuGeom}
  <Self
    items={openSubmenuItem.submenu ?? []}
    x={submenuGeom.x}
    y={submenuGeom.y}
    depth={depth + 1}
    onClose={closeSubmenu}
    onCloseTree={rootCloseTree}
  />
{/if}

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
   *
   * Phase 2 D-V6：min-width 200px / max-width 320px + 长 label 末段 ellipsis
   * fallback（路径类 item 用 pathLabel JS 中段截断 + title tooltip 优先）。
   * Phase 2 D-V7：submenu 视觉与父菜单完全一致——bg/border/radius/shadow 不加深，
   * data-cm-depth 仅作 hook 不施加额外 style，由空间位移本身提供层次。
   */
  .cm-root {
    position: fixed;
    z-index: 100;
    min-width: 200px;
    max-width: var(--cm-max-width);
    background: var(--color-surface);
    border: 1px solid var(--color-border-emphasis);
    border-radius: 8px;
    padding: 4px;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.15);
    /* outline:none 让容器自身 focus 不画 ring；item 自己负责 active 视觉 */
    outline: none;
  }

  .cm-item {
    display: flex;
    align-items: center;
    gap: 8px;
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
    /* min-width: 0 让 flex 子元素允许 shrink 触发 ellipsis（CSS flex 默认 min-content） */
    min-width: 0;
  }

  .cm-item-label {
    /* 长 label 末段 ellipsis（D-V6 fallback；路径类用 pathLabel JS 中段截断） */
    flex: 1 1 auto;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /*
   * Shortcut hint（D-V2）：item 行内右对齐 + muted/mono 最低视觉权重。
   * 与 chevron 互斥（chevron 优先；hasSubmenu 时不渲染 shortcut）。
   */
  .cm-item-shortcut {
    flex-shrink: 0;
    margin-left: auto;
    padding-left: 16px;
    color: var(--cm-shortcut-color);
    /* `--cm-shortcut-font` 是 CSS font shorthand `<size> <family>`（详 app.css 注释）；
     * shorthand 应用时 `font-weight` 会被 reset 为 normal——刚好对应组件预期 400，
     * 但显式 font-weight 在 shorthand 后再覆盖一次确保未来 shorthand 改值不破。 */
    font: var(--cm-shortcut-font);
    font-weight: 400;
    /* 跟 label 同行不换行 */
    white-space: nowrap;
  }

  /*
   * Submenu chevron（D-V4）：结构性 affordance（"有子菜单"），不是装饰。
   * 与 shortcut hint 同位置但视觉更显眼少许（`›` U+203A 单字符宽度小，
   * font-weight 略加 + muted 色保持低权重）。
   */
  .cm-item-chevron {
    flex-shrink: 0;
    margin-left: auto;
    padding-left: 16px;
    color: var(--color-text-muted);
    font-size: 14px;
    line-height: 1;
    font-weight: 500;
  }

  /*
   * hover 与 keyboard active 共用同一态视觉（design.md D-V2：菜单 item 的瞬时
   * 焦点用 --tool-item-hover-bg，不沾 Focus Blue；键盘 active 额外加极淡 outline
   * 作为瞬时键盘焦点提示，是 a11y 必需的合规模式）。
   *
   * Submenu 已展开时（cm-item-active 由 submenuOpenIdx 也触发）parent item
   * 保持 hover bg 锁定，让用户感知"哪个 item 当前对应已开 submenu"。
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

<script lang="ts">
  import { onDestroy, tick } from "svelte";
  import { CHEVRON_DOWN } from "../icons";

  export interface DropdownOption {
    value: string;
    label: string;
  }

  interface Props {
    value: string;
    options: DropdownOption[];
    onChange: (v: string) => void;
    ariaLabel?: string;
    size?: "md" | "sm";
    minWidth?: number;
    disabled?: boolean;
  }

  let {
    value,
    options,
    onChange,
    ariaLabel,
    size = "md",
    minWidth,
    disabled = false,
  }: Props = $props();

  let anchorEl: HTMLButtonElement | undefined = $state(undefined);
  let popoverEl: HTMLDivElement | undefined = $state(undefined);
  let open = $state(false);
  let highlightIdx = $state(-1);
  let popoverStyle = $state("");

  const selectedIdx = $derived(options.findIndex((o) => o.value === value));
  const selectedLabel = $derived(
    selectedIdx >= 0 ? options[selectedIdx].label : "",
  );
  const effectiveMinWidth = $derived(minWidth ?? (size === "sm" ? 0 : 180));

  function placePopover() {
    if (!anchorEl) return;
    const r = anchorEl.getBoundingClientRect();
    const gap = 4;
    const margin = 8;
    const vw = window.innerWidth;
    const vh = window.innerHeight;

    const width = Math.max(r.width, effectiveMinWidth);
    const itemH = size === "sm" ? 28 : 32;
    const pad = 8;
    const estHeight = Math.min(options.length * itemH + pad, 320);

    const spaceBelow = vh - r.bottom - margin;
    const spaceAbove = r.top - margin;
    const placeAbove = spaceBelow < estHeight && spaceAbove > spaceBelow;

    let top: number;
    if (placeAbove) {
      const h = Math.min(estHeight, spaceAbove);
      top = Math.max(margin, r.top - h - gap);
    } else {
      top = r.bottom + gap;
    }

    let left = r.left;
    if (left + width > vw - margin) left = Math.max(margin, vw - width - margin);

    const availH = placeAbove ? r.top - margin - gap : vh - top - margin;
    const maxH = Math.max(0, availH);

    popoverStyle =
      `position: fixed; top: ${top}px; left: ${left}px; ` +
      `min-width: ${width}px; max-height: ${maxH}px;`;
  }

  async function openMenu() {
    if (disabled || open) return;
    highlightIdx = selectedIdx >= 0 ? selectedIdx : 0;
    // 必须在 `open = true` 之前同步算好 popoverStyle，否则首次打开时
    // popover 会先以空 style 渲染一帧（默认 top/left 在视口角落）才被重定位
    placePopover();
    open = true;
    await tick();
    // popover 已渲染，按高亮项滚动定位
    scrollHighlightIntoView();
  }

  function closeMenu(returnFocus = true) {
    if (!open) return;
    open = false;
    if (returnFocus && anchorEl) anchorEl.focus();
  }

  function pick(idx: number) {
    const opt = options[idx];
    if (!opt) return;
    onChange(opt.value);
    closeMenu(true);
  }

  function onAnchorKeydown(e: KeyboardEvent) {
    if (disabled) return;
    if (!open) {
      if (e.key === "Enter" || e.key === " " || e.key === "ArrowDown" || e.key === "ArrowUp") {
        e.preventDefault();
        openMenu();
      }
      return;
    }
    if (e.key === "Escape") {
      e.preventDefault();
      closeMenu(true);
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      highlightIdx = (highlightIdx + 1) % options.length;
      scrollHighlightIntoView();
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      highlightIdx = (highlightIdx - 1 + options.length) % options.length;
      scrollHighlightIntoView();
    } else if (e.key === "Home") {
      e.preventDefault();
      highlightIdx = 0;
      scrollHighlightIntoView();
    } else if (e.key === "End") {
      e.preventDefault();
      highlightIdx = options.length - 1;
      scrollHighlightIntoView();
    } else if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      pick(highlightIdx);
    } else if (e.key === "Tab") {
      closeMenu(false);
    }
  }

  function scrollHighlightIntoView() {
    if (!popoverEl) return;
    const items = popoverEl.querySelectorAll<HTMLElement>(".dd-opt");
    const el = items[highlightIdx];
    if (el && typeof el.scrollIntoView === "function") {
      el.scrollIntoView({ block: "nearest" });
    }
  }

  function onDocMouseDown(e: MouseEvent) {
    if (!open) return;
    const t = e.target as Node;
    if (anchorEl?.contains(t) || popoverEl?.contains(t)) return;
    closeMenu(false);
  }

  function onWindowBlur() {
    closeMenu(false);
  }

  function onScrollOrResize() {
    if (!open) return;
    if (anchorEl) {
      const r = anchorEl.getBoundingClientRect();
      if (r.bottom < 0 || r.top > window.innerHeight) {
        closeMenu(false);
        return;
      }
    }
    placePopover();
  }

  $effect(() => {
    if (open) {
      document.addEventListener("mousedown", onDocMouseDown, true);
      window.addEventListener("blur", onWindowBlur);
      window.addEventListener("resize", onScrollOrResize);
      window.addEventListener("scroll", onScrollOrResize, true);
      return () => {
        document.removeEventListener("mousedown", onDocMouseDown, true);
        window.removeEventListener("blur", onWindowBlur);
        window.removeEventListener("resize", onScrollOrResize);
        window.removeEventListener("scroll", onScrollOrResize, true);
      };
    }
  });

  onDestroy(() => {
    document.removeEventListener("mousedown", onDocMouseDown, true);
    window.removeEventListener("blur", onWindowBlur);
    window.removeEventListener("resize", onScrollOrResize);
    window.removeEventListener("scroll", onScrollOrResize, true);
  });
</script>

<button
  type="button"
  class="dd-anchor"
  class:dd-anchor-sm={size === "sm"}
  class:dd-anchor-open={open}
  bind:this={anchorEl}
  aria-haspopup="listbox"
  aria-expanded={open}
  aria-label={ariaLabel}
  {disabled}
  onclick={() => (open ? closeMenu(false) : openMenu())}
  onkeydown={onAnchorKeydown}
  style={effectiveMinWidth ? `min-width: ${effectiveMinWidth}px;` : ""}
>
  <span class="dd-anchor-label">{selectedLabel}</span>
  <svg
    class="dd-anchor-chevron"
    class:dd-anchor-chevron-open={open}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="2"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
  >
    <path d={CHEVRON_DOWN} />
  </svg>
</button>

{#if open}
  <div
    class="dd-popover"
    class:dd-popover-sm={size === "sm"}
    bind:this={popoverEl}
    role="listbox"
    aria-label={ariaLabel}
    style={popoverStyle}
  >
    {#each options as opt, i (opt.value)}
      <button
        type="button"
        class="dd-opt"
        class:dd-opt-active={i === highlightIdx}
        class:dd-opt-selected={opt.value === value}
        role="option"
        aria-selected={opt.value === value}
        onmouseenter={() => (highlightIdx = i)}
        onclick={() => pick(i)}
      >
        <span class="dd-opt-check">
          {#if opt.value === value}✓{/if}
        </span>
        <span class="dd-opt-label">{opt.label}</span>
      </button>
    {/each}
  </div>
{/if}

<style>
  .dd-anchor {
    display: inline-flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    height: 30px;
    padding: 0 8px 0 10px;
    border: 1px solid var(--color-border);
    border-radius: 6px;
    background: var(--color-surface);
    color: var(--color-text);
    font: inherit;
    font-size: 13px;
    cursor: pointer;
    outline: none;
    transition: border-color 0.12s, box-shadow 0.12s;
  }
  .dd-anchor:hover:not(:disabled) {
    border-color: var(--color-border-emphasis, var(--color-border));
  }
  .dd-anchor:focus-visible,
  .dd-anchor-open {
    border-color: var(--color-switch-on);
    box-shadow: 0 0 0 3px color-mix(in oklch, var(--color-switch-on) 18%, transparent);
  }
  .dd-anchor:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }
  .dd-anchor-sm {
    height: 24px;
    padding: 0 4px 0 6px;
    font-size: 11px;
    gap: 4px;
    border-radius: 5px;
    background: var(--color-surface-overlay, var(--badge-neutral-bg));
    border-color: transparent;
    color: var(--color-text-secondary);
  }
  .dd-anchor-sm:hover:not(:disabled) {
    border-color: var(--color-border);
  }
  .dd-anchor-label {
    flex: 1;
    text-align: left;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .dd-anchor-chevron {
    width: 14px;
    height: 14px;
    flex-shrink: 0;
    color: var(--color-text-muted);
    transition: transform 0.15s;
  }
  .dd-anchor-chevron-open {
    transform: rotate(180deg);
  }
  .dd-anchor-sm .dd-anchor-chevron {
    width: 12px;
    height: 12px;
  }

  .dd-popover {
    z-index: 1000;
    box-sizing: border-box;
    padding: 4px;
    background: var(--color-surface);
    border: 1px solid var(--color-border-emphasis, var(--color-border));
    border-radius: 8px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.18);
    overflow-y: auto;
    /* scrollbar-gutter-exempt: 浮层打开即定尺寸，滚动条首帧即在，无生命周期内宽度跳变 */
    overscroll-behavior: contain;
  }
  .dd-popover-sm {
    padding: 3px;
    border-radius: 6px;
  }

  .dd-opt {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px 8px 6px 6px;
    background: none;
    border: none;
    border-radius: 4px;
    font: inherit;
    font-size: 13px;
    color: var(--color-text);
    text-align: left;
    cursor: pointer;
  }
  .dd-popover-sm .dd-opt {
    padding: 4px 6px 4px 4px;
    font-size: 11px;
  }
  .dd-opt-check {
    flex-shrink: 0;
    width: 14px;
    text-align: center;
    color: var(--color-switch-on);
    font-size: 12px;
    line-height: 1;
  }
  .dd-popover-sm .dd-opt-check {
    width: 12px;
    font-size: 10px;
  }
  .dd-opt-label {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .dd-opt-active {
    background: var(--tool-item-hover-bg, var(--color-surface-overlay));
  }
  .dd-opt-selected {
    color: var(--color-text);
    font-weight: 500;
  }
</style>

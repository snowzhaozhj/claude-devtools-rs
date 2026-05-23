<script lang="ts">
  import { onDestroy, tick, untrack } from "svelte";
  import { openPath } from "@tauri-apps/plugin-opener";
  import { isTauriRuntime } from "../lib/runtime";
  import { MORE_HORIZONTAL_SVG } from "../lib/icons";

  interface Props {
    cwd: string | undefined;
    sessionId: string;
  }
  let { cwd, sessionId }: Props = $props();

  type ItemKey = "open-finder" | "copy-cwd" | "copy-id";
  type FeedbackKind = "copied" | "open-fail" | "copy-fail";

  let triggerEl: HTMLButtonElement | undefined = $state();
  let menuEl: HTMLDivElement | undefined = $state();
  let open = $state(false);
  let highlightIdx = $state(-1);
  let menuStyle = $state("");
  let toastStyle = $state("");
  let feedback: FeedbackKind | null = $state(null);
  let feedbackTimer: ReturnType<typeof setTimeout> | null = null;

  const menuId = untrack(() => `session-meta-menu-${sessionId.slice(0, 8)}`);
  const tauri = isTauriRuntime();

  const items = $derived.by(() => {
    const cwdAvailable = typeof cwd === "string" && cwd.length > 0;
    const list: Array<{ key: ItemKey; label: string; disabled: boolean }> = [];
    if (tauri) {
      list.push({
        key: "open-finder",
        label: navigator.platform.startsWith("Mac") ? "在 Finder 中打开" : "在文件管理器中打开",
        disabled: !cwdAvailable,
      });
    }
    list.push({ key: "copy-cwd", label: "复制工作目录路径", disabled: !cwdAvailable });
    list.push({ key: "copy-id", label: "复制 Session ID", disabled: false });
    return list;
  });

  const enabledIndices = $derived(
    items.map((it, i) => (it.disabled ? -1 : i)).filter((i) => i >= 0),
  );

  function placeMenu() {
    if (!triggerEl) return;
    const r = triggerEl.getBoundingClientRect();
    const gap = 4;
    const margin = 8;
    const vw = window.innerWidth;
    let right = vw - r.right;
    // 防左溢出：测 menu 实际宽度，确保 left 边界 ≥ margin。
    // left = vw - right - width；left ≥ margin → right ≤ vw - width - margin。
    // menuEl 在首次 placeMenu 已挂载（openMenu await tick 后调用）；
    // 取不到时 fallback 到 max-width 260（CSS 上限）保守估计。
    const width = menuEl?.getBoundingClientRect().width ?? 260;
    const maxRight = vw - width - margin;
    if (right > maxRight) right = Math.max(maxRight, margin);
    if (right < margin) right = margin;
    menuStyle = `top: ${r.bottom + gap}px; right: ${right}px;`;
  }

  function placeToast() {
    if (!triggerEl) return;
    const r = triggerEl.getBoundingClientRect();
    const gap = 4;
    const margin = 8;
    const vw = window.innerWidth;
    let right = vw - r.right;
    // toast 比 menu 短（~60-80px），左溢出概率低，但仍 clamp 保险
    if (right < margin) right = margin;
    toastStyle = `top: ${r.bottom + gap}px; right: ${right}px;`;
  }

  async function openMenu() {
    open = true;
    highlightIdx = enabledIndices[0] ?? -1;
    await tick();
    placeMenu();
    menuEl?.focus();
  }

  function closeMenu() {
    if (!open) return;
    open = false;
    triggerEl?.focus();
  }

  function toggleMenu() {
    if (open) closeMenu();
    else openMenu();
  }

  function setFeedback(kind: FeedbackKind) {
    if (feedbackTimer) clearTimeout(feedbackTimer);
    feedback = kind;
    placeToast();
    feedbackTimer = setTimeout(() => {
      feedback = null;
      feedbackTimer = null;
    }, 1500);
  }

  async function runItem(key: ItemKey) {
    closeMenu();
    if (key === "open-finder") {
      if (!cwd) return;
      try {
        await openPath(cwd);
      } catch (e) {
        console.warn("[SessionMetaMenu] openPath failed:", e);
        setFeedback("open-fail");
      }
    } else if (key === "copy-cwd") {
      if (!cwd) return;
      try {
        await navigator.clipboard.writeText(cwd);
        setFeedback("copied");
      } catch (e) {
        console.warn("[SessionMetaMenu] copy cwd failed:", e);
        setFeedback("copy-fail");
      }
    } else {
      try {
        await navigator.clipboard.writeText(sessionId);
        setFeedback("copied");
      } catch (e) {
        console.warn("[SessionMetaMenu] copy sessionId failed:", e);
        setFeedback("copy-fail");
      }
    }
  }

  function onKeydown(e: KeyboardEvent) {
    if (!open) return;
    if (e.key === "Escape") {
      e.preventDefault();
      closeMenu();
      return;
    }
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      if (enabledIndices.length === 0) return;
      const cur = enabledIndices.indexOf(highlightIdx);
      const len = enabledIndices.length;
      const next =
        e.key === "ArrowDown"
          ? enabledIndices[(cur + 1) % len]
          : enabledIndices[(cur - 1 + len) % len];
      highlightIdx = next;
      return;
    }
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      const it = items[highlightIdx];
      if (it && !it.disabled) runItem(it.key);
    }
  }

  function onWindowMousedown(e: MouseEvent) {
    if (!open) return;
    const t = e.target as Node | null;
    if (!t) return;
    if (triggerEl && triggerEl.contains(t)) return;
    if (menuEl && menuEl.contains(t)) return;
    closeMenu();
  }

  function onViewportChange() {
    if (open) placeMenu();
    if (feedback) placeToast();
  }

  $effect(() => {
    if (!open && !feedback) return;
    window.addEventListener("mousedown", onWindowMousedown);
    window.addEventListener("resize", onViewportChange);
    window.addEventListener("scroll", onViewportChange, true);
    return () => {
      window.removeEventListener("mousedown", onWindowMousedown);
      window.removeEventListener("resize", onViewportChange);
      window.removeEventListener("scroll", onViewportChange, true);
    };
  });

  onDestroy(() => {
    if (feedbackTimer) clearTimeout(feedbackTimer);
  });

  function feedbackText(kind: FeedbackKind): string {
    if (kind === "copied") return "已复制";
    if (kind === "open-fail") return "打开失败";
    return "复制失败";
  }
</script>

<svelte:window onkeydown={onKeydown} />

<button
  bind:this={triggerEl}
  type="button"
  class="meta-trigger"
  class:meta-trigger-active={open}
  aria-haspopup="menu"
  aria-expanded={open}
  aria-controls={menuId}
  aria-label="会话操作"
  onclick={toggleMenu}
>
  <svg
    class="meta-trigger-icon"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="2"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
  >
    {@html MORE_HORIZONTAL_SVG}
  </svg>
</button>

{#if open}
  <div
    bind:this={menuEl}
    id={menuId}
    role="menu"
    aria-orientation="vertical"
    class="meta-menu"
    style={menuStyle}
    tabindex="-1"
  >
    {#each items as item, i (item.key)}
      {#if tauri && item.key === "copy-id"}
        <!-- 仅 Tauri mode（含 open-finder 项）才在 copy-id 前渲染分隔线；
             HTTP server mode 只有两项 copy 操作，无视觉分组必要 -->
        <div class="meta-menu-sep" role="separator"></div>
      {/if}
      <button
        type="button"
        role="menuitem"
        class="meta-menu-item"
        class:meta-menu-item-highlight={highlightIdx === i && !item.disabled}
        aria-disabled={item.disabled || undefined}
        tabindex={item.disabled ? -1 : 0}
        onclick={() => !item.disabled && runItem(item.key)}
      >
        <span class="meta-menu-label">{item.label}</span>
      </button>
    {/each}
  </div>
{/if}

{#if feedback}
  <div
    class="meta-toast"
    class:meta-toast-error={feedback !== "copied"}
    style={toastStyle}
    role="status"
    aria-live="polite"
  >
    {feedbackText(feedback)}
  </div>
{/if}

<style>
  .meta-trigger {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 6px 8px;
    background: transparent;
    border: 1px solid transparent;
    border-radius: 6px;
    cursor: pointer;
    font-family: inherit;
    transition: background 120ms ease-out, border-color 120ms ease-out;
  }

  .meta-trigger-icon {
    width: 13px;
    height: 13px;
    color: var(--color-text-muted);
    flex-shrink: 0;
    transition: color 120ms ease-out;
  }

  .meta-trigger:hover {
    background: var(--color-surface-raised);
    border-color: var(--color-border);
  }
  .meta-trigger:hover .meta-trigger-icon {
    color: var(--color-text);
  }
  .meta-trigger:focus-visible {
    outline: 2px solid color-mix(in oklch, var(--color-accent-blue) 50%, transparent);
    outline-offset: 1px;
  }
  .meta-trigger-active {
    background: var(--color-surface-overlay);
    border-color: var(--color-border-emphasis);
  }
  .meta-trigger-active .meta-trigger-icon {
    color: var(--color-text);
  }

  .meta-menu {
    position: fixed;
    z-index: 200;
    min-width: 180px;
    max-width: 260px;
    padding: 4px;
    background: var(--color-surface-raised);
    border: 1px solid var(--color-border-emphasis);
    border-radius: 8px;
    box-shadow: 0 4px 14px rgba(0, 0, 0, 0.12);
    outline: none;
    animation: meta-menu-in 150ms cubic-bezier(0.16, 1, 0.3, 1);
  }

  :global([data-theme="dark"]) .meta-menu {
    box-shadow: 0 4px 14px rgba(0, 0, 0, 0.25);
  }

  @keyframes meta-menu-in {
    0% {
      opacity: 0;
      transform: translateY(-2px);
    }
    100% {
      opacity: 1;
      transform: translateY(0);
    }
  }

  .meta-menu-item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px 12px;
    background: transparent;
    border: none;
    border-radius: 4px;
    color: var(--color-text);
    font-size: 13px;
    font-family: inherit;
    text-align: left;
    cursor: pointer;
    transition: background 100ms ease-out;
  }

  .meta-menu-item:hover:not([aria-disabled]),
  .meta-menu-item.meta-menu-item-highlight {
    background: var(--color-surface-raised);
  }

  .meta-menu-item:active:not([aria-disabled]) {
    background: var(--color-surface-overlay);
  }

  .meta-menu-item[aria-disabled] {
    color: var(--color-text-muted);
    cursor: not-allowed;
  }

  .meta-menu-sep {
    height: 1px;
    background: var(--color-border-subtle);
    margin: 4px 4px;
  }

  .meta-toast {
    position: fixed;
    z-index: 200;
    font-size: 11px;
    padding: 4px 8px;
    border-radius: 4px;
    background: var(--color-surface-overlay);
    border: 1px solid var(--color-border);
    color: var(--color-text-secondary);
    white-space: nowrap;
    pointer-events: none;
    animation: meta-toast-in 100ms ease-out;
  }

  .meta-toast-error {
    color: var(--color-danger);
  }

  @keyframes meta-toast-in {
    0% {
      opacity: 0;
      transform: translateY(-2px);
    }
    100% {
      opacity: 1;
      transform: translateY(0);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .meta-menu,
    .meta-toast {
      animation: none;
    }
    .meta-trigger,
    .meta-trigger-icon,
    .meta-menu-item {
      transition: none;
    }
  }
</style>

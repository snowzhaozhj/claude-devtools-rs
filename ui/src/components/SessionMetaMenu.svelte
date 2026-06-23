<script lang="ts">
  import { onDestroy, tick, untrack } from "svelte";
  import { openPath } from "@tauri-apps/plugin-opener";
  import { isTauriRuntime } from "../lib/runtime";
  import {
    MORE_HORIZONTAL_SVG,
    EXTERNAL_LINK_SVG,
    COPY_SVG,
    HASH_SVG,
    FILE_TEXT_SVG,
    BRACES_SVG,
    CODE_SVG,
    CHECK_SVG,
    ALERT_CIRCLE_SVG,
  } from "../lib/icons";
  import { getSessionDetailForExport } from "../lib/api";
  import type { ExportFormat } from "../lib/export";
  import { exportSession, getExportFileName, getExportFilterExt } from "../lib/export";
  import { getTransport } from "../lib/transport";

  interface Props {
    cwd: string | undefined;
    sessionId: string;
    projectId: string;
  }
  let { cwd, sessionId, projectId }: Props = $props();

  type ItemKey = "open-finder" | "copy-cwd" | "copy-id" | "export-md" | "export-json" | "export-html";
  type FeedbackKind = "copied" | "open-fail" | "copy-fail" | "exported" | "export-fail";

  let triggerEl: HTMLButtonElement | undefined = $state();
  let menuEl: HTMLDivElement | undefined = $state();
  let open = $state(false);
  let highlightIdx = $state(-1);
  let menuStyle = $state("");
  let toastStyle = $state("");
  let feedback: FeedbackKind | null = $state(null);
  let toastExiting = $state(false);
  let feedbackTimer: ReturnType<typeof setTimeout> | null = null;
  let exitTimer: ReturnType<typeof setTimeout> | null = null;
  let itemEls: HTMLButtonElement[] = $state([]);

  const menuId = untrack(() => `session-meta-menu-${sessionId.slice(0, 8)}`);
  const tauri = isTauriRuntime();

  let exporting = $state(false);

  const ITEM_ICONS: Record<ItemKey, string> = {
    "open-finder": EXTERNAL_LINK_SVG,
    "copy-cwd": COPY_SVG,
    "copy-id": HASH_SVG,
    "export-md": FILE_TEXT_SVG,
    "export-json": BRACES_SVG,
    "export-html": CODE_SVG,
  };

  const items = $derived.by(() => {
    const cwdAvailable = typeof cwd === "string" && cwd.length > 0;
    const list: Array<{ key: ItemKey; label: string; disabled: boolean; icon: string }> = [];
    if (tauri) {
      list.push({
        key: "open-finder",
        label: navigator.platform.startsWith("Mac") ? "在 Finder 中打开" : "在文件管理器中打开",
        disabled: !cwdAvailable,
        icon: ITEM_ICONS["open-finder"],
      });
    }
    list.push({ key: "copy-cwd", label: "复制工作目录路径", disabled: !cwdAvailable, icon: ITEM_ICONS["copy-cwd"] });
    list.push({ key: "copy-id", label: "复制 Session ID", disabled: false, icon: ITEM_ICONS["copy-id"] });
    list.push({ key: "export-md", label: exporting ? "导出中..." : "导出为 Markdown", disabled: exporting, icon: ITEM_ICONS["export-md"] });
    list.push({ key: "export-json", label: exporting ? "导出中..." : "导出为 JSON", disabled: exporting, icon: ITEM_ICONS["export-json"] });
    list.push({ key: "export-html", label: exporting ? "导出中..." : "导出为 HTML", disabled: exporting, icon: ITEM_ICONS["export-html"] });
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
    // fallback to CSS max-width when menuEl not yet measured
    const width = menuEl?.getBoundingClientRect().width ?? 280;
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
    if (right < margin) right = margin;
    toastStyle = `top: ${r.bottom + gap}px; right: ${right}px;`;
  }

  async function openMenu() {
    open = true;
    highlightIdx = enabledIndices[0] ?? -1;
    await tick();
    placeMenu();
    const first = itemEls[highlightIdx];
    if (first) first.focus({ preventScroll: true });
    else menuEl?.focus();
  }

  $effect(() => {
    if (!open || highlightIdx < 0) return;
    const el = itemEls[highlightIdx];
    if (el && document.activeElement !== el) el.focus({ preventScroll: true });
  });

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
    if (exitTimer) clearTimeout(exitTimer);
    toastExiting = false;
    feedback = kind;
    placeToast();
    feedbackTimer = setTimeout(() => {
      toastExiting = true;
      exitTimer = setTimeout(() => {
        feedback = null;
        toastExiting = false;
        feedbackTimer = null;
        exitTimer = null;
      }, 120);
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
    } else if (key === "copy-id") {
      try {
        await navigator.clipboard.writeText(sessionId);
        setFeedback("copied");
      } catch (e) {
        console.warn("[SessionMetaMenu] copy sessionId failed:", e);
        setFeedback("copy-fail");
      }
    } else if (key === "export-md" || key === "export-json" || key === "export-html") {
      const formatMap: Record<string, ExportFormat> = {
        "export-md": "markdown",
        "export-json": "json",
        "export-html": "html",
      };
      await doExport(formatMap[key]);
    }
  }

  async function doExport(format: ExportFormat) {
    if (exporting) return;
    exporting = true;
    try {
      const resp = await getSessionDetailForExport(projectId, sessionId);
      if (resp.status !== "full" || !resp.detail) {
        console.error("[SessionMetaMenu] export: unexpected response", resp.status);
        setFeedback("export-fail");
        return;
      }

      const content = exportSession(resp.detail, format);
      const defaultName = getExportFileName(sessionId, format);
      const filterExt = getExportFilterExt(format);

      if (tauri) {
        const result = await getTransport().invoke<string | null>("export_save_session", {
          defaultName,
          filterExt,
          content,
        });
        if (result === null) return;
        setFeedback("exported");
      } else {
        triggerBrowserDownload(content, defaultName, format);
        setFeedback("exported");
      }
    } catch (e) {
      console.warn("[SessionMetaMenu] export failed:", e);
      setFeedback("export-fail");
    } finally {
      exporting = false;
    }
  }

  function triggerBrowserDownload(content: string, filename: string, format: ExportFormat) {
    const mimeMap: Record<ExportFormat, string> = {
      markdown: "text/markdown;charset=utf-8",
      json: "application/json;charset=utf-8",
      html: "text/html;charset=utf-8",
    };
    const blob = new Blob([content], { type: mimeMap[format] });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = filename;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
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
    if (exitTimer) clearTimeout(exitTimer);
  });

  function feedbackText(kind: FeedbackKind): string {
    if (kind === "copied") return "已复制";
    if (kind === "exported") return "已导出";
    if (kind === "open-fail") return "打开失败";
    if (kind === "export-fail") return "导出失败";
    return "复制失败";
  }

  function isSuccess(kind: FeedbackKind): boolean {
    return kind === "copied" || kind === "exported";
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
        <div class="meta-menu-sep" role="separator"></div>
      {/if}
      {#if item.key === "export-md"}
        <div class="meta-menu-sep" role="separator"></div>
      {/if}
      <button
        bind:this={itemEls[i]}
        type="button"
        role="menuitem"
        class="meta-menu-item"
        class:meta-menu-item-highlight={highlightIdx === i && !item.disabled}
        aria-disabled={item.disabled || undefined}
        tabindex="-1"
        onclick={() => !item.disabled && runItem(item.key)}
      >
        <svg
          class="meta-menu-item-icon"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
          aria-hidden="true"
        >
          {@html item.icon}
        </svg>
        <span class="meta-menu-label">{item.label}</span>
      </button>
    {/each}
  </div>
{/if}

{#if feedback}
  <div
    class="meta-toast"
    class:meta-toast-success={isSuccess(feedback)}
    class:meta-toast-error={!isSuccess(feedback)}
    class:meta-toast-exit={toastExiting}
    style={toastStyle}
    role="status"
    aria-live="polite"
  >
    <svg
      class="meta-toast-icon"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="2"
      stroke-linecap="round"
      stroke-linejoin="round"
      aria-hidden="true"
    >
      {#if isSuccess(feedback)}
        {@html CHECK_SVG}
      {:else}
        {@html ALERT_CIRCLE_SVG}
      {/if}
    </svg>
    {feedbackText(feedback)}
  </div>
{/if}

<style>
  .meta-trigger {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 6px;
    background: transparent;
    border: 1px solid transparent;
    border-radius: 6px;
    cursor: pointer;
    font-family: inherit;
    transition: background 120ms ease-out, border-color 120ms ease-out;
  }

  .meta-trigger-icon {
    width: 15px;
    height: 15px;
    color: var(--color-text-muted);
    flex-shrink: 0;
    transition: color 120ms ease-out;
  }

  .meta-trigger:hover {
    background: var(--color-surface-raised);
    border-color: var(--color-border);
  }
  .meta-trigger:hover .meta-trigger-icon {
    color: var(--color-text-secondary);
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
    min-width: 200px;
    max-width: 280px;
    padding: 4px;
    background: var(--color-surface);
    border: 1px solid var(--color-border-emphasis);
    border-radius: 8px;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.15);
    outline: none;
    animation: meta-menu-in 150ms cubic-bezier(0.16, 1, 0.3, 1);
  }

  :global([data-theme="dark"]) .meta-menu {
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.3);
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
    padding: 7px 12px;
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

  .meta-menu-item-icon {
    width: 14px;
    height: 14px;
    color: var(--color-text-secondary);
    flex-shrink: 0;
    transition: color 100ms ease-out;
  }

  .meta-menu-item:hover:not([aria-disabled]),
  .meta-menu-item.meta-menu-item-highlight {
    background: var(--tool-item-hover-bg);
  }

  .meta-menu-item:hover:not([aria-disabled]) .meta-menu-item-icon,
  .meta-menu-item.meta-menu-item-highlight .meta-menu-item-icon {
    color: var(--color-text);
  }

  .meta-menu-item:active:not([aria-disabled]) {
    background: var(--color-surface-overlay);
  }

  .meta-menu-item[aria-disabled] {
    color: var(--color-text-muted);
    cursor: not-allowed;
  }
  .meta-menu-item[aria-disabled] .meta-menu-item-icon {
    opacity: 0.5;
  }

  .meta-menu-sep {
    height: 1px;
    background: var(--color-border-subtle);
    margin: 4px 8px;
  }

  .meta-toast {
    position: fixed;
    z-index: 200;
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 11px;
    font-weight: 500;
    padding: 5px 10px;
    border-radius: 6px;
    white-space: nowrap;
    pointer-events: none;
    animation: meta-toast-in 150ms cubic-bezier(0.16, 1, 0.3, 1);
  }

  .meta-toast-icon {
    width: 12px;
    height: 12px;
    flex-shrink: 0;
  }

  .meta-toast-success {
    background: color-mix(in oklch, var(--color-success) 12%, var(--color-surface));
    border: 1px solid color-mix(in oklch, var(--color-success) 25%, var(--color-border));
    color: var(--color-success);
  }

  .meta-toast-error {
    background: color-mix(in oklch, var(--color-danger) 10%, var(--color-surface));
    border: 1px solid color-mix(in oklch, var(--color-danger) 20%, var(--color-border));
    color: var(--color-danger);
  }

  .meta-toast-exit {
    opacity: 0;
    transform: translateY(-4px);
    transition: opacity 120ms ease-out, transform 120ms ease-out;
  }

  @keyframes meta-toast-in {
    0% {
      opacity: 0;
      transform: translateY(-4px);
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
    .meta-menu-item,
    .meta-menu-item-icon,
    .meta-toast-exit {
      transition: none;
    }
  }
</style>

<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { updateStore } from "../lib/updateStore.svelte";
  import { renderMarkdown } from "../lib/render";

  interface Props {
    onClose: () => void;
  }

  let { onClose }: Props = $props();

  let popoverEl: HTMLDivElement | undefined = $state();
  let firstButton: HTMLButtonElement | undefined = $state();

  const progressPercent = $derived.by(() => {
    if (updateStore.contentLength <= 0) return 0;
    const p = (updateStore.downloaded / updateStore.contentLength) * 100;
    return Math.max(0, Math.min(100, Math.round(p)));
  });

  const progressLabel = $derived(
    updateStore.contentLength > 0 ? `${progressPercent}%` : "准备中"
  );

  async function handleInstall() {
    try {
      await updateStore.downloadAndInstall();
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      console.warn("[UpdatePopover] download failed:", msg);
      const releaseUrl = "https://github.com/snowzhaozhj/claude-devtools-rs/releases/latest";
      alert(
        `自动更新失败：${msg}\n\n` +
          `如果你使用 .deb 包安装，请到 GitHub Release 手动下载新版本：\n${releaseUrl}`
      );
    }
  }

  function handleRemindLater() {
    updateStore.remindLater();
    onClose();
  }

  async function handleSkip() {
    try {
      await updateStore.skipVersion();
    } catch (e) {
      console.warn("[UpdatePopover] skip failed:", e);
    }
    onClose();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      onClose();
    }
  }

  function handleOutsideClick(e: MouseEvent) {
    const target = e.target as Node | null;
    if (popoverEl && target && !popoverEl.contains(target)) {
      const pillBtn = (target as HTMLElement).closest?.(".update-pill");
      if (!pillBtn) onClose();
    }
  }

  onMount(() => {
    requestAnimationFrame(() => firstButton?.focus());
    document.addEventListener("mousedown", handleOutsideClick, true);
    document.addEventListener("keydown", handleKeydown);
  });

  onDestroy(() => {
    document.removeEventListener("mousedown", handleOutsideClick, true);
    document.removeEventListener("keydown", handleKeydown);
  });
</script>

<div
  class="update-popover"
  role="dialog"
  aria-label="应用更新详情"
  bind:this={popoverEl}
  data-tauri-drag-region="false"
>
  {#if updateStore.status === "available"}
    <div class="popover-header">
      <span class="popover-title">发现新版本</span>
      <span class="popover-version">v{updateStore.currentVersion} → <strong>v{updateStore.newVersion}</strong></span>
    </div>
    {#if updateStore.notes}
      <div class="release-notes">
        <!-- eslint-disable-next-line svelte/no-at-html-tags -->
        {@html renderMarkdown(updateStore.notes)}
      </div>
    {/if}
    <div class="popover-actions">
      <button class="btn-primary" bind:this={firstButton} onclick={handleInstall}>立即更新</button>
      <button class="btn-secondary" onclick={handleRemindLater}>稍后提醒</button>
      <button class="btn-tertiary" onclick={handleSkip}>跳过此版本</button>
    </div>
  {:else if updateStore.status === "downloading"}
    <div class="popover-header">
      <span class="popover-title">正在下载更新</span>
      <span class="popover-version">v{updateStore.newVersion}</span>
      <span class="progress-percent">{progressLabel}</span>
    </div>
    <div class="progress-row" aria-label={`下载进度 ${progressLabel}`}>
      <div class="progress-bar-track">
        <div class="progress-bar-fill" style:width="{progressPercent}%"></div>
      </div>
    </div>
    <div class="popover-note">下载启动后无法中断，请等待完成或失败。</div>
  {:else if updateStore.status === "downloaded"}
    <div class="popover-header">
      <span class="popover-title">更新已就绪</span>
      <span class="popover-version">v{updateStore.newVersion}</span>
    </div>
    <div class="popover-note">应用即将自动重启完成安装。</div>
  {:else if updateStore.status === "error"}
    <div class="popover-header">
      <span class="popover-title popover-title-error">更新失败</span>
    </div>
    <div class="popover-error">{updateStore.errorMessage || "未知错误"}</div>
    <div class="popover-actions">
      <button class="btn-secondary" bind:this={firstButton} onclick={onClose}>关闭</button>
      <button class="btn-primary" onclick={handleInstall}>重试</button>
    </div>
  {/if}
</div>

<style>
  .update-popover {
    width: 360px;
    max-width: calc(100vw - 24px);
    max-height: 60vh;
    overflow-y: auto;
    padding: 14px 16px;
    background: var(--color-surface);
    border: 1px solid var(--color-border-emphasis);
    border-radius: 10px;
    box-shadow: 0 12px 36px rgba(0, 0, 0, 0.2);
    color: var(--color-text);
    font-size: 13px;
    line-height: 1.5;
  }

  .popover-header {
    display: flex;
    align-items: baseline;
    gap: 8px;
    flex-wrap: wrap;
    margin-bottom: 8px;
  }

  .popover-title {
    font-weight: 600;
    color: var(--color-text);
  }

  .popover-title-error {
    color: var(--color-danger);
  }

  .popover-version {
    color: var(--color-text-secondary);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }

  .progress-percent {
    margin-left: auto;
    padding: 2px 8px;
    border-radius: 999px;
    background: var(--color-surface-overlay);
    border: 1px solid var(--color-border);
    font-variant-numeric: tabular-nums;
    color: var(--color-text-secondary);
    font-size: 11px;
    white-space: nowrap;
  }

  .release-notes {
    margin: 6px 0 12px;
    padding: 8px 10px;
    max-height: 220px;
    overflow-y: auto;
    background: var(--color-surface-overlay);
    border: 1px solid var(--color-border);
    border-radius: 6px;
    color: var(--color-text-secondary);
    font-size: 12px;
  }

  .release-notes :global(p) {
    margin: 0 0 6px;
  }

  .release-notes :global(p:last-child) {
    margin-bottom: 0;
  }

  .release-notes :global(code) {
    padding: 1px 4px;
    border-radius: 3px;
    background: rgba(0, 0, 0, 0.08);
    font-family: var(--font-mono);
    font-size: 11px;
  }

  .popover-actions {
    display: flex;
    gap: 8px;
    margin-top: 4px;
    flex-wrap: wrap;
  }

  .btn-primary,
  .btn-secondary,
  .btn-tertiary {
    padding: 6px 14px;
    border-radius: 5px;
    font-size: 12px;
    cursor: pointer;
    border: 1px solid transparent;
    font-weight: 500;
  }

  .btn-primary {
    background: var(--color-accent-blue-hover);
    color: var(--color-text-on-accent);
    border-color: var(--color-accent-blue-hover);
  }

  .btn-primary:hover {
    background: var(--color-accent-blue);
    border-color: var(--color-accent-blue);
  }

  .btn-secondary {
    background: transparent;
    color: var(--color-text-secondary);
    border-color: var(--color-border-emphasis);
  }

  .btn-secondary:hover {
    background: var(--tool-item-hover-bg);
  }

  .btn-tertiary {
    background: transparent;
    color: var(--color-text-muted);
    border-color: transparent;
  }

  .btn-tertiary:hover {
    color: var(--color-text-secondary);
    text-decoration: underline;
  }

  .progress-row {
    margin: 4px 0 8px;
  }

  .progress-bar-track {
    width: 100%;
    height: 6px;
    background: var(--color-surface-overlay);
    border: 1px solid var(--color-border);
    border-radius: 999px;
    overflow: hidden;
  }

  .progress-bar-fill {
    height: 100%;
    background: var(--color-accent-blue);
    border-radius: 999px;
    transition: width 0.24s cubic-bezier(0.22, 1, 0.36, 1);
  }

  .popover-note {
    font-size: 11px;
    color: var(--color-text-muted);
  }

  .popover-error {
    margin: 6px 0 10px;
    padding: 8px 10px;
    background: rgba(220, 38, 38, 0.08);
    border: 1px solid rgba(220, 38, 38, 0.3);
    border-radius: 6px;
    color: var(--color-text);
    font-size: 12px;
    word-break: break-word;
  }
</style>

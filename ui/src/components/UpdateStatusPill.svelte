<script lang="ts">
  import { relaunch } from "@tauri-apps/plugin-process";
  import { updateStore } from "../lib/updateStore.svelte";
  import { toastStore } from "../lib/toastStore.svelte";
  import {
    DOWNLOAD_CLOUD_SVG,
    ALERT_CIRCLE_SVG,
    CHECK_CIRCLE_SVG,
    ROTATE_CCW_SVG,
  } from "../lib/icons";
  import UpdatePopover from "./UpdatePopover.svelte";

  let popoverOpen = $state(false);

  // D3b：store 切到 idle 时强制关 popover，避免 popover 浮在 chrome 下方残影
  $effect(() => {
    if (updateStore.status === "idle") {
      popoverOpen = false;
    }
  });

  const progressPercent = $derived.by(() => {
    if (updateStore.contentLength <= 0) return 0;
    const p = (updateStore.downloaded / updateStore.contentLength) * 100;
    return Math.max(0, Math.min(100, Math.round(p)));
  });

  const ariaLabel = $derived.by(() => {
    switch (updateStore.status) {
      case "available":
        return `可用更新 v${updateStore.newVersion}，点击查看详情`;
      case "downloading":
        return `正在下载更新 v${updateStore.newVersion}，已完成 ${progressPercent}%`;
      case "downloaded":
        return `更新 v${updateStore.newVersion} 下载完成，点击重启应用`;
      case "error":
        return `更新失败：${updateStore.errorMessage || "未知错误"}，点击查看详情`;
      default:
        return "";
    }
  });

  async function handleClick() {
    if (updateStore.status === "downloaded") {
      try {
        await relaunch();
      } catch (e) {
        // relaunch 失败（二进制被锁 / Windows 权限）时给用户可见反馈 + 手动重启指引，
        // 否则 pill 保持 "重启更新" 态，用户反复点击无果无解释。
        console.warn("[UpdateStatusPill] relaunch failed:", e);
        toastStore.push("自动重启失败，请手动退出并重新打开应用以完成更新", "error");
      }
      return;
    }
    popoverOpen = !popoverOpen;
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      void handleClick();
    }
  }

  const show = $derived(updateStore.status !== "idle" && updateStore.visible);
</script>

{#if show}
  <div class="pill-wrapper">
    <button
      class="update-pill"
      class:pill-available={updateStore.status === "available"}
      class:pill-downloading={updateStore.status === "downloading"}
      class:pill-downloaded={updateStore.status === "downloaded"}
      class:pill-error={updateStore.status === "error"}
      data-tauri-drag-region="false"
      aria-label={ariaLabel}
      title={ariaLabel}
      onclick={handleClick}
      onkeydown={handleKeydown}
    >
      {#if updateStore.status === "available"}
        <svg class="pill-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          {@html DOWNLOAD_CLOUD_SVG}
        </svg>
        <span class="pill-text">v{updateStore.newVersion}</span>
      {:else if updateStore.status === "downloading"}
        <span class="pill-ring" role="presentation">
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <circle class="ring-track" cx="12" cy="12" r="9" fill="none" stroke-width="2.5" />
            <circle
              class="ring-fill"
              cx="12"
              cy="12"
              r="9"
              fill="none"
              stroke-width="2.5"
              stroke-linecap="round"
              stroke-dasharray={2 * Math.PI * 9}
              stroke-dashoffset={2 * Math.PI * 9 * (1 - progressPercent / 100)}
            />
          </svg>
        </span>
        <span class="pill-text">{updateStore.contentLength > 0 ? `${progressPercent}%` : "…"}</span>
      {:else if updateStore.status === "downloaded"}
        <svg class="pill-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          {@html ROTATE_CCW_SVG}
        </svg>
        <span class="pill-text">重启更新</span>
      {:else if updateStore.status === "error"}
        <svg class="pill-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          {@html ALERT_CIRCLE_SVG}
        </svg>
        <span class="pill-text">更新失败</span>
      {/if}
    </button>

    {#if popoverOpen && updateStore.status !== "downloaded"}
      <div class="popover-anchor">
        <UpdatePopover onClose={() => (popoverOpen = false)} />
      </div>
    {/if}
  </div>
{/if}

<style>
  .pill-wrapper {
    position: relative;
    display: inline-flex;
    align-items: center;
  }

  .update-pill {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 26px;
    padding: 0 10px;
    border-radius: 13px;
    border: 1px solid var(--color-border-emphasis);
    background: var(--color-surface);
    color: var(--color-text-secondary);
    font: inherit;
    font-size: 12px;
    font-weight: 500;
    line-height: 1;
    cursor: pointer;
    transition: background 0.12s ease, border-color 0.12s ease, color 0.12s ease;
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }

  .update-pill:hover {
    background: var(--tool-item-hover-bg);
    color: var(--color-text);
  }

  .update-pill:focus-visible {
    outline: 2px solid var(--color-accent-blue);
    outline-offset: 2px;
  }

  .pill-icon {
    width: 14px;
    height: 14px;
    flex-shrink: 0;
  }

  .pill-text {
    line-height: 1;
  }

  .pill-available {
    color: var(--color-accent-blue);
    border-color: var(--color-accent-blue);
  }

  .pill-available:hover {
    background: color-mix(in oklch, var(--color-accent-blue) 8%, transparent);
    color: var(--color-accent-blue);
  }

  .pill-downloading {
    color: var(--color-accent-blue);
    border-color: var(--color-accent-blue);
  }

  .pill-downloaded {
    color: var(--color-text-on-accent);
    background: var(--color-accent-blue);
    border-color: var(--color-accent-blue);
  }

  .pill-downloaded:hover {
    background: var(--color-accent-blue-hover);
    color: var(--color-text-on-accent);
  }

  .pill-error {
    color: var(--color-danger);
    border-color: var(--color-danger);
  }

  .pill-error:hover {
    background: color-mix(in oklch, var(--color-danger) 8%, transparent);
    color: var(--color-danger);
  }

  .pill-ring {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 14px;
    height: 14px;
    flex-shrink: 0;
  }

  .pill-ring svg {
    width: 14px;
    height: 14px;
    transform: rotate(-90deg);
  }

  .ring-track {
    stroke: var(--color-border);
  }

  .ring-fill {
    stroke: currentColor;
    transition: stroke-dashoffset 0.24s cubic-bezier(0.22, 1, 0.36, 1);
  }

  .popover-anchor {
    position: absolute;
    top: calc(100% + 8px);
    right: 0;
    z-index: 30;
  }

  @media (max-width: 640px) {
    .popover-anchor {
      position: fixed;
      top: 56px;
      left: 50%;
      right: auto;
      transform: translateX(-50%);
    }
  }
</style>

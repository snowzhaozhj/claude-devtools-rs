<script lang="ts">
  import { ALERT_TRIANGLE_SVG } from "../lib/icons";

  const STORAGE_KEY = "cdt-rosetta-dismissed-v1";
  const RELEASE_URL =
    "https://github.com/snowzhaozhj/claude-devtools-rs/releases/latest";

  let { visible }: { visible: boolean } = $props();

  let dismissed = $state(false);

  const isDismissedFromStorage = (() => {
    try {
      return localStorage.getItem(STORAGE_KEY) === "1";
    } catch {
      return false;
    }
  })();

  function handleClick(e: MouseEvent) {
    if (e.shiftKey) {
      try {
        localStorage.setItem(STORAGE_KEY, "1");
      } catch { /* localStorage 不可用时静默 */ }
      dismissed = true;
      return;
    }
    window.open(RELEASE_URL, "_blank");
  }

  const show = $derived(visible && !dismissed && !isDismissedFromStorage);
</script>

{#if show}
  <button
    class="rosetta-icon"
    data-tauri-drag-region="false"
    title={"运行在 Rosetta 翻译模式（x86_64）。点击下载 aarch64 原生版本；Shift + 点击隐藏此提醒。"}
    aria-label="Rosetta 翻译模式提示"
    onclick={handleClick}
  >
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      {@html ALERT_TRIANGLE_SVG}
    </svg>
  </button>
{/if}

<style>
  .rosetta-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    padding: 0;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: var(--color-warning-text, #b45309);
    cursor: pointer;
    transition: background 0.1s, color 0.1s;
  }

  .rosetta-icon:hover {
    background: var(--color-warning-bg, rgba(245, 158, 11, 0.12));
  }

  .rosetta-icon svg {
    width: 16px;
    height: 16px;
  }
</style>

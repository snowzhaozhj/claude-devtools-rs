<script lang="ts">
  // Rosetta 提示横幅：Apple Silicon Mac 上跑 x86_64 binary 时提示用户改装 ARM 版。
  // localStorage 持久化"不再提示"状态，避免每次启动都打扰。

  import { ALERT_TRIANGLE_SVG } from "../lib/icons";

  const STORAGE_KEY = "cdt-rosetta-dismissed-v1";
  const RELEASE_URL =
    "https://github.com/snowzhaozhj/claude-devtools-rs/releases/latest";

  let { visible }: { visible: boolean } = $props();

  let dismissed = $state(false);

  function handleDownload() {
    // Tauri webview 对外部 https 链接默认会用系统浏览器打开（capabilities 允许）。
    // 不引 tauri-plugin-shell——保持依赖最小。
    window.open(RELEASE_URL, "_blank");
  }

  function handleDismiss() {
    try {
      localStorage.setItem(STORAGE_KEY, "1");
    } catch {
      /* localStorage 不可用时静默：banner 下次启动还会出现，但功能不受影响 */
    }
    dismissed = true;
  }

  const isDismissedFromStorage = (() => {
    try {
      return localStorage.getItem(STORAGE_KEY) === "1";
    } catch {
      return false;
    }
  })();
</script>

{#if visible && !dismissed && !isDismissedFromStorage}
  <div class="rosetta-banner" role="region" aria-label="架构不匹配提示">
    <div class="banner-content">
      <div class="banner-header">
        <svg
          class="banner-icon"
          xmlns="http://www.w3.org/2000/svg"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
          aria-hidden="true"
        >
          {@html ALERT_TRIANGLE_SVG}
        </svg>
        <span class="banner-title">检测到 Rosetta 翻译运行</span>
      </div>
      <div class="banner-body">
        当前运行的是 <strong>Intel (x86_64)</strong> 版本，但你的 Mac 是 Apple
        Silicon。建议下载 <strong>aarch64</strong> 版本获得原生性能（CPU
        占用可显著降低）。
      </div>
      <div class="banner-actions">
        <button class="btn-primary" onclick={handleDownload}>
          下载 ARM 版
        </button>
        <button class="btn-tertiary" onclick={handleDismiss}>
          不再提示
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .rosetta-banner {
    position: relative;
    padding: 10px 16px;
    background: var(--color-warning-bg);
    border-bottom: 1px solid var(--color-warning-border);
    color: var(--color-text);
    font-size: 13px;
    line-height: 1.4;
  }

  .banner-content {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .banner-header {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .banner-icon {
    width: 16px;
    height: 16px;
    color: var(--color-warning-text);
    flex-shrink: 0;
  }

  .banner-title {
    font-weight: 600;
    color: var(--color-text);
  }

  .banner-body {
    color: var(--color-text-secondary);
    font-size: 12px;
  }

  .banner-body strong {
    color: var(--color-text);
  }

  .banner-actions {
    display: flex;
    gap: 8px;
    margin-top: 4px;
  }

  .btn-primary,
  .btn-tertiary {
    padding: 4px 12px;
    border-radius: 4px;
    font-size: 12px;
    cursor: pointer;
    border: 1px solid transparent;
  }

  /* btn bg 用 hover 色满足 WCAG AA on-accent 文字对比，hover 用 accent-blue */
  .btn-primary {
    background: var(--color-accent-blue-hover);
    color: var(--color-text-on-accent);
    border-color: var(--color-accent-blue-hover);
  }

  .btn-primary:hover {
    background: var(--color-accent-blue);
    border-color: var(--color-accent-blue);
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
</style>

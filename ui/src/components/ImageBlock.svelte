<script lang="ts">
  import { getImageAsset, type ImageSource } from "../lib/api";

  interface Props {
    source: ImageSource;
    rootSessionId: string;
    sessionId: string;
    blockId: string;
  }

  let { source, rootSessionId, sessionId, blockId }: Props = $props();

  let assetUrl = $state<string | null>(null);
  let loading = $state(false);
  let loadFailed = $state(false);
  let inViewportOnce = $state(false);
  let previewOpen = $state(false);

  // dataOmitted=false（回滚开关 / 老后端）→ 直接用 data: URI 不发额外 IPC。
  const directDataUri = $derived(
    !source.dataOmitted && source.data
      ? `data:${source.media_type};base64,${source.data}`
      : null
  );

  function openPreview() {
    if (assetUrl) {
      previewOpen = true;
    }
  }

  function closePreview() {
    previewOpen = false;
  }

  function handleKeydown(event: KeyboardEvent) {
    if (previewOpen && event.key === "Escape") {
      closePreview();
    }
  }

  function fetchAsset() {
    if (loading) return;
    loading = true;
    loadFailed = false;
    getImageAsset(rootSessionId, sessionId, blockId)
      .then((url) => {
        assetUrl = url;
      })
      .catch((err) => {
        console.warn("[ImageBlock] getImageAsset failed", err);
        // 后端 fallback 通常已返回 data: URI；走到这里说明取图真失败，
        // 标记失败态让占位符暴露重试按钮（inViewportOnce 已为 true，
        // observer 不会再自动重试）。
        loadFailed = true;
      })
      .finally(() => {
        loading = false;
      });
  }

  function retryLoad() {
    fetchAsset();
  }

  function attachObserver(el: HTMLElement) {
    if (directDataUri) {
      // 直接路径无需懒加载。
      assetUrl = directDataUri;
      return () => {};
    }
    const obs = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting && !inViewportOnce && !assetUrl && !loading) {
            inViewportOnce = true;
            fetchAsset();
          }
        }
      },
      { rootMargin: "200px" }
    );
    obs.observe(el);
    return () => obs.disconnect();
  }
</script>

<div class="image-block" {@attach attachObserver}>
  {#if assetUrl}
    <button class="image-trigger" type="button" onclick={openPreview} aria-label="放大查看图片">
      <img src={assetUrl} alt="inline image ({source.media_type})" />
    </button>
  {:else if loadFailed}
    <div class="placeholder placeholder-error" role="alert">
      <span class="placeholder-label">图片加载失败</span>
      <button class="placeholder-retry" type="button" onclick={retryLoad} disabled={loading}>
        {loading ? "重试中…" : "重试"}
      </button>
    </div>
  {:else}
    <div class="placeholder" aria-busy={loading}>
      <span class="placeholder-label">
        {loading ? "加载中…" : "Image"}
      </span>
      <span class="placeholder-meta">{source.media_type || "image"}</span>
    </div>
  {/if}
</div>

<svelte:window on:keydown={handleKeydown} />

{#if previewOpen && assetUrl}
  <div class="preview-layer" role="dialog" aria-modal="true" aria-label="图片预览">
    <button class="preview-backdrop" type="button" onclick={closePreview} aria-label="关闭图片预览背景"></button>
    <img src={assetUrl} alt="inline image ({source.media_type})" />
    <button class="preview-close" type="button" onclick={closePreview} aria-label="关闭图片预览">关闭</button>
  </div>
{/if}

<style>
  .image-block {
    margin: 0.5rem 0;
    max-width: 100%;
  }
  .image-trigger {
    display: block;
    max-width: 100%;
    padding: 0;
    border: 0;
    border-radius: 4px;
    background: transparent;
    cursor: zoom-in;
  }
  .image-trigger:focus-visible {
    outline: 2px solid var(--color-switch-on);
    outline-offset: 2px;
  }
  .image-trigger img {
    max-width: 100%;
    border-radius: 4px;
    display: block;
  }
  .preview-layer {
    position: fixed;
    inset: 0;
    z-index: 1000;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 2rem;
  }
  .preview-backdrop {
    position: absolute;
    inset: 0;
    border: 0;
    /* surface 88% 半透 scrim；旧 WebKitGTK 兜底用 rgba 黑色 modal 标准 */
    background: rgba(0, 0, 0, 0.55);
    background: color-mix(in srgb, var(--color-surface) 88%, transparent);
    cursor: zoom-out;
  }
  .preview-layer img {
    position: relative;
    max-width: min(100%, 1200px);
    max-height: 100%;
    border-radius: 6px;
    box-shadow: 0 24px 80px rgba(0, 0, 0, 0.45);
    box-shadow: 0 24px 80px color-mix(in srgb, var(--color-text) 35%, transparent);
    object-fit: contain;
  }
  .preview-close {
    position: absolute;
    top: 1rem;
    right: 1rem;
    padding: 0.375rem 0.625rem;
    border: 1px solid var(--color-border-emphasis);
    border-radius: 999px;
    background: var(--color-surface-raised);
    color: var(--color-text-secondary);
    font: inherit;
    font-size: 0.75rem;
    cursor: pointer;
  }
  .preview-close:hover,
  .preview-close:focus-visible {
    color: var(--color-text);
    border-color: var(--color-text-muted);
  }
  .placeholder {
    height: 200px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 0.25rem;
    background: var(--color-surface-2, #1f2937);
    border: 1px dashed var(--color-border, #374151);
    border-radius: 4px;
    color: var(--color-text-muted, #9ca3af);
    font-size: 0.75rem;
  }
  .placeholder-label {
    font-weight: 500;
  }
  .placeholder-meta {
    opacity: 0.7;
  }
  .placeholder-error {
    border-color: var(--color-danger);
    color: var(--color-danger);
  }
  .placeholder-retry {
    padding: 0.25rem 0.75rem;
    font: inherit;
    font-size: 0.75rem;
    color: var(--color-text);
    background: var(--color-surface-raised);
    border: 1px solid var(--color-border-emphasis);
    border-radius: 4px;
    cursor: pointer;
  }
  .placeholder-retry:hover:not(:disabled) {
    background: var(--tool-item-hover-bg);
  }
  .placeholder-retry:disabled {
    opacity: 0.6;
    cursor: default;
  }
</style>

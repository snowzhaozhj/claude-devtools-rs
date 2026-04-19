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
  let inViewportOnce = $state(false);

  // dataOmitted=false（回滚开关 / 老后端）→ 直接用 data: URI 不发额外 IPC。
  const directDataUri = $derived(
    !source.dataOmitted && source.data
      ? `data:${source.media_type};base64,${source.data}`
      : null
  );

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
            loading = true;
            getImageAsset(rootSessionId, sessionId, blockId)
              .then((url) => {
                assetUrl = url;
              })
              .catch((err) => {
                console.warn("[ImageBlock] getImageAsset failed", err);
                // 后端 fallback 已经返回 data: URI，正常情况下这里不会触发。
              })
              .finally(() => {
                loading = false;
              });
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
    <img src={assetUrl} alt="inline image ({source.media_type})" />
  {:else}
    <div class="placeholder" aria-busy={loading}>
      <span class="placeholder-label">
        {loading ? "加载中…" : "Image"}
      </span>
      <span class="placeholder-meta">{source.media_type || "image"}</span>
    </div>
  {/if}
</div>

<style>
  .image-block {
    margin: 0.5rem 0;
    max-width: 100%;
  }
  .image-block img {
    max-width: 100%;
    border-radius: 4px;
    display: block;
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
</style>

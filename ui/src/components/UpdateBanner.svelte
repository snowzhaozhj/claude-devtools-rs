<script lang="ts">
  import { updateStore } from '../lib/updateStore.svelte'
  import { renderMarkdown } from '../lib/render'

  let notesExpanded = $state(false)

  // 进度百分比，0..100；下载阶段且 contentLength > 0 时有效
  const progressPercent = $derived(() => {
    if (updateStore.contentLength <= 0) return 0
    const p = (updateStore.downloaded / updateStore.contentLength) * 100
    return Math.max(0, Math.min(100, Math.round(p)))
  })

  // 折叠 release notes：默认只显示前 ~120 字符
  const notesPreview = $derived(() =>
    updateStore.notes.length > 120
      ? updateStore.notes.slice(0, 120) + '…'
      : updateStore.notes
  )
  const hasMoreNotes = $derived(() => updateStore.notes.length > 120)

  async function handleInstall() {
    try {
      await updateStore.downloadAndInstall()
    } catch (e) {
      // .deb 平台或网络错误：保留 banner 错误态，让用户能看到
      const msg = e instanceof Error ? e.message : String(e)
      console.warn('[UpdateBanner] download failed:', msg)
      // 提示跳转 GitHub Release
      const releaseUrl = 'https://github.com/snowzhaozhj/claude-devtools-rs/releases/latest'
      // 用 alert 兜底，原版 dialog 待后续移植
      alert(
        `自动更新失败：${msg}\n\n` +
          `如果你使用 .deb 包安装，请到 GitHub Release 手动下载新版本：\n${releaseUrl}`
      )
    }
  }

  function handleSkip() {
    void updateStore.skipVersion()
  }

  function handleClose() {
    if (updateStore.status === 'downloading') {
      const ok = confirm('确定取消下载？已下载的内容会被丢弃。')
      if (!ok) return
    }
    updateStore.dismiss()
  }
</script>

{#if updateStore.visible && updateStore.status !== 'idle'}
  <div class="update-banner" role="region" aria-label="应用更新">
    {#if updateStore.status === 'available'}
      <div class="banner-content">
        <div class="banner-header">
          <span class="banner-title">发现新版本</span>
          <span class="banner-version">
            v{updateStore.currentVersion} → <strong>v{updateStore.newVersion}</strong>
          </span>
        </div>
        {#if updateStore.notes}
          <div class="release-notes">
            {#if notesExpanded}
              <!-- eslint-disable-next-line svelte/no-at-html-tags -->
              <div class="notes-body">{@html renderMarkdown(updateStore.notes)}</div>
              {#if hasMoreNotes()}
                <button class="link-button" onclick={() => (notesExpanded = false)}>
                  收起
                </button>
              {/if}
            {:else}
              <div class="notes-preview">{notesPreview()}</div>
              {#if hasMoreNotes()}
                <button class="link-button" onclick={() => (notesExpanded = true)}>
                  展开
                </button>
              {/if}
            {/if}
          </div>
        {/if}
        <div class="banner-actions">
          <button class="btn-primary" onclick={handleInstall}>立即更新</button>
          <button class="btn-secondary" onclick={() => updateStore.remindLater()}>
            稍后提醒
          </button>
          <button class="btn-tertiary" onclick={handleSkip}>跳过此版本</button>
        </div>
      </div>
    {:else if updateStore.status === 'downloading'}
      <div class="banner-content">
        <div class="banner-header">
          <span class="banner-title">正在下载更新</span>
          <span class="banner-version">v{updateStore.newVersion}</span>
          <span class="progress-percent">{progressPercent()}%</span>
        </div>
        <div class="progress-bar-track">
          <div class="progress-bar-fill" style:width="{progressPercent()}%"></div>
        </div>
      </div>
    {:else if updateStore.status === 'downloaded'}
      <div class="banner-content">
        <div class="banner-header">
          <span class="banner-title">更新已就绪，应用即将重启</span>
          <span class="banner-version">v{updateStore.newVersion}</span>
        </div>
      </div>
    {:else if updateStore.status === 'error'}
      <div class="banner-content">
        <div class="banner-header">
          <span class="banner-title banner-title-error">更新失败</span>
          <span class="banner-error-message">{updateStore.errorMessage}</span>
        </div>
      </div>
    {/if}

    <button
      class="banner-close"
      onclick={handleClose}
      aria-label="关闭更新提示"
      title="关闭"
    >
      ×
    </button>
  </div>
{/if}

<style>
  .update-banner {
    position: relative;
    padding: 10px 36px 10px 16px;
    background: var(--color-surface, #2d2d2d);
    border-bottom: 1px solid var(--color-border, #3a3a3a);
    color: var(--color-text, #e5e5e5);
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
    gap: 12px;
    flex-wrap: wrap;
  }

  .banner-title {
    font-weight: 600;
    color: var(--color-text, #e5e5e5);
  }

  .banner-title-error {
    color: var(--color-error, #ff6b6b);
  }

  .banner-version {
    color: var(--color-text-secondary, #a0a0a0);
    font-size: 12px;
  }

  .banner-error-message {
    color: var(--color-text-muted, #888);
    font-size: 12px;
  }

  .progress-percent {
    margin-left: auto;
    font-variant-numeric: tabular-nums;
    color: var(--color-text-secondary, #a0a0a0);
    font-size: 12px;
  }

  .release-notes {
    color: var(--color-text-secondary, #a0a0a0);
    font-size: 12px;
  }

  .notes-preview {
    white-space: pre-wrap;
  }

  .notes-body {
    max-height: 160px;
    overflow-y: auto;
    padding: 6px 8px;
    background: var(--color-surface-overlay, #232323);
    border: 1px solid var(--color-border, #3a3a3a);
    border-radius: 4px;
  }

  .link-button {
    margin-top: 4px;
    background: none;
    border: none;
    color: var(--color-link, #6ab0ff);
    font-size: 12px;
    cursor: pointer;
    padding: 0;
  }

  .link-button:hover {
    text-decoration: underline;
  }

  .banner-actions {
    display: flex;
    gap: 8px;
    margin-top: 4px;
  }

  .btn-primary,
  .btn-secondary,
  .btn-tertiary {
    padding: 4px 12px;
    border-radius: 4px;
    font-size: 12px;
    cursor: pointer;
    border: 1px solid transparent;
  }

  .btn-primary {
    background: var(--color-accent, #4a9eff);
    color: white;
    border-color: var(--color-accent, #4a9eff);
  }

  .btn-primary:hover {
    background: var(--color-accent-hover, #5badff);
  }

  .btn-secondary {
    background: transparent;
    color: var(--color-text-secondary, #a0a0a0);
    border-color: var(--color-border-emphasis, #4a4a4a);
  }

  .btn-secondary:hover {
    background: var(--color-surface-hover, #353535);
  }

  .btn-tertiary {
    background: transparent;
    color: var(--color-text-muted, #888);
    border-color: transparent;
  }

  .btn-tertiary:hover {
    color: var(--color-text-secondary, #a0a0a0);
    text-decoration: underline;
  }

  .progress-bar-track {
    height: 4px;
    background: var(--color-border, #3a3a3a);
    border-radius: 2px;
    overflow: hidden;
  }

  .progress-bar-fill {
    height: 100%;
    background: var(--color-accent, #4a9eff);
    border-radius: 2px;
    transition: width 0.2s ease-out;
  }

  .banner-close {
    position: absolute;
    top: 8px;
    right: 8px;
    width: 20px;
    height: 20px;
    background: none;
    border: none;
    color: var(--color-text-muted, #888);
    font-size: 18px;
    line-height: 1;
    cursor: pointer;
    border-radius: 4px;
  }

  .banner-close:hover {
    color: var(--color-text, #e5e5e5);
    background: var(--color-surface-hover, #353535);
  }
</style>

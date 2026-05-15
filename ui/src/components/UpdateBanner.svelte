<script lang="ts">
  import { updateStore } from '../lib/updateStore.svelte'
  import { renderMarkdown } from '../lib/render'

  let notesExpanded = $state(false)

  // macOS 隐藏 titlebar 时左上有 traffic light 控件，需要给 banner 左侧留出避让区；
  // 其他平台不需要，padding 走默认 18px。
  const trafficLightSafeArea =
    typeof navigator !== 'undefined' && navigator.userAgent.includes('Macintosh') ? '84px' : '18px'

  const progressPercent = $derived(() => {
    if (updateStore.contentLength <= 0) return 0
    const p = (updateStore.downloaded / updateStore.contentLength) * 100
    return Math.max(0, Math.min(100, Math.round(p)))
  })

  const progressLabel = $derived(() =>
    updateStore.contentLength > 0 ? `${progressPercent()}%` : '准备中'
  )

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
  <div
    class="update-banner"
    role="region"
    aria-label="应用更新"
    style:--traffic-light-safe-area={trafficLightSafeArea}
  >
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
      <div class="banner-content banner-content-progress">
        <div class="banner-header banner-header-progress">
          <span class="banner-title">正在下载更新</span>
          <span class="banner-version">v{updateStore.newVersion}</span>
          <span class="progress-percent">{progressLabel()}</span>
        </div>
        <div class="progress-row" aria-label="下载进度 {progressLabel()}">
          <div class="progress-bar-track">
            <div class="progress-bar-fill" style:width="{progressPercent()}%"></div>
          </div>
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
    /* macOS 下窗口左上有 traffic light，App.svelte 通过 data-platform 给 root 注入此 token；
       其他平台保持普通 padding。 */
    --traffic-light-safe-area: 18px;
    position: relative;
    padding: 12px 44px 12px var(--traffic-light-safe-area);
    background: var(--color-surface-raised);
    border-bottom: 1px solid var(--color-border-emphasis);
    color: var(--color-text);
    font-size: 13px;
    line-height: 1.4;
  }

  .banner-content {
    display: flex;
    flex-direction: column;
    gap: 7px;
    max-width: 960px;
  }

  .banner-content-progress {
    gap: 8px;
  }

  .banner-header {
    display: flex;
    align-items: baseline;
    gap: 12px;
    flex-wrap: wrap;
  }

  .banner-header-progress {
    align-items: center;
    flex-wrap: nowrap;
  }

  .banner-title {
    font-weight: 600;
    color: var(--color-text);
  }

  .banner-title-error {
    color: var(--color-danger);
  }

  .banner-version {
    color: var(--color-text-secondary);
    font-size: 12px;
  }

  .banner-error-message {
    color: var(--color-text-muted);
    font-size: 12px;
  }

  .progress-percent {
    margin-left: auto;
    padding: 2px 8px;
    border-radius: 999px;
    background: var(--color-surface-overlay);
    border: 1px solid var(--color-border);
    font-variant-numeric: tabular-nums;
    color: var(--color-text-secondary);
    font-size: 12px;
    white-space: nowrap;
  }

  .release-notes {
    color: var(--color-text-secondary);
    font-size: 12px;
  }

  .notes-preview {
    white-space: pre-wrap;
  }

  .notes-body {
    max-height: 160px;
    overflow-y: auto;
    padding: 6px 8px;
    background: var(--color-surface-overlay);
    border: 1px solid var(--color-border);
    border-radius: 4px;
  }

  .link-button {
    margin-top: 4px;
    background: none;
    border: none;
    color: var(--prose-link);
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

  /* btn bg 用 hover 色（浅 #2563eb / 深 #3b82f6）才能跟 --color-text-on-accent
     文字达 WCAG AA ≥ 4.5；hover 状态再深一档（浅）/ 浅一档（深，反方向）。 */
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
    display: flex;
    align-items: center;
    min-width: 0;
  }

  .progress-bar-track {
    width: 100%;
    height: 8px;
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

  .banner-close {
    position: absolute;
    top: 12px;
    right: 12px;
    width: 24px;
    height: 24px;
    background: none;
    border: none;
    color: var(--color-text-muted);
    font-size: 18px;
    line-height: 1;
    cursor: pointer;
    border-radius: 4px;
  }

  .banner-close:hover {
    color: var(--color-text);
    background: var(--tool-item-hover-bg);
  }
</style>

<script lang="ts">
  import { onMount } from "svelte";
  import { getConfig, updateConfig, type AppConfig } from "../lib/api";

  let config: AppConfig | null = $state(null);
  let loading = $state(true);
  let error: string | null = $state(null);
  let saveError: string | null = $state(null);
  let activeSection: "general" | "notifications" = $state("general");

  onMount(async () => {
    try {
      config = await getConfig();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  });

  async function updateGeneral(key: string, value: unknown) {
    if (!config) return;
    saveError = null;
    // 乐观更新
    config = {
      ...config,
      general: { ...config.general, [key]: value },
    };
    try {
      await updateConfig("general", { [key]: value });
    } catch (e) {
      saveError = `保存失败: ${e}`;
      // 回滚：重新加载
      try { config = await getConfig(); } catch { /* ignore */ }
    }
  }

  async function updateNotifications(key: string, value: unknown) {
    if (!config) return;
    saveError = null;
    config = {
      ...config,
      notifications: { ...config.notifications, [key]: value },
    };
    try {
      await updateConfig("notifications", { [key]: value });
    } catch (e) {
      saveError = `保存失败: ${e}`;
      try { config = await getConfig(); } catch { /* ignore */ }
    }
  }
</script>

<div class="settings-view">
  <div class="settings-header">
    <h2 class="settings-title">设置</h2>
    <div class="settings-tabs">
      <button
        class="section-tab"
        class:section-tab-active={activeSection === "general"}
        onclick={() => activeSection = "general"}
      >常规</button>
      <button
        class="section-tab"
        class:section-tab-active={activeSection === "notifications"}
        onclick={() => activeSection = "notifications"}
      >通知</button>
    </div>
  </div>

  <div class="settings-body">
    {#if saveError}
      <div class="save-error">{saveError}</div>
    {/if}

    {#if loading}
      <div class="state-msg">加载中...</div>
    {:else if error}
      <div class="state-msg state-err">{error}</div>
    {:else if config}
      {#if activeSection === "general"}
        <div class="section">
          <h3 class="section-title">常规</h3>

          <div class="setting-row">
            <div class="setting-info">
              <span class="setting-label">主题</span>
              <span class="setting-desc">应用的颜色方案</span>
            </div>
            <select
              class="setting-select"
              onchange={(e) => updateGeneral("theme", (e.target as HTMLSelectElement).value)}
            >
              <option value="dark" selected={config.general.theme === "dark"}>深色</option>
              <option value="light" selected={config.general.theme === "light"}>浅色</option>
              <option value="system" selected={config.general.theme === "system"}>跟随系统</option>
            </select>
          </div>

          <div class="setting-row">
            <div class="setting-info">
              <span class="setting-label">默认标签页</span>
              <span class="setting-desc">启动时打开的页面</span>
            </div>
            <select
              class="setting-select"
              onchange={(e) => updateGeneral("defaultTab", (e.target as HTMLSelectElement).value)}
            >
              <option value="dashboard" selected={config.general.defaultTab === "dashboard"}>仪表盘</option>
              <option value="last-session" selected={config.general.defaultTab === "last-session"}>上次会话</option>
            </select>
          </div>

          <div class="setting-row">
            <div class="setting-info">
              <span class="setting-label">自动展开 AI 组</span>
              <span class="setting-desc">打开会话时自动展开工具执行区域</span>
            </div>
            <button
              class="toggle-btn"
              class:toggle-on={config.general.autoExpandAiGroups}
              onclick={() => updateGeneral("autoExpandAiGroups", !config!.general.autoExpandAiGroups)}
            >{config.general.autoExpandAiGroups ? "开" : "关"}</button>
          </div>
        </div>

      {:else if activeSection === "notifications"}
        <div class="section">
          <h3 class="section-title">通知</h3>

          <div class="setting-row">
            <div class="setting-info">
              <span class="setting-label">启用通知</span>
              <span class="setting-desc">当触发器规则匹配时产生通知</span>
            </div>
            <button
              class="toggle-btn"
              class:toggle-on={config.notifications.enabled}
              onclick={() => updateNotifications("enabled", !config!.notifications.enabled)}
            >{config.notifications.enabled ? "开" : "关"}</button>
          </div>

          <div class="setting-row">
            <div class="setting-info">
              <span class="setting-label">提示音</span>
              <span class="setting-desc">收到通知时播放声音</span>
            </div>
            <button
              class="toggle-btn"
              class:toggle-on={config.notifications.soundEnabled}
              onclick={() => updateNotifications("soundEnabled", !config!.notifications.soundEnabled)}
            >{config.notifications.soundEnabled ? "开" : "关"}</button>
          </div>

          {#if config.notifications.triggers.length > 0}
            <h4 class="subsection-title">触发器规则</h4>
            <p class="subsection-desc">触发器定义了哪些会话事件会产生通知。工具执行错误、关键词匹配、token 超限都可以作为触发条件。</p>
            <div class="trigger-list">
              {#each config.notifications.triggers as trigger}
                <div class="trigger-row">
                  <span
                    class="trigger-color"
                    style:background={trigger.color || "var(--color-text-muted)"}
                  ></span>
                  <span class="trigger-name">{trigger.name}</span>
                  <span class="trigger-mode">{trigger.mode}</span>
                  <span class="trigger-status" class:trigger-disabled={!trigger.enabled}>
                    {trigger.enabled ? "启用" : "禁用"}
                  </span>
                </div>
              {/each}
            </div>
          {:else}
            <div class="empty-triggers">
              暂无触发器规则。触发器用于监控会话中的工具错误、关键词匹配等事件并自动产生通知。
            </div>
          {/if}
        </div>
      {/if}
    {/if}
  </div>
</div>

<style>
  .settings-view {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }

  .settings-header {
    padding: 16px 24px 0;
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .settings-title {
    font-size: 16px;
    font-weight: 600;
    color: var(--color-text);
    margin: 0 0 12px;
  }

  .settings-tabs {
    display: flex;
    gap: 0;
  }

  .section-tab {
    padding: 8px 16px;
    border: none;
    background: none;
    color: var(--color-text-muted);
    font: inherit;
    font-size: 13px;
    cursor: pointer;
    border-bottom: 2px solid transparent;
    transition: color 0.1s, border-color 0.1s;
  }

  .section-tab:hover {
    color: var(--color-text-secondary);
  }

  .section-tab-active {
    color: var(--color-text);
    border-bottom-color: var(--color-border-emphasis);
  }

  .settings-body {
    flex: 1;
    overflow-y: auto;
    padding: 20px 24px;
  }

  .save-error {
    padding: 8px 12px;
    margin-bottom: 12px;
    border-radius: 6px;
    background: rgba(229, 62, 62, 0.1);
    color: var(--tool-result-error-text);
    font-size: 13px;
  }

  .state-msg {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 200px;
    color: var(--color-text-muted);
    font-size: 14px;
  }
  .state-err { color: var(--tool-result-error-text); }

  .section { display: flex; flex-direction: column; gap: 4px; }

  .section-title {
    font-size: 14px;
    font-weight: 600;
    color: var(--color-text);
    margin: 0 0 12px;
  }

  .subsection-title {
    font-size: 13px;
    font-weight: 600;
    color: var(--color-text-secondary);
    margin: 20px 0 4px;
  }

  .subsection-desc {
    font-size: 12px;
    color: var(--color-text-muted);
    margin: 0 0 8px;
    line-height: 1.4;
  }

  .setting-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 14px;
    border-radius: 6px;
    background: var(--color-surface-raised, var(--color-surface));
  }

  .setting-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .setting-label {
    font-size: 13px;
    color: var(--color-text);
  }

  .setting-desc {
    font-size: 11px;
    color: var(--color-text-muted);
  }

  .setting-select {
    padding: 5px 10px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-surface);
    color: var(--color-text);
    font: inherit;
    font-size: 12px;
    cursor: pointer;
    outline: none;
  }

  .setting-select:focus {
    border-color: var(--color-border-emphasis);
  }

  .toggle-btn {
    padding: 5px 16px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-surface);
    color: var(--color-text-muted);
    font: inherit;
    font-size: 12px;
    cursor: pointer;
    transition: background 0.15s, color 0.15s, border-color 0.15s;
    min-width: 44px;
  }

  .toggle-on {
    background: var(--color-border-emphasis);
    color: var(--color-text);
    border-color: var(--color-border-emphasis);
  }

  .trigger-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .trigger-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    border-radius: 6px;
    background: var(--color-surface-raised, var(--color-surface));
    font-size: 13px;
  }

  .trigger-color {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .trigger-name {
    flex: 1;
    color: var(--color-text);
  }

  .trigger-mode {
    color: var(--color-text-muted);
    font-size: 11px;
    font-family: var(--font-mono);
  }

  .trigger-status {
    font-size: 11px;
    color: var(--color-text-secondary);
  }

  .trigger-disabled {
    color: var(--color-text-muted);
    opacity: 0.6;
  }

  .empty-triggers {
    padding: 16px 12px;
    color: var(--color-text-muted);
    font-size: 13px;
    text-align: center;
    line-height: 1.5;
  }
</style>

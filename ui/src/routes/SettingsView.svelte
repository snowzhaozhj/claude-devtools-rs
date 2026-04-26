<script lang="ts">
  import { onMount } from "svelte";
  import { getConfig, updateConfig, addTrigger, removeTrigger, checkForUpdate, type AppConfig, type NotificationTrigger, type CheckUpdateResult } from "../lib/api";
  import { applyTheme } from "../lib/theme";
  import SettingsToggle from "../lib/components/SettingsToggle.svelte";
  import { getVersion } from "@tauri-apps/api/app";
  import { updateStore } from "../lib/updateStore.svelte";

  let config: AppConfig | null = $state(null);
  let loading = $state(true);
  let error: string | null = $state(null);
  let saveError: string | null = $state(null);
  let activeSection: "general" | "notifications" | "about" = $state("general");

  // About / 更新 section 状态
  let appVersion = $state("");
  let checkInFlight = $state(false);
  let checkResult: CheckUpdateResult | null = $state(null);

  // 新建 trigger 表单
  let showAddForm = $state(false);
  let newName = $state("");
  let newMode: string = $state("error_status");
  let newColor = $state("#e53e3e");
  let addingTrigger = $state(false);

  onMount(async () => {
    try {
      config = await getConfig();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
    try {
      appVersion = await getVersion();
    } catch { /* mock 模式或非 Tauri 环境静默 */ }
  });

  async function updateUpdater(key: "autoUpdateCheckEnabled" | "skippedUpdateVersion", value: unknown) {
    if (!config) return;
    saveError = null;
    const prev = config.updater ?? { autoUpdateCheckEnabled: true };
    config = { ...config, updater: { ...prev, [key]: value } };
    try {
      await updateConfig("updater", { [key]: value });
    } catch (e) {
      saveError = `保存失败: ${e}`;
      try { config = await getConfig(); } catch { /* ignore */ }
    }
  }

  async function handleCheckUpdate() {
    checkInFlight = true;
    checkResult = null;
    try {
      checkResult = await checkForUpdate();
      // 如果发现新版本，写入 store 让横幅展示（手动检查也走横幅交互）
      if (checkResult.status === "available") {
        updateStore.showAvailable({
          currentVersion: checkResult.currentVersion,
          newVersion: checkResult.newVersion,
          notes: checkResult.notes,
          signatureOk: checkResult.signatureOk,
        });
      }
    } catch (e) {
      checkResult = { status: "error", message: String(e) };
    } finally {
      checkInFlight = false;
    }
  }

  async function updateGeneral(key: string, value: unknown) {
    if (!config) return;
    saveError = null;
    config = { ...config, general: { ...config.general, [key]: value } };
    if (key === "theme") applyTheme(value as string);
    try {
      await updateConfig("general", { [key]: value });
    } catch (e) {
      saveError = `保存失败: ${e}`;
      try { config = await getConfig(); applyTheme(config.general.theme); } catch { /* ignore */ }
    }
  }

  async function updateNotifications(key: string, value: unknown) {
    if (!config) return;
    saveError = null;
    config = { ...config, notifications: { ...config.notifications, [key]: value } };
    try {
      await updateConfig("notifications", { [key]: value });
    } catch (e) {
      saveError = `保存失败: ${e}`;
      try { config = await getConfig(); } catch { /* ignore */ }
    }
  }

  async function handleAddTrigger() {
    if (!newName.trim()) return;
    addingTrigger = true;
    saveError = null;
    const trigger = {
      id: `custom-${Date.now()}`,
      name: newName.trim(),
      enabled: true,
      contentType: "tool_result",
      mode: newMode,
      requireError: newMode === "error_status" ? true : undefined,
      color: newColor,
    };
    try {
      config = await addTrigger(trigger);
      showAddForm = false;
      newName = "";
      newMode = "error_status";
      newColor = "#e53e3e";
    } catch (e) {
      saveError = `添加失败: ${e}`;
    } finally {
      addingTrigger = false;
    }
  }

  async function handleRemoveTrigger(trigger: NotificationTrigger) {
    saveError = null;
    try {
      config = await removeTrigger(trigger.id);
    } catch (e) {
      saveError = `删除失败: ${e}`;
    }
  }

  async function handleToggleTrigger(trigger: NotificationTrigger) {
    if (!config) return;
    saveError = null;
    // 乐观更新
    config = {
      ...config,
      notifications: {
        ...config.notifications,
        triggers: config.notifications.triggers.map((t) =>
          t.id === trigger.id ? { ...t, enabled: !t.enabled } : t
        ),
      },
    };
    try {
      // 通过 update_config 更新整个 triggers 数组
      await updateConfig("notifications", {
        triggers: config.notifications.triggers,
      });
    } catch (e) {
      saveError = `更新失败: ${e}`;
      try { config = await getConfig(); } catch { /* ignore */ }
    }
  }

  const modeLabels: Record<string, string> = {
    error_status: "错误检测",
    content_match: "内容匹配",
    token_threshold: "Token 超限",
  };
</script>

<div class="settings-view">
  <div class="settings-header">
    <h2 class="settings-title">设置</h2>
    <div class="settings-tabs">
      <button class="section-tab" class:section-tab-active={activeSection === "general"} onclick={() => activeSection = "general"}>常规</button>
      <button class="section-tab" class:section-tab-active={activeSection === "notifications"} onclick={() => activeSection = "notifications"}>通知</button>
      <button class="section-tab" class:section-tab-active={activeSection === "about"} onclick={() => activeSection = "about"}>关于 / 更新</button>
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
            <select class="setting-select" onchange={(e) => updateGeneral("theme", (e.target as HTMLSelectElement).value)}>
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
            <select class="setting-select" onchange={(e) => updateGeneral("defaultTab", (e.target as HTMLSelectElement).value)}>
              <option value="dashboard" selected={config.general.defaultTab === "dashboard"}>仪表盘</option>
              <option value="last-session" selected={config.general.defaultTab === "last-session"}>上次会话</option>
            </select>
          </div>
          <div class="setting-row">
            <div class="setting-info">
              <span class="setting-label">自动展开 AI 组</span>
              <span class="setting-desc">打开会话时自动展开工具执行区域</span>
            </div>
            <SettingsToggle
              enabled={config.general.autoExpandAiGroups}
              onChange={(v) => updateGeneral("autoExpandAiGroups", v)}
              ariaLabel="自动展开 AI 组"
            />
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
            <SettingsToggle
              enabled={config.notifications.enabled}
              onChange={(v) => updateNotifications("enabled", v)}
              ariaLabel="启用通知"
            />
          </div>
          <div class="setting-row">
            <div class="setting-info">
              <span class="setting-label">提示音</span>
              <span class="setting-desc">收到通知时播放声音</span>
            </div>
            <SettingsToggle
              enabled={config.notifications.soundEnabled}
              onChange={(v) => updateNotifications("soundEnabled", v)}
              ariaLabel="提示音"
            />
          </div>

          <!-- 触发器区域 -->
          <div class="trigger-header">
            <h4 class="subsection-title">触发器规则</h4>
            <button class="add-btn" onclick={() => showAddForm = !showAddForm}>
              {showAddForm ? "取消" : "+ 新建"}
            </button>
          </div>
          <p class="subsection-desc">触发器监控会话中的工具错误、关键词匹配、token 超限等事件并自动产生通知。</p>

          <!-- 新建表单 -->
          {#if showAddForm}
            <div class="add-form">
              <div class="form-row">
                <label class="form-label">名称</label>
                <input class="form-input" type="text" placeholder="如：编译错误检测" bind:value={newName} />
              </div>
              <div class="form-row">
                <label class="form-label">模式</label>
                <select class="form-select" bind:value={newMode}>
                  <option value="error_status">错误检测（工具执行失败时触发）</option>
                  <option value="content_match">内容匹配（匹配关键词/正则时触发）</option>
                  <option value="token_threshold">Token 超限（token 用量超阈值时触发）</option>
                </select>
              </div>
              <div class="form-row">
                <label class="form-label">颜色</label>
                <input class="form-color" type="color" bind:value={newColor} />
              </div>
              <button class="form-submit" onclick={handleAddTrigger} disabled={!newName.trim() || addingTrigger}>
                {addingTrigger ? "添加中..." : "添加触发器"}
              </button>
            </div>
          {/if}

          <!-- 触发器列表 -->
          {#if config.notifications.triggers.length > 0}
            <div class="trigger-list">
              {#each config.notifications.triggers as trigger (trigger.id)}
                <div class="trigger-row">
                  <span class="trigger-color" style:background={trigger.color || "var(--color-text-muted)"}></span>
                  <span class="trigger-name">{trigger.name}</span>
                  <span class="trigger-mode">{modeLabels[trigger.mode] || trigger.mode}</span>
                  <SettingsToggle
                    enabled={trigger.enabled}
                    onChange={() => handleToggleTrigger(trigger)}
                    ariaLabel={trigger.enabled ? "点击禁用触发器" : "点击启用触发器"}
                  />
                  <button class="trigger-delete" onclick={() => handleRemoveTrigger(trigger)} title="删除触发器">×</button>
                </div>
              {/each}
            </div>
          {:else if !showAddForm}
            <div class="empty-triggers">暂无触发器。点击上方"+ 新建"创建第一个触发器。</div>
          {/if}
        </div>

      {:else if activeSection === "about"}
        <div class="section">
          <h3 class="section-title">关于 / 更新</h3>

          <div class="setting-row">
            <div class="setting-info">
              <span class="setting-label">启动时自动检查更新</span>
              <span class="setting-desc">应用启动 5 秒后后台检查新版本；关闭后仍可手动检查</span>
            </div>
            <SettingsToggle
              enabled={config.updater?.autoUpdateCheckEnabled ?? true}
              onChange={(v) => updateUpdater("autoUpdateCheckEnabled", v)}
              ariaLabel="启动时自动检查更新"
            />
          </div>

          <div class="setting-row">
            <div class="setting-info">
              <span class="setting-label">当前版本</span>
              <span class="setting-desc">{appVersion || "—"}</span>
            </div>
            <button class="add-btn" onclick={handleCheckUpdate} disabled={checkInFlight}>
              {checkInFlight ? "检查中..." : "检查更新"}
            </button>
          </div>

          {#if checkResult}
            <div class="check-result" class:check-error={checkResult.status === "error"}>
              {#if checkResult.status === "up_to_date"}
                已是最新版本 v{checkResult.currentVersion}
              {:else if checkResult.status === "available"}
                发现新版本 v{checkResult.newVersion}（横幅已展示，可在顶部更新）
              {:else}
                检查失败：{checkResult.message}
              {/if}
            </div>
          {/if}

          {#if config.updater?.skippedUpdateVersion}
            <div class="setting-row">
              <div class="setting-info">
                <span class="setting-label">已跳过版本</span>
                <span class="setting-desc">v{config.updater.skippedUpdateVersion}（不再提示该版本）</span>
              </div>
              <button class="add-btn" onclick={() => updateUpdater("skippedUpdateVersion", null)}>
                清除跳过
              </button>
            </div>
          {/if}
        </div>
      {/if}
    {/if}
  </div>
</div>

<style>
  .settings-view { display: flex; flex-direction: column; height: 100%; overflow: hidden; }
  .settings-header { padding: 16px 24px 0; border-bottom: 1px solid var(--color-border); flex-shrink: 0; }
  .settings-title { font-size: 16px; font-weight: 600; color: var(--color-text); margin: 0 0 12px; }
  .settings-tabs { display: flex; }
  .section-tab { padding: 8px 16px; border: none; background: none; color: var(--color-text-muted); font: inherit; font-size: 13px; cursor: pointer; border-bottom: 2px solid transparent; transition: color 0.1s, border-color 0.1s; }
  .section-tab:hover { color: var(--color-text-secondary); }
  .section-tab-active { color: var(--color-text); border-bottom-color: var(--color-border-emphasis); }
  .settings-body { flex: 1; overflow-y: auto; padding: 20px 24px; }
  .save-error { padding: 8px 12px; margin-bottom: 12px; border-radius: 6px; background: rgba(229, 62, 62, 0.1); color: var(--tool-result-error-text); font-size: 13px; }
  .state-msg { display: flex; align-items: center; justify-content: center; height: 200px; color: var(--color-text-muted); font-size: 14px; }
  .state-err { color: var(--tool-result-error-text); }
  .section { display: flex; flex-direction: column; gap: 4px; }
  .section-title { font-size: 14px; font-weight: 600; color: var(--color-text); margin: 0 0 12px; }
  .setting-row { display: flex; align-items: center; justify-content: space-between; padding: 12px 14px; border-radius: 6px; background: var(--color-surface-raised, var(--color-surface)); }
  .setting-info { display: flex; flex-direction: column; gap: 2px; }
  .setting-label { font-size: 13px; color: var(--color-text); }
  .setting-desc { font-size: 11px; color: var(--color-text-muted); }
  .setting-select { padding: 5px 10px; border: 1px solid var(--color-border); border-radius: 4px; background: var(--color-surface); color: var(--color-text); font: inherit; font-size: 12px; cursor: pointer; outline: none; }
  .setting-select:focus { border-color: var(--color-border-emphasis); }
  /* Trigger 区域 */
  .trigger-header { display: flex; align-items: center; justify-content: space-between; margin-top: 20px; }
  .subsection-title { font-size: 13px; font-weight: 600; color: var(--color-text-secondary); margin: 0; }
  .subsection-desc { font-size: 12px; color: var(--color-text-muted); margin: 4px 0 8px; line-height: 1.4; }
  .add-btn { padding: 4px 12px; border: 1px solid var(--color-border); border-radius: 4px; background: var(--color-surface); color: var(--color-text-secondary); font: inherit; font-size: 12px; cursor: pointer; transition: background 0.1s; }
  .add-btn:hover { background: var(--tool-item-hover-bg); }
  .add-btn:disabled { opacity: 0.5; cursor: not-allowed; }

  /* 关于 / 更新 section */
  .check-result { padding: 8px 12px; border-radius: 4px; background: var(--color-surface-raised, var(--color-surface)); border: 1px solid var(--color-border); color: var(--color-text-secondary); font-size: 12px; margin-top: 4px; }
  .check-error { color: var(--tool-result-error-text); border-color: rgba(229, 62, 62, 0.4); }

  /* 新建表单 */
  .add-form { display: flex; flex-direction: column; gap: 10px; padding: 14px; border-radius: 6px; background: var(--color-surface-raised, var(--color-surface)); border: 1px solid var(--color-border); margin-bottom: 8px; }
  .form-row { display: flex; align-items: center; gap: 10px; }
  .form-label { font-size: 12px; color: var(--color-text-secondary); min-width: 40px; }
  .form-input { flex: 1; padding: 5px 10px; border: 1px solid var(--color-border); border-radius: 4px; background: var(--color-surface); color: var(--color-text); font: inherit; font-size: 12px; outline: none; }
  .form-input:focus { border-color: var(--color-border-emphasis); }
  .form-select { flex: 1; padding: 5px 10px; border: 1px solid var(--color-border); border-radius: 4px; background: var(--color-surface); color: var(--color-text); font: inherit; font-size: 12px; outline: none; }
  .form-color { width: 32px; height: 28px; border: 1px solid var(--color-border); border-radius: 4px; padding: 2px; cursor: pointer; background: var(--color-surface); }
  .form-submit { align-self: flex-end; padding: 6px 16px; border: none; border-radius: 4px; background: var(--color-border-emphasis); color: var(--color-text); font: inherit; font-size: 12px; cursor: pointer; transition: opacity 0.1s; }
  .form-submit:hover { opacity: 0.85; }
  .form-submit:disabled { opacity: 0.5; cursor: default; }

  /* 触发器列表 */
  .trigger-list { display: flex; flex-direction: column; gap: 4px; }
  .trigger-row { display: flex; align-items: center; gap: 8px; padding: 8px 12px; border-radius: 6px; background: var(--color-surface-raised, var(--color-surface)); font-size: 13px; }
  .trigger-color { width: 10px; height: 10px; border-radius: 50%; flex-shrink: 0; }
  .trigger-name { flex: 1; color: var(--color-text); }
  .trigger-mode { color: var(--color-text-muted); font-size: 11px; font-family: var(--font-mono); }
  .trigger-delete { display: flex; align-items: center; justify-content: center; width: 22px; height: 22px; border: none; border-radius: 4px; background: transparent; color: var(--color-text-muted); font-size: 14px; cursor: pointer; flex-shrink: 0; transition: background 0.1s, color 0.1s; }
  .trigger-delete:hover { background: rgba(229, 62, 62, 0.15); color: var(--tool-result-error-text); }
  .empty-triggers { padding: 16px 12px; color: var(--color-text-muted); font-size: 13px; text-align: center; line-height: 1.5; }
</style>

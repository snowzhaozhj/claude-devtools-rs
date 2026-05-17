<script lang="ts">
  import { onMount } from "svelte";
  import { getConfig, updateConfig, addTrigger, removeTrigger, checkForUpdate, type AppConfig, type NotificationTrigger, type CheckUpdateResult } from "../lib/api";
  import { applyTheme } from "../lib/theme";
  import { applyFonts } from "../lib/fonts";
  import { setSessionClickBehavior, type SessionClickBehavior } from "../lib/tabStore.svelte";
  import SettingsToggle from "../lib/components/SettingsToggle.svelte";
  import SettingsGroup from "../lib/components/SettingsGroup.svelte";
  import SettingsField from "../lib/components/SettingsField.svelte";
  import SettingsButton from "../lib/components/SettingsButton.svelte";
  import SkeletonList from "../components/SkeletonList.svelte";
  import { getVersion } from "@tauri-apps/api/app";
  import { updateStore } from "../lib/updateStore.svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import {
    SLIDERS_HORIZONTAL_SVG,
    MONITOR_SVG,
    BELL,
    INFO_SVG,
    FOLDER_SVG,
    ROTATE_CCW_SVG,
    PLUS_SVG,
    X_SVG,
    CHECK_CIRCLE_SVG,
    DOWNLOAD_CLOUD_SVG,
    ALERT_CIRCLE_SVG,
    BELL_RING_SVG,
  } from "../lib/icons";

  type SectionId = "general" | "display" | "notifications" | "about";

  let config: AppConfig | null = $state(null);
  let loading = $state(true);
  let error: string | null = $state(null);
  let saveError: string | null = $state(null);
  let activeSection: SectionId = $state("general");

  let fontSansInput = $state("");
  let fontMonoInput = $state("");
  let claudeRootInput = $state("");

  const FONT_SANS_PLACEHOLDER = `-apple-system, BlinkMacSystemFont, "Segoe UI", "Roboto", sans-serif`;
  const FONT_MONO_PLACEHOLDER = `ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace`;

  let appVersion = $state("");
  let checkInFlight = $state(false);
  let checkResult: CheckUpdateResult | null = $state(null);

  let showAddForm = $state(false);
  let newName = $state("");
  let newMode: string = $state("error_status");
  let newColor = $state("#e53e3e");
  let addingTrigger = $state(false);

  /** 跟踪 viewport 是否窄屏，用于切 nav 方向 + 同步 aria-orientation。
   *  初值直接读 matchMedia，避免首帧 CSS（已横排）与 ARIA（仍 vertical）错配。 */
  let isNarrowViewport = $state(
    typeof window !== "undefined" && window.matchMedia?.("(max-width: 720px)").matches === true,
  );

  const sections: Array<{ id: SectionId; label: string; description: string; icon: string }> = [
    { id: "general", label: "常规", description: "主题、启动行为、数据目录", icon: SLIDERS_HORIZONTAL_SVG },
    { id: "display", label: "显示", description: "界面字体与视觉密度", icon: MONITOR_SVG },
    { id: "notifications", label: "通知", description: "事件触发与提示音", icon: BELL },
    { id: "about", label: "关于", description: "版本与更新", icon: INFO_SVG },
  ];

  onMount(async () => {
    try {
      config = await getConfig();
      fontSansInput = config.display?.fontSans ?? "";
      fontMonoInput = config.display?.fontMono ?? "";
      claudeRootInput = config!.general.claudeRootPath ?? "";
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
    try {
      appVersion = await getVersion();
    } catch {
      /* mock 模式或非 Tauri 环境静默 */
    }
  });

  $effect(() => {
    if (typeof window === "undefined" || typeof window.matchMedia !== "function") return;
    const mq = window.matchMedia("(max-width: 720px)");
    isNarrowViewport = mq.matches;
    const onChange = (e: MediaQueryListEvent) => (isNarrowViewport = e.matches);
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  });

  async function commitFont(key: "fontSans" | "fontMono", raw: string) {
    if (!config) return;
    saveError = null;
    const trimmed = raw.trim();
    const value: string | null = trimmed === "" ? null : trimmed;
    const prevDisplay = config.display ?? {};
    config = { ...config, display: { ...prevDisplay, [key]: value } };
    applyFonts(config);
    try {
      await updateConfig("display", { [key]: value });
    } catch (e) {
      saveError = `保存失败: ${e}`;
      try {
        config = await getConfig();
        fontSansInput = config.display?.fontSans ?? "";
        fontMonoInput = config.display?.fontMono ?? "";
        applyFonts(config);
      } catch {
        /* ignore */
      }
    }
  }

  async function resetFontsToDefault() {
    if (!config) return;
    saveError = null;
    const prevDisplay = config.display ?? {};
    config = { ...config, display: { ...prevDisplay, fontSans: null, fontMono: null } };
    fontSansInput = "";
    fontMonoInput = "";
    applyFonts(config);
    try {
      await updateConfig("display", { fontSans: null, fontMono: null });
    } catch (e) {
      saveError = `重置失败: ${e}`;
      try {
        config = await getConfig();
        fontSansInput = config.display?.fontSans ?? "";
        fontMonoInput = config.display?.fontMono ?? "";
        applyFonts(config);
      } catch {
        /* ignore */
      }
    }
  }

  async function updateUpdater(key: "autoUpdateCheckEnabled" | "skippedUpdateVersion", value: unknown) {
    if (!config) return;
    saveError = null;
    const prev = config.updater ?? { autoUpdateCheckEnabled: true };
    config = { ...config, updater: { ...prev, [key]: value } };
    try {
      await updateConfig("updater", { [key]: value });
    } catch (e) {
      saveError = `保存失败: ${e}`;
      try {
        config = await getConfig();
      } catch {
        /* ignore */
      }
    }
  }

  async function handleCheckUpdate() {
    checkInFlight = true;
    checkResult = null;
    try {
      checkResult = await checkForUpdate();
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
    if (key === "sessionClickBehavior" && (value === "replace" || value === "new-tab")) {
      setSessionClickBehavior(value as SessionClickBehavior);
    }
    try {
      await updateConfig("general", { [key]: value });
      if (key === "claudeRootPath") {
        window.dispatchEvent(new CustomEvent("cdt-refresh-projects"));
      }
    } catch (e) {
      saveError = `保存失败: ${e}`;
      try {
        config = await getConfig();
        claudeRootInput = config!.general.claudeRootPath ?? "";
        applyTheme(config!.general.theme);
      } catch {
        /* ignore */
      }
    }
  }

  async function commitClaudeRoot() {
    const value = claudeRootInput.trim() === "" ? null : claudeRootInput.trim();
    await updateGeneral("claudeRootPath", value);
    if (config) claudeRootInput = config!.general.claudeRootPath ?? "";
  }

  async function resetClaudeRoot() {
    claudeRootInput = "";
    await updateGeneral("claudeRootPath", null);
  }

  async function chooseClaudeRoot() {
    saveError = null;
    try {
      const selected = await open({ directory: true, multiple: false, title: "选择 Claude 数据根目录" });
      if (typeof selected !== "string") return;
      claudeRootInput = selected;
      await updateGeneral("claudeRootPath", selected);
    } catch (e) {
      saveError = `选择目录失败: ${e}`;
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
      try {
        config = await getConfig();
      } catch {
        /* ignore */
      }
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
    config = {
      ...config,
      notifications: {
        ...config.notifications,
        triggers: config!.notifications.triggers.map((t) =>
          t.id === trigger.id ? { ...t, enabled: !t.enabled } : t
        ),
      },
    };
    try {
      await updateConfig("notifications", {
        triggers: config!.notifications.triggers,
      });
    } catch (e) {
      saveError = `更新失败: ${e}`;
      try {
        config = await getConfig();
      } catch {
        /* ignore */
      }
    }
  }

  const modeLabels: Record<string, string> = {
    error_status: "错误检测",
    content_match: "内容匹配",
    token_threshold: "Token 超限",
  };

  const sectionMeta = $derived(sections.find((s) => s.id === activeSection)!);
</script>

<div class="settings-view">
  <nav class="settings-nav" aria-label="设置分类">
    <h2 class="nav-title">设置</h2>
    <ul class="nav-list" role="tablist" aria-orientation={isNarrowViewport ? "horizontal" : "vertical"}>
      {#each sections as section (section.id)}
        <li role="none">
          <button
            type="button"
            role="tab"
            id="settings-tab-{section.id}"
            aria-label={section.label}
            aria-selected={activeSection === section.id}
            aria-controls="settings-panel"
            tabindex={activeSection === section.id ? 0 : -1}
            class="nav-item"
            class:nav-item-active={activeSection === section.id}
            onclick={() => (activeSection = section.id)}
          >
            <span class="nav-icon" aria-hidden="true">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                {#if section.id === "notifications"}
                  <path d={section.icon} />
                {:else}
                  {@html section.icon}
                {/if}
              </svg>
            </span>
            <span class="nav-label">
              <span class="nav-label-title">{section.label}</span>
              <span class="nav-label-desc">{section.description}</span>
            </span>
          </button>
        </li>
      {/each}
    </ul>
  </nav>

  <div class="settings-content" id="settings-panel" role="tabpanel" tabindex="-1" aria-labelledby="settings-tab-{activeSection}">
    <header class="content-header">
      <h1 class="content-title">{sectionMeta.label}</h1>
      <p class="content-subtitle">{sectionMeta.description}</p>
      {#if saveError}
        <div class="banner banner-error" role="alert">
          <span class="banner-icon" aria-hidden="true">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{@html ALERT_CIRCLE_SVG}</svg>
          </span>
          <span>{saveError}</span>
        </div>
      {/if}
    </header>

    <div class="content-body">
      {#if loading && !config}
        <SkeletonList count={5} rowHeight={60} gap={10} padding="8px 0" label="正在加载设置" />
      {:else if error}
        <div class="state-msg state-err">{error}</div>
      {:else if config}
        {#if activeSection === "general"}
          <SettingsGroup title="外观">
            <SettingsField label="主题" description="深色 / 浅色 / 跟随系统">
              {#snippet control()}
                <select
                  class="control-select"
                  aria-label="主题"
                  onchange={(e) => updateGeneral("theme", (e.target as HTMLSelectElement).value)}
                >
                  <option value="dark" selected={config!.general.theme === "dark"}>深色</option>
                  <option value="light" selected={config!.general.theme === "light"}>浅色</option>
                  <option value="system" selected={config!.general.theme === "system"}>跟随系统</option>
                </select>
              {/snippet}
            </SettingsField>
          </SettingsGroup>

          <SettingsGroup title="启动与交互">
            <SettingsField label="默认打开页面" description="应用启动时显示的内容">
              {#snippet control()}
                <select
                  class="control-select"
                  aria-label="默认打开页面"
                  onchange={(e) => updateGeneral("defaultTab", (e.target as HTMLSelectElement).value)}
                >
                  <option value="dashboard" selected={config!.general.defaultTab === "dashboard"}>仪表盘</option>
                  <option value="last-session" selected={config!.general.defaultTab === "last-session"}>上次会话</option>
                </select>
              {/snippet}
            </SettingsField>
            <SettingsField
              label="点击会话默认行为"
              description="侧栏点击会话项的默认动作；Cmd / Ctrl + 点击始终翻转该默认"
            >
              {#snippet control()}
                <select
                  class="control-select"
                  aria-label="点击会话默认行为"
                  onchange={(e) => updateGeneral("sessionClickBehavior", (e.target as HTMLSelectElement).value)}
                >
                  <option value="replace" selected={(config!.general.sessionClickBehavior ?? "replace") === "replace"}>替换当前标签页</option>
                  <option value="new-tab" selected={config!.general.sessionClickBehavior === "new-tab"}>每次开新标签页</option>
                </select>
              {/snippet}
            </SettingsField>
            <SettingsField label="自动展开 AI 组" description="打开会话时自动展开工具执行区域">
              {#snippet control()}
                <SettingsToggle
                  enabled={config!.general.autoExpandAiGroups}
                  onChange={(v) => updateGeneral("autoExpandAiGroups", v)}
                  ariaLabel="自动展开 AI 组"
                />
              {/snippet}
            </SettingsField>
          </SettingsGroup>

          <SettingsGroup
            title="数据目录"
            description="留空使用默认目录；项目来自该目录下的 projects，待办来自 todos"
          >
            <SettingsField label="Claude 数据根目录" layout="stack" labelFor="claude-root-input">
              {#snippet control()}
                <input
                  id="claude-root-input"
                  class="control-input control-input-mono"
                  type="text"
                  placeholder="默认 ~/.claude"
                  aria-label="Claude 数据根目录"
                  bind:value={claudeRootInput}
                  onkeydown={(e) => {
                    if (e.key === "Enter") commitClaudeRoot();
                  }}
                />
                <SettingsButton variant="ghost" onClick={chooseClaudeRoot}>
                  {#snippet icon()}
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{@html FOLDER_SVG}</svg>
                  {/snippet}
                  选择目录
                </SettingsButton>
                <SettingsButton variant="ghost" onClick={commitClaudeRoot}>保存手动输入</SettingsButton>
                <SettingsButton
                  variant="ghost"
                  disabled={config!.general.claudeRootPath === null}
                  onClick={resetClaudeRoot}
                >
                  {#snippet icon()}
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{@html ROTATE_CCW_SVG}</svg>
                  {/snippet}
                  恢复默认
                </SettingsButton>
              {/snippet}
            </SettingsField>
          </SettingsGroup>
        {:else if activeSection === "display"}
          <SettingsGroup
            title="界面字体"
            description="留空使用应用默认字体栈，改动立即生效，无需重启"
          >
            <SettingsField label="界面字体（sans-serif）" layout="stack" labelFor="font-sans-input">
              {#snippet control()}
                <input
                  id="font-sans-input"
                  class="control-input control-input-mono"
                  type="text"
                  placeholder={FONT_SANS_PLACEHOLDER}
                  bind:value={fontSansInput}
                  onblur={() => commitFont("fontSans", fontSansInput)}
                  onkeydown={(e) => {
                    if (e.key === "Enter") (e.target as HTMLInputElement).blur();
                  }}
                />
              {/snippet}
              <div class="field-hint">示例：<code>"Inter", "PingFang SC", sans-serif</code></div>
            </SettingsField>
            <SettingsField label="等宽字体（monospace）" layout="stack" labelFor="font-mono-input">
              {#snippet control()}
                <input
                  id="font-mono-input"
                  class="control-input control-input-mono"
                  type="text"
                  placeholder={FONT_MONO_PLACEHOLDER}
                  bind:value={fontMonoInput}
                  onblur={() => commitFont("fontMono", fontMonoInput)}
                  onkeydown={(e) => {
                    if (e.key === "Enter") (e.target as HTMLInputElement).blur();
                  }}
                />
              {/snippet}
              <div class="field-hint">示例：<code>"JetBrains Mono", "Fira Code", monospace</code></div>
            </SettingsField>
            {#snippet footer()}
              <SettingsButton variant="ghost" onClick={resetFontsToDefault}>
                {#snippet icon()}
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{@html ROTATE_CCW_SVG}</svg>
                {/snippet}
                恢复默认字体
              </SettingsButton>
            {/snippet}
          </SettingsGroup>
        {:else if activeSection === "notifications"}
          <SettingsGroup title="基础通知">
            <SettingsField label="启用通知" description="当触发器规则匹配时产生通知">
              {#snippet control()}
                <SettingsToggle
                  enabled={config!.notifications.enabled}
                  onChange={(v) => updateNotifications("enabled", v)}
                  ariaLabel="启用通知"
                />
              {/snippet}
            </SettingsField>
            <SettingsField label="提示音" description="收到通知时播放声音">
              {#snippet control()}
                <SettingsToggle
                  enabled={config!.notifications.soundEnabled}
                  onChange={(v) => updateNotifications("soundEnabled", v)}
                  ariaLabel="提示音"
                />
              {/snippet}
            </SettingsField>
          </SettingsGroup>

          <SettingsGroup
            title="触发器规则"
            description="监控会话中的工具错误、关键词匹配、token 超限等事件并自动产生通知"
          >
            {#if showAddForm}
              <div class="trigger-form">
                <div class="trigger-form-row">
                  <label class="trigger-form-label" for="trigger-name-input">名称</label>
                  <input
                    id="trigger-name-input"
                    class="control-input"
                    type="text"
                    placeholder="如：编译错误检测"
                    bind:value={newName}
                  />
                </div>
                <div class="trigger-form-row">
                  <label class="trigger-form-label" for="trigger-mode-select">模式</label>
                  <select id="trigger-mode-select" class="control-select" bind:value={newMode}>
                    <option value="error_status">错误检测（工具执行失败时触发）</option>
                    <option value="content_match">内容匹配（匹配关键词或正则时触发）</option>
                    <option value="token_threshold">Token 超限（token 用量超阈值时触发）</option>
                  </select>
                </div>
                <div class="trigger-form-row">
                  <label class="trigger-form-label" for="trigger-color-input">颜色</label>
                  <input id="trigger-color-input" class="control-color" type="color" bind:value={newColor} />
                </div>
                <div class="trigger-form-actions">
                  <SettingsButton variant="ghost" onClick={() => (showAddForm = false)}>取消</SettingsButton>
                  <SettingsButton
                    variant="primary"
                    disabled={!newName.trim() || addingTrigger}
                    onClick={handleAddTrigger}
                  >
                    {addingTrigger ? "添加中..." : "添加触发器"}
                  </SettingsButton>
                </div>
              </div>
            {:else if config!.notifications.triggers.length > 0}
              {#each config!.notifications.triggers as trigger (trigger.id)}
                <div class="trigger-row">
                  <span
                    class="trigger-dot"
                    style:background={trigger.color || "var(--color-text-muted)"}
                    aria-hidden="true"
                  ></span>
                  <div class="trigger-meta">
                    <span class="trigger-name">{trigger.name}</span>
                    <span class="trigger-mode">{modeLabels[trigger.mode] || trigger.mode}</span>
                  </div>
                  <SettingsToggle
                    enabled={trigger.enabled}
                    onChange={() => handleToggleTrigger(trigger)}
                    ariaLabel={trigger.enabled ? "禁用触发器" : "启用触发器"}
                  />
                  <SettingsButton
                    variant="danger"
                    size="sm"
                    iconOnly
                    ariaLabel="删除触发器"
                    title="删除触发器"
                    onClick={() => handleRemoveTrigger(trigger)}
                  >
                    {#snippet icon()}
                      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{@html X_SVG}</svg>
                    {/snippet}
                  </SettingsButton>
                </div>
              {/each}
            {:else}
              <div class="empty-state">
                <span class="empty-icon" aria-hidden="true">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">{@html BELL_RING_SVG}</svg>
                </span>
                <div class="empty-title">暂无触发器</div>
                <p class="empty-desc">创建一个触发器以在错误、关键词匹配或 token 超限时收到通知</p>
              </div>
            {/if}
            {#snippet footer()}
              {#if !showAddForm}
                <SettingsButton variant="primary" onClick={() => (showAddForm = true)}>
                  {#snippet icon()}
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">{@html PLUS_SVG}</svg>
                  {/snippet}
                  新建触发器
                </SettingsButton>
              {/if}
            {/snippet}
          </SettingsGroup>
        {:else if activeSection === "about"}
          <div class="about-hero">
            <div class="about-app">
              <div class="about-app-mark" aria-hidden="true">cdt</div>
              <div class="about-app-meta">
                <div class="about-app-name">claude-devtools</div>
                <div class="about-app-version">版本 {appVersion || "—"}</div>
              </div>
            </div>
            <SettingsButton variant="primary" disabled={checkInFlight} onClick={handleCheckUpdate}>
              {#snippet icon()}
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{@html DOWNLOAD_CLOUD_SVG}</svg>
              {/snippet}
              {checkInFlight ? "检查中..." : "检查更新"}
            </SettingsButton>
          </div>

          {#if checkResult}
            <div
              class="banner"
              class:banner-success={checkResult.status === "up_to_date"}
              class:banner-info={checkResult.status === "available"}
              class:banner-error={checkResult.status === "error"}
              role="status"
            >
              <span class="banner-icon" aria-hidden="true">
                {#if checkResult.status === "up_to_date"}
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{@html CHECK_CIRCLE_SVG}</svg>
                {:else if checkResult.status === "available"}
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{@html DOWNLOAD_CLOUD_SVG}</svg>
                {:else}
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{@html ALERT_CIRCLE_SVG}</svg>
                {/if}
              </span>
              <span>
                {#if checkResult.status === "up_to_date"}
                  已是最新版本 v{checkResult.currentVersion}
                {:else if checkResult.status === "available"}
                  发现新版本 v{checkResult.newVersion}，横幅已展示，可在顶部更新
                {:else}
                  检查失败：{checkResult.message}
                {/if}
              </span>
            </div>
          {/if}

          <SettingsGroup title="更新">
            <SettingsField
              label="启动时自动检查更新"
              description="应用启动 5 秒后后台检查；关闭后仍可手动检查"
            >
              {#snippet control()}
                <SettingsToggle
                  enabled={config!.updater?.autoUpdateCheckEnabled ?? true}
                  onChange={(v) => updateUpdater("autoUpdateCheckEnabled", v)}
                  ariaLabel="启动时自动检查更新"
                />
              {/snippet}
            </SettingsField>
            {#if config!.updater?.skippedUpdateVersion}
              <SettingsField
                label="已跳过版本"
                description="v{config.updater.skippedUpdateVersion}，不再提示该版本"
              >
                {#snippet control()}
                  <SettingsButton variant="ghost" onClick={() => updateUpdater("skippedUpdateVersion", null)}>
                    清除跳过
                  </SettingsButton>
                {/snippet}
              </SettingsField>
            {/if}
          </SettingsGroup>
        {/if}
      {/if}
    </div>
  </div>
</div>

<style>
  .settings-view {
    display: flex;
    height: 100%;
    overflow: hidden;
    background: var(--color-surface);
  }

  /* 左侧分类导航 */
  .settings-nav {
    flex-shrink: 0;
    width: 220px;
    padding: 18px 12px;
    border-right: 1px solid var(--color-border);
    background: var(--color-surface-sidebar);
    overflow-y: auto;
  }
  .nav-title {
    margin: 0 8px 14px;
    font-size: 11px;
    font-weight: 600;
    color: var(--color-text-muted);
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  .nav-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .nav-item {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    width: 100%;
    padding: 8px 10px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: var(--color-text-secondary);
    font: inherit;
    text-align: left;
    cursor: pointer;
    transition: background-color 0.12s, color 0.12s;
  }
  .nav-item:hover {
    background: var(--tool-item-hover-bg);
    color: var(--color-text);
  }
  .nav-item:focus-visible {
    outline: 2px solid var(--color-switch-on);
    outline-offset: -2px;
  }
  .nav-item-active {
    background: var(--color-surface-raised);
    color: var(--color-text);
  }
  .nav-icon {
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 16px;
    height: 16px;
    margin-top: 1px;
  }
  .nav-icon :global(svg) {
    width: 16px;
    height: 16px;
  }
  .nav-label {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .nav-label-title {
    font-size: 13px;
    font-weight: 500;
    line-height: 1.3;
  }
  .nav-label-desc {
    font-size: 11px;
    color: var(--color-text-muted);
    line-height: 1.35;
  }
  .nav-item-active .nav-label-desc {
    color: var(--color-text-secondary);
  }

  /* 右侧内容 */
  .settings-content {
    flex: 1;
    overflow-y: auto;
    padding: 28px 36px 48px;
    min-width: 0;
  }
  .settings-content:focus {
    outline: none;
  }
  .content-header {
    margin-bottom: 24px;
    max-width: 720px;
  }
  .content-title {
    margin: 0 0 4px;
    font-size: 22px;
    font-weight: 600;
    color: var(--color-text);
    letter-spacing: -0.012em;
  }
  .content-subtitle {
    margin: 0;
    font-size: 13px;
    color: var(--color-text-secondary);
  }
  .content-body {
    display: flex;
    flex-direction: column;
    gap: 28px;
    max-width: 720px;
  }

  .state-msg {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 200px;
    color: var(--color-text-muted);
    font-size: 14px;
  }
  .state-err {
    color: var(--tool-result-error-text);
  }

  /* Banner */
  .banner {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    margin-top: 14px;
    padding: 10px 14px;
    border: 1px solid var(--color-border);
    border-radius: 8px;
    background: var(--color-surface-raised);
    color: var(--color-text-secondary);
    font-size: 13px;
    line-height: 1.5;
  }
  .banner-icon {
    flex-shrink: 0;
    display: inline-flex;
    width: 16px;
    height: 16px;
    margin-top: 1px;
  }
  .banner-icon :global(svg) {
    width: 16px;
    height: 16px;
  }
  .banner-error {
    border-color: color-mix(in oklch, var(--color-danger-bright) 35%, var(--color-border));
    background: color-mix(in oklch, var(--color-danger-bright) 8%, var(--color-surface));
    color: var(--tool-result-error-text);
  }
  .banner-success {
    border-color: color-mix(in oklch, var(--color-success-bright) 35%, var(--color-border));
    background: color-mix(in oklch, var(--color-success-bright) 8%, var(--color-surface));
    color: var(--color-success);
  }
  .banner-info {
    border-color: color-mix(in oklch, var(--color-accent-blue) 35%, var(--color-border));
    background: color-mix(in oklch, var(--color-accent-blue) 8%, var(--color-surface));
    color: var(--color-info-text);
  }

  /* 统一控件 */
  .content-body :global(.control-input),
  .content-body :global(.control-select) {
    flex: 1;
    height: 30px;
    padding: 0 10px;
    border: 1px solid var(--color-border);
    border-radius: 6px;
    background: var(--color-surface);
    color: var(--color-text);
    font: inherit;
    font-size: 13px;
    outline: none;
    transition: border-color 0.12s, box-shadow 0.12s;
  }
  .content-body :global(.control-select) {
    cursor: pointer;
    min-width: 180px;
  }
  .content-body :global(.control-input:focus),
  .content-body :global(.control-select:focus) {
    border-color: var(--color-switch-on);
    box-shadow: 0 0 0 3px color-mix(in oklch, var(--color-switch-on) 18%, transparent);
  }
  .content-body :global(.control-input-mono) {
    font-family: var(--font-mono);
    font-size: 12px;
  }
  .content-body :global(.control-color) {
    width: 38px;
    height: 30px;
    padding: 2px;
    border: 1px solid var(--color-border);
    border-radius: 6px;
    background: var(--color-surface);
    cursor: pointer;
  }

  /* Display 段提示 */
  .field-hint {
    margin-top: -2px;
    font-size: 11px;
    color: var(--color-text-muted);
  }
  .field-hint code {
    padding: 1px 5px;
    border-radius: 3px;
    background: var(--color-surface-overlay);
    font-family: var(--font-mono);
    font-size: 11px;
  }

  /* 触发器表单 */
  .trigger-form {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 16px;
    background: var(--color-surface-raised);
  }
  .trigger-form-row {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .trigger-form-label {
    flex-shrink: 0;
    width: 56px;
    font-size: 12px;
    font-weight: 500;
    color: var(--color-text-secondary);
  }
  .trigger-form-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding-top: 4px;
  }

  /* 触发器列表 */
  .trigger-row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 16px;
    background: var(--color-surface);
  }
  .trigger-row:hover {
    background: var(--tool-item-hover-bg);
  }
  .trigger-dot {
    flex-shrink: 0;
    width: 10px;
    height: 10px;
    border-radius: 50%;
  }
  .trigger-meta {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .trigger-name {
    font-size: 13px;
    font-weight: 500;
    color: var(--color-text);
  }
  .trigger-mode {
    font-size: 11px;
    color: var(--color-text-muted);
  }

  /* Empty state */
  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    padding: 32px 16px;
    text-align: center;
  }
  .empty-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 36px;
    height: 36px;
    margin-bottom: 4px;
    border-radius: 50%;
    background: var(--color-surface-raised);
    color: var(--color-text-muted);
  }
  .empty-icon :global(svg) {
    width: 18px;
    height: 18px;
  }
  .empty-title {
    font-size: 14px;
    font-weight: 500;
    color: var(--color-text);
  }
  .empty-desc {
    margin: 0;
    max-width: 320px;
    font-size: 12px;
    color: var(--color-text-secondary);
    line-height: 1.5;
  }

  /* About hero */
  .about-hero {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 20px 24px;
    border: 1px solid var(--color-border);
    border-radius: 10px;
    background: linear-gradient(
      135deg,
      color-mix(in oklch, var(--color-switch-on) 6%, var(--color-surface-raised)),
      var(--color-surface-raised)
    );
  }
  .about-app {
    display: flex;
    align-items: center;
    gap: 14px;
    min-width: 0;
  }
  .about-app-mark {
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 44px;
    height: 44px;
    border-radius: 10px;
    background: var(--color-surface);
    border: 1px solid var(--color-border-emphasis);
    font-family: var(--font-mono);
    font-size: 13px;
    font-weight: 600;
    color: var(--color-switch-on);
    letter-spacing: -0.02em;
  }
  .about-app-meta {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .about-app-name {
    font-size: 15px;
    font-weight: 600;
    color: var(--color-text);
  }
  .about-app-version {
    font-size: 12px;
    color: var(--color-text-secondary);
    font-family: var(--font-mono);
  }

  /* 窄 viewport：左 nav 改顶部 chip 横排 + 内容区放宽 padding */
  @media (max-width: 720px) {
    .settings-view {
      flex-direction: column;
    }
    .settings-nav {
      width: 100%;
      padding: 12px 14px;
      border-right: none;
      border-bottom: 1px solid var(--color-border);
      overflow-y: visible;
    }
    .nav-title {
      margin: 0 6px 10px;
    }
    .nav-list {
      flex-direction: row;
      gap: 4px;
      overflow-x: auto;
      scrollbar-width: none;
    }
    .nav-list::-webkit-scrollbar {
      display: none;
    }
    .nav-item {
      flex-shrink: 0;
      padding: 6px 12px;
    }
    .nav-label-desc {
      display: none;
    }
    .settings-content {
      padding: 20px 16px 40px;
      overflow-x: auto;
    }
    .content-header {
      max-width: none;
    }
    .content-body {
      max-width: none;
    }
    .about-hero {
      flex-direction: column;
      align-items: stretch;
      gap: 14px;
    }
  }
</style>

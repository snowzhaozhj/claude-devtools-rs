<script lang="ts">
  import { onMount } from "svelte";
  import SettingsGroup from "../../lib/components/SettingsGroup.svelte";
  import SettingsField from "../../lib/components/SettingsField.svelte";
  import SettingsButton from "../../lib/components/SettingsButton.svelte";
  import Dropdown from "../../lib/components/Dropdown.svelte";
  import Modal from "../../lib/components/Modal.svelte";
  import ConnectionStatusBadge from "../../lib/components/ConnectionStatusBadge.svelte";
  import { connectionStore } from "../../lib/stores/connection.svelte";
  import {
    ALERT_CIRCLE_SVG,
    ALERT_TRIANGLE_SVG,
    CHECK_CIRCLE_SVG,
    CHEVRON_DOWN,
    SERVER_SVG,
    WIFI_OFF_SVG,
  } from "../../lib/icons";
  import type { AuthAttempt, AuthSource, AuthOutcome } from "../../lib/types/ssh";

  let hostFocused = $state(false);
  let hostCombobox: HTMLDivElement | null = $state(null);
  let showConfigHosts = $state(false);
  let showSaveProfile = $state(false);
  let profileName = $state("");
  let savingProfile = $state(false);
  let diagnosticOpen = $state(false);

  const isWindowsPlatform =
    typeof navigator !== "undefined" && /Windows/i.test(navigator.userAgent);

  const filteredHosts = $derived.by(() => {
    const query = connectionStore.host.trim().toLowerCase();
    if (!query) return connectionStore.configHosts;
    if (connectionStore.configHosts.some((alias) => alias.toLowerCase() === query)) return connectionStore.configHosts;
    return connectionStore.configHosts.filter((alias) => alias.toLowerCase().includes(query));
  });

  onMount(() => {
    void connectionStore.initialize();
    void connectionStore.startListening();
    const onPointerDown = (event: PointerEvent) => {
      if (!hostCombobox?.contains(event.target as Node)) hostFocused = false;
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") hostFocused = false;
    };
    document.addEventListener("pointerdown", onPointerDown);
    document.addEventListener("keydown", onKeyDown);
    return () => {
      connectionStore.stopListening();
      document.removeEventListener("pointerdown", onPointerDown);
      document.removeEventListener("keydown", onKeyDown);
    };
  });

  $effect(() => {
    if (connectionStore.status === "error" || connectionStore.testResult?.success === false) {
      diagnosticOpen = true;
    } else if (connectionStore.status === "connected") {
      diagnosticOpen = false;
    }
  });

  function sourceLabel(source: AuthSource): string {
    switch (source.type) {
      case "identityAgent": return `IdentityAgent ${source.data}`;
      case "envAgent": return "SSH_AUTH_SOCK";
      case "launchctlAgent": return "launchctl agent";
      case "onePasswordAgent": return `1Password ${source.data}`;
      case "identityFile": return `IdentityFile ${source.data}`;
      case "defaultKey": return `默认密钥 ${source.data}`;
      case "password": return "密码";
    }
  }

  function outcomeLabel(outcome: AuthOutcome): string {
    switch (outcome.type) {
      case "success": return "通过";
      case "failure": return `失败：${outcome.data}`;
      case "skipped": return `跳过：${outcome.data}`;
    }
  }

  function authMethodLabel(method: string): string {
    return method === "password" ? "密码" : "SSH 配置";
  }

  async function selectAlias(alias: string) {
    hostFocused = false;
    await connectionStore.resolveHost(alias);
  }

  function openSaveProfile() {
    profileName = connectionStore.host || "";
    showSaveProfile = true;
  }

  async function confirmSaveProfile() {
    const trimmed = profileName.trim() || connectionStore.host;
    if (!trimmed) return;
    savingProfile = true;
    try {
      await connectionStore.saveProfile(trimmed);
      profileName = "";
      showSaveProfile = false;
    } finally {
      savingProfile = false;
    }
  }

  function cancelSaveProfile() {
    showSaveProfile = false;
    profileName = "";
  }

  const validPort = $derived(Number.isInteger(connectionStore.port) && connectionStore.port >= 1 && connectionStore.port <= 65535);
  const canSubmit = $derived(connectionStore.host.trim().length > 0 && validPort && !connectionStore.actionInFlight);
  const diagnosticChain = $derived(
    connectionStore.errorDetail?.attempts ?? connectionStore.authChain,
  );
  const isConnected = $derived(connectionStore.status === "connected");
  const showQuickConnect = $derived(
    !isConnected && (connectionStore.savedProfiles.length > 0 || connectionStore.configHosts.length > 0),
  );
</script>

<!-- 1. 连接状态 -->
<SettingsGroup>
  <div class="status-card status-card-{connectionStore.status}">
    <ConnectionStatusBadge
      status={connectionStore.status}
      contextId={connectionStore.activeContextId}
      error={connectionStore.error}
      showText={false}
    />
    <div class="status-copy">
      {#if isConnected}
        <strong>已连接 · {connectionStore.connectedHost ?? connectionStore.host}</strong>
        <span>项目列表来自远端</span>
      {:else if connectionStore.status === "connecting"}
        <strong>正在连接 {connectionStore.host}</strong>
        <span>依次尝试 ssh config、agent、IdentityFile、密码</span>
      {:else if connectionStore.status === "error"}
        <strong>连接失败</strong>
        <span>{connectionStore.error ?? "请展开下方诊断查看详情"}</span>
      {:else}
        <strong>本地模式</strong>
        <span>未连接到远端</span>
      {/if}
    </div>
    {#if isConnected}
      <SettingsButton
        variant="ghost"
        size="sm"
        disabled={connectionStore.actionInFlight}
        onClick={() => void connectionStore.disconnect()}
        ariaLabel="断开连接"
      >
        {#snippet icon()}
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{@html WIFI_OFF_SVG}</svg>
        {/snippet}
        断开连接
      </SettingsButton>
    {/if}
  </div>
</SettingsGroup>

<!-- 2. 快速连接 -->
{#if showQuickConnect}
  <SettingsGroup title="快速连接" description="从已保存的预设或 ~/.ssh/config 识别的 host 中一键选择">
    {#if connectionStore.savedProfiles.length > 0}
      <div class="profile-list" aria-label="已保存的连接预设">
        {#each connectionStore.savedProfiles as profile (profile.id)}
          <button
            type="button"
            class="profile-row"
            onclick={() => connectionStore.selectProfile(profile)}
          >
            <span class="profile-icon" aria-hidden="true">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{@html SERVER_SVG}</svg>
            </span>
            <span class="profile-name">{profile.name}</span>
            <span class="profile-target">{profile.username}@{profile.host}:{profile.port}</span>
            <span class="profile-auth">{authMethodLabel(profile.authMethod)}</span>
          </button>
        {/each}
      </div>
    {/if}

    {#if connectionStore.configHosts.length > 0}
      <div class="config-hosts-row">
        <button
          type="button"
          class="config-hosts-toggle"
          aria-expanded={showConfigHosts}
          onclick={() => (showConfigHosts = !showConfigHosts)}
        >
          <span class="toggle-icon" class:toggle-icon-open={showConfigHosts} aria-hidden="true">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d={CHEVRON_DOWN} />
            </svg>
          </span>
          <span><code>~/.ssh/config</code> 已识别 {connectionStore.configHosts.length} 个 host</span>
        </button>
        {#if showConfigHosts}
          <div class="config-hosts-list">
            {#each connectionStore.configHosts as alias (alias)}
              <button
                type="button"
                class="config-host-chip"
                onclick={() => void selectAlias(alias)}
              >
                {alias}
              </button>
            {/each}
          </div>
        {/if}
      </div>
    {/if}
  </SettingsGroup>
{/if}

<!-- 3. 手动配置 -->
{#if !isConnected}
  <SettingsGroup title="手动配置" description="密码仅保存在内存中，不会写入配置文件">
    {#if isWindowsPlatform}
      <div class="inline-warn" role="note">
        <span class="warn-icon" aria-hidden="true">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{@html ALERT_TRIANGLE_SVG}</svg>
        </span>
        <span>当前 v1 Windows 仅支持密码或 IdentityFile 直读，命名管道 ssh-agent 计划在 v2 加入。</span>
      </div>
    {/if}

    <SettingsField label="主机" description="支持 ~/.ssh/config alias，也可直接输入 hostname" layout="stack" labelFor="ssh-host-input">
      {#snippet control()}
        <div class="host-combobox" bind:this={hostCombobox}>
          <input
            id="ssh-host-input"
            class="control-input control-input-mono"
            type="text"
            autocomplete="off"
            autocorrect="off"
            autocapitalize="off"
            spellcheck="false"
            placeholder="myserver 或 example.com"
            bind:value={connectionStore.host}
            onfocus={() => { hostFocused = true; void connectionStore.loadConfigHosts(); }}
            oninput={() => { hostFocused = true; }}
          />
          {#if hostFocused && filteredHosts.length > 0}
            <div class="host-menu" role="listbox" aria-label="SSH config host 候选">
              {#each filteredHosts as alias (alias)}
                <button
                  type="button"
                  role="option"
                  aria-selected={connectionStore.host === alias}
                  class="host-option"
                  onclick={() => void selectAlias(alias)}
                >
                  <span>{alias}</span>
                </button>
              {/each}
            </div>
          {/if}
        </div>
      {/snippet}
    </SettingsField>

    <div class="form-grid">
      <SettingsField label="用户名" labelFor="ssh-username-input">
        {#snippet control()}
          <input
            id="ssh-username-input"
            class="control-input"
            type="text"
            autocomplete="username"
            bind:value={connectionStore.username}
            placeholder="user"
          />
        {/snippet}
      </SettingsField>
      <SettingsField label="端口" labelFor="ssh-port-input">
        {#snippet control()}
          <input
            id="ssh-port-input"
            class="control-input small-input"
            type="number"
            min="1"
            max="65535"
            bind:value={connectionStore.port}
          />
        {/snippet}
      </SettingsField>
    </div>

    <SettingsField label="鉴权方式" description="选 SSH 配置时会按 IdentityAgent → agent → IdentityFile 顺序尝试">
      {#snippet control()}
        <Dropdown
          value={connectionStore.authMethod}
          options={[
            { value: "sshConfig", label: "SSH 配置" },
            { value: "password", label: "密码" },
          ]}
          onChange={(value) => { connectionStore.authMethod = value === "password" ? "password" : "sshConfig"; }}
          ariaLabel="鉴权方式"
        />
      {/snippet}
    </SettingsField>

    {#if connectionStore.authMethod === "password"}
      <SettingsField label="密码" description="仅保存在内存中" labelFor="ssh-password-input">
        {#snippet control()}
          <input
            id="ssh-password-input"
            class="control-input"
            type="password"
            autocomplete="current-password"
            bind:value={connectionStore.password}
          />
        {/snippet}
      </SettingsField>
    {/if}

    {#if connectionStore.testResult}
      <div class="test-result" class:test-result-ok={connectionStore.testResult.success} role="status">
        <span class="test-icon" aria-hidden="true">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            {#if connectionStore.testResult.success}{@html CHECK_CIRCLE_SVG}{:else}{@html ALERT_CIRCLE_SVG}{/if}
          </svg>
        </span>
        <span>{connectionStore.testResult.success ? "测试通过，未切换当前数据源" : `测试失败：${connectionStore.testResult.error}`}</span>
      </div>
    {/if}

    {#if diagnosticChain.length > 0}
      <div class="diagnostic">
        <button
          type="button"
          class="diagnostic-toggle"
          aria-expanded={diagnosticOpen}
          onclick={() => (diagnosticOpen = !diagnosticOpen)}
        >
          <span class="toggle-icon" class:toggle-icon-open={diagnosticOpen} aria-hidden="true">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d={CHEVRON_DOWN} />
            </svg>
          </span>
          <span>鉴权诊断（{diagnosticChain.length} 次尝试）</span>
        </button>
        {#if diagnosticOpen}
          <div class="auth-chain" aria-label="鉴权诊断详情">
            {#each diagnosticChain as attempt (sourceLabel(attempt.source) + attempt.elapsedMs)}
              <div class="auth-row">
                <span class="auth-source">{sourceLabel(attempt.source)}</span>
                <span class="auth-outcome outcome-{attempt.outcome.type}">{outcomeLabel(attempt.outcome)}</span>
                <span class="auth-time">{attempt.elapsedMs}ms</span>
              </div>
            {/each}
          </div>
        {/if}
      </div>
    {/if}

    {#snippet footer()}
      <div class="actions">
        <div class="actions-primary">
          <SettingsButton variant="ghost" disabled={!canSubmit || connectionStore.testing} onClick={() => void connectionStore.testConnection()}>
            {connectionStore.testing ? "测试中…" : "测试连接"}
          </SettingsButton>
          <SettingsButton variant="primary" disabled={!canSubmit} onClick={() => void connectionStore.connect()}>
            {connectionStore.actionInFlight ? "连接中…" : "连接"}
          </SettingsButton>
        </div>
        <SettingsButton
          variant="ghost"
          size="sm"
          disabled={!connectionStore.host.trim()}
          onClick={openSaveProfile}
        >
          保存为预设
        </SettingsButton>
      </div>
    {/snippet}
  </SettingsGroup>
{/if}

<Modal
  open={showSaveProfile}
  title="保存为预设"
  primaryLabel={savingProfile ? "保存中…" : "保存"}
  primaryDisabled={savingProfile || !profileName.trim()}
  cancelLabel="取消"
  onPrimary={confirmSaveProfile}
  onClose={cancelSaveProfile}
>
  <p class="modal-hint">下次可在「快速连接」中一键载入。当前主机 / 端口 / 用户名 / 鉴权方式会一并保存（密码不会）。</p>
  <label class="modal-field" for="save-profile-name-input">
    <span>预设名称</span>
    <input
      id="save-profile-name-input"
      class="control-input"
      type="text"
      placeholder="例如：prod-aws"
      autocomplete="off"
      bind:value={profileName}
      onkeydown={(e) => { if (e.key === "Enter" && !savingProfile && profileName.trim()) void confirmSaveProfile(); }}
    />
  </label>
</Modal>

<style>
  .status-card {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 14px 16px;
    background: var(--color-surface);
  }
  .status-copy {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .status-copy strong {
    color: var(--color-text);
    font-size: 14px;
    font-weight: 600;
  }
  .status-copy span {
    color: var(--color-text-secondary);
    font-size: 12px;
    overflow-wrap: anywhere;
  }
  .status-copy code {
    padding: 1px 5px;
    border-radius: 3px;
    background: var(--color-surface-overlay);
    font-family: var(--font-mono);
    font-size: 11px;
  }

  .profile-list {
    display: flex;
    flex-direction: column;
    background: var(--color-surface);
  }
  .profile-list > :global(* + *) {
    border-top: 1px solid var(--color-border-subtle);
  }
  .profile-row {
    display: grid;
    grid-template-columns: 20px minmax(120px, 1fr) minmax(160px, 1.4fr) auto;
    align-items: center;
    gap: 12px;
    width: 100%;
    padding: 12px 16px;
    border: none;
    background: transparent;
    color: var(--color-text);
    font: inherit;
    text-align: left;
    cursor: pointer;
    transition: background-color 0.12s;
  }
  .profile-row:hover {
    background: var(--tool-item-hover-bg);
  }
  .profile-row:focus-visible {
    outline: 2px solid var(--color-switch-on);
    outline-offset: -2px;
  }
  .profile-icon {
    display: inline-flex;
    width: 16px;
    height: 16px;
    color: var(--color-text-muted);
  }
  .profile-icon :global(svg) {
    width: 16px;
    height: 16px;
  }
  .profile-name {
    font-size: 13px;
    font-weight: 500;
    color: var(--color-text);
  }
  .profile-target {
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--color-text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .profile-auth {
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.04em;
    color: var(--color-text-secondary);
    padding: 2px 8px;
    border: 1px solid var(--color-border);
    border-radius: 999px;
    background: var(--color-surface-raised);
  }

  .config-hosts-row {
    display: flex;
    flex-direction: column;
    background: var(--color-surface);
  }
  .config-hosts-toggle {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 12px 16px;
    border: none;
    background: transparent;
    color: var(--color-text-secondary);
    font: inherit;
    font-size: 12px;
    text-align: left;
    cursor: pointer;
    transition: background-color 0.12s, color 0.12s;
  }
  .config-hosts-toggle:hover {
    background: var(--tool-item-hover-bg);
    color: var(--color-text);
  }
  .config-hosts-toggle:focus-visible {
    outline: 2px solid var(--color-switch-on);
    outline-offset: -2px;
  }
  .config-hosts-toggle code {
    padding: 1px 5px;
    border-radius: 3px;
    background: var(--color-surface-overlay);
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--color-text);
  }
  .config-hosts-list {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    padding: 4px 16px 14px;
  }
  .config-host-chip {
    padding: 4px 10px;
    border: 1px solid var(--color-border);
    border-radius: 999px;
    background: transparent;
    color: var(--color-text-secondary);
    font: inherit;
    font-family: var(--font-mono);
    font-size: 11px;
    cursor: pointer;
    transition: background-color 0.12s, color 0.12s, border-color 0.12s;
  }
  .config-host-chip:hover {
    background: var(--tool-item-hover-bg);
    color: var(--color-text);
    border-color: var(--color-border-emphasis);
  }
  .config-host-chip:focus-visible {
    outline: 2px solid var(--color-switch-on);
    outline-offset: 1px;
  }

  .inline-warn {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    padding: 10px 14px;
    background: color-mix(in oklch, var(--color-warning) 8%, var(--color-surface));
    color: var(--color-text-secondary);
    font-size: 12px;
    line-height: 1.5;
  }
  .warn-icon {
    flex-shrink: 0;
    display: inline-flex;
    width: 14px;
    height: 14px;
    margin-top: 1px;
    color: var(--color-warning);
  }
  .warn-icon :global(svg) {
    width: 14px;
    height: 14px;
  }
  .host-combobox {
    position: relative;
    width: 100%;
  }
  .host-menu {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    right: 0;
    z-index: 20;
    max-height: 190px;
    overflow-y: auto;
    padding: 4px;
    border: 1px solid var(--color-border-emphasis);
    border-radius: 8px;
    background: var(--color-surface-sidebar);
    box-shadow: 0 8px 18px rgba(0, 0, 0, 0.12);
  }
  .host-option {
    display: flex;
    width: 100%;
    padding: 7px 9px;
    border: none;
    border-radius: 5px;
    background: transparent;
    color: var(--color-text);
    font: inherit;
    font-family: var(--font-mono);
    font-size: 12px;
    text-align: left;
    cursor: pointer;
  }
  .host-option:hover,
  .host-option:focus-visible {
    background: var(--tool-item-hover-bg);
    outline: none;
  }
  .form-grid {
    display: grid;
    grid-template-columns: minmax(220px, 1fr) minmax(140px, 0.5fr);
  }
  .small-input {
    max-width: 120px;
  }

  .test-result {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 11px 16px;
    color: var(--color-danger);
    background: color-mix(in oklch, var(--color-danger-bright) 8%, var(--color-surface));
    font-size: 12px;
  }
  .test-result-ok {
    color: var(--color-success);
    background: color-mix(in oklch, var(--color-success-bright) 8%, var(--color-surface));
  }
  .test-icon {
    display: inline-flex;
    width: 15px;
    height: 15px;
  }
  .test-icon :global(svg) {
    width: 15px;
    height: 15px;
  }

  .diagnostic {
    display: flex;
    flex-direction: column;
    background: var(--color-surface-raised);
  }
  .diagnostic-toggle {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 10px 16px;
    border: none;
    background: transparent;
    color: var(--color-text-secondary);
    font: inherit;
    font-size: 12px;
    font-weight: 500;
    text-align: left;
    cursor: pointer;
    transition: background-color 0.12s, color 0.12s;
  }
  .diagnostic-toggle:hover {
    background: var(--tool-item-hover-bg);
    color: var(--color-text);
  }
  .diagnostic-toggle:focus-visible {
    outline: 2px solid var(--color-switch-on);
    outline-offset: -2px;
  }
  .toggle-icon {
    display: inline-flex;
    width: 12px;
    height: 12px;
    transition: transform 0.15s ease;
  }
  .toggle-icon-open {
    transform: rotate(180deg);
  }
  .toggle-icon :global(svg) {
    width: 12px;
    height: 12px;
  }
  .auth-chain {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 0 16px 12px;
  }
  .auth-row {
    display: grid;
    grid-template-columns: minmax(140px, 1fr) minmax(140px, 1.2fr) 56px;
    gap: 10px;
    align-items: center;
    padding: 4px 0;
    font-size: 11px;
  }
  .auth-source,
  .auth-time {
    color: var(--color-text-muted);
    font-family: var(--font-mono);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .auth-outcome {
    color: var(--color-text-secondary);
    overflow-wrap: anywhere;
  }
  .outcome-success { color: var(--color-success); }
  .outcome-failure { color: var(--color-danger); }
  .outcome-skipped { color: var(--color-warning); }

  .actions {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    flex-wrap: wrap;
  }
  .actions-primary {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .modal-hint {
    margin: 0 0 12px;
    font-size: 12px;
    color: var(--color-text-muted);
    line-height: 1.55;
  }
  .modal-field {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .modal-field span {
    font-size: 12px;
    font-weight: 500;
    color: var(--color-text-secondary);
  }

  @media (max-width: 720px) {
    .status-card,
    .form-grid {
      display: flex;
      flex-direction: column;
      align-items: stretch;
    }
    .profile-row {
      grid-template-columns: 20px 1fr;
      grid-template-rows: auto auto;
      gap: 4px 12px;
    }
    .profile-target {
      grid-column: 2;
    }
    .profile-auth {
      grid-column: 2;
      justify-self: start;
    }
    .auth-row {
      grid-template-columns: 1fr;
      gap: 3px;
    }
    .actions {
      flex-direction: column;
      align-items: stretch;
    }
    .actions-primary {
      width: 100%;
      justify-content: stretch;
    }
    .actions-primary :global(button) {
      flex: 1;
    }
  }
</style>

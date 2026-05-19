<script lang="ts">
  import { onMount } from "svelte";
  import SettingsGroup from "../../lib/components/SettingsGroup.svelte";
  import SettingsField from "../../lib/components/SettingsField.svelte";
  import SettingsButton from "../../lib/components/SettingsButton.svelte";
  import Dropdown from "../../lib/components/Dropdown.svelte";
  import ConnectionStatusBadge from "../../lib/components/ConnectionStatusBadge.svelte";
  import { connectionStore } from "../../lib/stores/connection.svelte";
  import { ALERT_CIRCLE_SVG, CHECK_CIRCLE_SVG, SERVER_SVG, WIFI_OFF_SVG } from "../../lib/icons";
  import type { AuthAttempt, AuthSource, AuthOutcome } from "../../lib/types/ssh";

  let hostFocused = $state(false);
  let profileName = $state("");
  let savingProfile = $state(false);

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
    return () => connectionStore.stopListening();
  });

  function sourceLabel(source: AuthSource): string {
    switch (source.type) {
      case "identityAgent": return `IdentityAgent ${source.data}`;
      case "envAgent": return "SSH_AUTH_SOCK";
      case "launchctlAgent": return "launchctl agent";
      case "onePasswordAgent": return `1Password ${source.data}`;
      case "identityFile": return `IdentityFile ${source.data}`;
      case "defaultKey": return `Default key ${source.data}`;
      case "password": return "Password";
    }
  }

  function outcomeLabel(outcome: AuthOutcome): string {
    switch (outcome.type) {
      case "success": return "成功";
      case "failure": return `失败：${outcome.data}`;
      case "skipped": return `跳过：${outcome.data}`;
    }
  }

  async function selectAlias(alias: string) {
    hostFocused = false;
    await connectionStore.resolveHost(alias);
  }

  async function saveProfile() {
    savingProfile = true;
    try {
      await connectionStore.saveProfile(profileName || connectionStore.host);
      profileName = "";
    } finally {
      savingProfile = false;
    }
  }

  const validPort = $derived(Number.isInteger(connectionStore.port) && connectionStore.port >= 1 && connectionStore.port <= 65535);
  const canSubmit = $derived(connectionStore.host.trim().length > 0 && validPort && !connectionStore.actionInFlight);
  const diagnosticChain = $derived(
    connectionStore.errorDetail?.attempts ?? connectionStore.authChain,
  );
</script>

<SettingsGroup title="连接状态" description="远端工作区通过 SSH 读取 Claude Code 会话，ProjectList 与 SessionDetail 继续复用现有数据视图">
  <div class="status-row">
    <ConnectionStatusBadge status={connectionStore.status} contextId={connectionStore.activeContextId} error={connectionStore.error} />
    <div class="status-copy">
      {#if connectionStore.status === "connected"}
        <strong>已连接到 {connectionStore.connectedHost ?? connectionStore.host}</strong>
        <span>当前项目列表来自远端 ~/.claude/projects</span>
      {:else if connectionStore.status === "connecting"}
        <strong>正在连接 {connectionStore.host}</strong>
        <span>按 SSH config、agent、IdentityFile 与 password 顺序尝试鉴权</span>
      {:else if connectionStore.status === "error"}
        <strong>连接失败</strong>
        <span>{connectionStore.error}</span>
      {:else}
        <strong>Local</strong>
        <span>当前使用本机 Claude 数据目录</span>
      {/if}
    </div>
    {#if connectionStore.status === "connected"}
      <SettingsButton variant="ghost" disabled={connectionStore.actionInFlight} onClick={() => void connectionStore.disconnect()}>
        断开连接
      </SettingsButton>
    {/if}
  </div>
</SettingsGroup>

<SettingsGroup title="SSH Connection" description="选择 ssh config alias 或手动输入 hostname；密码仅保存在当前内存中">
  {#if isWindowsPlatform}
    <div class="inline-note" role="note">
      v1 Windows 仅支持密码模式或 IdentityFile 直读，命名管道 ssh-agent 计划在 v2 加入。
    </div>
  {/if}

  {#if connectionStore.savedProfiles.length > 0}
    <div class="profile-list" aria-label="已保存连接配置">
      {#each connectionStore.savedProfiles as profile (profile.id)}
        <button type="button" class="profile-chip" onclick={() => connectionStore.selectProfile(profile)}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">{@html SERVER_SVG}</svg>
          <span>{profile.name}</span>
          <small>{profile.username}@{profile.host}</small>
        </button>
      {/each}
    </div>
  {/if}

  <SettingsField label="Host" description="支持 ~/.ssh/config Host alias，也可输入 hostname" layout="stack" labelFor="ssh-host-input">
    {#snippet control()}
      <div class="host-combobox">
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
          <div class="host-menu" role="listbox" aria-label="SSH config hosts">
            {#each filteredHosts as alias (alias)}
              <button type="button" role="option" aria-selected={connectionStore.host === alias} class="host-option" onclick={() => void selectAlias(alias)}>
                <span>{alias}</span>
              </button>
            {/each}
          </div>
        {/if}
      </div>
    {/snippet}
  </SettingsField>

  <div class="form-grid">
    <SettingsField label="Port" labelFor="ssh-port-input">
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
    <SettingsField label="Username" labelFor="ssh-username-input">
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
  </div>

  <SettingsField label="Authentication" description="SSH config 会依次尝试 IdentityAgent、agent 与 IdentityFile">
    {#snippet control()}
      <Dropdown
        value={connectionStore.authMethod}
        options={[
          { value: "sshConfig", label: "SSH config" },
          { value: "password", label: "Password" },
        ]}
        onChange={(value) => { connectionStore.authMethod = value === "password" ? "password" : "sshConfig"; }}
        ariaLabel="Authentication"
      />
    {/snippet}
  </SettingsField>

  {#if connectionStore.authMethod === "password"}
    <SettingsField label="Password" description="不会写入配置文件" labelFor="ssh-password-input">
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
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        {#if connectionStore.testResult.success}{@html CHECK_CIRCLE_SVG}{:else}{@html ALERT_CIRCLE_SVG}{/if}
      </svg>
      <span>{connectionStore.testResult.success ? "测试成功，active context 未切换" : `测试失败：${connectionStore.testResult.error}`}</span>
    </div>
  {/if}

  {#if connectionStore.error && diagnosticChain.length > 0}
    <div class="auth-chain" aria-label="鉴权诊断">
      <div class="auth-title">鉴权尝试</div>
      {#each diagnosticChain as attempt (sourceLabel(attempt.source) + attempt.elapsedMs)}
        <div class="auth-row">
          <span class="auth-source">{sourceLabel(attempt.source)}</span>
          <span class="auth-outcome outcome-{attempt.outcome.type}">{outcomeLabel(attempt.outcome)}</span>
          <span class="auth-time">{attempt.elapsedMs}ms</span>
        </div>
      {/each}
    </div>
  {/if}

  {#snippet footer()}
    <div class="actions">
      <SettingsButton variant="ghost" disabled={!canSubmit || connectionStore.testing} onClick={() => void connectionStore.testConnection()}>
        {connectionStore.testing ? "测试中..." : "Test connection"}
      </SettingsButton>
      <SettingsButton variant="primary" disabled={!canSubmit} onClick={() => void connectionStore.connect()}>
        {connectionStore.actionInFlight ? "连接中..." : "Connect"}
      </SettingsButton>
      <input
        class="profile-name-input"
        type="text"
        placeholder="profile name"
        aria-label="Profile name"
        bind:value={profileName}
      />
      <SettingsButton variant="ghost" disabled={!connectionStore.host.trim() || savingProfile} onClick={saveProfile}>
        Save as profile
      </SettingsButton>
      {#if connectionStore.status === "connected"}
        <SettingsButton variant="ghost" onClick={() => void connectionStore.disconnect()}>
          {#snippet icon()}
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{@html WIFI_OFF_SVG}</svg>
          {/snippet}
          Disconnect
        </SettingsButton>
      {/if}
    </div>
  {/snippet}
</SettingsGroup>

<style>
  .status-row {
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
  }
  .status-copy span {
    color: var(--color-text-secondary);
    font-size: 12px;
    overflow-wrap: anywhere;
  }
  .inline-note {
    padding: 10px 14px;
    background: color-mix(in oklch, var(--color-warning) 9%, var(--color-surface));
    color: var(--color-text-secondary);
    font-size: 12px;
    line-height: 1.5;
  }
  .profile-list {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    padding: 14px 16px;
  }
  .profile-chip {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    min-height: 30px;
    padding: 0 10px;
    border: 1px solid var(--color-border);
    border-radius: 999px;
    background: transparent;
    color: var(--color-text-secondary);
    font: inherit;
    font-size: 12px;
    cursor: pointer;
  }
  .profile-chip:hover {
    background: var(--tool-item-hover-bg);
    color: var(--color-text);
  }
  .profile-chip svg {
    width: 13px;
    height: 13px;
  }
  .profile-chip small {
    color: var(--color-text-muted);
    font-family: var(--font-mono);
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
    grid-template-columns: minmax(160px, 0.55fr) minmax(220px, 1fr);
  }
  .small-input {
    max-width: 120px;
  }
  .test-result,
  .auth-chain {
    border-top: 1px solid var(--color-border-subtle);
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
  .test-result svg {
    width: 15px;
    height: 15px;
  }
  .auth-chain {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 12px 16px;
    background: var(--color-surface-raised);
  }
  .auth-title {
    margin-bottom: 4px;
    color: var(--color-text-muted);
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  .auth-row {
    display: grid;
    grid-template-columns: minmax(120px, 1fr) minmax(160px, 1.2fr) 56px;
    gap: 10px;
    align-items: center;
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
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .profile-name-input {
    height: 30px;
    width: 140px;
    padding: 0 10px;
    border: 1px solid var(--color-border);
    border-radius: 6px;
    background: var(--color-surface);
    color: var(--color-text);
    font: inherit;
    font-size: 12px;
  }
  @media (max-width: 720px) {
    .status-row,
    .form-grid {
      display: flex;
      flex-direction: column;
      align-items: stretch;
    }
    .auth-row {
      grid-template-columns: 1fr;
      gap: 3px;
    }
  }
</style>

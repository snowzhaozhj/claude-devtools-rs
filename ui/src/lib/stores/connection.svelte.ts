import type { UnlistenFn } from "@tauri-apps/api/event";

import {
  getConfig,
  sshConnect,
  sshDisconnect,
  sshGetConfigHosts,
  sshGetLastConnection,
  sshGetState,
  sshResolveHost,
  sshSaveLastConnection,
  sshTestConnection,
  updateConfig,
  listenSshStatus,
  type AppConfig,
} from "../api";
import type {
  AuthAttempt,
  AuthMethodKind,
  SshConnectionRequest,
  SshErrorDetail,
  SshHostConfig,
  SshLastConnection,
  SshProfile,
  SshStatus,
  SshStatusChange,
} from "../types/ssh";

export interface TestConnectionResult {
  success: boolean;
  authChain: AuthAttempt[];
  error: string | null;
}

let host = $state("");
let port = $state(22);
let username = $state("");
let authMethod: AuthMethodKind = $state("sshConfig");
let password = $state("");
let configHosts: string[] = $state([]);
let savedProfiles: SshProfile[] = $state([]);
let lastConnection: SshLastConnection | null = $state(null);
let status: SshStatus = $state("disconnected");
let activeContextId: string | null = $state(null);
let connectedHost: string | null = $state(null);
let authChain: AuthAttempt[] = $state([]);
let error: string | null = $state(null);
let errorDetail: SshErrorDetail | null = $state(null);
let loadingHosts = $state(false);
let actionInFlight = $state(false);
let testing = $state(false);
let testResult: TestConnectionResult | null = $state(null);
let resolvedHost: SshHostConfig | null = $state(null);
let unlistenStatus: UnlistenFn | null = null;

function errorText(value: unknown): string {
  if (value instanceof Error) return value.message;
  if (typeof value === "object" && value !== null) {
    const maybe = value as { message?: unknown; reason?: unknown; code?: unknown };
    return String(maybe.message ?? maybe.reason ?? maybe.code ?? value);
  }
  return String(value);
}

function requestFromForm(): SshConnectionRequest {
  return {
    host: host.trim(),
    port: Number.isFinite(port) ? port : 22,
    username: username.trim() || null,
    authMethod,
    password: authMethod === "password" ? password : null,
  };
}

function safePort(value: number | null | undefined): number {
  return value && value > 0 ? value : 22;
}

function applyLastConnection(value: SshLastConnection | null) {
  lastConnection = value;
  if (!value) return;
  host = value.host;
  port = safePort(value.port);
  username = value.username ?? "";
  authMethod = value.authMethod;
  password = "";
}

function applyStatusChange(change: SshStatusChange) {
  activeContextId = change.contextId;
  status = change.status;
  authChain = change.authChain ?? [];
  errorDetail = change.error ?? null;
  error = change.error ? errorText(change.error) : null;
  if (change.status === "connected") {
    connectedHost = host || change.contextId.replace(/^ssh-/, "");
    error = null;
    errorDetail = null;
  }
  if (change.status === "disconnected") {
    connectedHost = null;
  }
}

async function loadConfig() {
  let config: AppConfig | null = null;
  try {
    config = await getConfig();
  } catch {
    config = null;
  }
  savedProfiles = config?.ssh?.profiles ?? [];
}

export function getConnectionStore() {
  return {
    get host() { return host; },
    set host(value: string) { host = value; },
    get port() { return port; },
    set port(value: number) { port = value; },
    get username() { return username; },
    set username(value: string) { username = value; },
    get authMethod() { return authMethod; },
    set authMethod(value: AuthMethodKind) { authMethod = value; },
    get password() { return password; },
    set password(value: string) { password = value; },
    get configHosts() { return configHosts; },
    get savedProfiles() { return savedProfiles; },
    get lastConnection() { return lastConnection; },
    get status() { return status; },
    get activeContextId() { return activeContextId; },
    get connectedHost() { return connectedHost; },
    get authChain() { return authChain; },
    get error() { return error; },
    get errorDetail() { return errorDetail; },
    get loadingHosts() { return loadingHosts; },
    get actionInFlight() { return actionInFlight; },
    get testing() { return testing; },
    get testResult() { return testResult; },
    get resolvedHost() { return resolvedHost; },

    async initialize() {
      await Promise.all([this.loadConfigHosts(), this.loadLastConnection(), loadConfig()]);
      try {
        const state = await sshGetState();
        activeContextId = state.activeContextId ?? null;
        const active = state.contexts.find((ctx) => ctx.contextId === state.activeContextId) ?? state.contexts[0];
        if (active) {
          status = active.status;
          authChain = active.authChain ?? [];
          errorDetail = active.error ?? null;
          error = active.error ? errorText(active.error) : null;
        }
      } catch {
        status = "disconnected";
      }
    },

    async startListening() {
      if (unlistenStatus) return;
      unlistenStatus = await listenSshStatus(applyStatusChange);
    },

    stopListening() {
      unlistenStatus?.();
      unlistenStatus = null;
    },

    async loadConfigHosts() {
      loadingHosts = true;
      try {
        configHosts = await sshGetConfigHosts();
      } catch {
        configHosts = [];
      } finally {
        loadingHosts = false;
      }
    },

    async resolveHost(alias: string) {
      const trimmed = alias.trim();
      if (!trimmed) return null;
      resolvedHost = await sshResolveHost(trimmed);
      host = alias;
      port = safePort(resolvedHost.port);
      username = resolvedHost.user ?? resolvedHost.username ?? username;
      authMethod = "sshConfig";
      testResult = null;
      return resolvedHost;
    },

    async loadLastConnection() {
      applyLastConnection(await sshGetLastConnection());
    },

    selectProfile(profile: SshProfile) {
      host = profile.host;
      port = safePort(profile.port);
      username = profile.username;
      authMethod = profile.authMethod;
      password = "";
      testResult = null;
    },

    async connect() {
      actionInFlight = true;
      status = "connecting";
      error = null;
      errorDetail = null;
      authChain = [];
      try {
        const request = requestFromForm();
        const result = await sshConnect(request);
        activeContextId = result.contextId;
        status = result.status;
        authChain = result.authChain ?? [];
        connectedHost = request.host;
        const saved = {
          host: request.host,
          port: request.port ?? 22,
          username: request.username ?? null,
          authMethod: request.authMethod,
          contextId: result.contextId,
        } satisfies SshLastConnection;
        await sshSaveLastConnection(saved);
        lastConnection = saved;
        window.dispatchEvent(new CustomEvent("cdt-refresh-projects"));
      } catch (e) {
        status = "error";
        error = errorText(e);
        const nextErrorDetail: SshErrorDetail = e && typeof e === "object" ? e as SshErrorDetail : { message: error };
        errorDetail = authChain.length && !nextErrorDetail.attempts?.length
          ? { ...nextErrorDetail, attempts: authChain }
          : nextErrorDetail;
      } finally {
        actionInFlight = false;
      }
    },

    async testConnection() {
      testing = true;
      testResult = null;
      try {
        const result = await sshTestConnection(requestFromForm());
        testResult = { success: true, authChain: result.authChain ?? [], error: null };
      } catch (e) {
        testResult = { success: false, authChain: [], error: errorText(e) };
      } finally {
        testing = false;
      }
    },

    async disconnect() {
      if (!activeContextId) return;
      actionInFlight = true;
      try {
        await sshDisconnect(activeContextId);
        status = "disconnected";
        activeContextId = "local";
        connectedHost = null;
        error = null;
        errorDetail = null;
        window.dispatchEvent(new CustomEvent("cdt-refresh-projects"));
      } catch (e) {
        error = errorText(e);
      } finally {
        actionInFlight = false;
      }
    },

    async saveProfile(name: string) {
      const trimmed = name.trim();
      if (!trimmed) return;
      await loadConfig();
      const nextProfile: SshProfile = {
        id: `ssh-${Date.now()}`,
        name: trimmed,
        host: host.trim(),
        port: safePort(port),
        username: username.trim(),
        authMethod,
        passwordRequired: authMethod === "password",
      };
      const nextProfiles = [...savedProfiles.filter((p) => p.name !== trimmed), nextProfile];
      savedProfiles = nextProfiles;
      await updateConfig("ssh", { profiles: nextProfiles });
    },

    async saveLastConnection() {
      const request = requestFromForm();
      const saved = {
        host: request.host,
        port: request.port ?? 22,
        username: request.username ?? null,
        authMethod: request.authMethod,
        contextId: request.contextId ?? activeContextId,
      } satisfies SshLastConnection;
      lastConnection = await sshSaveLastConnection(saved);
    },
  };
}

export const connectionStore = getConnectionStore();

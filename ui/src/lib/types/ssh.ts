export type AuthMethodKind = "sshConfig" | "password";

export type AuthSource =
  | { type: "identityAgent"; data: string }
  | { type: "envAgent" }
  | { type: "launchctlAgent" }
  | { type: "onePasswordAgent"; data: string }
  | { type: "identityFile"; data: string }
  | { type: "defaultKey"; data: string }
  | { type: "password" };

export type AuthOutcome =
  | { type: "success" }
  | { type: "failure"; data: string }
  | { type: "skipped"; data: string };

export interface AuthAttempt {
  source: AuthSource;
  outcome: AuthOutcome;
  elapsedMs: number;
}

export type SshStatus = "disconnected" | "connecting" | "connected" | "error";

export interface SshErrorDetail {
  code?: string;
  message?: string;
  host?: string;
  reason?: string;
  stage?: string;
  tried?: string[];
  attempts?: AuthAttempt[];
  [key: string]: unknown;
}

export interface SshStatusChange {
  contextId: string;
  status: SshStatus;
  authChain?: AuthAttempt[];
  error?: SshErrorDetail | null;
}

export interface SshConnectionStatus {
  contextId?: string | null;
  status: SshStatus;
  authChain?: AuthAttempt[];
  error?: SshErrorDetail | null;
}

export interface SshConnectionRequest {
  host: string;
  port?: number | null;
  username?: string | null;
  authMethod: AuthMethodKind;
  password?: string | null;
  contextId?: string | null;
}

export interface SshConnectionResult {
  contextId: string;
  status: SshStatus;
  authChain: AuthAttempt[];
}

export type ContextKind = "local" | "ssh";

export interface ContextSummary {
  id: string;
  kind: ContextKind | string;
  label?: string | null;
  status?: SshStatus | string | null;
  isActive?: boolean;
  host?: string | null;
}

export type ContextInfo = ContextSummary;

export interface ContextChanged {
  activeContextId?: string | null;
  kind: ContextKind;
}

export interface SshProfile {
  id: string;
  name: string;
  host: string;
  port: number;
  username: string;
  authMethod: AuthMethodKind;
  privateKeyPath?: string | null;
  passwordRequired?: boolean;
}

export interface SshLastConnection {
  host: string;
  port?: number | null;
  username?: string | null;
  authMethod: AuthMethodKind;
  contextId?: string | null;
}

export interface SshHostConfig {
  host: string;
  hostname?: string | null;
  port: number;
  user?: string | null;
  username?: string | null;
  identityFile?: string | null;
  identityFiles?: string[];
  identityAgent?: string | null;
  degraded: boolean;
}

export type ResolvedHost = SshHostConfig;

export interface SshState {
  activeContextId?: string | null;
  contexts: SshConnectionStatus[];
}

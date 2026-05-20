import type { UnlistenFn } from "@tauri-apps/api/event";

import {
  getActiveContext,
  listContexts,
  listenContextChanged,
  switchContext as switchContextIpc,
} from "../api";
import { loadProjectData } from "../projectDataStore.svelte";
import type { ContextChanged, ContextSummary } from "../types/ssh";

let availableContexts: ContextSummary[] = $state([]);
let activeContextId = $state("local");
let switching = $state(false);
let switchingTo: string | null = $state(null);
let error: string | null = $state(null);
let unlistenContext: UnlistenFn | null = null;

function normalizeContexts(contexts: ContextSummary[]): ContextSummary[] {
  if (contexts.some((ctx) => ctx.id === "local")) return contexts;
  return [{ id: "local", kind: "local", label: "Local", status: "connected", isActive: activeContextId === "local" }, ...contexts];
}

function errorText(value: unknown): string {
  if (value instanceof Error) return value.message;
  return String(value);
}

async function refreshAfterContextChange(change?: ContextChanged) {
  // 后端 emit `ContextChanged { activeContextId: null, kind: Local }` 表示当前
  // 已无活跃 SSH context（典型：被动 ssh_disconnect 把当前 active 清空）。
  // 之前 `if (nextActiveContextId)` 在 null 时不更新 store，导致 contextStore
  // 卡在被断开的 SSH context id，UI 各处依赖此 id 判断"当前 context"会读到
  // 错误值（症状：SSH 断开后本机功能受影响）。统一把 null 视为切到 "local"。
  const resolvedContextId = change?.activeContextId ?? "local";
  activeContextId = resolvedContextId;
  switching = false;
  switchingTo = null;
  window.dispatchEvent(new CustomEvent("cdt-refresh-projects"));
  void loadProjectData({ refresh: true });
  await contextStore.loadContexts();
  // loadContexts 可能从 listContexts() 拿到 active=true 的项覆盖 activeContextId；
  // 再次以事件 payload 为准（事件是单一真相源）。
  activeContextId = resolvedContextId;
}

export function getContextStore() {
  return {
    get availableContexts() { return availableContexts; },
    get activeContextId() { return activeContextId; },
    get switching() { return switching; },
    get switchingTo() { return switchingTo; },
    get error() { return error; },

    async initialize() {
      await this.getActiveContext();
      await this.loadContexts();
    },

    async startListening() {
      if (unlistenContext) return;
      unlistenContext = await listenContextChanged((payload) => {
        void refreshAfterContextChange(payload);
      });
    },

    stopListening() {
      unlistenContext?.();
      unlistenContext = null;
    },

    async loadContexts() {
      try {
        availableContexts = normalizeContexts(await listContexts());
        const active = availableContexts.find((ctx) => ctx.isActive);
        if (active) activeContextId = active.id;
        error = null;
      } catch (e) {
        availableContexts = normalizeContexts([]);
        error = errorText(e);
      }
    },

    async getActiveContext() {
      try {
        const active = await getActiveContext();
        activeContextId = active.id;
        error = null;
        return active;
      } catch (e) {
        error = errorText(e);
        return null;
      }
    },

    async switchContext(contextId: string) {
      if (contextId === activeContextId || switching) return;
      switching = true;
      switchingTo = contextId;
      error = null;
      try {
        await switchContextIpc(contextId);
        await refreshAfterContextChange({ activeContextId: contextId, kind: contextId === "local" ? "local" : "ssh" });
      } catch (e) {
        switching = false;
        switchingTo = null;
        error = errorText(e);
      }
    },

    finishSwitch() {
      switching = false;
      switchingTo = null;
    },
  };
}

export const contextStore = getContextStore();

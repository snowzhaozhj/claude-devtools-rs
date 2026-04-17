import { invoke } from "@tauri-apps/api/core";

export interface AgentConfig {
  name: string;
  color: string | null;
  description: string | null;
  scope: { kind: "global" } | { kind: "project"; projectId: string };
  filePath: string;
}

let configs: AgentConfig[] = $state([]);
let loaded = false;

export async function loadAgentConfigs(): Promise<void> {
  if (loaded) return;
  try {
    const result = await invoke<AgentConfig[]>("read_agent_configs");
    configs = result ?? [];
  } catch (e) {
    console.warn("[agentConfigsStore] loadAgentConfigs failed:", e);
    configs = [];
  } finally {
    loaded = true;
  }
}

export function getAgentConfigs(): AgentConfig[] {
  return configs;
}

/** 返回 `{ name -> { color } }` 映射，方便传给 `getSubagentTypeColorSet`。 */
export function getAgentConfigsByName(): Record<string, { color?: string | null }> {
  const map: Record<string, { color?: string | null }> = {};
  for (const c of configs) {
    map[c.name] = { color: c.color };
  }
  return map;
}
